use crate::{
    bank_signer, check,
    state::{
        bank::BankVaultType,
        marginfi_account::{BankAccountWrapper, MarginfiAccountImpl},
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{transfer_checked, TokenAccount, TransferChecked};
use fixed::types::I80F48;
use juplend_mocks::juplend_earn::cpi::accounts::{Deposit, UpdateRate, Withdraw as WithdrawCpi};
use juplend_mocks::juplend_earn::cpi::{
    deposit as cpi_juplend_deposit, update_rate, withdraw as cpi_juplend_withdraw,
};
use juplend_mocks::state::{
    expected_assets_for_redeem_from_rate, expected_shares_for_deposit_from_rates,
    expected_shares_for_withdraw_from_rate, Lending as JuplendLending,
};
use marginfi_type_crate::types::{Bank, MarginfiAccount, ACCOUNT_IN_RECEIVERSHIP};

use super::{CommonDeposit, CommonWithdraw};

/// Expected protocol_accounts layout for JupLend deposit:
/// 0: lending, 1: f_token_mint, 2: fToken vault, 3: lending_admin,
/// 4: supply_token_reserves_liquidity, 5: lending_supply_position_on_liquidity,
/// 6: rate_model, 7: vault, 8: liquidity, 9: liquidity_program,
/// 10: rewards_rate_model, 11: juplend_program, 12: associated_token_program,
/// 13: system_program
pub const DEPOSIT_ACCOUNTS: usize = 14;

/// Expected protocol_accounts layout for JupLend withdraw:
/// 0: lending, 1: f_token_mint, 2: fToken vault, 3: withdraw intermediary ATA,
/// 4: lending_admin, 5: supply_token_reserves_liquidity,
/// 6: lending_supply_position_on_liquidity, 7: rate_model, 8: vault,
/// 9: claim_account, 10: liquidity, 11: liquidity_program,
/// 12: rewards_rate_model, 13: juplend_program, 14: associated_token_program,
/// 15: system_program
pub const WITHDRAW_ACCOUNTS: usize = 16;

/// Validates lending account fields match the expected protocol accounts.
fn validate_lending_accounts(
    lending: &JuplendLending,
    f_token_mint_key: Pubkey,
    reserves_liquidity_key: Pubkey,
    supply_position_key: Pubkey,
) -> MarginfiResult {
    check!(
        lending.f_token_mint == f_token_mint_key,
        MarginfiError::InvalidJuplendLending
    );
    check!(
        lending.token_reserves_liquidity == reserves_liquidity_key,
        MarginfiError::InvalidJuplendLending
    );
    check!(
        lending.supply_position_on_liquidity == supply_position_key,
        MarginfiError::InvalidJuplendLending
    );
    Ok(())
}

pub(crate) fn deposit<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonDeposit<'_, 'info>,
    amount: u64,
    authority_bump: u8,
) -> MarginfiResult<(u64, u64)> {
    check!(
        protocol_accounts.len() >= DEPOSIT_ACCOUNTS,
        MarginfiError::IntegrationAccountCountMismatch
    );

    let bank = common.bank.load()?;
    check!(
        protocol_accounts[0].key() == bank.integration_acc_1,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[2].key() == bank.integration_acc_2,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    drop(bank);

    check!(
        protocol_accounts[9].key() == juplend_mocks::liquidity::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[11].key() == juplend_mocks::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[12].key() == anchor_spl::associated_token::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[13].key() == system_program::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );

    let lending_loader = AccountLoader::<JuplendLending>::try_from(&protocol_accounts[0])?;
    {
        let lending = lending_loader.load()?;
        validate_lending_accounts(
            &lending,
            protocol_accounts[1].key(),
            protocol_accounts[4].key(),
            protocol_accounts[5].key(),
        )?;
    }

    // CPI: Update rate
    {
        let accounts = UpdateRate {
            lending: protocol_accounts[0].clone(),
            mint: common.mint.to_account_info(),
            f_token_mint: protocol_accounts[1].clone(),
            supply_token_reserves_liquidity: protocol_accounts[4].clone(),
            rewards_rate_model: protocol_accounts[10].clone(),
        };
        let cpi_ctx = CpiContext::new(protocol_accounts[11].clone(), accounts);
        update_rate(cpi_ctx)?;
    }

    let expected_shares = {
        let lending = lending_loader.load()?;
        expected_shares_for_deposit_from_rates(
            amount,
            lending.liquidity_exchange_price,
            lending.token_exchange_price,
        )
        .ok_or_else(|| error!(MarginfiError::MathError))?
    };

    let pre_f_token_balance = accessor::amount(&protocol_accounts[2])?;

    // CPI: JupLend deposit
    {
        let accounts = Deposit {
            signer: common.liquidity_vault_authority.to_account_info(),
            depositor_token_account: common.liquidity_vault.to_account_info(),
            recipient_token_account: protocol_accounts[2].clone(),
            mint: common.mint.to_account_info(),
            lending_admin: protocol_accounts[3].clone(),
            lending: protocol_accounts[0].clone(),
            f_token_mint: protocol_accounts[1].clone(),
            supply_token_reserves_liquidity: protocol_accounts[4].clone(),
            lending_supply_position_on_liquidity: protocol_accounts[5].clone(),
            rate_model: protocol_accounts[6].clone(),
            vault: protocol_accounts[7].clone(),
            liquidity: protocol_accounts[8].clone(),
            liquidity_program: protocol_accounts[9].clone(),
            rewards_rate_model: protocol_accounts[10].clone(),
            token_program: common.token_program.to_account_info(),
            associated_token_program: protocol_accounts[12].clone(),
            system_program: protocol_accounts[13].clone(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[11].clone(), accounts, signer_seeds);
        cpi_juplend_deposit(cpi_ctx, amount)?;
    }

    let post_f_token_balance = accessor::amount(&protocol_accounts[2])?;
    let minted_shares = post_f_token_balance
        .checked_sub(pre_f_token_balance)
        .ok_or_else(|| error!(MarginfiError::MathError))?;
    require_eq!(
        minted_shares,
        expected_shares,
        MarginfiError::JuplendDepositFailed
    );

    Ok((minted_shares, amount))
}

/// Called before the common pre-withdraw block to refresh exchange rate.
pub(crate) fn pre_refresh<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonWithdraw<'_, 'info>,
) -> MarginfiResult {
    check!(
        protocol_accounts.len() >= WITHDRAW_ACCOUNTS,
        MarginfiError::IntegrationAccountCountMismatch
    );

    let bank = common.bank.load()?;
    check!(
        protocol_accounts[0].key() == bank.integration_acc_1,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[2].key() == bank.integration_acc_2,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[3].key() == bank.integration_acc_3,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    drop(bank);

    check!(
        protocol_accounts[11].key() == juplend_mocks::liquidity::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[13].key() == juplend_mocks::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[14].key() == anchor_spl::associated_token::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );
    check!(
        protocol_accounts[15].key() == system_program::ID,
        MarginfiError::IntegrationAccountKeyMismatch
    );

    let lending_loader = AccountLoader::<JuplendLending>::try_from(&protocol_accounts[0])?;
    {
        let lending = lending_loader.load()?;
        validate_lending_accounts(
            &lending,
            protocol_accounts[1].key(),
            protocol_accounts[5].key(),
            protocol_accounts[6].key(),
        )?;
    }

    let intermediary = InterfaceAccount::<TokenAccount>::try_from(&protocol_accounts[3])?;
    check!(
        intermediary.owner == common.liquidity_vault_authority.key(),
        MarginfiError::InvalidJuplendWithdrawIntermediaryAta
    );
    check!(
        intermediary.mint == common.mint.key(),
        MarginfiError::InvalidJuplendWithdrawIntermediaryAta
    );

    let accounts = UpdateRate {
        lending: protocol_accounts[0].clone(),
        mint: common.mint.to_account_info(),
        f_token_mint: protocol_accounts[1].clone(),
        supply_token_reserves_liquidity: protocol_accounts[5].clone(),
        rewards_rate_model: protocol_accounts[12].clone(),
    };
    let cpi_ctx = CpiContext::new(protocol_accounts[13].clone(), accounts);
    update_rate(cpi_ctx)?;
    Ok(())
}

/// Protocol-specific pre-withdraw balance computation for JupLend.
/// Returns (token_amount, shares_to_burn).
pub(crate) fn pre_withdraw<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    bank: &mut Bank,
    marginfi_account: &mut MarginfiAccount,
    bank_key: &Pubkey,
    amount: u64,
    withdraw_all: bool,
) -> MarginfiResult<(u64, u64)> {
    check!(
        protocol_accounts.len() >= WITHDRAW_ACCOUNTS,
        MarginfiError::IntegrationAccountCountMismatch
    );

    let lending_loader = AccountLoader::<JuplendLending>::try_from(&protocol_accounts[0])?;
    let lending = lending_loader.load()?;

    let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
    let mut bank_account =
        BankAccountWrapper::find(bank_key, bank, &mut marginfi_account.lending_account)?;

    let (token_amount, shares_to_burn) = if withdraw_all {
        let f_tokens_balance = bank_account.withdraw_all(in_receivership)?;

        let token_amount =
            expected_assets_for_redeem_from_rate(f_tokens_balance, lending.token_exchange_price)
                .ok_or_else(|| error!(MarginfiError::MathError))?;

        let shares_to_burn =
            expected_shares_for_withdraw_from_rate(token_amount, lending.token_exchange_price)
                .ok_or_else(|| error!(MarginfiError::MathError))?;

        require!(shares_to_burn <= f_tokens_balance, MarginfiError::MathError);

        (token_amount, shares_to_burn)
    } else {
        let shares_to_burn =
            expected_shares_for_withdraw_from_rate(amount, lending.token_exchange_price)
                .ok_or_else(|| error!(MarginfiError::MathError))?;

        bank_account.withdraw(I80F48::from_num(shares_to_burn))?;

        (amount, shares_to_burn)
    };

    Ok((token_amount, shares_to_burn))
}

/// Protocol-specific CPI for JupLend withdraw.
pub(crate) fn withdraw_cpi<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonWithdraw<'_, 'info>,
    token_amount: u64,
    shares_to_burn: u64,
    authority_bump: u8,
) -> MarginfiResult<u64> {
    if token_amount == 0 {
        return Ok(0);
    }

    let pre_intermediary_balance = accessor::amount(&protocol_accounts[3])?;
    let pre_f_token_balance = accessor::amount(&protocol_accounts[2])?;

    // CPI: JupLend withdraw
    {
        let accounts = WithdrawCpi {
            signer: common.liquidity_vault_authority.to_account_info(),
            owner_token_account: protocol_accounts[2].clone(),
            recipient_token_account: protocol_accounts[3].clone(),
            lending_admin: protocol_accounts[4].clone(),
            lending: protocol_accounts[0].clone(),
            mint: common.mint.to_account_info(),
            f_token_mint: protocol_accounts[1].clone(),
            supply_token_reserves_liquidity: protocol_accounts[5].clone(),
            lending_supply_position_on_liquidity: protocol_accounts[6].clone(),
            rate_model: protocol_accounts[7].clone(),
            vault: protocol_accounts[8].clone(),
            claim_account: Some(protocol_accounts[9].clone()),
            liquidity: protocol_accounts[10].clone(),
            liquidity_program: protocol_accounts[11].clone(),
            rewards_rate_model: protocol_accounts[12].clone(),
            token_program: common.token_program.to_account_info(),
            associated_token_program: protocol_accounts[14].clone(),
            system_program: protocol_accounts[15].clone(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[13].clone(), accounts, signer_seeds);
        cpi_juplend_withdraw(cpi_ctx, token_amount)?;
    }

    let post_intermediary_balance = accessor::amount(&protocol_accounts[3])?;
    let post_f_token_balance = accessor::amount(&protocol_accounts[2])?;

    let received_underlying = post_intermediary_balance
        .checked_sub(pre_intermediary_balance)
        .ok_or_else(|| error!(MarginfiError::MathError))?;
    require_eq!(
        received_underlying,
        token_amount,
        MarginfiError::JuplendWithdrawFailed
    );

    let burned_shares = pre_f_token_balance
        .checked_sub(post_f_token_balance)
        .ok_or_else(|| error!(MarginfiError::MathError))?;
    require_eq!(
        burned_shares,
        shares_to_burn,
        MarginfiError::JuplendWithdrawFailed
    );

    // Transfer intermediary ATA -> destination
    {
        let program = common.token_program.to_account_info();
        let cpi_accounts = TransferChecked {
            from: protocol_accounts[3].clone(),
            to: common.destination_token_account.to_account_info(),
            authority: common.liquidity_vault_authority.to_account_info(),
            mint: common.mint.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, cpi_accounts, signer_seeds);
        transfer_checked(cpi_ctx, received_underlying, common.mint_decimals)?;
    }

    Ok(received_underlying)
}
