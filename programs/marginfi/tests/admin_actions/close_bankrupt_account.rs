use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, marginfi_account::MarginfiAccountFixture, prelude::*};
use marginfi::{prelude::MarginfiError, state::marginfi_account::MarginfiAccountImpl};
use marginfi_type_crate::types::{
    Bank, BankOperationalState, MarginfiAccount, OrderTrigger, RiskTier, WrappedI80F48,
    ACCOUNT_DISABLED, ACCOUNT_FROZEN, ACCOUNT_IN_RECEIVERSHIP,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, transaction::Transaction};
use test_case::test_case;

const BORROW_AMOUNT: f64 = 10.;
/// 0.001 USDC
const DUST: I80F48 = I80F48!(1_000);

/// An LP funds the SOL bank; the user deposits USDC and borrows `BORROW_AMOUNT` SOL. Returns the
/// user's account, holding a USDC asset balance and a SOL liability balance.
async fn setup_borrower(test_f: &mut TestFixture) -> anyhow::Result<MarginfiAccountFixture> {
    let lp = test_f.create_marginfi_account().await;
    let lp_sol = test_f
        .get_bank(&BankMint::Sol)
        .mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lp.try_bank_deposit(lp_sol.key, test_f.get_bank(&BankMint::Sol), 100, None)
        .await?;

    let user = test_f.create_marginfi_account().await;
    let collateral = test_f
        .get_sufficient_collateral_for_outflow(BORROW_AMOUNT, &BankMint::Usdc, &BankMint::Sol)
        .await;
    let user_usdc = test_f
        .get_bank_mut(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(collateral * 2.)
        .await;
    let user_sol = test_f
        .get_bank_mut(&BankMint::Sol)
        .mint
        .create_empty_token_account()
        .await;
    user.try_bank_deposit(
        user_usdc.key,
        test_f.get_bank(&BankMint::Usdc),
        collateral,
        None,
    )
    .await?;
    user.try_bank_borrow(user_sol.key, test_f.get_bank(&BankMint::Sol), BORROW_AMOUNT)
        .await?;
    Ok(user)
}

/// Wipes the user's USDC collateral down to `DUST` and bankrupts its SOL debt, leaving the
/// account disabled with residual balances.
async fn bankrupt(test_f: &TestFixture, user: &MarginfiAccountFixture) -> anyhow::Result<()> {
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).key;
    edit_account(user, |account| {
        let balance = account
            .lending_account
            .balances
            .iter_mut()
            .find(|balance| balance.bank_pk == usdc_bank)
            .unwrap();
        balance.asset_shares = DUST.into();
    })
    .await?;
    test_f
        .marginfi_group
        .try_handle_bankruptcy(test_f.get_bank(&BankMint::Sol), user)
        .await?;
    Ok(())
}

async fn setup_bankrupt(test_f: &mut TestFixture) -> anyhow::Result<MarginfiAccountFixture> {
    let user = setup_borrower(test_f).await?;
    bankrupt(test_f, &user).await?;
    Ok(user)
}

async fn edit_account(
    user: &MarginfiAccountFixture,
    edit: impl FnOnce(&mut MarginfiAccount),
) -> anyhow::Result<()> {
    let mut account = user.load().await;
    edit(&mut account);
    user.set_account(&account).await
}

async fn edit_bank(test_f: &TestFixture, bank: Pubkey, edit: impl FnOnce(&mut Bank)) {
    let mut ctx = test_f.context.borrow_mut();
    let mut bank_ai = ctx.banks_client.get_account(bank).await.unwrap().unwrap();
    edit(bytemuck::from_bytes_mut::<Bank>(
        &mut bank_ai.data.as_mut_slice()[8..],
    ));
    ctx.set_account(&bank, &bank_ai.into());
}

async fn lamports(test_f: &TestFixture, key: Pubkey) -> Option<u64> {
    let ctx = test_f.context.borrow_mut();
    ctx.banks_client
        .get_account(key)
        .await
        .unwrap()
        .map(|account| account.lamports)
}

fn balance_shares(account: &MarginfiAccount, bank: Pubkey) -> (I80F48, I80F48) {
    let balance = account.lending_account.get_balance(&bank).unwrap();
    (balance.asset_shares.into(), balance.liability_shares.into())
}

/// Purges the residual balances and seizes rent and the fee pool to the fee wallet.
#[tokio::test]
async fn close_bankrupt_account_after_bankruptcy() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_bankrupt(&mut test_f).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol).key;

    let payer = test_f.payer();
    let top_up = user
        .make_top_up_rebalance_fee_pool_ix(payer, 1_000_000)
        .await;
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let ctx = test_f.context.borrow_mut();
        let tx =
            Transaction::new_signed_with_payer(&[top_up], Some(&payer), &[&ctx.payer], blockhash);
        ctx.banks_client.process_transaction(tx).await?;
    }
    let fee_pool = user.rebalance_fee_pool_pda();
    let pool_lamports = lamports(&test_f, fee_pool).await.unwrap();

    let account = user.load().await;
    assert!(account.get_flag(ACCOUNT_DISABLED));
    let (_, sol_residue) = balance_shares(&account, sol_bank);
    let usdc_before = test_f.get_bank(&BankMint::Usdc).load().await;
    let sol_before = test_f.get_bank(&BankMint::Sol).load().await;
    let fee_wallet = test_f.marginfi_group.fee_wallet;
    let fee_wallet_before = lamports(&test_f, fee_wallet).await.unwrap_or(0);
    let account_lamports = lamports(&test_f, user.key).await.unwrap();

    let banks = user.active_balance_banks().await;
    assert_eq!(banks.len(), 2);
    user.try_admin_close_bankrupt_account(fee_wallet, &banks, None)
        .await?;

    assert_eq!(lamports(&test_f, user.key).await, None);
    assert_eq!(lamports(&test_f, fee_pool).await, None);
    assert_eq!(
        lamports(&test_f, fee_wallet).await.unwrap(),
        fee_wallet_before + account_lamports + pool_lamports
    );

    // The dust collateral is removed from the bank, and stops counting as a lending position
    let usdc_after = test_f.get_bank(&BankMint::Usdc).load().await;
    assert_eq!(
        I80F48::from(usdc_after.total_asset_shares),
        I80F48::from(usdc_before.total_asset_shares) - DUST
    );
    assert_eq!(
        usdc_after.lending_position_count,
        usdc_before.lending_position_count - 1
    );

    // The settled debt balance's residue is removed; bankruptcy already stopped counting it
    let sol_after = test_f.get_bank(&BankMint::Sol).load().await;
    assert_eq!(
        I80F48::from(sol_after.total_liability_shares),
        I80F48::from(sol_before.total_liability_shares) - sol_residue
    );
    assert_eq!(
        sol_after.borrowing_position_count,
        sol_before.borrowing_position_count
    );
    Ok(())
}

/// A bank closed while bankruptcy residue still references it doesn't block the close. A closed
/// bank can't hold more than dust, so a larger balance there is rejected.
#[test_case(BankMint::Sol; "residue")]
#[test_case(BankMint::Usdc; "above dust")]
#[tokio::test]
async fn close_bankrupt_account_with_closed_bank(closed: BankMint) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_bankrupt(&mut test_f).await?;
    let closed_bank = test_f.get_bank(&closed).key;
    test_f.context.borrow_mut().set_account(
        &closed_bank,
        &solana_sdk::account::Account::default().into(),
    );

    let banks = user.active_balance_banks().await;
    let res = user
        .try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await;
    if closed == BankMint::Sol {
        res?;
        assert_eq!(lamports(&test_f, user.key).await, None);
    } else {
        assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidBankAccount);
    }
    Ok(())
}

/// Dust debt in a live bank is removed, and the bank accrues interest up to now first.
#[tokio::test]
async fn close_bankrupt_account_dust_debt_accrues_bank() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    // Another borrower keeps the SOL bank accruing
    setup_borrower(&mut test_f).await?;
    let user = setup_borrower(&mut test_f).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol).key;
    let dust_debt = I80F48!(0.00001);
    edit_account(&user, |account| {
        account.set_flag(ACCOUNT_DISABLED, false);
        let balance = account
            .lending_account
            .balances
            .iter_mut()
            .find(|balance| balance.bank_pk == sol_bank)
            .unwrap();
        balance.liability_shares = dust_debt.into();
    })
    .await?;
    let sol_before = test_f.get_bank(&BankMint::Sol).load().await;
    test_f.advance_time(86_400).await;

    let banks = user.active_balance_banks().await;
    user.try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await?;

    let sol_after = test_f.get_bank(&BankMint::Sol).load().await;
    assert_eq!(
        I80F48::from(sol_after.total_liability_shares),
        I80F48::from(sol_before.total_liability_shares) - dust_debt
    );
    assert!(I80F48::from(sol_after.asset_share_value) > I80F48::from(sol_before.asset_share_value));
    Ok(())
}

/// A leg worth less than the remaining collateral can still be bankrupted once the account is
/// disabled, so it never blocks the close.
#[test_case(false; "not disabled")]
#[test_case(true; "disabled")]
#[tokio::test]
async fn close_bankrupt_account_leg_below_assets(disabled: bool) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_borrower(&mut test_f).await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).key;
    let sol_bank = test_f.get_bank(&BankMint::Sol).key;
    // $0.001 of collateral against ~$0.00001 of debt
    edit_account(&user, |account| {
        for balance in account.lending_account.balances.iter_mut() {
            if balance.bank_pk == usdc_bank {
                balance.asset_shares = DUST.into();
            } else if balance.bank_pk == sol_bank {
                balance.liability_shares = I80F48!(1_000).into();
            }
        }
        if disabled {
            account.set_flag(ACCOUNT_DISABLED, false);
        }
    })
    .await?;

    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(test_f.get_bank(&BankMint::Sol), &user)
        .await;
    if !disabled {
        assert_custom_error!(res.unwrap_err(), MarginfiError::AccountNotBankrupt);
        return Ok(());
    }
    res?;

    let banks = user.active_balance_banks().await;
    user.try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await?;
    assert_eq!(lamports(&test_f, user.key).await, None);
    Ok(())
}

/// Unsettled debt must go through bankruptcy, which uses insurance first. In a bank already killed
/// by bankruptcy it is removed as is.
#[test_case(false; "live bank")]
#[test_case(true; "killed bank")]
#[tokio::test]
async fn close_bankrupt_account_unsettled_debt(killed: bool) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_borrower(&mut test_f).await?;
    edit_account(&user, |account| account.set_flag(ACCOUNT_DISABLED, false)).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol).key;
    if killed {
        edit_bank(&test_f, sol_bank, |bank| {
            bank.config.operational_state = BankOperationalState::KilledByBankruptcy
        })
        .await;
    }
    let (_, debt_shares) = balance_shares(&user.load().await, sol_bank);
    let sol_before = test_f.get_bank(&BankMint::Sol).load().await;

    let banks = user.active_balance_banks().await;
    let res = user
        .try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await;

    if killed {
        res?;
        let sol_after = test_f.get_bank(&BankMint::Sol).load().await;
        assert_eq!(
            I80F48::from(sol_after.total_liability_shares),
            I80F48::from(sol_before.total_liability_shares) - debt_shares
        );
        assert_eq!(sol_after.asset_share_value, sol_before.asset_share_value);
    } else {
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::BankruptAccountHasUnsettledDebt
        );
    }
    Ok(())
}

/// Isolated-tier collateral carries no weight in the bankruptcy check, so it may still be worth
/// real tokens. Dust below one native unit is not.
#[test_case(false; "meaningful isolated collateral")]
#[test_case(true; "isolated dust")]
#[tokio::test]
async fn close_bankrupt_account_isolated_collateral(dust_only: bool) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).key;
    let user = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .get_bank(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(1_000)
        .await;
    user.try_bank_deposit(user_usdc.key, test_f.get_bank(&BankMint::Usdc), 100, None)
        .await?;
    edit_account(&user, |account| {
        account.set_flag(ACCOUNT_DISABLED, false);
        if dust_only {
            account.lending_account.balances[0].asset_shares = I80F48!(0.5).into();
        }
    })
    .await?;
    edit_bank(&test_f, usdc_bank, |bank| {
        bank.config.risk_tier = RiskTier::Isolated
    })
    .await;

    let res = user
        .try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &[usdc_bank], None)
        .await;

    if dust_only {
        res?;
        assert_eq!(lamports(&test_f, user.key).await, None);
    } else {
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::BankruptAccountHasIsolatedCollateral
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Ineligible {
    NotDisabled,
    Migrated,
    Flag(u64),
    LiquidationRecord,
}

#[test_case(Ineligible::NotDisabled)]
#[test_case(Ineligible::Migrated)]
#[test_case(Ineligible::Flag(ACCOUNT_FROZEN))]
#[test_case(Ineligible::Flag(ACCOUNT_IN_RECEIVERSHIP))]
#[test_case(Ineligible::LiquidationRecord)]
#[tokio::test]
async fn close_bankrupt_account_rejects_ineligible(case: Ineligible) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_borrower(&mut test_f).await?;
    edit_account(&user, |account| {
        if !matches!(case, Ineligible::NotDisabled) {
            account.set_flag(ACCOUNT_DISABLED, false);
        }
        match case {
            Ineligible::Migrated => account.migrated_to = Pubkey::new_unique(),
            Ineligible::Flag(flag) => account.set_flag(flag, false),
            Ineligible::LiquidationRecord => account.liquidation_record = Pubkey::new_unique(),
            Ineligible::NotDisabled => {}
        }
    })
    .await?;

    let banks = user.active_balance_banks().await;
    let res = user
        .try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);
    Ok(())
}

/// Open orders don't block the close; the orphaned order can still be closed with
/// `marginfi_account_keeper_close_order` afterwards.
#[tokio::test]
async fn close_bankrupt_account_with_open_order() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_borrower(&mut test_f).await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc).key;
    let sol_bank = test_f.get_bank(&BankMint::Sol).key;
    let order = user
        .try_place_order(
            vec![usdc_bank, sol_bank],
            OrderTrigger::TakeProfit {
                threshold: WrappedI80F48::from(I80F48!(1_000_000)),
                max_slippage: 0,
            },
        )
        .await?;
    bankrupt(&test_f, &user).await?;
    assert_eq!(user.load().await.active_orders, 1);

    let fee_wallet = test_f.marginfi_group.fee_wallet;
    let banks = user.active_balance_banks().await;
    user.try_admin_close_bankrupt_account(fee_wallet, &banks, None)
        .await?;
    assert_eq!(lamports(&test_f, user.key).await, None);

    let keeper = test_f.payer_keypair();
    user.try_keeper_close_order(order, &keeper, fee_wallet)
        .await?;
    assert_eq!(lamports(&test_f, order).await, None);
    Ok(())
}

#[tokio::test]
async fn close_bankrupt_account_rejects_non_risk_admin() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_bankrupt(&mut test_f).await?;

    let banks = user.active_balance_banks().await;
    let res = user
        .try_admin_close_bankrupt_account(
            test_f.marginfi_group.fee_wallet,
            &banks,
            Some(&Keypair::new()),
        )
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);
    Ok(())
}

#[tokio::test]
async fn close_bankrupt_account_rejects_wrong_fee_wallet() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_bankrupt(&mut test_f).await?;

    let banks = user.active_balance_banks().await;
    let res = user
        .try_admin_close_bankrupt_account(Pubkey::new_unique(), &banks, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidGlobalFeeWallet);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum BadBanks {
    Missing,
    Extra,
    OutOfOrder,
    WrongBank,
}

/// A wrong bank list must fail outright, never skip a balance.
#[test_case(BadBanks::Missing)]
#[test_case(BadBanks::Extra)]
#[test_case(BadBanks::OutOfOrder)]
#[test_case(BadBanks::WrongBank)]
#[tokio::test]
async fn close_bankrupt_account_rejects_bad_bank_list(case: BadBanks) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = setup_bankrupt(&mut test_f).await?;
    let other_bank = test_f.get_bank(&BankMint::SolEquivalent).key;

    let mut banks = user.active_balance_banks().await;
    match case {
        BadBanks::Missing => {
            banks.pop();
        }
        BadBanks::Extra => banks.push(other_bank),
        BadBanks::OutOfOrder => banks.reverse(),
        BadBanks::WrongBank => banks[1] = other_bank,
    }

    let res = user
        .try_admin_close_bankrupt_account(test_f.marginfi_group.fee_wallet, &banks, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidBankAccount);
    Ok(())
}
