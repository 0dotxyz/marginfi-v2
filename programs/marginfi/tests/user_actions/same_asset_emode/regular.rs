use super::common::*;
use fixed::types::I80F48 as FixedI80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::{assert_eq_with_tolerance, errors::MarginfiError};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{compute_same_asset_emode_weight, EmodeEntry},
};
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

fn midpoint(left: FixedI80F48, right: FixedI80F48) -> FixedI80F48 {
    (left + right) / FixedI80F48::from_num(2)
}

async fn configure_regular_liability_emode(
    test_f: &TestFixture,
    matching_collateral_bank: &BankFixture,
    classic_emode_collateral_bank: &BankFixture,
    liability_banks: &[&BankFixture],
    matching_collateral_weight: f64,
    classic_emode_collateral_weight: f64,
) -> anyhow::Result<()> {
    let matching_collateral_tag = 11u16;
    let classic_emode_collateral_tag = 22u16;

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(
            matching_collateral_bank,
            matching_collateral_tag,
            &[],
        )
        .await?;
    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(
            classic_emode_collateral_bank,
            classic_emode_collateral_tag,
            &[],
        )
        .await?;

    let entries = vec![
        EmodeEntry {
            collateral_bank_emode_tag: matching_collateral_tag,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: FixedI80F48::from_num(matching_collateral_weight).into(),
            asset_weight_maint: FixedI80F48::from_num(matching_collateral_weight).into(),
        },
        EmodeEntry {
            collateral_bank_emode_tag: classic_emode_collateral_tag,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: FixedI80F48::from_num(classic_emode_collateral_weight).into(),
            asset_weight_maint: FixedI80F48::from_num(classic_emode_collateral_weight).into(),
        },
    ];

    for (index, liability_bank) in liability_banks.iter().enumerate() {
        test_f
            .marginfi_group
            .try_lending_pool_configure_bank_emode(liability_bank, 101 + index as u16, &entries)
            .await?;
    }

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_regular_same_mint_position_is_healthy_then_turns_unhealthy_when_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = FixedI80F48::from_num(13.4);
    let healthy_init_leverage = 63;
    let healthy_maint_leverage = 65;
    let tightened_init_leverage = 60;
    let tightened_maint_leverage = 62;
    let sol_price = FixedI80F48::from_num(10);

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    let sol_bank_a = test_f.get_bank(&BankMint::Sol).clone();
    let sol_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED).await?;
    configure_same_asset_pair(
        &test_f,
        &sol_bank_a,
        &sol_bank_b,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let lp = test_f.create_marginfi_account().await;
    let lp_sol = test_f.sol_mint.create_token_account_and_mint_to(24.0).await;
    lp.try_bank_deposit(lp_sol.key, &sol_bank_b, 24.0, None)
        .await?;

    let user = test_f.create_marginfi_account().await;
    let user_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(deposit_ui.to_num::<f64>())
        .await;
    user.try_bank_deposit(user_sol.key, &sol_bank_a, deposit_ui.to_num::<f64>(), None)
        .await?;

    // Deposit = 13.4 SOL at $10, so the raw collateral value is $134.
    // Healthy init same-asset weight = 62 / 63 ~= 0.984127, so the healthy init liability limit
    // is $134 * 62 / 63 ~= $131.904762.
    // Tightened maint same-asset weight = 61 / 62 ~= 0.983871, so the tightened maint limit is
    // $134 * 61 / 62 ~= $131.838710.
    // Borrow is the midpoint between those two limits:
    // (($131.904762 + $131.838710) / 2) / $10 ~= 13.187174 SOL.
    // That leaves a tiny positive margin before the tighten and flips both init and maint health
    // negative after the tighten.
    let deposit_value = deposit_ui * sol_price;
    let healthy_init_limit = deposit_value
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = deposit_value
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui =
        (midpoint(healthy_init_limit, tightened_maint_limit) / sol_price).to_num::<f64>();

    let borrow_destination = test_f.sol_mint.create_empty_token_account().await;
    user.try_bank_borrow(borrow_destination.key, &sol_bank_b, borrow_ui)
        .await?;

    reconfigure_same_asset_leverage(&test_f, tightened_init_leverage, tightened_maint_leverage)
        .await?;
    user.try_lending_account_pulse_health().await?;

    let tightened = user.load().await;
    let tightened_init_health = FixedI80F48::from(tightened.health_cache.asset_value)
        - FixedI80F48::from(tightened.health_cache.liability_value);
    let tightened_maint_health = FixedI80F48::from(tightened.health_cache.asset_value_maint)
        - FixedI80F48::from(tightened.health_cache.liability_value_maint);
    assert!(tightened_init_health < FixedI80F48::ZERO);
    assert!(tightened_maint_health < FixedI80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_regular_same_value_borrow_fails_once_the_liability_mint_changes(
) -> anyhow::Result<()> {
    let deposit_ui = FixedI80F48::from_num(14.6);
    let same_asset_init_leverage = 57;
    let same_asset_maint_leverage = 60;
    let sol_price = FixedI80F48::from_num(10);

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    let sol_bank_a = test_f.get_bank(&BankMint::Sol).clone();
    let sol_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED).await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();
    configure_same_asset_pair(
        &test_f,
        &sol_bank_a,
        &sol_bank_b,
        0.5,
        0.5,
        same_asset_init_leverage,
        same_asset_maint_leverage,
    )
    .await?;

    let lp_sol_account = test_f.create_marginfi_account().await;
    let lp_sol = test_f.sol_mint.create_token_account_and_mint_to(24.0).await;
    lp_sol_account
        .try_bank_deposit(lp_sol.key, &sol_bank_b, 24.0, None)
        .await?;

    let lp_usdc_account = test_f.create_marginfi_account().await;
    let lp_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(300.0)
        .await;
    lp_usdc_account
        .try_bank_deposit(lp_usdc.key, &usdc_bank, 300.0, None)
        .await?;

    let user = test_f.create_marginfi_account().await;
    let user_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(deposit_ui.to_num::<f64>())
        .await;
    user.try_bank_deposit(user_sol.key, &sol_bank_a, deposit_ui.to_num::<f64>(), None)
        .await?;

    // Deposit = 14.6 SOL at $10, so the raw collateral value is $146.
    // Same-asset init weight = 56 / 57 ~= 0.982456, so the same-mint SOL liability limit is
    // 14.6 * 56 / 57 ~= 14.343860 SOL, or about $143.438596.
    // The equal-value USDC borrow keeps the same ~$143.398596 notional, but once the liability
    // mint changes the matching SOL collateral falls back to the plain regular 0.5 weight:
    // $146 * 0.5 = $73. That makes the same-value USDC borrow far too large and it must fail.
    let same_asset_limit = deposit_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(same_asset_init_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = (same_asset_limit - FixedI80F48::from_num(0.004)).to_num::<f64>();
    let equivalent_usdc_borrow_ui = (FixedI80F48::from_num(borrow_ui) * sol_price).to_num::<f64>();

    let user_sol_borrow = test_f.sol_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_sol_borrow.key, &sol_bank_b, borrow_ui)
        .await?;

    user.try_bank_repay(user_sol_borrow.key, &sol_bank_b, 0.0, Some(true))
        .await?;

    let user_usdc_borrow = test_f.usdc_mint.create_empty_token_account().await;
    let res = user
        .try_bank_borrow(user_usdc_borrow.key, &usdc_bank, equivalent_usdc_borrow_ui)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_regular_matching_collateral_uses_the_largest_available_weight(
) -> anyhow::Result<()> {
    let matching_collateral_ui = FixedI80F48::from_num(5.0);
    let classic_emode_collateral_ui = FixedI80F48::from_num(20.0);
    let plain_collateral_ui = FixedI80F48::from_num(6.0);
    let matching_liability_bank_b_ui = 2.2;
    let matching_liability_bank_c_ui = 1.8;
    let sol_price = FixedI80F48::from_num(10);
    let usdc_price = FixedI80F48::ONE;
    let fixed_price = FixedI80F48::from_num(2);

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Fixed,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    let sol_bank_a = test_f.get_bank(&BankMint::Sol).clone();
    let sol_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED).await?;
    let sol_bank_c =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED + 1).await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();
    let fixed_bank = test_f.get_bank(&BankMint::Fixed).clone();

    configure_same_asset_pair(&test_f, &sol_bank_a, &sol_bank_b, 0.81, 0.81, 4, 5).await?;
    set_bank_asset_weights(&sol_bank_c, 0.81, 0.81).await?;
    set_bank_asset_weights(&usdc_bank, 0.63, 0.63).await?;
    set_bank_asset_weights(&fixed_bank, 0.37, 0.37).await?;
    configure_regular_liability_emode(
        &test_f,
        &sol_bank_a,
        &usdc_bank,
        &[&sol_bank_b, &sol_bank_c],
        0.79,
        0.60,
    )
    .await?;

    let lp = test_f.create_marginfi_account().await;
    let lp_sol_b = test_f.sol_mint.create_token_account_and_mint_to(8.0).await;
    let lp_sol_c = test_f.sol_mint.create_token_account_and_mint_to(8.0).await;
    lp.try_bank_deposit(lp_sol_b.key, &sol_bank_b, 4.0, None)
        .await?;
    lp.try_bank_deposit(lp_sol_c.key, &sol_bank_c, 4.0, None)
        .await?;

    let user = test_f.create_marginfi_account().await;
    let user_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(matching_collateral_ui.to_num::<f64>())
        .await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(classic_emode_collateral_ui.to_num::<f64>())
        .await;
    let user_fixed = test_f
        .fixed_mint
        .create_token_account_and_mint_to(plain_collateral_ui.to_num::<f64>())
        .await;
    user.try_bank_deposit(
        user_sol.key,
        &sol_bank_a,
        matching_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;
    user.try_bank_deposit(
        user_usdc.key,
        &usdc_bank,
        classic_emode_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;
    user.try_bank_deposit(
        user_fixed.key,
        &fixed_bank,
        plain_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;

    let user_sol_borrow_a = test_f.sol_mint.create_empty_token_account().await;
    let user_sol_borrow_b = test_f.sol_mint.create_empty_token_account().await;
    user.try_bank_borrow(
        user_sol_borrow_a.key,
        &sol_bank_b,
        matching_liability_bank_b_ui,
    )
    .await?;
    user.try_bank_borrow(
        user_sol_borrow_b.key,
        &sol_bank_c,
        matching_liability_bank_c_ui,
    )
    .await?;

    // Phase 1: regular weight is the largest input for the matching SOL collateral.
    // Init SOL deposit contribution = 5 SOL * $10 * max(0.81, 0.79, 0.75) = $40.5.
    // Maint SOL deposit contribution = 5 SOL * $10 * max(0.81, 0.79, 0.8) = $40.5.
    // USDC deposit contribution = 20 USDC * max(0.63, 0.60) = $12.6.
    // FIXED deposit contribution = 6 * $2 * 0.37 = $4.44.
    // Total weighted assets = $57.54 for both init and maint because regular still wins.
    user.try_lending_account_pulse_health().await?;
    let regular_phase = user.load().await;
    let expected_regular_asset_value =
        matching_collateral_ui * sol_price * FixedI80F48::from_num(0.81)
            + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.63)
            + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.37);
    assert_eq!(
        FixedI80F48::from(regular_phase.health_cache.asset_value),
        expected_regular_asset_value
    );
    assert_eq!(
        FixedI80F48::from(regular_phase.health_cache.asset_value_maint),
        expected_regular_asset_value
    );

    // Phase 2: classic emode becomes the largest input for both the matching SOL collateral and
    // the USDC collateral, while FIXED keeps its regular weight.
    // SOL contribution = 5 SOL * $10 * max(0.81, 0.87, 0.75) = $43.5.
    // USDC contribution = 20 USDC * max(0.63, 0.70) = $14.0.
    // FIXED contribution = $4.44.
    // Total weighted assets = $61.94.
    configure_regular_liability_emode(
        &test_f,
        &sol_bank_a,
        &usdc_bank,
        &[&sol_bank_b, &sol_bank_c],
        0.87,
        0.70,
    )
    .await?;
    user.try_lending_account_pulse_health().await?;

    let classic_emode_phase = user.load().await;
    let expected_classic_emode_asset_value =
        matching_collateral_ui * sol_price * FixedI80F48::from_num(0.87)
            + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.70)
            + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.37);
    assert_eq!(
        FixedI80F48::from(classic_emode_phase.health_cache.asset_value),
        expected_classic_emode_asset_value
    );
    assert_eq!(
        FixedI80F48::from(classic_emode_phase.health_cache.asset_value_maint),
        expected_classic_emode_asset_value
    );

    // Phase 3: same-asset leverage is increased so the matching SOL collateral uses
    // init max(0.81, 0.87, 0.90) = 0.90 and maint max(0.81, 0.87, 10/11) = 10/11.
    // The non-matching collateral continues to use max(regular, emode) with no same-asset lift.
    // Init total weighted assets = 5 SOL * $10 * 0.90 + $14.0 + $4.44 = $63.44.
    // Maint total weighted assets = 5 SOL * $10 * (10/11) + $14.0 + $4.44 = $63.894545...
    reconfigure_same_asset_leverage(&test_f, 10, 11).await?;
    user.try_lending_account_pulse_health().await?;

    let same_asset_phase = user.load().await;
    let expected_same_asset_init_asset_value = matching_collateral_ui
        * sol_price
        * compute_same_asset_emode_weight(FixedI80F48::from_num(10), FixedI80F48::ONE)
        + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.70)
        + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.37);
    let expected_same_asset_maint_asset_value = matching_collateral_ui
        * sol_price
        * compute_same_asset_emode_weight(FixedI80F48::from_num(11), FixedI80F48::ONE)
        + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.70)
        + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.37);
    assert_eq_with_tolerance!(
        FixedI80F48::from(same_asset_phase.health_cache.asset_value),
        expected_same_asset_init_asset_value,
        FixedI80F48::from_num(0.000001)
    );
    assert_eq_with_tolerance!(
        FixedI80F48::from(same_asset_phase.health_cache.asset_value_maint),
        expected_same_asset_maint_asset_value,
        FixedI80F48::from_num(0.000001)
    );

    // Phase 4: same-asset leverage is reduced back to 4/5, so the matching SOL collateral falls
    // back to classic emode.
    // SOL contribution = 5 SOL * $10 * max(0.81, 0.87, 0.75) = $43.5.
    // USDC contribution = 20 USDC * max(0.63, 0.70) = $14.0.
    // FIXED contribution = $4.44.
    // Total weighted assets = $61.94 again for both init and maint because classic emode wins.
    reconfigure_same_asset_leverage(&test_f, 4, 5).await?;
    user.try_lending_account_pulse_health().await?;

    let reversed_classic_emode_phase = user.load().await;
    assert_eq!(
        FixedI80F48::from(reversed_classic_emode_phase.health_cache.asset_value),
        expected_classic_emode_asset_value
    );
    assert_eq!(
        FixedI80F48::from(reversed_classic_emode_phase.health_cache.asset_value_maint),
        expected_classic_emode_asset_value
    );

    // Phase 5: the matching-collateral classic emode weight is reduced back below the regular SOL
    // bank weight, while same-asset stays at 0.75 / 0.8. That returns the matching SOL collateral
    // to the plain regular bank weight.
    // Init SOL deposit contribution = 5 SOL * $10 * max(0.81, 0.79, 0.75) = $40.5.
    // Maint SOL deposit contribution = 5 SOL * $10 * max(0.81, 0.79, 0.8) = $40.5.
    // USDC deposit contribution = 20 USDC * max(0.63, 0.60) = $12.6.
    // FIXED deposit contribution = 6 * $2 * 0.37 = $4.44.
    // Total weighted assets = $57.54 again for both init and maint because regular wins.
    configure_regular_liability_emode(
        &test_f,
        &sol_bank_a,
        &usdc_bank,
        &[&sol_bank_b, &sol_bank_c],
        0.79,
        0.60,
    )
    .await?;
    user.try_lending_account_pulse_health().await?;

    let reversed_regular_phase = user.load().await;
    assert_eq!(
        FixedI80F48::from(reversed_regular_phase.health_cache.asset_value),
        expected_regular_asset_value
    );
    assert_eq!(
        FixedI80F48::from(reversed_regular_phase.health_cache.asset_value_maint),
        expected_regular_asset_value
    );

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_regular_same_asset_weight_turns_off_when_one_liability_mint_differs(
) -> anyhow::Result<()> {
    let matching_collateral_ui = FixedI80F48::from_num(4.6);
    let classic_emode_collateral_ui = FixedI80F48::from_num(18.0);
    let plain_collateral_ui = FixedI80F48::from_num(5.0);
    let odd_liability_ui = 0.75;
    let sol_price = FixedI80F48::from_num(10);
    let usdc_price = FixedI80F48::ONE;
    let fixed_price = FixedI80F48::from_num(2);

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Fixed,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    let sol_bank_a = test_f.get_bank(&BankMint::Sol).clone();
    let sol_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED).await?;
    let sol_bank_c =
        add_same_asset_regular_bank(&test_f, BankMint::Sol, SAME_ASSET_BANK_SEED + 1).await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();
    let fixed_bank = test_f.get_bank(&BankMint::Fixed).clone();
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD).clone();

    configure_same_asset_pair(&test_f, &sol_bank_a, &sol_bank_b, 0.80, 0.80, 12, 13).await?;
    set_bank_asset_weights(&sol_bank_c, 0.80, 0.80).await?;
    set_bank_asset_weights(&usdc_bank, 0.61, 0.61).await?;
    set_bank_asset_weights(&fixed_bank, 0.36, 0.36).await?;
    configure_regular_liability_emode(
        &test_f,
        &sol_bank_a,
        &usdc_bank,
        &[&sol_bank_b, &sol_bank_c, &pyusd_bank],
        0.88,
        0.69,
    )
    .await?;

    let lp = test_f.create_marginfi_account().await;
    let lp_sol_b = test_f.sol_mint.create_token_account_and_mint_to(8.0).await;
    let lp_sol_c = test_f.sol_mint.create_token_account_and_mint_to(8.0).await;
    let lp_pyusd = test_f
        .pyusd_mint
        .create_token_account_and_mint_to(40.0)
        .await;
    lp.try_bank_deposit(lp_sol_b.key, &sol_bank_b, 4.0, None)
        .await?;
    lp.try_bank_deposit(lp_sol_c.key, &sol_bank_c, 4.0, None)
        .await?;
    lp.try_bank_deposit(lp_pyusd.key, &pyusd_bank, 40.0, None)
        .await?;

    let user = test_f.create_marginfi_account().await;
    let user_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(matching_collateral_ui.to_num::<f64>())
        .await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(classic_emode_collateral_ui.to_num::<f64>())
        .await;
    let user_fixed = test_f
        .fixed_mint
        .create_token_account_and_mint_to(plain_collateral_ui.to_num::<f64>())
        .await;
    user.try_bank_deposit(
        user_sol.key,
        &sol_bank_a,
        matching_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;
    user.try_bank_deposit(
        user_usdc.key,
        &usdc_bank,
        classic_emode_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;
    user.try_bank_deposit(
        user_fixed.key,
        &fixed_bank,
        plain_collateral_ui.to_num::<f64>(),
        None,
    )
    .await?;

    let user_sol_borrow_a = test_f.sol_mint.create_empty_token_account().await;
    let user_sol_borrow_b = test_f.sol_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_sol_borrow_a.key, &sol_bank_b, 1.7)
        .await?;
    user.try_bank_borrow(user_sol_borrow_b.key, &sol_bank_c, 1.1)
        .await?;
    // With only SOL liabilities active, same-asset uses 11 / 12 ~= 0.916667 for init and
    // 12 / 13 ~= 0.923077 for maint, both larger than the regular 0.80 and classic emode 0.88
    // weights for the matching SOL collateral.
    // Matching SOL contribution:
    // - init = 4.6 SOL * $10 * 11 / 12 ~= $42.166667
    // - maint = 4.6 SOL * $10 * 12 / 13 ~= $42.461538
    // USDC contribution = 18 * $1 * 0.69 = $12.42.
    // FIXED contribution = 5 * $2 * 0.36 = $3.6.
    // Total weighted assets are therefore about $58.186667 for init and $58.481538 for maint.
    user.try_lending_account_pulse_health().await?;
    let same_asset_active = user.load().await;
    let expected_same_asset_init_asset_value = matching_collateral_ui
        * sol_price
        * compute_same_asset_emode_weight(FixedI80F48::from_num(12), FixedI80F48::ONE)
        + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.69)
        + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.36);
    let expected_same_asset_maint_asset_value = matching_collateral_ui
        * sol_price
        * compute_same_asset_emode_weight(FixedI80F48::from_num(13), FixedI80F48::ONE)
        + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.69)
        + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.36);
    assert_eq_with_tolerance!(
        FixedI80F48::from(same_asset_active.health_cache.asset_value),
        expected_same_asset_init_asset_value,
        FixedI80F48::from_num(0.000001)
    );
    assert_eq_with_tolerance!(
        FixedI80F48::from(same_asset_active.health_cache.asset_value_maint),
        expected_same_asset_maint_asset_value,
        FixedI80F48::from_num(0.000001)
    );

    let user_pyusd_borrow = test_f.pyusd_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_pyusd_borrow.key, &pyusd_bank, odd_liability_ui)
        .await?;
    user.try_lending_account_pulse_health().await?;
    // Adding a single PYUSD liability means the active liabilities are no longer all SOL, so the
    // same-asset lift disappears entirely.
    // The classic emode entries still remain common across every liability bank, so the matching
    // SOL collateral now falls back to max(regular, emode) = 0.88 instead of 11 / 12.
    // Matching SOL contribution becomes 4.6 SOL * $10 * 0.88 = $40.48.
    // USDC contribution stays at $12.42 and FIXED stays at $3.6.
    // Total weighted assets therefore collapse to $56.5 for both init and maint.
    let odd_liability_phase = user.load().await;
    let expected_without_same_asset =
        matching_collateral_ui * sol_price * FixedI80F48::from_num(0.88)
            + classic_emode_collateral_ui * usdc_price * FixedI80F48::from_num(0.69)
            + plain_collateral_ui * fixed_price * FixedI80F48::from_num(0.36);
    assert_eq_with_tolerance!(
        FixedI80F48::from(odd_liability_phase.health_cache.asset_value),
        expected_without_same_asset,
        FixedI80F48::from_num(0.000001)
    );
    assert_eq_with_tolerance!(
        FixedI80F48::from(odd_liability_phase.health_cache.asset_value_maint),
        expected_without_same_asset,
        FixedI80F48::from_num(0.000001)
    );

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_position_can_be_liquidated_after_leverage_tightens() -> anyhow::Result<()>
{
    let deposit_ui = FixedI80F48::from_num(118.4);
    let healthy_init_leverage = 20;
    let healthy_maint_leverage = 21;
    let tightened_init_leverage = 18;
    let tightened_maint_leverage = 19;
    let partial_liquidation_ui = 8.75;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank_a = test_f.get_bank(&BankMint::Usdc).clone();
    let usdc_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Usdc, SAME_ASSET_BANK_SEED).await?;
    configure_same_asset_pair(
        &test_f,
        &usdc_bank_a,
        &usdc_bank_b,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidator_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(300.0)
        .await;
    liquidator
        .try_bank_deposit(liquidator_usdc.key, &usdc_bank_b, 180.0, None)
        .await?;

    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(deposit_ui.to_num::<f64>())
        .await;
    liquidatee
        .try_bank_deposit(
            liquidatee_usdc.key,
            &usdc_bank_a,
            deposit_ui.to_num::<f64>(),
            None,
        )
        .await?;

    // Deposit = 118.4 USDC, so the raw collateral value is $118.4.
    // Healthy init same-asset weight = 19 / 20 = 0.95, so the healthy init liability limit is
    // $118.4 * 19 / 20 = $112.48.
    // Tightened maint same-asset weight = 18 / 19 ~= 0.947368, so the tightened maint limit is
    // $118.4 * 18 / 19 ~= $112.168421.
    // Borrow is the midpoint between those two limits, about $112.324211, so the account starts
    // healthy and becomes maintenance-unhealthy only after the small 20/21 -> 18/19 tighten.
    let healthy_init_limit = deposit_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = deposit_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = midpoint(healthy_init_limit, tightened_maint_limit).to_num::<f64>();

    let borrow_destination = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(borrow_destination.key, &usdc_bank_b, borrow_ui)
        .await?;

    reconfigure_same_asset_leverage(&test_f, tightened_init_leverage, tightened_maint_leverage)
        .await?;

    liquidator
        .try_liquidate(
            &liquidatee,
            &usdc_bank_a,
            partial_liquidation_ui,
            &usdc_bank_b,
        )
        .await?;

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_position_can_be_deleveraged_after_leverage_tightens() -> anyhow::Result<()>
{
    let deposit_ui = FixedI80F48::from_num(107.2);
    let healthy_init_leverage = 96;
    let healthy_maint_leverage = 99;
    let tightened_init_leverage = 92;
    let tightened_maint_leverage = 95;
    let partial_deleverage_ui = 9.25;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank_a = test_f.get_bank(&BankMint::Usdc).clone();
    let usdc_bank_b =
        add_same_asset_regular_bank(&test_f, BankMint::Usdc, SAME_ASSET_BANK_SEED).await?;
    configure_same_asset_pair(
        &test_f,
        &usdc_bank_a,
        &usdc_bank_b,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;
    let authority = Keypair::new();
    let risk_admin = test_f.payer().clone();

    let lp = test_f.create_marginfi_account().await;
    let lp_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(320.0)
        .await;
    lp.try_bank_deposit(lp_usdc_acc.key, &usdc_bank_b, 180.0, None)
        .await?;

    let deleveragee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let deleveragee_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(
            &authority.pubkey(),
            deposit_ui.to_num::<f64>(),
        )
        .await;
    let deleveragee_borrow_destination = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&authority.pubkey())
        .await;

    deleveragee
        .try_bank_deposit_with_authority(
            deleveragee_usdc.key,
            &usdc_bank_a,
            deposit_ui.to_num::<f64>(),
            None,
            &authority,
        )
        .await?;

    // Deposit = 107.2 USDC, so the raw collateral value is $107.2.
    // Healthy init same-asset weight = 95 / 96 ~= 0.989583, so the healthy init liability limit
    // is $107.2 * 95 / 96 ~= $106.083333.
    // Tightened maint same-asset weight = 94 / 95 ~= 0.989474, so the tightened maint limit is
    // $107.2 * 94 / 95 ~= $106.071579.
    // Borrow is the midpoint between those two limits, about $106.077456, so the account starts
    // just barely healthy and the 96/99 -> 92/95 tighten creates a real but narrow deleverage
    // need.

    let healthy_init_limit = deposit_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = deposit_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = midpoint(healthy_init_limit, tightened_maint_limit).to_num::<f64>();

    deleveragee
        .try_bank_borrow_with_authority(
            deleveragee_borrow_destination.key,
            &usdc_bank_b,
            borrow_ui,
            0,
            &authority,
        )
        .await?;

    reconfigure_same_asset_leverage(&test_f, tightened_init_leverage, tightened_maint_leverage)
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );
    let risk_admin_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(220.0)
        .await;

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(
            risk_admin_usdc_acc.key,
            &usdc_bank_a,
            partial_deleverage_ui,
            None,
        )
        .await;
    let repay_ix = deleveragee
        .make_repay_ix(
            risk_admin_usdc_acc.key,
            &usdc_bank_b,
            partial_deleverage_ui,
            None,
        )
        .await;
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    Ok(())
}
