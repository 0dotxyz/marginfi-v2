use anchor_lang::prelude::*;
use marginfi_type_crate::types::{BankMetadata, MarginfiGroup};

use crate::MarginfiError;

pub fn write_bank_metadata(
    ctx: Context<WriteBankMetadata>,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Result<()> {
    let mut metadata = ctx.accounts.metadata.load_mut()?;

    if let Some(bytes) = ticker {
        let cap = metadata.ticker.len();
        if bytes.len() > cap {
            msg!("too long got {:?} cap is: {:?}", bytes.len(), cap);
            return err!(MarginfiError::MetadataTooLong);
        }

        // Fill with zeros in case existing data, then copy
        metadata.ticker.fill(0);
        metadata.ticker[..bytes.len()].copy_from_slice(&bytes);

        // Record last byte index to help parsers do their thing
        metadata.end_ticker_byte = if bytes.is_empty() {
            0
        } else {
            (bytes.len() - 1) as u8
        };
    }

    if let Some(bytes) = description {
        let cap = metadata.description.len();
        if bytes.len() > cap {
            msg!("too long got {:?} cap is: {:?}", bytes.len(), cap);
            return err!(MarginfiError::MetadataTooLong);
        }

        metadata.description.fill(0);
        metadata.description[..bytes.len()].copy_from_slice(&bytes);

        metadata.end_description_byte = if bytes.is_empty() {
            0
        } else {
            (bytes.len() - 1) as u16
        };
    }

    Ok(())
}

#[derive(Accounts)]
pub struct WriteBankMetadata<'info> {
    #[account(
        has_one = metadata_admin,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// CHECK: Bank may not be initialized yet. Validated via metadata.bank.
    pub bank: UncheckedAccount<'info>,

    #[account(mut)]
    pub metadata_admin: Signer<'info>,

    #[account(
        mut,
        has_one = bank
    )]
    pub metadata: AccountLoader<'info, BankMetadata>,
}
