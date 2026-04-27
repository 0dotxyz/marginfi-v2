use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, BankMetadata, MarginfiGroup};

use super::write_bank_metadata::apply_metadata_write;

pub fn write_bank_metadata_no_seed(
    ctx: Context<WriteBankMetadataNoSeed>,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Result<()> {
    let mut metadata = ctx.accounts.metadata.load_mut()?;
    apply_metadata_write(&mut metadata, ticker, description)
}

#[derive(Accounts)]
pub struct WriteBankMetadataNoSeed<'info> {
    #[account(
        has_one = metadata_admin,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        has_one = group,
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub metadata_admin: Signer<'info>,

    #[account(
        mut,
        has_one = bank
    )]
    pub metadata: AccountLoader<'info, BankMetadata>,
}
