// Global fee admin calls this to edit the variable-borrow premium fields on FeeStateV2.
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::FEE_STATE_V2_SEED, types::FeeStateV2};

pub fn edit_fee_state_v2_premium(
    ctx: Context<EditFeeStateV2Premium>,
    premium_wallet: Option<Pubkey>,
) -> Result<()> {
    let mut fee_state_v2 = ctx.accounts.fee_state_v2.load_mut()?;
    if let Some(premium_wallet) = premium_wallet {
        msg!(
            "Updating premium_wallet: {:?} -> {:?}",
            fee_state_v2.premium_wallet,
            premium_wallet
        );
        fee_state_v2.premium_wallet = premium_wallet;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct EditFeeStateV2Premium<'info> {
    /// Admin of the global FeeStateV2. Populated by `copy_fee_state_to_v2`, which must have
    /// run at least once before this instruction can succeed.
    pub global_fee_admin: Signer<'info>,

    // Note: there is just one FeeStateV2 per program, so no further check is required.
    #[account(
        mut,
        seeds = [FEE_STATE_V2_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin @ MarginfiError::Unauthorized
    )]
    pub fee_state_v2: AccountLoader<'info, FeeStateV2>,
}
