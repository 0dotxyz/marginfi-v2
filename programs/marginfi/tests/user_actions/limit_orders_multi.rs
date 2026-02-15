use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::assert_custom_error;
use fixtures::{
    bank::BankFixture, marginfi_account::MarginfiAccountFixture, prelude::*, ui_to_native,
};
use marginfi::{prelude::MarginfiError, state::bank::BankVaultType};
use marginfi_type_crate::types::{centi_to_u32, BalanceSide, OrderTrigger};
use solana_program_test::tokio;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program, sysvar,
    transaction::Transaction,
};

use super::limit_orders_common::{create_account_with_positions, test_settings_16_banks};

async fn fund_keeper_for_fees(test_f: &TestFixture, keeper: &Keypair) -> anyhow::Result<()> {
    let mut ctx = test_f.context.borrow_mut();
    let rent = ctx.banks_client.get_rent().await?;
    let min_balance = rent.minimum_balance(0);
    let account = Account {
        lamports: min_balance + 1_000_000_000,
        data: vec![],
        owner: solana_sdk::system_program::ID,
        executable: false,
        rent_epoch: 0,
    };
    ctx.set_account(&keeper.pubkey(), &account.into());
    Ok(())
}

fn slippage_bps(bps: u32) -> u32 {
    centi_to_u32(I80F48::from_num(bps as f64 / 10_000.0))
}

async fn make_start_execute_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    order: Pubkey,
    executor: Pubkey,
) -> anyhow::Result<(Instruction, Pubkey)> {
    let marginfi_account = marginfi_account_f.load().await;
    let (execute_record, _) = find_execute_order_pda(&order);

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::StartExecuteOrder {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_f.key,
            fee_payer: executor,
            executor,
            order,
            execute_record,
            instruction_sysvar: sysvar::instructions::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountStartExecuteOrder {}.data(),
    };

    ix.accounts.extend_from_slice(
        &marginfi_account_f
            .load_observation_account_metas(vec![], vec![])
            .await,
    );

    Ok((ix, execute_record))
}

async fn make_end_execute_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    order: Pubkey,
    execute_record: Pubkey,
    executor: Pubkey,
    fee_recipient: Pubkey,
    exclude_banks: Vec<Pubkey>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::EndExecuteOrder {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_f.key,
            executor,
            fee_recipient,
            order,
            execute_record,
            fee_state: Pubkey::find_program_address(
                &[marginfi_type_crate::constants::FEE_STATE_SEED.as_bytes()],
                &marginfi::ID,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountEndExecuteOrder {}.data(),
    };

    ix.accounts.extend_from_slice(
        &marginfi_account_f
            .load_observation_account_metas(vec![], exclude_banks)
            .await,
    );

    Ok(ix)
}

// Note: In limit orders, ui_amount generally doesn't matter, because repay_all must always be enabled.
async fn make_repay_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    bank_f: &BankFixture,
    authority: Pubkey,
    signer_token_account: Pubkey,
    ui_amount: f64,
    repay_all: Option<bool>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut accounts = marginfi::accounts::LendingAccountRepay {
        group: marginfi_account.group,
        marginfi_account: marginfi_account_f.key,
        authority,
        bank: bank_f.key,
        signer_token_account,
        liquidity_vault: bank_f.get_vault(BankVaultType::Liquidity).0,
        token_program: bank_f.get_token_program(),
    }
    .to_account_metas(Some(true));

    if bank_f.mint.token_program == anchor_spl::token_2022::ID {
        accounts.push(AccountMeta::new_readonly(bank_f.mint.key, false));
    }

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountRepay {
            amount: ui_to_native!(ui_amount, bank_f.mint.mint.decimals),
            repay_all,
        }
        .data(),
    };

    if repay_all.unwrap_or(false) {
        ix.accounts.extend_from_slice(
            &marginfi_account_f
                .load_observation_account_metas(vec![bank_f.key], vec![])
                .await,
        );
    }

    Ok(ix)
}

async fn make_withdraw_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    bank_f: &BankFixture,
    authority: Pubkey,
    destination: Pubkey,
    ui_amount: f64,
    withdraw_all: Option<bool>,
    exclude_banks: Vec<Pubkey>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut accounts = marginfi::accounts::LendingAccountWithdraw {
        group: marginfi_account.group,
        marginfi_account: marginfi_account_f.key,
        authority,
        bank: bank_f.key,
        destination_token_account: destination,
        bank_liquidity_vault_authority: bank_f.get_vault_authority(BankVaultType::Liquidity).0,
        liquidity_vault: bank_f.get_vault(BankVaultType::Liquidity).0,
        token_program: bank_f.get_token_program(),
    }
    .to_account_metas(Some(true));

    if bank_f.mint.token_program == anchor_spl::token_2022::ID {
        accounts.push(AccountMeta::new_readonly(bank_f.mint.key, false));
    }

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountWithdraw {
            amount: ui_to_native!(ui_amount, bank_f.mint.mint.decimals),
            withdraw_all,
        }
        .data(),
    };

    if withdraw_all.unwrap_or(false) {
        ix.accounts.extend_from_slice(
            &marginfi_account_f
                .load_observation_account_metas_close_last(bank_f.key, vec![], exclude_banks)
                .await,
        );
    } else {
        ix.accounts.extend_from_slice(
            &marginfi_account_f
                .load_observation_account_metas(vec![], exclude_banks)
                .await,
        );
    }

    Ok(ix)
}

// Note: repay_all will be applied to the `liab_bank`
async fn execute_order_with_withdraw(
    test_f: &TestFixture,
    borrower: &MarginfiAccountFixture,
    order_pda: Pubkey,
    keeper: &Keypair,
    liab_bank: &BankFixture,
    liab_account: Pubkey,
    asset_bank: &BankFixture,
    asset_account: Pubkey,
    withdraw_amount: f64,
    withdraw_all: Option<bool>,
    exclude_banks: Vec<Pubkey>,
) -> Result<(), solana_program_test::BanksClientError> {
    let (start_ix, execute_record) = make_start_execute_ix(borrower, order_pda, keeper.pubkey())
        .await
        .unwrap();

    let repay_ix = make_repay_ix(
        borrower,
        liab_bank,
        keeper.pubkey(),
        liab_account,
        0.0,
        Some(true),
    )
    .await
    .unwrap();

    let withdraw_ix = make_withdraw_ix(
        borrower,
        asset_bank,
        keeper.pubkey(),
        asset_account,
        withdraw_amount,
        withdraw_all,
        vec![liab_bank.key],
    )
    .await
    .unwrap();

    let end_ix = make_end_execute_ix(
        borrower,
        order_pda,
        execute_record,
        keeper.pubkey(),
        keeper.pubkey(),
        exclude_banks,
    )
    .await
    .unwrap();

    test_f.refresh_blockhash().await;
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, repay_ix, withdraw_ix, end_ix],
        Some(&keeper.pubkey()),
        &[keeper],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    ctx.banks_client.process_transaction(tx).await
}

// User has $50 SOL, $200 Fixed, borrowing $50 USDC/PyUSD. We put an order for SOL/USDC (A/B) and
// SOL/PyUSD (A/D). We note several things here:
// * (1) that the SOL/USDC order cannot execute if it attempts to close the entire $50 SOL balance
// using withdraw-all, because orders can only close one position (the liability side).
// * (2) the SOL/PyUSD order can't execute at all once the SOL balance is consumed to execute
//   SOL/USDC, because there is not enough SOL to do so.
// * (3) The SOL/PyUSD order STAYS ON THE BOOKS. It does not automatically close, if the user
//   deposits SOL again later, it could become eligible to execute again! However, if the user
//   closes the SOL Balance and deposits SOL later, then this Order CANNOT EXECUTE, it is orphaned
//   from that Balance because it doesn't share a tag with it.
//
// This is pretty consistent with e.g. Drift, which leaves open orders on the books when a perp is
//   closed, so a stop loss might stay open when a take-profit is executed, etc. Here there is much
//   more naunce, since A could be involved in up to 30 orders (15x stop losses and take profits).
#[tokio::test]
async fn limit_orders_overlap_ab_nearly_closes_a_ad_fails() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // A/B (asset/liability) and C/D (asset/liability)
    // Set SOL and USDC to equal value so the A/B order can close A without slippage.
    let assets = vec![(BankMint::Sol, 5.0), (BankMint::Fixed, 100.0)];
    let liabilities = vec![(BankMint::Usdc, 50.0), (BankMint::PyUSD, 50.0)];

    let borrower = create_account_with_positions(&test_f, &assets, &liabilities).await?;

    // set emissions destination to the authority before placing order
    let authority = borrower.load().await.authority;
    borrower.try_set_emissions_destination(authority).await?;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);

    // Order on A/B
    let order_ab = borrower
        .try_place_order(
            vec![sol_bank.key, usdc_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    test_f.refresh_blockhash().await;
    // Order on A/D
    let order_ad = borrower
        .try_place_order(
            vec![sol_bank.key, pyusd_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;
    let keeper_usdc_account = usdc_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let _keeper_pyusd_account = pyusd_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_sol_account = sol_bank
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    // Same thing with the withdraw_all flag explicitly set
    let result = execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ab,
        &keeper,
        usdc_bank,
        keeper_usdc_account,
        sol_bank,
        keeper_sol_account,
        5.0,
        Some(true),
        vec![usdc_bank.key, sol_bank.key],
    )
    .await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::IllegalBalanceState);

    // Execute A/B and withdraw most of A (leave a small balance so execution succeeds)
    execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ab,
        &keeper,
        usdc_bank,
        keeper_usdc_account,
        sol_bank,
        keeper_sol_account,
        4.9,
        None,
        vec![usdc_bank.key],
    )
    .await?;

    // Keeper cannot close A/D yet because both tags are still live (SOL not closed yet).
    test_f.refresh_blockhash().await;
    let result = borrower
        .try_keeper_close_order(order_ad, &keeper, keeper.pubkey())
        .await;
    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::LiquidatorOrderCloseNotAllowed
    );

    // Now close A outside of order execution.
    test_f.refresh_blockhash().await;
    let sol_destination = sol_bank.mint.create_empty_token_account().await;
    borrower
        .try_bank_withdraw(sol_destination.key, sol_bank, 0.0, Some(true))
        .await?;

    test_f.refresh_blockhash().await;
    // A is closed, so start on A/D should fail
    let (start_ix, _execute_record) =
        make_start_execute_ix(&borrower, order_ad, keeper.pubkey()).await?;

    let result = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix],
            Some(&keeper.pubkey()),
            &[&keeper],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client.process_transaction(tx).await
    };

    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::LendingAccountBalanceNotFound
    );

    // The SOL/USDC order should have been closed as part of execution.
    let order_ab_after = test_f.try_load(&order_ab).await?;
    assert!(
        order_ab_after.is_none(),
        "SOL/USDC order should already be closed"
    );

    // The user can close the SOL/PyUSD order explicitly. Note: once the SOL balance is closed,
    // a later SOL deposit creates a new tag, so the old order is no longer executable.
    let fee_recipient = test_f.payer();
    borrower.try_close_order(order_ad, fee_recipient).await?;
    let order_ad_after = test_f.try_load(&order_ad).await?;
    assert!(
        order_ad_after.is_none(),
        "SOL/PyUSD order should be closed after close_order"
    );

    Ok(())
}

// Here we have essentially the same setup as above, noting that withdrawing $50 from A is perfectly
// fine as long as we don't withdraw-all.
#[tokio::test]
async fn limit_orders_overlap_ab_nearly_closes_a_no_withdraw_all_ok() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let assets = vec![(BankMint::Sol, 5.0), (BankMint::Fixed, 100.0)];
    let liabilities = vec![(BankMint::Usdc, 50.0), (BankMint::PyUSD, 50.0)];

    let borrower = create_account_with_positions(&test_f, &assets, &liabilities).await?;

    // set emissions destination to the authority before placing order
    let authority = borrower.load().await.authority;
    borrower.try_set_emissions_destination(authority).await?;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Order on A/B
    let order_ab = borrower
        .try_place_order(
            vec![sol_bank.key, usdc_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;
    let keeper_usdc_account = usdc_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_sol_account = sol_bank
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    // Execute A/B and withdraw the full amount without closing the asset balance.
    execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ab,
        &keeper,
        usdc_bank,
        keeper_usdc_account,
        sol_bank,
        keeper_sol_account,
        5.0,  // <- the entire SOL balance
        None, // <- not withdraw_all
        vec![usdc_bank.key],
    )
    .await?;

    let mfi_after = borrower.load().await;
    let sol_balance = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == sol_bank.key);
    assert!(sol_balance.is_some(), "SOL balance should remain");
    let sol_balance = sol_balance.unwrap();
    assert!(sol_balance.is_active(), "SOL balance should remain active");
    assert!(
        sol_balance.is_empty(BalanceSide::Assets),
        "SOL asset shares should be near zero"
    );
    assert!(
        sol_balance.is_empty(BalanceSide::Liabilities),
        "SOL liability shares should be zero"
    );

    let usdc_balance = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == usdc_bank.key);
    assert!(usdc_balance.is_none(), "USDC liability should be closed");

    Ok(())
}

// Here we demonstrate that fully closing the SOL balance and re-opening it later doesn't work, the
// SOL/PyUSD Order was created with a different Balance tag and cannot use the new SOL Balance. If
// the user had deposited SOL without first doing withdraw_all to close the SOL balance, then the
// SOL/PyUSD order would still be able to execute.
#[tokio::test]
async fn limit_orders_overlap_ab_close_a_reopen_a_ad_fails() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let assets = vec![(BankMint::Sol, 5.0), (BankMint::Fixed, 100.0)];
    let liabilities = vec![(BankMint::Usdc, 50.0), (BankMint::PyUSD, 50.0)];

    let borrower = create_account_with_positions(&test_f, &assets, &liabilities).await?;

    // set emissions destination to the authority before placing order
    let authority = borrower.load().await.authority;
    borrower.try_set_emissions_destination(authority).await?;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);

    // Order on A/B
    let order_ab = borrower
        .try_place_order(
            vec![sol_bank.key, usdc_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    // Order on A/D
    let order_ad = borrower
        .try_place_order(
            vec![sol_bank.key, pyusd_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;
    let keeper_usdc_account = usdc_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_sol_account = sol_bank
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    // Execute A/B and leave a tiny SOL balance so the order can complete.
    execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ab,
        &keeper,
        usdc_bank,
        keeper_usdc_account,
        sol_bank,
        keeper_sol_account,
        4.9,
        None,
        vec![usdc_bank.key],
    )
    .await?;

    // Close SOL outside order execution.
    test_f.refresh_blockhash().await;
    let sol_destination = sol_bank.mint.create_empty_token_account().await;
    borrower
        .try_bank_withdraw(sol_destination.key, sol_bank, 0.0, Some(true))
        .await?;

    // Reopen SOL with a new deposit (new tag, old order tag is now orphaned).
    test_f.refresh_blockhash().await;
    let sol_deposit = sol_bank.mint.create_token_account_and_mint_to(1.0).await;
    borrower
        .try_bank_deposit(sol_deposit.key, sol_bank, 1.0, None)
        .await?;

    let order_ad_after = borrower.load_order(order_ad).await;
    let mfi_after = borrower.load().await;
    let sol_balance = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == sol_bank.key)
        .unwrap();
    assert_ne!(
        sol_balance.tag, order_ad_after.tags[0],
        "reopened SOL balance should not reuse the old order tag"
    );

    test_f.refresh_blockhash().await;
    let (start_ix, _execute_record) =
        make_start_execute_ix(&borrower, order_ad, keeper.pubkey()).await?;
    let result = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix],
            Some(&keeper.pubkey()),
            &[&keeper],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client.process_transaction(tx).await
    };
    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::LendingAccountBalanceNotFound
    );

    Ok(())
}

// Here we hit OrderExecutionOverWithdrawal by attempting to withdraw too much.
#[tokio::test]
async fn limit_orders_overlap_ab_reduces_a_ad_fails_end() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let assets = vec![(BankMint::Sol, 20.0), (BankMint::Fixed, 100.0)];
    let liabilities = vec![(BankMint::Usdc, 50.0), (BankMint::PyUSD, 50.0)];

    let borrower = create_account_with_positions(&test_f, &assets, &liabilities).await?;

    // set emissions destination to the authority before placing order
    let authority = borrower.load().await.authority;
    borrower.try_set_emissions_destination(authority).await?;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);

    // Order on A/B with large slippage to allow big withdrawal
    let order_ab = borrower
        .try_place_order(
            vec![sol_bank.key, usdc_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(200.0).into(),
                // Note: the user is a silly person that sets 99.99% slippage.
                max_slippage: slippage_bps(9_999),
            },
        )
        .await?;

    test_f.refresh_blockhash().await;
    // Order on A/D with zero slippage (no profit allowed)
    let order_ad = borrower
        .try_place_order(
            vec![sol_bank.key, pyusd_bank.key],
            OrderTrigger::StopLoss {
                threshold: fp!(200.0).into(),
                max_slippage: 0,
            },
        )
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;
    let keeper_usdc_account = usdc_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_pyusd_account = pyusd_bank
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_sol_account = sol_bank
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    // Execute A/B and withdraw most of A (leave ~6 SOL)
    execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ab,
        &keeper,
        usdc_bank,
        keeper_usdc_account,
        sol_bank,
        keeper_sol_account,
        14.0,
        None,
        vec![usdc_bank.key],
    )
    .await?;

    // At this point there is 6 SOL left ($60) and a debt of $50 PyUSD

    test_f.refresh_blockhash().await;
    // Execute A/D, but withdraw slightly too much from remaining A (end should fail)
    let result = execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ad,
        &keeper,
        pyusd_bank,
        keeper_pyusd_account,
        sol_bank,
        keeper_sol_account,
        5.1,
        None,
        vec![pyusd_bank.key],
    )
    .await;

    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::OrderExecutionOverWithdrawal
    );

    // Executing A/D with just enough still works as expected.
    let result = execute_order_with_withdraw(
        &test_f,
        &borrower,
        order_ad,
        &keeper,
        pyusd_bank,
        keeper_pyusd_account,
        sol_bank,
        keeper_sol_account,
        5.0,
        None,
        vec![pyusd_bank.key],
    )
    .await;

    assert!(result.is_ok());
    Ok(())
}

// Here we demonstrate the theoretical max orders, 64. This would be quite silly to do in practice,
// but hey, it's your life, and they still execute as expected.
#[tokio::test]
async fn limit_orders_open_max_count() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let test_f = TestFixture::new(Some(test_settings_16_banks())).await;

    let assets = vec![
        (BankMint::Sol, 25.0),
        (BankMint::Fixed, 25.0),
        (BankMint::SolEquivalent, 25.0),
        (BankMint::SolEquivalent1, 25.0),
        (BankMint::SolEquivalent2, 25.0),
        (BankMint::SolEquivalent3, 25.0),
        (BankMint::SolEquivalent4, 25.0),
        (BankMint::SolEquivalent5, 25.0),
    ];
    let liabilities = vec![
        (BankMint::Usdc, 5.0),
        (BankMint::PyUSD, 5.0),
        (BankMint::UsdcT22, 5.0),
        (BankMint::FixedLow, 5.0),
        (BankMint::T22WithFee, 5.0),
        (BankMint::SolSwbPull, 5.0),
        (BankMint::SolSwbOrigFee, 5.0),
        (BankMint::SolEquivalent6, 5.0),
    ];

    let borrower = create_account_with_positions(&test_f, &assets, &liabilities).await?;

    // set emissions destination to the authority before placing order
    let authority = borrower.load().await.authority;
    borrower.try_set_emissions_destination(authority).await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let mut all_orders: Vec<Pubkey> = Vec::new();
    let mut exec_orders: Vec<(Pubkey, BankFixture, BankFixture, f64)> = Vec::new();

    for (asset_idx, (asset_mint, _)) in assets.iter().enumerate() {
        let asset_bank = test_f.get_bank(asset_mint);
        let asset_price = get_mint_price(asset_mint.clone());

        for (liab_idx, (liab_mint, liab_amount)) in liabilities.iter().enumerate() {
            let liab_bank = test_f.get_bank(liab_mint);
            let liab_price = get_mint_price(liab_mint.clone());
            let withdraw_amount = (liab_amount * liab_price) / asset_price;

            test_f.refresh_blockhash().await;
            let order = borrower
                .try_place_order(
                    vec![asset_bank.key, liab_bank.key],
                    OrderTrigger::StopLoss {
                        threshold: fp!(1_000_000.0).into(),
                        max_slippage: 0,
                    },
                )
                .await?;

            if asset_idx == liab_idx {
                // Leave a tiny buffer to avoid rounding pushing end net below start net.
                exec_orders.push((
                    order,
                    asset_bank.clone(),
                    liab_bank.clone(),
                    withdraw_amount * 0.999,
                ));
            }

            all_orders.push(order);
        }
    }

    assert_eq!(
        all_orders.len(),
        assets.len() * liabilities.len(),
        "expected max orders"
    );

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;

    let mut keeper_asset_accounts: std::collections::HashMap<Pubkey, Pubkey> =
        std::collections::HashMap::new();
    let mut keeper_liab_accounts: std::collections::HashMap<Pubkey, Pubkey> =
        std::collections::HashMap::new();

    for (bank_mint, _) in assets.iter() {
        let bank = test_f.get_bank(bank_mint);
        let account = bank
            .mint
            .create_empty_token_account_with_owner(&keeper.pubkey())
            .await
            .key;
        keeper_asset_accounts.insert(bank.key, account);
    }

    for (bank_mint, amount) in liabilities.iter() {
        let bank = test_f.get_bank(bank_mint);
        let mint_amount = get_max_deposit_amount_pre_fee(*amount);
        let account = bank
            .mint
            .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), mint_amount)
            .await
            .key;
        keeper_liab_accounts.insert(bank.key, account);
    }

    let mut executed = std::collections::HashSet::new();

    // Execute a subset of orders (one per corresponding asset/liability index).
    for (order_pda, asset_bank, liab_bank, withdraw_amount) in exec_orders.iter() {
        execute_order_with_withdraw(
            &test_f,
            &borrower,
            *order_pda,
            &keeper,
            liab_bank,
            *keeper_liab_accounts
                .get(&liab_bank.key)
                .expect("missing keeper liab account"),
            asset_bank,
            *keeper_asset_accounts
                .get(&asset_bank.key)
                .expect("missing keeper asset account"),
            *withdraw_amount,
            None,
            vec![liab_bank.key],
        )
        .await?;

        executed.insert(*order_pda);

        let order_after = test_f.try_load(order_pda).await?;
        assert!(
            order_after.is_none(),
            "stop-loss order should be closed after execution"
        );
    }

    for order_pda in all_orders.iter() {
        if executed.contains(order_pda) {
            continue;
        }
        let order_after = test_f.try_load(order_pda).await?;
        assert!(order_after.is_some(), "order should remain open");
    }

    Ok(())
}
