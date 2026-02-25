use crate::{
    ix_utils::{get_discrim_hash, Hashable},
    prelude::*,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{LiquidationCache, LiquidationEntry, LiquidationRecord, MarginfiAccount},
};

pub fn initialize_liquidation_record(ctx: Context<InitLiquidationRecord>) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_init()?;
    let now_ts = Clock::get()?.unix_timestamp;

    liq_record.key = ctx.accounts.liquidation_record.key();
    liq_record.record_payer = ctx.accounts.fee_payer.key();
    liq_record.marginfi_account = ctx.accounts.marginfi_account.key();
    liq_record.entries = [LiquidationEntry::default(); 4];
    liq_record.cache = LiquidationCache::default();
    liq_record.last_activity_ts = now_ts;

    // Link the record back to the MarginfiAccount. This also serves to inform liquidators if the
    // record exists without performing a fetch. If this field is non-default, it exists.
    marginfi_account.liquidation_record = ctx.accounts.liquidation_record.key();

    Ok(())
}

#[derive(Accounts)]
pub struct InitLiquidationRecord<'info> {
    #[account(mut)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        init,
        payer = fee_payer,
        seeds = [LIQUIDATION_RECORD_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<LiquidationRecord>()
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    pub system_program: Program<'info, System>,
}

impl Hashable for InitLiquidationRecord<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_init_liq_record")
    }
}
