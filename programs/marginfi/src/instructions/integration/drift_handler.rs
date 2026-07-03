use crate::{
    bank_signer, check,
    state::{
        bank::BankVaultType,
        marginfi_account::{BankAccountWrapper, MarginfiAccountImpl},
    },
    utils::{expect_protocol_accounts, optional_account},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_spl::token::accessor;
use drift_mocks::drift::cpi::accounts::{Deposit, UpdateSpotMarketCumulativeInterest, Withdraw};
use drift_mocks::drift::cpi::{
    deposit as cpi_drift_deposit, update_spot_market_cumulative_interest,
    withdraw as cpi_drift_withdraw,
};
use drift_mocks::state::MinimalUser;
use fixed::types::I80F48;
use marginfi_type_crate::pdas::DRIFT_PROGRAM_ID;
use marginfi_type_crate::types::{Bank, MarginfiAccount, ACCOUNT_IN_RECEIVERSHIP};

use super::{cpi_transfer_to_destination, CommonDeposit, CommonWithdraw, WithdrawAmounts};

/// Expected protocol_accounts layout for Drift deposit:
/// 0: drift_state
/// 1: integration_acc_2 (user) - mut
/// 2: integration_acc_3 (user_stats) - mut
/// 3: integration_acc_1 (spot_market) - mut
/// 4: drift_spot_market_vault - mut
/// 5: drift_program
/// 6: system_program
/// 7: drift_oracle (optional)
pub const DEPOSIT_ACCOUNTS: usize = 8;
/// The trailing drift_oracle slot is optional, so only the accounts before it are required.
const DEPOSIT_REQUIRED_ACCOUNTS: usize = DEPOSIT_ACCOUNTS - 1;
const DEPOSIT_PROGRAM_INDEX: usize = 5;
const DEPOSIT_SYSTEM_PROGRAM_INDEX: usize = 6;

/// Expected protocol_accounts layout for Drift withdraw:
/// 0-7: same as deposit but with drift_signer at [5], drift_program at [6], system_program at [7]
/// 8: drift_oracle (optional)
/// 9-14: reward oracles/spot_markets/mints (optional)
pub const WITHDRAW_ACCOUNTS: usize = 15;
const WITHDRAW_PROGRAM_INDEX: usize = 6;
const WITHDRAW_SYSTEM_PROGRAM_INDEX: usize = 7;

/// Layout slots filled from the bank's integration accounts: (slot, integration account number).
/// The unified instructions take these as named accounts and weave them into the layout, so
/// callers pass only the other slots via remaining accounts.
pub const INTEGRATION_SLOTS: &[(usize, u8)] = &[(1, 2), (2, 3), (3, 1)];

/// Validates bank integration keys, drift program IDs, and spot market mint.
fn validate_bank_keys<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    bank: &Bank,
    min_count: usize,
    program_id_index: usize,
    system_program_index: usize,
    mint_key: Pubkey,
) -> MarginfiResult {
    expect_protocol_accounts(
        protocol_accounts,
        min_count,
        &[
            (3, bank.integration_acc_1),
            (1, bank.integration_acc_2),
            (2, bank.integration_acc_3),
            (program_id_index, DRIFT_PROGRAM_ID),
            (system_program_index, system_program::ID),
        ],
    )?;

    let spot_market_loader =
        AccountLoader::<drift_mocks::state::MinimalSpotMarket>::try_from(&protocol_accounts[3])?;
    check!(
        spot_market_loader.load()?.mint == mint_key,
        MarginfiError::DriftSpotMarketMintMismatch
    );

    let user_loader = AccountLoader::<MinimalUser>::try_from(&protocol_accounts[1])?;
    let user = user_loader.load()?;
    let market_index = spot_market_loader.load()?.market_index;
    check!(
        user.validate_spot_position(market_index).is_ok(),
        MarginfiError::DriftInvalidSpotPositions
    );

    Ok(())
}

fn validate_withdraw_setup<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonWithdraw<'_, 'info>,
) -> MarginfiResult {
    let bank = common.bank.load()?;
    validate_bank_keys(
        protocol_accounts,
        &bank,
        WITHDRAW_ACCOUNTS,
        WITHDRAW_PROGRAM_INDEX,
        WITHDRAW_SYSTEM_PROGRAM_INDEX,
        common.mint.key(),
    )?;
    drop(bank);

    let user_loader = AccountLoader::<MinimalUser>::try_from(&protocol_accounts[1])?;
    let user = user_loader.load()?;
    check!(
        user.validate_reward_accounts(
            optional_account(protocol_accounts.get(11)).is_none(),
            optional_account(protocol_accounts.get(12)).is_none(),
        )
        .is_ok(),
        MarginfiError::DriftMissingRewardAccounts
    );
    check!(
        user.validate_not_bricked_by_admin_deposits().is_ok(),
        MarginfiError::DriftBrickedAccount
    );

    Ok(())
}

pub(crate) fn deposit<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonDeposit<'_, 'info>,
    amount: u64,
    authority_bump: u8,
) -> MarginfiResult<(u64, u64)> {
    let bank = common.bank.load()?;
    validate_bank_keys(
        protocol_accounts,
        &bank,
        DEPOSIT_REQUIRED_ACCOUNTS,
        DEPOSIT_PROGRAM_INDEX,
        DEPOSIT_SYSTEM_PROGRAM_INDEX,
        common.mint.key(),
    )?;
    drop(bank);

    let spot_market_loader =
        AccountLoader::<drift_mocks::state::MinimalSpotMarket>::try_from(&protocol_accounts[3])?;
    let user_loader = AccountLoader::<MinimalUser>::try_from(&protocol_accounts[1])?;
    let market_index = spot_market_loader.load()?.market_index;

    // CPI: Update spot market cumulative interest
    {
        let oracle_info = optional_account(protocol_accounts.get(7))
            .unwrap_or_else(|| protocol_accounts[6].clone());

        let accounts = UpdateSpotMarketCumulativeInterest {
            state: protocol_accounts[0].clone(),
            spot_market: protocol_accounts[3].clone(),
            oracle: oracle_info,
            spot_market_vault: protocol_accounts[4].clone(),
        };
        let cpi_ctx = CpiContext::new(protocol_accounts[5].key(), accounts);
        update_spot_market_cumulative_interest(cpi_ctx)?;
    }

    let expected_scaled_balance_change = spot_market_loader
        .load()?
        .get_scaled_balance_increment(amount)?;

    let initial_scaled_balance = user_loader.load()?.get_scaled_balance(market_index);

    // CPI: Drift deposit
    {
        let accounts = Deposit {
            state: protocol_accounts[0].clone(),
            user: protocol_accounts[1].clone(),
            user_stats: protocol_accounts[2].clone(),
            authority: common.liquidity_vault_authority.to_account_info(),
            spot_market_vault: protocol_accounts[4].clone(),
            user_token_account: common.liquidity_vault.to_account_info(),
            token_program: common.token_program.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let mut cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[5].key(), accounts, signer_seeds);

        let mut remaining = Vec::new();
        if let Some(oracle) = optional_account(protocol_accounts.get(7)) {
            remaining.push(oracle.clone());
        }
        remaining.push(protocol_accounts[3].clone()); // spot market
        remaining.push(common.mint.to_account_info()); // token mint
        cpi_ctx = cpi_ctx.with_remaining_accounts(remaining);

        cpi_drift_deposit(cpi_ctx, market_index, amount, false)?;
    }

    let final_scaled_balance = user_loader.load()?.get_scaled_balance(market_index);
    let scaled_balance_change = final_scaled_balance - initial_scaled_balance;
    require_eq!(
        scaled_balance_change,
        expected_scaled_balance_change,
        MarginfiError::DriftScaledBalanceMismatch
    );

    Ok((scaled_balance_change, amount))
}

/// Called before the common pre-withdraw block to refresh spot market interest.
pub(crate) fn pre_refresh<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonWithdraw<'_, 'info>,
) -> MarginfiResult {
    validate_withdraw_setup(protocol_accounts, common)?;

    let oracle_info =
        optional_account(protocol_accounts.get(8)).unwrap_or_else(|| protocol_accounts[7].clone());

    let accounts = UpdateSpotMarketCumulativeInterest {
        state: protocol_accounts[0].clone(),
        spot_market: protocol_accounts[3].clone(),
        oracle: oracle_info,
        spot_market_vault: protocol_accounts[4].clone(),
    };
    let cpi_ctx = CpiContext::new(protocol_accounts[6].key(), accounts);
    update_spot_market_cumulative_interest(cpi_ctx)?;
    Ok(())
}

/// Protocol-specific pre-withdraw balance computation for Drift.
pub(crate) fn pre_withdraw<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    bank: &mut Bank,
    marginfi_account: &mut MarginfiAccount,
    bank_key: &Pubkey,
    amount: u64,
    withdraw_all: bool,
) -> MarginfiResult<WithdrawAmounts> {
    check!(
        protocol_accounts.len() >= WITHDRAW_ACCOUNTS,
        MarginfiError::IntegrationAccountCountMismatch
    );

    let spot_market_loader =
        AccountLoader::<drift_mocks::state::MinimalSpotMarket>::try_from(&protocol_accounts[3])?;

    let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
    let mut bank_account =
        BankAccountWrapper::find(bank_key, bank, &mut marginfi_account.lending_account)?;

    let (token_amount, expected_scaled_balance_change, share_amount) = if withdraw_all {
        let (scaled_balance, share_amount) = bank_account.withdraw_all(in_receivership)?;

        let mut token_amount = spot_market_loader
            .load()?
            .get_withdraw_token_amount(scaled_balance)?;
        let mut expected_scaled_balance_change = spot_market_loader
            .load()?
            .get_scaled_balance_decrement(token_amount)?;

        // Rounding fix: if Drift would decrement one extra scaled unit, reduce token amount by 1
        if expected_scaled_balance_change == scaled_balance + 1 && token_amount > 0 {
            token_amount = token_amount.saturating_sub(1);
            expected_scaled_balance_change = spot_market_loader
                .load()?
                .get_scaled_balance_decrement(token_amount)?;
        }

        require_gte!(
            scaled_balance,
            expected_scaled_balance_change,
            MarginfiError::MathError
        );

        (token_amount, expected_scaled_balance_change, share_amount)
    } else {
        let mut scaled_decrement = spot_market_loader
            .load()?
            .get_scaled_balance_decrement(amount)?;
        let mut token_amount = amount;

        let asset_shares_i80f48: I80F48 = bank_account.balance.asset_shares.into();
        let asset_shares = asset_shares_i80f48.to_num::<u64>();

        if scaled_decrement > asset_shares + 1 {
            return Err(error!(MarginfiError::OperationWithdrawOnly));
        } else if scaled_decrement == asset_shares + 1 {
            // Rounding fix: clamp to exact balance when off by one scaled unit
            token_amount = spot_market_loader
                .load()?
                .get_withdraw_token_amount(asset_shares)?;
            scaled_decrement = spot_market_loader
                .load()?
                .get_scaled_balance_decrement(token_amount)?;
        }

        let share_amount = bank_account.withdraw(I80F48::from_num(scaled_decrement))?;

        (token_amount, scaled_decrement, share_amount)
    };

    Ok(WithdrawAmounts {
        balance_units: expected_scaled_balance_change,
        tokens: token_amount,
        event_amount: token_amount,
        shares: share_amount,
    })
}

/// Protocol-specific CPI for Drift withdraw.
pub(crate) fn withdraw_cpi<'info>(
    protocol_accounts: &'info [AccountInfo<'info>],
    common: &CommonWithdraw<'_, 'info>,
    amounts: &WithdrawAmounts,
    authority_bump: u8,
) -> MarginfiResult {
    let token_amount = amounts.tokens;
    let expected_scaled_balance_change = amounts.balance_units;
    let spot_market_loader =
        AccountLoader::<drift_mocks::state::MinimalSpotMarket>::try_from(&protocol_accounts[3])?;
    let user_loader = AccountLoader::<MinimalUser>::try_from(&protocol_accounts[1])?;
    let market_index = spot_market_loader.load()?.market_index;

    if token_amount == 0 {
        return Ok(());
    }

    let initial_scaled_balance = user_loader.load()?.get_scaled_balance(market_index);
    let pre_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;

    // CPI: Drift withdraw
    {
        let accounts = Withdraw {
            state: protocol_accounts[0].clone(),
            user: protocol_accounts[1].clone(),
            user_stats: protocol_accounts[2].clone(),
            authority: common.liquidity_vault_authority.to_account_info(),
            spot_market_vault: protocol_accounts[4].clone(),
            drift_signer: protocol_accounts[5].clone(),
            user_token_account: common.liquidity_vault.to_account_info(),
            token_program: common.token_program.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, common.bank.key(), authority_bump);
        let mut cpi_ctx =
            CpiContext::new_with_signer(protocol_accounts[6].key(), accounts, signer_seeds);

        let mut remaining = Vec::new();
        if let Some(oracle) = optional_account(protocol_accounts.get(8)) {
            remaining.push(oracle.clone());
        }
        if let Some(reward_oracle) = optional_account(protocol_accounts.get(9)) {
            remaining.push(reward_oracle.clone());
        }
        if let Some(reward_oracle_2) = optional_account(protocol_accounts.get(10)) {
            remaining.push(reward_oracle_2.clone());
        }
        remaining.push(protocol_accounts[3].clone()); // spot market
        if let Some(reward_sm) = optional_account(protocol_accounts.get(11)) {
            remaining.push(reward_sm.clone());
        }
        if let Some(reward_sm_2) = optional_account(protocol_accounts.get(12)) {
            remaining.push(reward_sm_2.clone());
        }
        remaining.push(common.mint.to_account_info()); // token mint
        if let Some(reward_mint) = optional_account(protocol_accounts.get(13)) {
            remaining.push(reward_mint.clone());
        }
        if let Some(reward_mint_2) = optional_account(protocol_accounts.get(14)) {
            remaining.push(reward_mint_2.clone());
        }

        cpi_ctx = cpi_ctx.with_remaining_accounts(remaining);
        cpi_drift_withdraw(cpi_ctx, market_index, token_amount, true)?;
    }

    let final_scaled_balance = user_loader.load()?.get_scaled_balance(market_index);
    let post_vault_balance = accessor::amount(&common.liquidity_vault.to_account_info())?;

    let actual_received = post_vault_balance - pre_vault_balance;
    let actual_scaled_change = initial_scaled_balance - final_scaled_balance;

    require_eq!(
        actual_received,
        token_amount,
        MarginfiError::DriftWithdrawFailed
    );
    require_eq!(
        actual_scaled_change,
        expected_scaled_balance_change,
        MarginfiError::DriftScaledBalanceMismatch
    );

    cpi_transfer_to_destination(
        common,
        common.liquidity_vault.clone(),
        authority_bump,
        actual_received,
    )?;

    Ok(())
}
