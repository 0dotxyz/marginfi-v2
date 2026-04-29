use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token;
use fixed::types::I80F48;
use fixtures::prelude::*;
use fixtures::{assert_custom_error, native};
use marginfi::state::bank::{BankImpl, BankVaultType};
use marginfi::{assert_eq_with_tolerance, prelude::*};
use marginfi_type_crate::types::{BankConfig, BankConfigOpt};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::clock::Clock;
use solana_sdk::transaction::Transaction;
use solana_sdk::{instruction::Instruction, signer::Signer};
use test_case::test_case;

#[test_case(0.0, BankMint::Usdc)]
#[test_case(1_000.0, BankMint::Usdc)]
#[test_case(0.05, BankMint::Sol)]
#[test_case(0.05, BankMint::PyUSD)]
#[test_case(15_002.0, BankMint::PyUSD)]
#[test_case(0.0, BankMint::T22WithFee)]
#[test_case(0.05, BankMint::T22WithFee)]
#[test_case(15_002.0, BankMint::T22WithFee)]
#[test_case(0.05, BankMint::Fixed)]
#[test_case(5_000., BankMint::FixedLow)]
#[tokio::test]
async fn marginfi_account_deposit_success(
    deposit_amount: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let token_account_f = TokenAccountFixture::new(
        test_f.context.clone(),
        &test_f.get_bank(&bank_mint).mint,
        &test_f.payer(),
    )
    .await;

    // This is just to test that the account's last_update field is properly updated upon modification
    let pre_last_update = user_mfi_account_f.load().await.last_update;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 sec
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    let bank_f = test_f.get_bank_mut(&bank_mint);
    bank_f
        .mint
        .mint_to(&token_account_f.key, user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let pre_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    let res = user_mfi_account_f
        .try_bank_deposit(token_account_f.key, &bank_f, deposit_amount, None)
        .await;
    assert!(res.is_ok());

    let post_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let marginfi_account = user_mfi_account_f.load().await;
    if deposit_amount > 0.0 {
        assert_eq!(marginfi_account.last_update, pre_last_update + 1);
    }

    let expected_liquidity_vault_delta =
        I80F48::from(native!(deposit_amount, bank_f.mint.mint.decimals, f64));
    let actual_liquidity_vault_delta = I80F48::from(post_vault_balance - pre_vault_balance);
    assert_eq!(expected_liquidity_vault_delta, actual_liquidity_vault_delta);

    // If deposit_amount == 0, bank account doesn't get created -- no need to check balances
    if deposit_amount > 0. {
        let active_balance_count = marginfi_account
            .lending_account
            .get_active_balances_iter()
            .count();
        assert_eq!(1, active_balance_count);
        let maybe_balance = marginfi_account.lending_account.get_balance(&bank_f.key);
        assert!(maybe_balance.is_some());

        let balance = maybe_balance.unwrap();
        let accounted_user_balance_delta = bank_f
            .load()
            .await
            .get_asset_amount(balance.asset_shares.into())
            .unwrap();
        assert_eq_with_tolerance!(
            expected_liquidity_vault_delta,
            accounted_user_balance_delta,
            1
        );
    }

    Ok(())
}

#[test_case(1_000., 456., 2345., BankMint::Usdc)]
#[test_case(1_000., 456., 2345., BankMint::Sol)]
#[test_case(1_000., 456., 2345., BankMint::PyUSD)]
#[test_case(1_000., 456., 2345., BankMint::T22WithFee)]
#[test_case(1_000., 999.999999, 1000., BankMint::T22WithFee)]
#[test_case(1_000., 456., 2345., BankMint::Fixed)]
#[test_case(1_000., 456., 2345., BankMint::FixedLow)]
#[tokio::test]
async fn marginfi_account_deposit_failure_capacity_exceeded(
    deposit_cap: f64,
    deposit_amount_ok: f64,
    deposit_amount_failed: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount_failed);
    let bank_f = test_f.get_bank(&bank_mint);
    let user_token_account = bank_f
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    bank_f
        .update_config(
            BankConfigOpt {
                deposit_limit: Some(native!(deposit_cap, bank_f.mint.mint.decimals, f64)),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = user_mfi_account_f
        .try_bank_deposit(user_token_account.key, bank_f, deposit_amount_failed, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankAssetCapacityExceeded);

    let res = user_mfi_account_f
        .try_bank_deposit(user_token_account.key, bank_f, deposit_amount_ok, None)
        .await;
    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_account_deposit_failure_wrong_token_program() -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let deposit_amount = 1_000.;
    let bank_mint = BankMint::T22WithFee;

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let bank_f = test_f.get_bank(&bank_mint);
    let user_token_account = bank_f
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let marginfi_account = user_mfi_account_f.load().await;

    let accounts = marginfi::accounts::LendingAccountDeposit {
        group: marginfi_account.group,
        marginfi_account: user_mfi_account_f.key,
        authority: test_f.context.borrow().payer.pubkey(),
        bank: bank_f.key,
        signer_token_account: user_token_account.key,
        liquidity_vault: bank_f.get_vault(BankVaultType::Liquidity).0,
        token_program: spl_token::ID,
    }
    .to_account_metas(Some(true));

    let deposit_ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountDeposit {
            amount: native!(deposit_amount, bank_f.mint.mint.decimals, f64),
            deposit_up_to_limit: None,
        }
        .data(),
    };

    let tx = {
        let ctx = test_f.context.borrow();
        Transaction::new_signed_with_payer(
            &[deposit_ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        )
    };

    let ctx = test_f.context.borrow_mut();
    let res = ctx.banks_client.process_transaction(tx).await;
    assert!(res.is_err());

    Ok(())
}

#[test_case(1_000., 500., 800., 500., BankMint::Usdc)]
#[test_case(1_000., 500., 800., 500., BankMint::Sol)]
#[test_case(1_000., 500., 800., 500., BankMint::PyUSD)]
#[test_case(1_000., 500., 800., 500., BankMint::T22WithFee)]
#[tokio::test]
async fn marginfi_account_deposit_up_to_limit_success(
    deposit_cap: f64,
    first_deposit: f64,
    second_deposit: f64,
    third_deposit: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User
    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance =
        get_max_deposit_amount_pre_fee(first_deposit + second_deposit + third_deposit);
    let bank_f = test_f.get_bank(&bank_mint);
    let user_token_account = bank_f
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    bank_f
        .update_config(
            BankConfigOpt {
                deposit_limit: Some(native!(deposit_cap, bank_f.mint.mint.decimals, f64)),
                ..Default::default()
            },
            None,
        )
        .await?;

    // First deposit stays under limit
    let res = user_mfi_account_f
        .try_bank_deposit(user_token_account.key, bank_f, first_deposit, None)
        .await;
    assert!(res.is_ok());

    // Second deposit goes over limit -- with deposit_up_to_limit set
    let pre_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    let res = user_mfi_account_f
        .try_bank_deposit(user_token_account.key, bank_f, second_deposit, Some(true))
        .await;
    assert!(res.is_ok());

    let post_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    let expected_remaining_capacity = deposit_cap - first_deposit;
    let expected_second_deposit = I80F48::from(native!(
        expected_remaining_capacity.min(second_deposit),
        bank_f.mint.mint.decimals,
        f64
    ));
    let actual_deposit = I80F48::from(post_vault_balance - pre_vault_balance);

    assert_eq_with_tolerance!(expected_second_deposit, actual_deposit, 1);

    // Third deposit goes over limit -- with deposit_up_to_limit set -- when already at capacity
    // Should succeed with no balance changes
    let pre_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    let res = user_mfi_account_f
        .try_bank_deposit(user_token_account.key, bank_f, third_deposit, Some(true))
        .await;
    assert!(res.is_ok());

    let post_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    assert_eq!(pre_vault_balance, post_vault_balance);

    Ok(())
}

#[tokio::test]
async fn deposit_up_to_limit_post_interest_accrual() -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    const SECONDS_PER_THIRTY_DAYS: i64 = 30 * 24 * 60 * 60;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    deposit_limit: native!(1_000, "USDC"),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48::from_num(1u32).into(),
                    asset_weight_maint: I80F48::from_num(1u32).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let usdc_decimals = usdc_bank_f.mint.mint.decimals as u32;

    let lender_account = test_f.create_marginfi_account().await;
    let lender_usdc = usdc_bank_f
        .mint
        .create_token_account_and_mint_to(get_max_deposit_amount_pre_fee(600.0))
        .await;
    lender_account
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 600, None)
        .await?;

    // Liabilities on the USDC bank ensure interest accrues during the time advance.
    let borrower_account = test_f.create_marginfi_account().await;
    let borrower_sol = sol_bank_f
        .mint
        .create_token_account_and_mint_to(get_max_deposit_amount_pre_fee(10_000.0))
        .await;
    borrower_account
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 10_000, None)
        .await?;
    let borrower_usdc = usdc_bank_f.mint.create_empty_token_account().await;
    borrower_account
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 400)
        .await?;

    let stale_capacity_native: u64 = usdc_bank_f
        .load()
        .await
        .get_remaining_deposit_capacity()
        .expect("should compute remaining capacity before time advance");
    assert!(
        stale_capacity_native > 0,
        "pre-accrual capacity must be positive"
    );

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------
    // Advance 30 days, enough for interest to accrue and reduce the remaining
    // capacity, but not so much that total assets exceed the deposit limit.
    {
        let mut clock: Clock = test_f
            .context
            .borrow_mut()
            .banks_client
            .get_sysvar()
            .await?;
        clock.unix_timestamp += SECONDS_PER_THIRTY_DAYS;
        test_f.context.borrow_mut().set_sysvar(&clock);
    }

    // Prepare a third account with enough USDC to try both paths.
    let depositor_account = test_f.create_marginfi_account().await;
    let depositor_usdc = usdc_bank_f
        .mint
        .create_token_account_and_mint_to(get_max_deposit_amount_pre_fee(500.0))
        .await;

    let vault_before = usdc_bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    // Depositing exactly the pre-accrual remaining capacity must fail,
    // interest accrual (which runs first inside the instruction) causes the
    // post-accrual deposit total to exceed the limit.
    let stale_capacity_ui: f64 = stale_capacity_native as f64 / 10_f64.powi(usdc_decimals as i32);
    let res = depositor_account
        .try_bank_deposit(depositor_usdc.key, usdc_bank_f, stale_capacity_ui, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankAssetCapacityExceeded);

    // Deposit up to limit.
    depositor_account
        .try_bank_deposit(depositor_usdc.key, usdc_bank_f, 500, Some(true))
        .await
        .expect("deposit up to limit must succeed");

    // The amount actually transferred must be strictly less than the pre-accrual capacity.
    let vault_after = usdc_bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let actual_deposited = vault_after - vault_before;
    assert!(
        actual_deposited < stale_capacity_native,
        "actual deposit {} native must be < pre-accrual capacity {}",
        actual_deposited,
        stale_capacity_native
    );

    // Total assets must remain strictly below the deposit limit.
    let bank_final = usdc_bank_f.load().await;
    let total_after: I80F48 = bank_final.get_asset_amount(bank_final.total_asset_shares.into())?;
    let limit: I80F48 = I80F48::from_num(bank_final.config.deposit_limit);
    assert!(
        total_after < limit,
        "total deposits {} must remain below deposit_limit {}",
        total_after,
        limit
    );

    Ok(())
}
