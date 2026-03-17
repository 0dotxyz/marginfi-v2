use crate::constants::SOLEND_PROGRAM_ID;
use crate::{
    bank_signer, check, state::bank::BankVaultType, utils::assert_within_one_token, MarginfiError,
    MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token::accessor;
use marginfi_type_crate::types::Bank;
use solend_mocks::cpi::accounts::DepositReserveLiquidityAndObligationCollateral;
use solend_mocks::cpi::accounts::WithdrawObligationCollateralAndRedeemReserveCollateral;
use solend_mocks::cpi::deposit_reserve_liquidity_and_obligation_collateral;
use solend_mocks::cpi::withdraw_obligation_collateral_and_redeem_reserve_collateral;
use solend_mocks::state::{
    get_solend_obligation_deposit_amount, validate_solend_obligation, SolendMinimalReserve,
};

use super::{cpi_transfer_vault_to_destination, IntegrationDeposit, IntegrationWithdraw};

/// Expected protocol_accounts layout for Solend deposit:
/// 0: integration_acc_2 (obligation) - mut, UncheckedAccount (owner == SOLEND_PROGRAM_ID)
/// 1: lending_market
/// 2: lending_market_authority
/// 3: integration_acc_1 (reserve) - mut, AccountLoader<SolendMinimalReserve>
/// 4: reserve_liquidity_supply - mut
/// 5: reserve_collateral_mint - mut
/// 6: reserve_collateral_supply - mut
/// 7: user_collateral - mut
/// 8: pyth_price
/// 9: switchboard_feed
/// 10: solend_program
pub const DEPOSIT_ACCOUNTS: usize = 11;

/// Expected protocol_accounts layout for Solend withdraw:
/// 0: integration_acc_2 (obligation) - mut, UncheckedAccount (owner == SOLEND_PROGRAM_ID)
/// 1: lending_market - mut
/// 2: lending_market_authority
/// 3: integration_acc_1 (reserve) - mut, AccountLoader<SolendMinimalReserve>
/// 4: reserve_liquidity_supply - mut
/// 5: reserve_collateral_mint - mut
/// 6: reserve_collateral_supply - mut
/// 7: user_collateral - mut
/// 8: solend_program
pub const WITHDRAW_ACCOUNTS: usize = 9;

/// Validates bank integration keys, obligation ownership, program ID, and reserve staleness.
fn validate_solend_setup<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    bank: &Bank,
    min_count: usize,
    program_id_index: usize,
) -> MarginfiResult<AccountLoader<'info, SolendMinimalReserve>> {
    check!(
        protocol_accounts.len() >= min_count,
        MarginfiError::IntegrationAccountCountMismatch
    );
    check!(
        protocol_accounts[3].key() == bank.integration_acc_1,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[0].key() == bank.integration_acc_2,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[0].owner == &SOLEND_PROGRAM_ID,
        MarginfiError::InvalidSolendAccount
    );
    check!(
        protocol_accounts[program_id_index].key() == SOLEND_PROGRAM_ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    validate_solend_obligation(&protocol_accounts[0], protocol_accounts[3].key())?;

    let reserve_loader = AccountLoader::<SolendMinimalReserve>::try_from(&protocol_accounts[3])?;
    check!(
        !reserve_loader.load()?.is_stale()?,
        MarginfiError::SolendReserveStale
    );
    Ok(reserve_loader)
}

pub fn deposit<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &IntegrationDeposit<'info>,
    amount: u64,
    authority_bump: u8,
) -> MarginfiResult<(u64, u64)> {
    let bank = common.bank.load()?;
    let reserve_loader = validate_solend_setup(protocol_accounts, &bank, DEPOSIT_ACCOUNTS, 10)?;
    drop(bank);

    // Pre-CPI state
    let initial_obligation_deposited = get_solend_obligation_deposit_amount(&protocol_accounts[0])?;
    let expected_collateral = reserve_loader.load()?.liquidity_to_collateral(amount)?;

    // CPI: Solend deposit
    {
        let accounts = DepositReserveLiquidityAndObligationCollateral {
            source_liquidity_info: common.liquidity_vault.to_account_info(),
            user_collateral_info: protocol_accounts[7].clone(),
            reserve_info: protocol_accounts[3].clone(),
            reserve_liquidity_supply_info: protocol_accounts[4].clone(),
            reserve_collateral_mint_info: protocol_accounts[5].clone(),
            lending_market_info: protocol_accounts[1].clone(),
            lending_market_authority_info: protocol_accounts[2].clone(),
            destination_deposit_collateral_info: protocol_accounts[6].clone(),
            obligation_info: protocol_accounts[0].clone(),
            obligation_owner_info: common.liquidity_vault_authority.to_account_info(),
            pyth_price_info: protocol_accounts[8].clone(),
            switchboard_feed_info: protocol_accounts[9].clone(),
            user_transfer_authority_info: common.liquidity_vault_authority.to_account_info(),
            token_program_info: common.token_program.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[10].clone(), accounts, signer_seeds);
        deposit_reserve_liquidity_and_obligation_collateral(cpi_ctx, amount)?;
    }

    // Verify deposit
    let final_obligation_deposited = get_solend_obligation_deposit_amount(&protocol_accounts[0])?;
    let balance_change = final_obligation_deposited - initial_obligation_deposited;
    assert_within_one_token(
        balance_change,
        expected_collateral,
        MarginfiError::SolendDepositFailed,
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
    let reserve_loader = validate_solend_setup(protocol_accounts, &bank, WITHDRAW_ACCOUNTS, 8)?;
    drop(bank);

    // Pre-CPI state
    let initial_obligation_deposited = get_solend_obligation_deposit_amount(&protocol_accounts[0])?;
    let pre_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;
    let expected_liquidity = reserve_loader
        .load()?
        .collateral_to_liquidity(collateral_amount)?;

    // CPI: Solend withdraw
    {
        let accounts = WithdrawObligationCollateralAndRedeemReserveCollateral {
            source_collateral_info: protocol_accounts[6].clone(),
            destination_collateral_info: protocol_accounts[7].clone(),
            reserve_info: protocol_accounts[3].clone(),
            obligation_info: protocol_accounts[0].clone(),
            lending_market_info: protocol_accounts[1].clone(),
            lending_market_authority_info: protocol_accounts[2].clone(),
            destination_liquidity_info: common.liquidity_vault.to_account_info(),
            reserve_collateral_mint_info: protocol_accounts[5].clone(),
            reserve_liquidity_supply_info: protocol_accounts[4].clone(),
            obligation_owner_info: common.liquidity_vault_authority.to_account_info(),
            user_transfer_authority_info: common.liquidity_vault_authority.to_account_info(),
            token_program_info: common.token_program.to_account_info(),
            deposit_reserve_info: protocol_accounts[3].clone(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[8].clone(), accounts, signer_seeds);
        withdraw_obligation_collateral_and_redeem_reserve_collateral(cpi_ctx, collateral_amount)?;
    }

    // Verify obligation deposit decreased
    let final_obligation_deposited = get_solend_obligation_deposit_amount(&protocol_accounts[0])?;
    let obligation_change = initial_obligation_deposited - final_obligation_deposited;
    assert_within_one_token(
        obligation_change,
        collateral_amount,
        MarginfiError::SolendWithdrawFailed,
    )?;

    // Verify vault received expected liquidity
    let post_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;
    let received = post_vault_balance - pre_vault_balance;
    assert_within_one_token(
        received,
        expected_liquidity,
        MarginfiError::SolendWithdrawFailed,
    )?;

    // Transfer vault -> destination
    cpi_transfer_vault_to_destination(common, common.bank.key(), authority_bump, received)?;

    Ok(received)
}
