use super::staked_pool_utils::derive_single_pool_keys_from_vote_and_validate_owner;
use crate::{check, check_eq, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::ASSET_TAG_STAKED,
    types::{Bank, OracleSetup},
};

/// (permissionless) Backfill validator vote account on pre-upgrade staked banks.
///
/// The vote account is validated by SPL single-pool PDA chain:
/// `vote -> stake_pool -> (mint, sol_pool)`.
pub fn lending_pool_backfill_staked_bank_validator_vote_account(
    ctx: Context<LendingPoolBackfillStakedBankValidatorVoteAccount>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.config.asset_tag == ASSET_TAG_STAKED,
        MarginfiError::AssetTagMismatch
    );
    check!(
        bank.config.oracle_setup == OracleSetup::StakedWithPythPush,
        MarginfiError::StakePoolValidationFailed
    );

    let validator_vote = ctx.accounts.validator_vote_account.key();
    let (_stake_pool, exp_mint, exp_sol_pool) =
        derive_single_pool_keys_from_vote_and_validate_owner(
            &ctx.accounts.validator_vote_account.to_account_info(),
        )?;

    check_eq!(
        exp_mint,
        bank.mint,
        MarginfiError::StakePoolValidationFailed
    );
    check_eq!(
        exp_mint,
        bank.config.oracle_keys[1],
        MarginfiError::StakePoolValidationFailed
    );
    check_eq!(
        exp_sol_pool,
        bank.config.oracle_keys[2],
        MarginfiError::StakePoolValidationFailed
    );

    // Idempotent: if already set, it must match.
    if bank.integration_acc_1 != Pubkey::default() {
        check_eq!(
            bank.integration_acc_1,
            validator_vote,
            MarginfiError::StakePoolValidationFailed
        );
        return Ok(());
    }

    bank.integration_acc_1 = validator_vote;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolBackfillStakedBankValidatorVoteAccount<'info> {
    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated in handler by vote-program owner check + PDA derivation.
    pub validator_vote_account: UncheckedAccount<'info>,
}
