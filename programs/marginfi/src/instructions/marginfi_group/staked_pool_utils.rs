use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::pdas::derive_single_pool_keys_from_vote;

/// Validate the vote account owner and derive the SPL single-pool PDA chain:
/// `vote_account -> stake_pool -> (lst_mint, sol_pool, pool_onramp)`.
pub(crate) fn derive_single_pool_keys_from_vote_and_validate_owner(
    validator_vote_account: &AccountInfo<'_>,
) -> MarginfiResult<(Pubkey, Pubkey, Pubkey, Pubkey)> {
    check!(
        validator_vote_account.owner.as_ref() == solana_vote_interface::program::id().as_ref(),
        MarginfiError::StakePoolValidationFailed
    );

    Ok(derive_single_pool_keys_from_vote(
        validator_vote_account.key(),
    ))
}
