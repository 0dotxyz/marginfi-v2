use anchor_lang::{InstructionData, ToAccountMetas};
use fixed_macro::types::I80F48 as fp;
use fixtures::assert_custom_error;
use fixtures::{
    bank::BankFixture, marginfi_account::MarginfiAccountFixture, prelude::*, ui_to_native,
};
use marginfi::{prelude::MarginfiError, state::bank::BankVaultType};
use marginfi_type_crate::types::{BalanceSide, OrderTrigger};
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

    let ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountRepay {
            amount: ui_to_native!(ui_amount, bank_f.mint.mint.decimals),
            repay_all,
        }
        .data(),
    };

    Ok(ix)
}

async fn make_withdraw_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    bank_f: &BankFixture,
    authority: Pubkey,
    destination: Pubkey,
    ui_amount: f64,
    withdraw_all: Option<bool>,
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

    ix.accounts.extend_from_slice(
        &marginfi_account_f
            .load_observation_account_metas(vec![], vec![])
            .await,
    );

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
        ctx.last_blockhash,
    );

    ctx.banks_client.process_transaction(tx).await
}

// User has $50 SOL, $200 Fixed, borrowing $50 USDC/PyUSD. We put an order for SOL/USDC (A/B) and
// SOL/PyUSD (A/D). We note two things here:
// * (1) that the SOL/USDC order cannot execute if it attempts to close the entire $50 SOL balance
// using withdraw-all, because orders can only close the liability side.
// * (2) the SOL/PyUSD order can't execute at all once the SOL balance is closed.
#[tokio::test]
async fn limit_orders_overlap_ab_nearly_closes_a_ad_fails_start() -> anyhow::Result<()> {
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

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix],
        Some(&keeper.pubkey()),
        &[&keeper],
        ctx.last_blockhash,
    );

    let result = ctx.banks_client.process_transaction(tx).await;

    assert_custom_error!(
        result.unwrap_err(),
        MarginfiError::LendingAccountBalanceNotFound
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
                max_slippage: 9_999,
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

    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderTriggerNotMet);
    Ok(())
}

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
    for ((asset_mint, _), (liab_mint, _)) in assets.iter().zip(liabilities.iter()) {
        let asset_bank = test_f.get_bank(asset_mint);
        let liab_bank = test_f.get_bank(liab_mint);
        test_f.refresh_blockhash().await;
        borrower
            .try_place_order(
                vec![asset_bank.key, liab_bank.key],
                OrderTrigger::StopLoss {
                    threshold: fp!(10.0).into(),
                    max_slippage: 0,
                },
            )
            .await?;
    }

    Ok(())
}
