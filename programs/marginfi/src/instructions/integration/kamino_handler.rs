use crate::{
    bank_signer, check,
    constants::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID},
    state::bank::BankVaultType,
    utils::{assert_within_one_token, optional_account},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;
use anchor_spl::token::accessor;
use kamino_mocks::kamino_lending::cpi::deposit_reserve_liquidity_and_obligation_collateral_v2;
use kamino_mocks::kamino_lending::cpi::withdraw_obligation_collateral_and_redeem_reserve_collateral_v2;
use kamino_mocks::{
    kamino_lending::cpi::accounts::{
        DepositReserveLiquidityAndObligationCollateral,
        DepositReserveLiquidityAndObligationCollateralV2, FarmsAccounts,
        WithdrawObligationCollateralAndRedeemReserveCollateral,
        WithdrawObligationCollateralAndRedeemReserveCollateralV2,
    },
    state::{MinimalObligation, MinimalReserve},
};

use super::{cpi_transfer_vault_to_destination, IntegrationDeposit, IntegrationWithdraw};

/// Expected protocol_accounts layout for Kamino deposit/withdraw:
/// 0: integration_acc_2 (obligation) - mut
/// 1: lending_market
/// 2: lending_market_authority
/// 3: integration_acc_1 (reserve) - mut
/// 4: reserve_liquidity_supply - mut
/// 5: reserve_collateral_mint - mut
/// 6: reserve_collateral_supply/source - mut
/// 7: kamino_program
/// 8: farms_program
/// 9: collateral_token_program
/// 10: instruction_sysvar_account
/// 11: obligation_farm_user_state (optional, system_program sentinel if absent)
/// 12: reserve_farm_state (optional, system_program sentinel if absent)
const MIN_REQUIRED_ACCOUNTS: usize = 11;
pub const DEPOSIT_ACCOUNTS: usize = 13;
pub const WITHDRAW_ACCOUNTS: usize = 13;

/// Validates protocol account keys match the bank's stored integration accounts and known programs.
fn validate_protocol_accounts(
    protocol_accounts: &[AccountInfo],
    obligation_key: Pubkey,
    reserve_key: Pubkey,
) -> MarginfiResult {
    check!(
        protocol_accounts.len() >= MIN_REQUIRED_ACCOUNTS,
        MarginfiError::IntegrationAccountCountMismatch
    );
    check!(
        protocol_accounts[3].key() == reserve_key,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[0].key() == obligation_key,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[7].key() == KAMINO_PROGRAM_ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[8].key() == FARMS_PROGRAM_ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[9].key() == anchor_spl::token::spl_token::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[10].key() == sysvar::instructions::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    Ok(())
}

/// Validates the obligation has exactly one deposit position linked to the reserve.
fn validate_obligation<'a>(
    obligation_info: &'a AccountInfo<'a>,
    reserve_key: Pubkey,
) -> MarginfiResult<AccountLoader<'a, MinimalObligation>> {
    let loader = AccountLoader::<MinimalObligation>::try_from(obligation_info)?;
    let obligation = loader.load()?;
    check!(
        obligation.deposits[0].deposit_reserve == reserve_key,
        MarginfiError::ObligationDepositReserveMismatch
    );
    check!(
        obligation
            .deposits
            .iter()
            .skip(1)
            .all(|d| d.deposited_amount == 0),
        MarginfiError::InvalidObligationDepositCount
    );
    drop(obligation);
    Ok(loader)
}

fn farms_accounts<'a>(protocol_accounts: &'a [AccountInfo<'a>]) -> FarmsAccounts<'a> {
    FarmsAccounts {
        obligation_farm_user_state: optional_account(protocol_accounts.get(11)),
        reserve_farm_state: optional_account(protocol_accounts.get(12)),
    }
}

pub fn deposit<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &IntegrationDeposit<'info>,
    amount: u64,
    authority_bump: u8,
) -> MarginfiResult<(u64, u64)> {
    let bank = common.bank.load()?;
    validate_protocol_accounts(
        protocol_accounts,
        bank.integration_acc_2,
        bank.integration_acc_1,
    )?;
    drop(bank);

    let reserve_info = &protocol_accounts[3];
    let obligation_loader = validate_obligation(&protocol_accounts[0], reserve_info.key())?;

    let initial_deposited = obligation_loader.load()?.deposits[0].deposited_amount;

    let reserve_loader = AccountLoader::<MinimalReserve>::try_from(reserve_info)?;
    let expected_collateral = reserve_loader.load()?.liquidity_to_collateral(amount)?;

    // CPI: Kamino deposit
    {
        let deposit_accounts = DepositReserveLiquidityAndObligationCollateral {
            owner: common.liquidity_vault_authority.to_account_info(),
            obligation: protocol_accounts[0].clone(),
            lending_market: protocol_accounts[1].clone(),
            lending_market_authority: protocol_accounts[2].clone(),
            reserve: reserve_info.clone(),
            reserve_liquidity_mint: common.mint.to_account_info(),
            reserve_liquidity_supply: protocol_accounts[4].clone(),
            reserve_collateral_mint: protocol_accounts[5].clone(),
            reserve_destination_deposit_collateral: protocol_accounts[6].clone(),
            user_source_liquidity: common.liquidity_vault.to_account_info(),
            placeholder_user_destination_collateral: None,
            collateral_token_program: protocol_accounts[9].clone(),
            liquidity_token_program: common.token_program.to_account_info(),
            instruction_sysvar_account: protocol_accounts[10].clone(),
        };

        let accounts = DepositReserveLiquidityAndObligationCollateralV2 {
            deposit_accounts,
            deposit_farms_accounts: farms_accounts(protocol_accounts),
            farms_program: protocol_accounts[8].clone(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[7].clone(), accounts, signer_seeds);
        deposit_reserve_liquidity_and_obligation_collateral_v2(cpi_ctx, amount)?;
    }

    let final_deposited = obligation_loader.load()?.deposits[0].deposited_amount;
    let balance_change = final_deposited - initial_deposited;
    assert_within_one_token(
        balance_change,
        expected_collateral,
        MarginfiError::KaminoDepositFailed,
    )?;

    Ok((balance_change, amount))
}

pub fn withdraw_cpi<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &IntegrationWithdraw<'info>,
    collateral_amount: u64,
    authority_bump: u8,
) -> MarginfiResult<u64> {
    let bank = common.bank.load()?;
    validate_protocol_accounts(
        protocol_accounts,
        bank.integration_acc_2,
        bank.integration_acc_1,
    )?;
    drop(bank);

    let reserve_info = &protocol_accounts[3];
    let obligation_loader = validate_obligation(&protocol_accounts[0], reserve_info.key())?;

    let pre_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;
    let initial_deposited = obligation_loader.load()?.deposits[0].deposited_amount;

    let reserve_loader = AccountLoader::<MinimalReserve>::try_from(reserve_info)?;
    let expected_liquidity = reserve_loader
        .load()?
        .collateral_to_liquidity(collateral_amount)?;

    // CPI: Kamino withdraw
    {
        let withdraw_accounts = WithdrawObligationCollateralAndRedeemReserveCollateral {
            collateral_token_program: protocol_accounts[9].clone(),
            instruction_sysvar_account: protocol_accounts[10].clone(),
            lending_market: protocol_accounts[1].clone(),
            lending_market_authority: protocol_accounts[2].clone(),
            liquidity_token_program: common.token_program.to_account_info(),
            obligation: protocol_accounts[0].clone(),
            owner: common.liquidity_vault_authority.to_account_info(),
            placeholder_user_destination_collateral: None,
            reserve_collateral_mint: protocol_accounts[5].clone(),
            reserve_liquidity_mint: common.mint.to_account_info(),
            reserve_liquidity_supply: protocol_accounts[4].clone(),
            reserve_source_collateral: protocol_accounts[6].clone(),
            user_destination_liquidity: common.liquidity_vault.to_account_info(),
            withdraw_reserve: reserve_info.clone(),
        };

        let accounts = WithdrawObligationCollateralAndRedeemReserveCollateralV2 {
            withdraw_accounts,
            withdraw_farms_accounts: farms_accounts(protocol_accounts),
            farms_program: protocol_accounts[8].clone(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[7].clone(), accounts, signer_seeds);
        withdraw_obligation_collateral_and_redeem_reserve_collateral_v2(
            cpi_ctx,
            collateral_amount,
        )?;
    }

    let final_deposited = obligation_loader.load()?.deposits[0].deposited_amount;
    let actual_decrease = initial_deposited - final_deposited;
    require_eq!(
        actual_decrease,
        collateral_amount,
        MarginfiError::KaminoWithdrawFailed
    );

    let post_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;
    let received = post_vault_balance - pre_vault_balance;
    assert_within_one_token(
        received,
        expected_liquidity,
        MarginfiError::KaminoWithdrawFailed,
    )?;

    cpi_transfer_vault_to_destination(common, common.bank.key(), authority_bump, received)?;

    Ok(received)
}
