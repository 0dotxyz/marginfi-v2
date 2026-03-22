use anchor_lang::prelude::*;

use crate::{errors::ErrorCodes, state::TransferType, state::UserClaim};
use library::{structs::TokenTransferParams, token::*};

/// Handle transfer or claim
/// Returns amount of tokens added for claim
pub fn handle_transfer_or_claim(
    transfer_type: &TransferType,
    claimer: Pubkey,
    claim_account: Option<&AccountLoader<UserClaim>>,
    last_stored_claim_amount: u64,
    transfer_params: TokenTransferParams,
) -> Result<u64> {
    let balance = balance_of(&transfer_params.source)?;

    if last_stored_claim_amount > balance {
        return Err(ErrorCodes::InvalidClaimAmount.into());
    }

    if balance - last_stored_claim_amount < transfer_params.amount {
        return Err(ErrorCodes::InsufficientBalance.into());
    }

    match transfer_type {
        TransferType::DIRECT => {
            transfer_spl_tokens(transfer_params)?;
            Ok(0)
        }
        TransferType::CLAIM => {
            let mut claim_account = if let Some(claim_account) = claim_account {
                claim_account.load_mut()?
            } else {
                return Err(ErrorCodes::ClaimAccountRequired.into());
            };

            if claim_account.user != claimer {
                return Err(ErrorCodes::InvalidUserClaim.into());
            }

            claim_account.approve(transfer_params.amount)?;
            return Ok(transfer_params.amount);
        }
        _ => return Err(ErrorCodes::InvalidTransferType.into()),
    }
}
