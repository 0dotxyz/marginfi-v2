use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::METADATA_SEED,
    types::{Bank, BankMetadata},
};

pub fn init_bank_metadata_no_seed(ctx: Context<InitBankMetadataNoSeed>) -> Result<()> {
    let mut metadata = ctx.accounts.metadata.load_init()?;

    metadata.bank = ctx.accounts.bank.key();
    metadata.bump = ctx.bumps.metadata;

    Ok(())
}

#[derive(Accounts)]
pub struct InitBankMetadataNoSeed<'info> {
    pub bank: AccountLoader<'info, Bank>,

    /// Pays the init fee
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Note: unique per-bank.
    #[account(
        init,
        seeds = [
            METADATA_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump,
        payer = fee_payer,
        space = 8 + BankMetadata::LEN,
    )]
    pub metadata: AccountLoader<'info, BankMetadata>,

    pub system_program: Program<'info, System>,
}
