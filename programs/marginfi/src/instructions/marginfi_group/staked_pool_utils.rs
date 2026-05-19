use crate::{check, constants::SPL_SINGLE_POOL_ID, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;

/// Validate the vote account owner and derive the SPL single-pool PDA chain:
/// `vote_account -> stake_pool -> (lst_mint, sol_pool)`.
pub(crate) fn derive_single_pool_keys_from_vote_and_validate_owner(
    validator_vote_account: &AccountInfo<'_>,
) -> MarginfiResult<(Pubkey, Pubkey, Pubkey)> {
    check!(
        validator_vote_account.owner == &anchor_lang::solana_program::vote::program::id(),
        MarginfiError::StakePoolValidationFailed
    );

    let vote_account = validator_vote_account.key();
    let vote_account_bytes = vote_account.to_bytes();
    let (stake_pool, _) =
        Pubkey::find_program_address(&[b"pool", &vote_account_bytes], &SPL_SINGLE_POOL_ID);

    let stake_pool_bytes = stake_pool.to_bytes();
    let (lst_mint, _) =
        Pubkey::find_program_address(&[b"mint", &stake_pool_bytes], &SPL_SINGLE_POOL_ID);
    let (sol_pool, _) =
        Pubkey::find_program_address(&[b"stake", &stake_pool_bytes], &SPL_SINGLE_POOL_ID);

    Ok((stake_pool, lst_mint, sol_pool))
}
