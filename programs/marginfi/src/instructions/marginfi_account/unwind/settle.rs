use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token::{self, SyncNative};
use fixed::types::I80F48;
use solana_borsh::v1::try_from_slice_unchecked;
use solana_stake_interface::{
    instruction as stake_ix, stake_history::StakeHistoryGetEntry, state::StakeStateV2,
    sysvar::stake_history::StakeHistorySysvar,
};

use crate::{state::bank::BankImpl, MarginfiError};
use marginfi_type_crate::{constants::LIQUIDITY_VAULT_AUTHORITY_SEED, types::Bank};

use super::SettleUnwindTicket;

pub(super) fn require_stake_deactivation_elapsed<T: StakeHistoryGetEntry>(
    stake_state: &StakeStateV2,
    current_epoch: u64,
    stake_history: &T,
) -> Result<()> {
    match stake_state {
        StakeStateV2::Stake(_, stake, _) => {
            let deactivation_epoch = stake.delegation.deactivation_epoch;
            require!(
                deactivation_epoch != u64::MAX && current_epoch > deactivation_epoch,
                MarginfiError::EpochNotOver
            );

            // Accept either cooldown model; the Stake Program CPI applies the active one.
            let old_rate_status = stake.delegation.stake_activating_and_deactivating(
                current_epoch,
                stake_history,
                None,
            );
            let new_rate_status = stake.delegation.stake_activating_and_deactivating(
                current_epoch,
                stake_history,
                Some(0),
            );
            require!(
                old_rate_status.deactivating == 0 || new_rate_status.deactivating == 0,
                MarginfiError::EpochNotOver
            );
            Ok(())
        }
        _ => err!(MarginfiError::StakePoolValidationFailed),
    }
}

pub(super) fn load_stake_state(stake_account: &AccountInfo) -> Result<StakeStateV2> {
    let stake_data = stake_account.try_borrow_data()?;
    try_from_slice_unchecked::<StakeStateV2>(&stake_data[..])
        .map_err(|_| error!(MarginfiError::StakePoolValidationFailed))
}

fn withdraw_lamports_from_stake_account<'info>(
    accounts: &SettleUnwindTicket<'info>,
    destination: &AccountInfo<'info>,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> Result<()> {
    invoke_signed(
        &stake_ix::withdraw(
            &accounts.stake_account.key(),
            &accounts.sol_liquidity_vault_authority.key(),
            destination.key,
            lamports,
            None,
        ),
        &[
            accounts.stake_account.to_account_info(),
            destination.clone(),
            accounts.clock.to_account_info(),
            accounts.stake_history.to_account_info(),
            accounts.sol_liquidity_vault_authority.to_account_info(),
            accounts.stake_program.to_account_info(),
        ],
        &[signer_seeds],
    )?;
    Ok(())
}

pub(super) fn book_unwind_settlement(
    bank: &mut Bank,
    received_lamports: u64,
    principal_lamports: u64,
) -> Result<()> {
    let settlement_pnl_lamports = I80F48::from_num(received_lamports)
        .checked_sub(I80F48::from_num(principal_lamports))
        .ok_or(MarginfiError::MathError)?;
    bank.apply_pnl_to_asset_share_value(settlement_pnl_lamports)?;
    bank.release_pending_unwind(principal_lamports)?;
    Ok(())
}

pub fn settle_unwind_ticket(ctx: Context<SettleUnwindTicket>) -> Result<()> {
    let clock = Clock::get()?;
    let accounts = &ctx.accounts;
    let principal_lamports = accounts.unwind_ticket.principal_lamports;
    let stake_rent_prefund_lamports = accounts.unwind_ticket.stake_rent_prefund_lamports;

    require!(
        clock.epoch > accounts.unwind_ticket.created_epoch,
        MarginfiError::EpochNotOver
    );

    let stake_state = load_stake_state(&accounts.stake_account.to_account_info())?;
    let stake_history = StakeHistorySysvar(clock.epoch);
    require_stake_deactivation_elapsed(&stake_state, clock.epoch, &stake_history)?;

    let sol_bank_key = accounts.sol_bank.key();
    let sol_vault_authority_bump = {
        let bank = accounts.sol_bank.load()?;
        bank.liquidity_vault_authority_bump
    };
    let sol_vault_authority_seeds: &[&[u8]] = &[
        LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
        sol_bank_key.as_ref(),
        &[sol_vault_authority_bump],
    ];

    let amount_to_vault_lamports = accounts
        .stake_account
        .lamports()
        .checked_sub(stake_rent_prefund_lamports)
        .ok_or(MarginfiError::MathError)?;
    withdraw_lamports_from_stake_account(
        accounts,
        &accounts.sol_liquidity_vault.to_account_info(),
        amount_to_vault_lamports,
        sol_vault_authority_seeds,
    )?;
    token::sync_native(CpiContext::new(
        accounts.token_program.key(),
        SyncNative {
            account: accounts.sol_liquidity_vault.to_account_info(),
        },
    ))?;
    withdraw_lamports_from_stake_account(
        accounts,
        &accounts.rent_recipient.to_account_info(),
        stake_rent_prefund_lamports,
        sol_vault_authority_seeds,
    )?;

    let group = accounts.group.load()?;
    let mut bank = accounts.sol_bank.load_mut()?;
    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        sol_bank_key,
    )?;
    book_unwind_settlement(&mut bank, amount_to_vault_lamports, principal_lamports)?;
    bank.update_bank_cache(&group)?;
    Ok(())
}
