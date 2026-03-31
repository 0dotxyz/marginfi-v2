use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::{
    prelude::MarginfiError,
    state::bank::{BankImpl, BankVaultType},
};
use marginfi_type_crate::types::BankConfig;
use pretty_assertions::assert_eq;
use solana_program_test::*;
use test_case::test_case;

#[test_case(BankMint::Usdc, BankMint::Sol)]
#[test_case(BankMint::Sol, BankMint::Usdc)]
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_failure_not_bankrupt(
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let borrow_amount = 10_000.;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let debt_bank_f = test_f.get_bank(&debt_mint);

    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank_f, &user_mfi_account_f)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountNotBankrupt);

    Ok(())
}

#[test_case(BankMint::Usdc, BankMint::Sol)]
#[test_case(BankMint::Sol, BankMint::Usdc)]
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_failure_no_debt(
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let borrow_amount = 10_000.;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let mut user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let collateral_bank_f = test_f.get_bank(&collateral_mint);

    // Artificially nullify the collateral to place the account in a bankrupt state
    user_mfi_account_f
        .nullify_assets_for_bank(collateral_bank_f.key)
        .await?;

    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(collateral_bank_f, &user_mfi_account_f)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BalanceNotBadDebt);

    Ok(())
}

#[test_case(BankMint::Usdc, BankMint::Sol)]
#[test_case(BankMint::Sol, BankMint::Usdc)]
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_success(
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let borrow_amount = 10_000.;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let mut user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Artificially nullify the collateral to place the account in a bankrupt state
    let collateral_bank_pk = test_f.get_bank(&collateral_mint).key;
    user_mfi_account_f
        .nullify_assets_for_bank(collateral_bank_pk)
        .await?;

    let debt_bank_f = test_f.get_bank(&debt_mint);

    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank_f, &user_mfi_account_f)
        .await;
    assert!(res.is_ok());

    Ok(())
}

#[test_case(10_000., BankMint::Usdc, BankMint::Sol)]
#[test_case(10_000., BankMint::Sol, BankMint::Usdc)]
#[test_case(10_000., BankMint::Usdc, BankMint::SolSwbOrigFee)] // Sol @ ~ $153
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_success_fully_insured(
    borrow_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let mut user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    assert_eq!(
        user_debt_token_account_f.balance().await,
        native!(
            borrow_amount,
            test_f.get_bank(&debt_mint).mint.mint.decimals,
            f64
        )
    );

    // Artificially nullify the collateral to place the account in a bankrupt state
    let collateral_bank_pk = test_f.get_bank(&collateral_mint).key;
    user_mfi_account_f
        .nullify_assets_for_bank(collateral_bank_pk)
        .await?;

    let borrow_amount_native = native!(
        borrow_amount,
        test_f.get_bank(&debt_mint).mint.mint.decimals,
        f64
    );
    let origination_fee_rate: I80F48 = test_f
        .get_bank(&debt_mint)
        .load()
        .await
        .config
        .interest_rate_config
        .protocol_origination_fee
        .into();
    let origination_fee: I80F48 = I80F48::from_num(borrow_amount_native)
        .checked_mul(origination_fee_rate)
        .unwrap()
        .ceil();
    let origination_fee_u64: u64 = origination_fee.checked_to_num().expect("out of bounds");
    let actual_borrow_position = borrow_amount_native + origination_fee_u64;
    let actual_borrow_position_ui = actual_borrow_position as f64
        / 10_u64.pow(test_f.get_bank(&debt_mint).mint.mint.decimals as u32) as f64;

    {
        let (insurance_vault, _) = test_f
            .get_bank(&debt_mint)
            .get_vault(BankVaultType::Insurance);
        let max_amount_to_cover_bad_debt =
            get_max_deposit_amount_pre_fee(actual_borrow_position_ui);

        test_f
            .get_bank_mut(&debt_mint)
            .mint
            .mint_to(&insurance_vault, max_amount_to_cover_bad_debt)
            .await;
    }

    let debt_bank = test_f.get_bank(&debt_mint);

    let (pre_liquidity_vault_balance, pre_insurance_vault_balance) = (
        debt_bank
            .get_vault_token_account(BankVaultType::Liquidity)
            .await
            .balance()
            .await,
        debt_bank
            .get_vault_token_account(BankVaultType::Insurance)
            .await
            .balance()
            .await,
    );

    test_f
        .marginfi_group
        .try_handle_bankruptcy(test_f.get_bank(&debt_mint), &user_mfi_account_f)
        .await?;

    let (post_liquidity_vault_balance, post_insurance_vault_balance) = (
        debt_bank
            .get_vault_token_account(BankVaultType::Liquidity)
            .await
            .balance()
            .await,
        debt_bank
            .get_vault_token_account(BankVaultType::Insurance)
            .await
            .balance()
            .await,
    );

    let user_mfi_account = user_mfi_account_f.load().await;
    // Due to balances sorting, debt may be not at index 1 -> find its actual index first
    let debt_bank_f = test_f.get_bank(&debt_mint).key;
    let debt_index = user_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == debt_bank_f)
        .unwrap();
    let user_collateral_balance = user_mfi_account.lending_account.balances[debt_index];

    // Check that all user debt has been covered
    assert_eq!(
        I80F48::from(user_collateral_balance.liability_shares),
        I80F48::ZERO
    );

    let lp_mfi_account = lp_mfi_account_f.load().await;
    let debt_bank = test_f.get_bank(&debt_mint).load().await;

    let lp_collateral_index = lp_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.bank_pk == debt_bank_f)
        .unwrap();

    let lp_collateral_value = debt_bank.get_asset_amount(
        lp_mfi_account.lending_account.balances[lp_collateral_index]
            .asset_shares
            .into(),
    )?;

    assert_eq_noise!(
        lp_collateral_value,
        I80F48::from(native!(
            lp_deposit_amount,
            test_f.get_bank(&debt_mint).mint.mint.decimals,
            f64
        )),
        I80F48::ONE
    );

    let expected_insurance_vault_delta = I80F48::from(actual_borrow_position);

    let expected_liquidity_vault_delta = I80F48::from(actual_borrow_position);

    let actual_liquidity_vault_delta = post_liquidity_vault_balance - pre_liquidity_vault_balance;
    let actual_insurance_vault_delta = pre_insurance_vault_balance - post_insurance_vault_balance;

    assert_eq!(expected_insurance_vault_delta, actual_insurance_vault_delta);
    assert_eq!(expected_liquidity_vault_delta, actual_liquidity_vault_delta);

    // Test account is disabled

    // Deposit 1 SOL
    let res = user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            1,
            None,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountDisabled);

    // Withdraw 1 SOL
    let res = user_mfi_account_f
        .try_bank_withdraw(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            1,
            None,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountDisabled);

    // Borrow 1 USDC
    let res = user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            1,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountDisabled);

    // Repay 1 USDC
    let res = user_mfi_account_f
        .try_bank_repay(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            1,
            None,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountDisabled);

    Ok(())
}

#[test_case(10_000., 5000., BankMint::Usdc, BankMint::Sol)]
#[test_case(10_000., 5000., BankMint::Sol, BankMint::Usdc)]
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_success_partially_insured(
    borrow_amount: f64,
    initial_insurance_vault_balance: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let mut user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Artificially nullify the collateral to place the account in a bankrupt state
    let collateral_bank_pk = test_f.get_bank(&collateral_mint).key;
    user_mfi_account_f
        .nullify_assets_for_bank(collateral_bank_pk)
        .await?;

    // Load up the insurance vault with the requested balance
    let insurance_vault = test_f.get_bank(&debt_mint).load().await.insurance_vault;
    test_f
        .get_bank_mut(&debt_mint)
        .mint
        .mint_to(&insurance_vault, initial_insurance_vault_balance)
        .await;

    let debt_bank_f = test_f.get_bank(&debt_mint);
    let debt_bank = debt_bank_f.load().await;
    let collateral_bank = test_f.get_bank(&collateral_mint).load().await;

    // Due to balances sorting, debt may be not at index 1 -> find it first
    let user_mfi_account = user_mfi_account_f.load().await;
    let debt_index = user_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == debt_bank_f.key)
        .unwrap();

    let (pre_lp_collateral_amount, pre_user_debt_amount, pre_liquidity_vault_balance) = (
        collateral_bank.get_liability_amount(
            lp_mfi_account_f.load().await.lending_account.balances[0]
                .asset_shares
                .into(),
        )?,
        debt_bank.get_liability_amount(
            user_mfi_account_f.load().await.lending_account.balances[debt_index]
                .liability_shares
                .into(),
        )?,
        debt_bank_f
            .get_vault_token_account(BankVaultType::Liquidity)
            .await
            .balance()
            .await,
    );

    test_f
        .marginfi_group
        .try_handle_bankruptcy(test_f.get_bank(&debt_mint), &user_mfi_account_f)
        .await?;

    let borrower_mfi_account = user_mfi_account_f.load().await;
    let (post_user_debt_balance, post_liquidity_vault_balance) = (
        borrower_mfi_account.lending_account.balances[debt_index],
        debt_bank_f
            .get_vault_token_account(BankVaultType::Liquidity)
            .await
            .balance()
            .await,
    );

    assert_eq!(
        I80F48::from(post_user_debt_balance.liability_shares),
        I80F48::ZERO
    );

    let lp_mfi_account = lp_mfi_account_f.load().await;
    let debt_bank = test_f.get_bank(&debt_mint).load().await;

    let actual_post_lender_collateral_amount = debt_bank.get_asset_amount(
        lp_mfi_account.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    let initial_insurance_vault_balance_native = native!(
        initial_insurance_vault_balance,
        test_f.get_bank(&debt_mint).mint.mint.decimals,
        f64
    );
    let available_insurance_amount = initial_insurance_vault_balance_native;

    let amount_not_covered = pre_user_debt_amount - I80F48::from(available_insurance_amount);

    let expected_post_lender_collateral_amount = pre_lp_collateral_amount - amount_not_covered;
    assert_eq_noise!(
        expected_post_lender_collateral_amount,
        actual_post_lender_collateral_amount,
        I80F48::ONE
    );

    let insurance_amount = test_f
        .get_bank(&debt_mint)
        .get_vault_token_account(BankVaultType::Insurance)
        .await;

    assert_eq!(insurance_amount.balance().await, 0);

    let actual_liquidity_vault_delta = post_liquidity_vault_balance - pre_liquidity_vault_balance;
    assert_eq!(available_insurance_amount, actual_liquidity_vault_delta);

    Ok(())
}

#[test_case(10_000., BankMint::Usdc, BankMint::Sol)]
#[test_case(10_000., BankMint::Sol, BankMint::Usdc)]
#[tokio::test]
async fn marginfi_group_handle_bankruptcy_success_not_insured(
    borrow_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let mut user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;
    user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            test_f.get_bank(&debt_mint),
            borrow_amount,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Artificially nullify the collateral to place the account in a bankrupt state
    let collateral_bank_pk = test_f.get_bank(&collateral_mint).key;
    user_mfi_account_f
        .nullify_assets_for_bank(collateral_bank_pk)
        .await?;

    test_f
        .marginfi_group
        .try_handle_bankruptcy(test_f.get_bank(&debt_mint), &user_mfi_account_f)
        .await?;

    // Due to balances sorting, debt may be not at index 1 -> find its actual index first
    let debt_bank_f = test_f.get_bank(&debt_mint);
    let user_mfi_account = user_mfi_account_f.load().await;
    let debt_index = user_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == debt_bank_f.key)
        .unwrap();

    let user_collateral_balance = user_mfi_account.lending_account.balances[debt_index];

    assert_eq!(
        I80F48::from(user_collateral_balance.liability_shares),
        I80F48::ZERO
    );

    let lp_mfi_account = lp_mfi_account_f.load().await;
    let debt_bank = debt_bank_f.load().await;

    let actual_lender_collateral_amount = debt_bank.get_asset_amount(
        lp_mfi_account.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;
    let borrow_amount_native = native!(
        borrow_amount,
        test_f.get_bank(&debt_mint).mint.mint.decimals,
        f64
    );
    let borrow_amount_with_fee = borrow_amount_native;
    let expected_lender_collateral_amount = I80F48::from(
        native!(
            lp_deposit_amount,
            test_f.get_bank(&debt_mint).mint.mint.decimals,
            f64
        ) - borrow_amount_with_fee,
    );

    assert_eq_noise!(
        actual_lender_collateral_amount,
        expected_lender_collateral_amount,
        I80F48::ONE
    );

    Ok(())
}

#[tokio::test]
async fn marginfi_group_handle_bankruptcy_success_not_insured_3_depositors() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_1_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_1_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_1_mfi_account_f
        .try_bank_deposit(lender_1_token_account.key, usdc_bank_f, 100_000, None)
        .await?;

    let lender_2_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_2_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_2_mfi_account_f
        .try_bank_deposit(lender_2_token_account.key, usdc_bank_f, 100_000, None)
        .await?;

    let lender_3_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_3_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_3_mfi_account_f
        .try_bank_deposit(lender_3_token_account.key, usdc_bank_f, 100_000, None)
        .await?;

    let mut borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_001)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 1_001, None)
        .await?;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 10_000)
        .await?;

    borrower_mfi_account_f
        .nullify_assets_for_bank(sol_bank_f.key)
        .await?;

    test_f
        .marginfi_group
        .try_handle_bankruptcy(usdc_bank_f, &borrower_mfi_account_f)
        .await?;

    let borrower_mfi_account = borrower_mfi_account_f.load().await;
    let borrower_usdc_balance = borrower_mfi_account.lending_account.balances[1];

    assert_eq!(
        I80F48::from(borrower_usdc_balance.liability_shares),
        I80F48::ZERO
    );

    let lender_1_mfi_account = lender_1_mfi_account_f.load().await;
    let usdc_bank = usdc_bank_f.load().await;

    let lender_usdc_value = usdc_bank.get_asset_amount(
        lender_1_mfi_account.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    assert_eq_noise!(
        lender_usdc_value,
        I80F48::from(native!(96_666, "USDC")),
        I80F48::from(native!(1, "USDC"))
    );

    Ok(())
}
