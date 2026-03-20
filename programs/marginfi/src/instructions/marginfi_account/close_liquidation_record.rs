use crate::prelude::*;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{LiquidationRecord, MarginfiAccount, MarginfiGroup},
};

pub fn marginfi_account_close_liquidation_record(
    ctx: Context<MarginfiAccountCloseLiquidationRecord>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    marginfi_account.liquidation_record = Pubkey::default();

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiAccountCloseLiquidationRecord<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub risk_admin: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        mut,
        close = fee_payer,
        seeds = [
            LIQUIDATION_RECORD_SEED.as_bytes(),
            marginfi_account.key().as_ref(),
        ],
        bump,
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,
}
