use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::time;
use fixtures::{assert_eq_noise, prelude::*, ui_to_native};
use marginfi::state::bank::BankImpl;
use solana_program_test::*;

#[tokio::test]
async fn super_admin_withdraw_and_deposit_adjust_asset_share_value_from_non_one_base(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Sol,
                ..TestBankSetting::default()
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(5_000.0)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank, 5_000.0, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank, 1_000.0, None)
        .await?;
    let borrower_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(20_000.0)
        .await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank, 2_500.0)
        .await?;

    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += time!(180, "d");
        ctx.set_sysvar(&clock);
    }

    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    borrower
        .try_bank_repay(borrower_usdc.key, usdc_bank, 0.0, Some(true))
        .await?;

    let bank_pre = usdc_bank.load().await;
    let old_share_value: I80F48 = bank_pre.asset_share_value.into();
    let total_asset_shares: I80F48 = bank_pre.total_asset_shares.into();
    let total_assets_pre = bank_pre.get_asset_amount(total_asset_shares)?;

    assert!(old_share_value > I80F48::ONE);

    let withdraw_ui = 120.0;
    let withdraw_native = ui_to_native!(withdraw_ui, usdc_bank.mint.mint.decimals);
    let admin_destination = test_f.usdc_mint.create_empty_token_account().await;
    test_f
        .marginfi_group
        .try_super_admin_withdraw_native(usdc_bank, admin_destination.key, withdraw_native)
        .await?;
    assert_eq!(admin_destination.balance().await, withdraw_native);

    let bank_after_withdraw = usdc_bank.load().await;
    let share_after_withdraw: I80F48 = bank_after_withdraw.asset_share_value.into();
    let expected_share_after_withdraw = total_assets_pre
        .checked_sub(I80F48::from_num(withdraw_native))
        .unwrap()
        .checked_div(total_asset_shares)
        .unwrap();
    assert_eq_noise!(
        share_after_withdraw,
        expected_share_after_withdraw,
        I80F48!(0.000000001)
    );
    assert!(share_after_withdraw < old_share_value);

    let deposit_ui = 45.0;
    let deposit_native = ui_to_native!(deposit_ui, usdc_bank.mint.mint.decimals);
    let admin_funding = test_f
        .usdc_mint
        .create_token_account_and_mint_to(deposit_ui)
        .await;
    test_f
        .marginfi_group
        .try_super_admin_deposit_native(usdc_bank, admin_funding.key, deposit_native)
        .await?;
    assert_eq!(admin_funding.balance().await, 0);

    let bank_after_deposit = usdc_bank.load().await;
    let share_after_deposit: I80F48 = bank_after_deposit.asset_share_value.into();
    let expected_share_after_deposit = total_assets_pre
        .checked_sub(I80F48::from_num(withdraw_native))
        .unwrap()
        .checked_add(I80F48::from_num(deposit_native))
        .unwrap()
        .checked_div(total_asset_shares)
        .unwrap();
    assert_eq_noise!(
        share_after_deposit,
        expected_share_after_deposit,
        I80F48!(0.000000001)
    );
    assert!(share_after_deposit > share_after_withdraw);

    Ok(())
}

#[tokio::test]
async fn super_admin_haircut_then_all_depositors_withdraw_all_get_expected_amounts(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Sol,
                ..TestBankSetting::default()
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    let depositor_ui_amounts = [1_000.0, 2_000.0, 3_000.0];
    let mut depositors = Vec::new();
    for ui_amount in depositor_ui_amounts {
        let account = test_f.create_marginfi_account().await;
        let token_account = test_f
            .usdc_mint
            .create_token_account_and_mint_to(ui_amount)
            .await;
        account
            .try_bank_deposit(token_account.key, usdc_bank, ui_amount, None)
            .await?;
        depositors.push((account, token_account, ui_amount));
    }

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank, 1_000.0, None)
        .await?;
    let borrower_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(30_000.0)
        .await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank, 4_000.0)
        .await?;

    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += time!(365, "d");
        ctx.set_sysvar(&clock);
    }

    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    borrower
        .try_bank_repay(borrower_usdc.key, usdc_bank, 0.0, Some(true))
        .await?;

    let bank_before_haircut = usdc_bank.load().await;
    let old_share_value: I80F48 = bank_before_haircut.asset_share_value.into();
    let total_asset_shares: I80F48 = bank_before_haircut.total_asset_shares.into();
    let total_assets_before = bank_before_haircut.get_asset_amount(total_asset_shares)?;
    assert!(old_share_value > I80F48::ONE);

    let haircut_native = total_assets_before
        .checked_mul(I80F48!(0.02))
        .unwrap()
        .checked_floor()
        .unwrap()
        .to_num::<u64>();
    assert!(haircut_native > 0);

    let admin_destination = test_f.usdc_mint.create_empty_token_account().await;
    test_f
        .marginfi_group
        .try_super_admin_withdraw_native(usdc_bank, admin_destination.key, haircut_native)
        .await?;

    let bank_after_haircut = usdc_bank.load().await;
    let new_share_value: I80F48 = bank_after_haircut.asset_share_value.into();
    let expected_new_share = total_assets_before
        .checked_sub(I80F48::from_num(haircut_native))
        .unwrap()
        .checked_div(total_asset_shares)
        .unwrap();
    assert_eq_noise!(new_share_value, expected_new_share, I80F48!(0.000000001));

    let haircut_ratio = I80F48::from_num(haircut_native)
        .checked_div(total_assets_before)
        .unwrap();

    for (depositor, token_account, initial_ui) in &depositors {
        assert_eq!(token_account.balance().await, 0);

        let depositor_state = depositor.load().await;
        let shares: I80F48 = depositor_state
            .lending_account
            .get_balance(&usdc_bank.key)
            .unwrap()
            .asset_shares
            .into();

        let pre_haircut_amount = bank_before_haircut.get_asset_amount(shares)?;
        let expected_after_haircut_from_share = shares.checked_mul(new_share_value).unwrap();
        let proportional_haircut = pre_haircut_amount.checked_mul(haircut_ratio).unwrap();
        let initial_native =
            I80F48::from_num(ui_to_native!(*initial_ui, usdc_bank.mint.mint.decimals));
        let interest_earned = pre_haircut_amount.checked_sub(initial_native).unwrap();
        assert!(interest_earned > I80F48::ZERO);
        let expected_after_haircut_from_formula = initial_native
            .checked_add(interest_earned)
            .unwrap()
            .checked_sub(proportional_haircut)
            .unwrap();

        assert_eq_noise!(
            expected_after_haircut_from_share,
            expected_after_haircut_from_formula,
            I80F48!(0.5)
        );

        let expected_withdraw_native = expected_after_haircut_from_share
            .checked_floor()
            .unwrap()
            .checked_to_num::<u64>()
            .unwrap();

        depositor
            .try_bank_withdraw(token_account.key, usdc_bank, 0.0, Some(true))
            .await?;

        let actual_received = token_account.balance().await;
        assert_eq!(actual_received, expected_withdraw_native);
    }

    Ok(())
}
