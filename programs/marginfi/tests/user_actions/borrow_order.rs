use super::borrow_order_common::{apr, round_trip, setup, usdc, Params, FLOAT, WINDOW};
use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::test::DEFAULT_USDC_TEST_BANK_CONFIG;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::{
    constants::ORDER_INIT_FLAT_FEE_DEFAULT, prelude::MarginfiError, state::bank::BankImpl,
};
use marginfi_type_crate::constants::BORROW_ORDER_FILL_DUST_ATOMS;
use marginfi_type_crate::types::BankConfigOpt;
use solana_program_test::{tokio, BanksClientError};
use solana_sdk::{instruction::Instruction, signer::Signer as _, transaction::Transaction};

#[tokio::test]
async fn a_fill_opens_once_a_reading_is_a_window_old() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    let before = fx.owner_usdc_balance().await;
    fx.fill(100.0).await?;

    let order = fx.order_state().await;
    assert_eq!(order.filled, usdc(100.0));
    assert_eq!(order.remaining(), 0);
    assert_eq!(fx.owner_usdc_balance().await - before, usdc(100.0));
    Ok(())
}

#[tokio::test]
async fn a_fill_before_the_window_has_no_measurement() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW - 1).await;
    let res = fx.fill(100.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderHistoryTooShort);
    Ok(())
}

#[tokio::test]
async fn placement_charges_the_flat_fee_and_refuses_a_bank_the_account_lends_in(
) -> anyhow::Result<()> {
    let fx = setup(Params::default()).await?;
    let fee_wallet = fx.test_f.marginfi_group.fee_wallet;
    let flat_fee: u64 = fx
        .test_f
        .load_and_deserialize::<marginfi_type_crate::types::FeeState>(
            &fx.test_f.marginfi_group.fee_state,
        )
        .await
        .order_init_flat_sol_fee
        .into();
    assert_eq!(flat_fee, u64::from(ORDER_INIT_FLAT_FEE_DEFAULT));

    let before = fx.lamports(fee_wallet).await;
    let fixed = fx.test_f.get_bank(&BankMint::Fixed);
    fx.place_on(fixed.key, &Params::default()).await?;
    assert_eq!(fx.lamports(fee_wallet).await, before + flat_fee);

    let res = fx.place_on(fx.sol().key, &Params::default()).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderInvalidConfig);

    let fx = setup(Params {
        redeploy: true,
        ..Default::default()
    })
    .await?;
    let lender_usdc = fx.usdc().mint.create_token_account_and_mint_to(100.0).await;
    let lender = fx.test_f.create_marginfi_account().await;
    lender
        .try_bank_deposit(lender_usdc.key, fx.dst(), 100.0, None)
        .await?;
    let own = fx.usdc().mint.create_empty_token_account().await.key;
    fx.account_f.try_bank_borrow(own, fx.dst(), 10.0).await?;
    let other_usdc = fx
        .test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(
            &fx.test_f.usdc_mint,
            None,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
            902,
        )
        .await?;
    let res = fx.place_on(other_usdc.key, &Params::default()).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderInvalidConfig);
    Ok(())
}

#[tokio::test]
async fn a_rate_above_the_level_does_not_open() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        utilize: FLOAT * 0.8,
        open_below: apr(0.5),
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    let res = fx.fill(100.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderRateNotLowEnough);
    Ok(())
}

#[tokio::test]
async fn a_fill_takes_exactly_what_fits_under_the_level() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // The test curve reaches 10% at ~8% utilization, well short of the float.
        open_below: apr(10.0),
        amount: FLOAT,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;
    let max = fx.max_fill().await;
    let granule = fx.order_state().await.granule();
    // Scenario guard: the fit sits strictly inside the order; its exact size is asserted below.
    assert!(max > 2 * granule && max < usdc(FLOAT));

    let res = fx.fill_native(max + 2 * granule).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillOvershoots);
    let res = fx.fill_native(max - 2 * granule).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillNotMaximal);

    fx.fill_native(max).await?;
    let order = fx.order_state().await;
    assert_eq!(order.filled, max);
    assert_eq!(order.remaining(), usdc(FLOAT) - max);

    let res = fx.fill_native(1).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillBelowGranule);
    Ok(())
}

#[tokio::test]
async fn a_fill_bounded_by_a_bank_limit_is_maximal() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        redeploy: true,
        ..Default::default()
    })
    .await?;
    fx.test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            fx.usdc(),
            BankConfigOpt {
                borrow_limit: Some(usdc(500.0)),
                ..Default::default()
            },
        )
        .await?;
    fx.advance(WINDOW).await;
    let res = fx.fill(400.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillNotMaximal);
    fx.fill(499.0).await?;

    let mut fx = setup(Params {
        redeploy: true,
        ..Default::default()
    })
    .await?;
    fx.test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            fx.dst(),
            BankConfigOpt {
                deposit_limit: Some(usdc(300.0)),
                ..Default::default()
            },
        )
        .await?;
    fx.advance(WINDOW).await;
    let res = fx.fill(200.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillNotMaximal);
    fx.fill(299.0).await?;
    assert_eq!(fx.order_state().await.filled, usdc(299.0));
    Ok(())
}

/// The test curve prices 90% utilization at 252% APR: ten minutes of it over six hours reads as
/// ~7%, the same peak held all six hours as 252%.
#[tokio::test]
async fn a_brief_spike_does_not_move_the_measured_rate_but_a_sustained_one_does(
) -> anyhow::Result<()> {
    let params = || Params {
        open_below: apr(20.0),
        amount: 100.0,
        ..Default::default()
    };

    let mut fx = setup(params()).await?;
    let (driver, driver_usdc) = fx.spike_utilization(0.9).await?;
    fx.advance(600).await;
    driver
        .try_bank_repay(driver_usdc, fx.usdc(), 0, Some(true))
        .await?;
    fx.advance(WINDOW).await;
    fx.fill(100.0).await?;

    let mut fx = setup(params()).await?;
    let _driver = fx.spike_utilization(0.9).await?;
    fx.advance(WINDOW).await;
    let res = fx.fill(100.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderRateNotLowEnough);
    Ok(())
}

#[tokio::test]
async fn a_gap_in_readings_lengthens_the_span_rather_than_blocking() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(3 * WINDOW).await;
    fx.fill(100.0).await?;
    assert_eq!(fx.order_state().await.filled, usdc(100.0));
    Ok(())
}

#[tokio::test]
async fn the_cooldown_spaces_consecutive_fills() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        cooldown: 600,
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    fx.fill(100.0).await?;
    fx.update(Some(usdc(200.0)), None).await?;

    let res = fx.fill(100.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCooldown);

    fx.advance(600).await;
    fx.fill(100.0).await?;
    assert_eq!(fx.order_state().await.filled, usdc(200.0));
    Ok(())
}

#[tokio::test]
async fn a_fill_cannot_exceed_what_the_order_has_left() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    let res = fx.fill(101.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderExceedsRemaining);
    Ok(())
}

#[tokio::test]
async fn a_redeploying_fill_deposits_into_the_destination_bank() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        redeploy: true,
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    fx.fill(100.0).await?;

    assert_eq!(fx.redeployed().await, I80F48::from_num(usdc(100.0)));
    let (_, shares) = fx.debt().await;
    assert_eq!(
        I80F48::from(fx.order_state().await.liability_shares),
        shares
    );
    Ok(())
}

#[tokio::test]
async fn a_fill_on_a_bank_with_an_origination_fee_books_the_fee_and_passes() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        origination_fee: 0.01,
        amount: 100.0,
        ..Default::default()
    })
    .await?;

    fx.advance(WINDOW).await;
    fx.fill(100.0).await?;

    // The fee rate is quantized by its I80F48 storage.
    assert_eq!(fx.order_state().await.filled, usdc(100.0));
    let (debt, _) = fx.debt().await;
    let stored_fee: I80F48 = fx
        .usdc()
        .load()
        .await
        .config
        .interest_rate_config
        .protocol_origination_fee
        .into();
    let delivered = I80F48::from_num(usdc(100.0));
    assert_eq!(debt, delivered + delivered * stored_fee);
    Ok(())
}

#[tokio::test]
async fn a_wallet_fill_delivered_to_the_keeper_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;

    let usdc_bank = fx.usdc();
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.keeper_usdc, 100.0).await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderWrongDestination);
    Ok(())
}

#[tokio::test]
async fn a_redeploying_fill_that_deposits_a_fraction_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        redeploy: true,
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;

    let usdc_bank = fx.usdc();
    let dst = fx.dst();
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.keeper_usdc, 100.0).await,
        fx.deposit_ix(dst, 0.000001).await,
        fx.end_open_ix(vec![usdc_bank.key, dst.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillMismatch);
    Ok(())
}

#[tokio::test]
async fn a_borrow_leg_on_another_bank_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;

    let usdc_bank = fx.usdc();
    let stray = fx.sol().mint.create_empty_token_account().await.key;
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.owner_usdc, 100.0).await,
        fx.borrow_ix(fx.sol(), stray, 1.0).await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderLegBankMismatch);
    Ok(())
}

#[tokio::test]
async fn a_leg_on_another_account_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;

    // A second account the keeper itself owns, so the leg's own signer check passes.
    let other = MarginfiAccountFixture::new_with_authority(
        fx.test_f.context.clone(),
        &fx.test_f.marginfi_group.key,
        &fx.keeper,
    )
    .await;
    let other_sol = fx
        .sol()
        .mint
        .create_token_account_and_mint_to_with_owner(&fx.keeper.pubkey(), 10.0)
        .await;
    other
        .try_bank_deposit_with_authority(other_sol.key, fx.sol(), 10.0, None, &fx.keeper)
        .await?;

    let usdc_bank = fx.usdc();
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.owner_usdc, 100.0).await,
        fx.borrow_ix_for(&other, usdc_bank, fx.keeper_usdc, 1.0)
            .await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::BorrowOrderForeignAccountLeg
    );
    Ok(())
}

#[tokio::test]
async fn deposit_legs_are_bound_to_the_destination() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;
    let usdc_bank = fx.usdc();
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.owner_usdc, 100.0).await,
        fx.deposit_ix(fx.sol(), 0.1).await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderLegBankMismatch);

    let mut fx = setup(Params {
        redeploy: true,
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;
    let usdc_bank = fx.usdc();
    let ixs = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.keeper_usdc, 100.0).await,
        fx.deposit_ix(fx.sol(), 0.1).await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderLegBankMismatch);
    Ok(())
}

#[tokio::test]
async fn a_malformed_sandwich_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;
    let usdc_bank = fx.usdc();

    let no_end = [
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.owner_usdc, 100.0).await,
    ];
    let res = fx.process(&no_end, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::EndNotLast);

    let two_starts = [
        fx.start_open_ix(),
        fx.start_open_ix(),
        fx.borrow_ix(usdc_bank, fx.owner_usdc, 100.0).await,
        fx.end_open_ix(vec![usdc_bank.key]).await,
    ];
    let res = fx.process(&two_starts, &fx.keeper).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::BorrowOrderMalformedSandwich
    );
    Ok(())
}

#[tokio::test]
async fn an_update_respects_the_filled_amount_and_the_close_rules() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        amount: 100.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;
    fx.fill(100.0).await?;

    let res = fx.update(Some(usdc(50.0)), None).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderExceedsRemaining);
    let res = fx.update(None, Some(apr(150.0))).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderNoCloseSide);

    let mut fx = setup(Params {
        redeploy: true,
        ..Default::default()
    })
    .await?;
    let _driver = fx.open_then_spike().await?;
    let res = fx.close(1_000.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderNoCloseSide);

    fx.update(None, Some(apr(150.0))).await?;
    assert!(fx.order_state().await.has_close_side());
    fx.update(None, Some(0)).await?;
    assert!(!fx.order_state().await.has_close_side());
    let res = fx.close(1_000.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderNoCloseSide);
    Ok(())
}

#[tokio::test]
async fn an_integration_bank_cannot_be_the_borrow_bank() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Sol,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let account_f = setup.test_f.create_marginfi_account().await;
    let payer = setup.test_f.context.borrow().payer.insecure_clone();
    let bank = setup.bank_f.key;
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::PlaceBorrowOrder {
            group: setup.test_f.marginfi_group.key,
            marginfi_account: account_f.key,
            authority: payer.pubkey(),
            bank,
            destination_bank: None,
            borrow_order: account_f.borrow_order_pda(bank),
            fee_state: setup.test_f.marginfi_group.fee_state,
            global_fee_wallet: setup.test_f.marginfi_group.fee_wallet,
            fee_payer: payer.pubkey(),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountPlaceBorrowOrder {
            amount: 1_000_000,
            open_below_apr: apr(100.0),
            close_above_apr: None,
            cooldown_seconds: None,
            window_seconds: None,
            keeper_tip: None,
        }
        .data(),
    };
    let blockhash = setup.test_f.get_latest_blockhash().await;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);
    let res = setup.test_f.banks_client().process_transaction(tx).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderUnsupportedBank);
    Ok(())
}

#[tokio::test]
async fn a_redeploying_fill_passes_health_only_once_the_deposit_counts() -> anyhow::Result<()> {
    // $500 of SOL against a $600 borrow: short on its own, covered once the $600 is redeployed.
    let mut fx = setup(Params {
        redeploy: true,
        collateral: 50.0,
        amount: 600.0,
        ..Default::default()
    })
    .await?;
    fx.advance(WINDOW).await;

    let stray = fx.usdc().mint.create_empty_token_account().await.key;
    let plain = fx.account_f.try_bank_borrow(stray, fx.usdc(), 600.0).await;
    assert_custom_error!(plain.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    fx.fill(600.0).await?;
    assert_eq!(fx.order_state().await.filled, usdc(600.0));
    Ok(())
}

#[tokio::test]
async fn a_fill_tips_the_keeper_from_the_fee_pool() -> anyhow::Result<()> {
    let tip = 1_000_000;
    let mut fx = setup(Params {
        keeper_tip: tip,
        cooldown: 0,
        amount: 200.0,
        ..round_trip()
    })
    .await?;
    let payer = fx.payer();
    let pool = fx.account_f.rebalance_fee_pool_pda();
    let rent_floor = fx
        .test_f
        .banks_client()
        .get_rent()
        .await?
        .minimum_balance(0);

    let top_up = fx
        .account_f
        .make_top_up_rebalance_fee_pool_ix(payer.pubkey(), tip / 2)
        .await;
    fx.process(&[top_up], &payer).await?;
    fx.advance(WINDOW).await;
    let keeper_before = fx.lamports(fx.keeper.pubkey()).await;
    fx.fill(200.0).await?;
    let fee = fx.tx_fee().await;
    assert_eq!(fx.lamports(pool).await, rent_floor);
    assert_eq!(
        fx.lamports(fx.keeper.pubkey()).await,
        keeper_before + tip / 2 - fee
    );

    let top_up = fx
        .account_f
        .make_top_up_rebalance_fee_pool_ix(payer.pubkey(), 3 * tip)
        .await;
    fx.process(&[top_up], &payer).await?;
    fx.user_withdraws_from_destination(100.0).await?;
    let _driver = fx.spike_utilization(0.8).await?;
    fx.advance(WINDOW).await;
    let keeper_before = fx.lamports(fx.keeper.pubkey()).await;
    fx.close(100.0, false).await?;
    let earned = tip * usdc(100.0) / usdc(200.0);
    assert_eq!(fx.lamports(pool).await, rent_floor + 3 * tip - earned);
    assert_eq!(
        fx.lamports(fx.keeper.pubkey()).await,
        keeper_before + earned - fee
    );
    Ok(())
}

#[tokio::test]
async fn a_close_repays_from_the_destination_once_the_rate_rises_over_the_level(
) -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    let (owed, shares_before) = fx.debt().await;
    // Scenario guard: the curve's compounding is asserted exactly through the shares below.
    assert!(owed > I80F48::from_num(usdc(1_000.0)));
    let repaid_shares = fx
        .usdc()
        .load()
        .await
        .get_liability_shares(I80F48::from_num(usdc(1_000.0)))?;
    let expected_filled = fx.filled_after_repaying(usdc(1_000.0)).await;

    fx.close(1_000.0, false).await?;

    let order = fx.order_state().await;
    let (_, shares) = fx.debt().await;
    assert_eq!(shares, shares_before - repaid_shares);
    assert_eq!(I80F48::from(order.liability_shares), shares);
    assert_eq!(order.filled, expected_filled);
    assert_eq!(order.remaining(), usdc(1_000.0) - expected_filled);
    assert_eq!(fx.redeployed().await, I80F48::ZERO);
    Ok(())
}

#[tokio::test]
async fn a_close_before_the_rate_rises_is_refused() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    fx.advance(WINDOW).await;
    fx.fill(1_000.0).await?;

    fx.advance(WINDOW).await;
    let res = fx.close(1_000.0, false).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::BorrowOrderRateNotHighEnough
    );
    Ok(())
}

#[tokio::test]
async fn a_full_close_clears_the_debt_and_the_order_can_open_again() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    fx.advance(WINDOW).await;
    fx.fill(1_000.0).await?;
    fx.user_tops_up_destination(10.0).await?;

    let (driver, driver_usdc) = fx.spike_utilization(0.8).await?;
    fx.advance(WINDOW).await;
    let (owed, _) = fx.debt().await;
    let ui = owed.floor().to_num::<u64>() as f64 / 1e6;
    fx.close(ui, true).await?;

    let order = fx.order_state().await;
    assert_eq!(order.filled, 0);
    assert_eq!(I80F48::from(order.liability_shares), I80F48::ZERO);
    assert_eq!(fx.debt().await, (I80F48::ZERO, I80F48::ZERO));
    assert_eq!(
        fx.redeployed().await,
        I80F48::from_num(usdc(1_010.0) - usdc(ui))
    );

    driver
        .try_bank_repay(driver_usdc, fx.usdc(), 0, Some(true))
        .await?;
    fx.advance(WINDOW).await;
    fx.fill(1_000.0).await?;
    assert_eq!(fx.order_state().await.filled, usdc(1_000.0));
    Ok(())
}

#[tokio::test]
async fn a_close_repays_only_what_the_order_opened() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    fx.advance(WINDOW).await;
    fx.fill(1_000.0).await?;
    let own = fx.usdc().mint.create_empty_token_account().await.key;
    fx.account_f.try_bank_borrow(own, fx.usdc(), 500.0).await?;
    fx.user_tops_up_destination(600.0).await?;

    let _driver = fx.spike_utilization(0.75).await?;
    fx.advance(WINDOW).await;
    let res = fx.close(1_200.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderExceedsRemaining);

    let (_, shares_before) = fx.debt().await;
    let bank = fx.usdc().load().await;
    let order_shares_before = I80F48::from(fx.order_state().await.liability_shares);
    let repaid = bank
        .get_liability_amount(order_shares_before)?
        .floor()
        .to_num::<u64>();
    let repaid_shares = bank.get_liability_shares(I80F48::from_num(repaid))?;
    fx.close(repaid as f64 / 1e6, false).await?;

    let (_, shares) = fx.debt().await;
    assert_eq!(shares, shares_before - repaid_shares);
    assert_eq!(
        I80F48::from(fx.order_state().await.liability_shares),
        order_shares_before - repaid_shares
    );
    Ok(())
}

#[tokio::test]
async fn a_close_must_repay_all_the_destination_can_cover() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    let res = fx.close(400.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCloseIncomplete);
    fx.user_withdraws_from_destination(600.0).await?;
    let res = fx.close(300.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCloseIncomplete);
    fx.close(400.0, false).await?;
    assert_eq!(fx.redeployed().await, I80F48::ZERO);

    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    fx.user_withdraws_from_destination(995.0).await?;
    let res = fx.close_from_own_funds(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCloseIncomplete);
    fx.close(5.0, false).await?;
    assert_eq!(fx.redeployed().await, I80F48::ZERO);
    let res = fx.close_from_own_funds(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderNothingToClose);

    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    let other = fx.test_f.create_marginfi_account().await;
    let other_sol = fx.sol().mint.create_token_account_and_mint_to(300.0).await;
    other
        .try_bank_deposit(other_sol.key, fx.sol(), 300.0, None)
        .await?;
    let other_usdc = fx.usdc().mint.create_empty_token_account().await.key;
    other.try_bank_borrow(other_usdc, fx.dst(), 700.0).await?;
    let res = fx.close(200.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCloseIncomplete);
    fx.close(300.0, false).await?;
    assert_eq!(fx.redeployed().await, I80F48::from_num(usdc(700.0)));
    Ok(())
}

#[tokio::test]
async fn a_close_on_a_premium_active_bank_balances() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        premium: true,
        ..round_trip()
    })
    .await?;
    let _driver = fx.open_then_spike().await?;
    let premium_before: I80F48 = fx.usdc().load().await.collected_premium_outstanding.into();
    let (_, shares_before) = fx.debt().await;

    fx.close(1_000.0, false).await?;

    let bank = fx.usdc().load().await;
    let premium_settled = I80F48::from(bank.collected_premium_outstanding) - premium_before;
    // Scenario guard: the premium exceeds the dust tolerance, so only the credit lets it pass.
    assert!(premium_settled > I80F48::from_num(BORROW_ORDER_FILL_DUST_ATOMS));
    let (_, shares) = fx.debt().await;
    let principal = I80F48::from_num(usdc(1_000.0)) - premium_settled;
    assert_eq!(
        shares,
        shares_before - bank.get_liability_shares(principal)?
    );
    assert_eq!(
        I80F48::from(fx.order_state().await.liability_shares),
        shares
    );
    assert_eq!(fx.redeployed().await, I80F48::ZERO);
    Ok(())
}

#[tokio::test]
async fn a_close_is_not_counted_against_the_destination_outflow_limit() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    let payer = fx.payer();
    fx.test_f
        .marginfi_group
        .try_update_with_flow_admin(
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
        )
        .await?;
    let limit = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::ConfigureBankRateLimits {
            group: fx.test_f.marginfi_group.key,
            admin: payer.pubkey(),
            bank: fx.dst().key,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureBankRateLimits {
            hourly_max_outflow: Some(usdc(500.0)),
            daily_max_outflow: None,
        }
        .data(),
    };
    fx.process(&[limit], &payer).await?;

    let res = fx.user_withdraws_from_destination(1_000.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankHourlyRateLimitExceeded);
    fx.close(1_000.0, false).await?;
    assert_eq!(fx.redeployed().await, I80F48::ZERO);
    Ok(())
}

#[tokio::test]
async fn a_close_after_a_hand_repayment_empties_the_order() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    fx.advance(WINDOW).await;
    fx.fill(1_000.0).await?;
    let own = fx.usdc().mint.create_token_account_and_mint_to(400.0).await;
    fx.account_f
        .try_bank_repay(own.key, fx.usdc(), 400.0, None)
        .await?;

    let _driver = fx.spike_utilization(0.8).await?;
    fx.advance(WINDOW).await;
    let (owed, _) = fx.debt().await;
    fx.close(owed.floor().to_num::<u64>() as f64 / 1e6, true)
        .await?;

    let order = fx.order_state().await;
    assert_eq!(order.filled, 0);
    assert_eq!(I80F48::from(order.liability_shares), I80F48::ZERO);
    assert_eq!(fx.debt().await, (I80F48::ZERO, I80F48::ZERO));
    Ok(())
}

#[tokio::test]
async fn a_close_after_the_spike_has_passed_is_refused() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let (driver, driver_usdc) = fx.open_then_spike().await?;
    driver
        .try_bank_repay(driver_usdc, fx.usdc(), 0, Some(true))
        .await?;
    let res = fx.close(1_000.0, false).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::BorrowOrderRateNotHighEnough
    );
    Ok(())
}

#[tokio::test]
async fn a_close_with_nothing_opened_is_refused() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let _driver = fx.spike_utilization(0.9).await?;
    fx.advance(WINDOW).await;
    let res = fx.close(1.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderNothingToClose);
    Ok(())
}

#[tokio::test]
async fn a_wallet_order_cannot_have_a_close_side() -> anyhow::Result<()> {
    let fx = setup(Params {
        close_above: Some(apr(150.0)),
        ..Default::default()
    })
    .await;
    assert_custom_error!(
        fx.err().unwrap().downcast::<BanksClientError>().unwrap(),
        MarginfiError::BorrowOrderNoCloseSide
    );
    Ok(())
}

#[tokio::test]
async fn the_cooldown_spans_an_open_and_the_close_after_it() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        cooldown: 2 * WINDOW as u32,
        ..round_trip()
    })
    .await?;
    let _driver = fx.open_then_spike().await?;
    let res = fx.close(1_000.0, false).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderCooldown);

    fx.advance(WINDOW).await;
    fx.close(1_000.0, false).await?;
    Ok(())
}

#[tokio::test]
async fn close_legs_are_bound_to_the_order() -> anyhow::Result<()> {
    let mut fx = setup(round_trip()).await?;
    let _driver = fx.open_then_spike().await?;
    let usdc_bank = fx.usdc();
    let dst = fx.dst();

    // Withdrawing collateral from another bank.
    let ixs = [
        fx.start_close_ix(),
        fx.withdraw_ix(fx.sol(), 1.0).await,
        fx.repay_ix(usdc_bank, 1_000.0, false).await,
        fx.end_close_ix(vec![]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderLegBankMismatch);

    // Taking more from the destination than is repaid.
    let ixs = [
        fx.start_close_ix(),
        fx.withdraw_ix(dst, 1_000.0).await,
        fx.repay_ix(usdc_bank, 990.0, false).await,
        fx.end_close_ix(vec![]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BorrowOrderFillMismatch);

    // Borrowing inside a close.
    let ixs = [
        fx.start_close_ix(),
        fx.withdraw_ix(dst, 1_000.0).await,
        fx.borrow_ix(usdc_bank, fx.keeper_usdc, 1.0).await,
        fx.repay_ix(usdc_bank, 1_000.0, false).await,
        fx.end_close_ix(vec![]).await,
    ];
    let res = fx.process(&ixs, &fx.keeper).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::ForbiddenIx);
    Ok(())
}
