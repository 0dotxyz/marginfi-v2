use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::BANK_SEED_KNOWN,
    types::{Bank, BankMetadata, MarginfiGroup},
};

use crate::{check_eq, MarginfiError};

pub fn write_bank_metadata(
    ctx: Context<WriteBankMetadata>,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Result<()> {
    // When the bank's seed is on-chain, recompute the canonical PDA and verify the passed bank
    // account matches it. Legacy keypair banks (BANK_SEED_KNOWN unset) skip the check.
    {
        let bank = ctx.accounts.bank.load()?;
        if (bank.flags & BANK_SEED_KNOWN) != 0 {
            let (expected, _) = Pubkey::find_program_address(
                &[
                    bank.group.as_ref(),
                    bank.mint.as_ref(),
                    &bank.bank_seed.to_le_bytes(),
                ],
                &crate::ID,
            );
            check_eq!(
                expected,
                ctx.accounts.bank.key(),
                MarginfiError::InvalidBankAccount
            );
        }
    }

    let mut metadata = ctx.accounts.metadata.load_mut()?;
    apply_metadata_write(&mut metadata, ticker, description)
}

pub fn write_bank_metadata_pre_init(
    ctx: Context<WriteBankMetadataPreInit>,
    _bank_seed: u64,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Result<()> {
    let mut metadata = ctx.accounts.metadata.load_mut()?;
    apply_metadata_write(&mut metadata, ticker, description)
}

pub(super) fn apply_metadata_write(
    metadata: &mut BankMetadata,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Result<()> {
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
    #[account(has_one = metadata_admin)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// Must be initialized. The metadata-to-bank binding is enforced by `metadata.has_one = bank`,
    /// and `bank.has_one = group` ties this bank to the admin's group.
    #[account(has_one = group)]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub metadata_admin: Signer<'info>,

    #[account(mut, has_one = bank)]
    pub metadata: AccountLoader<'info, BankMetadata>,
}

#[derive(Accounts)]
#[instruction(bank_seed: u64)]
pub struct WriteBankMetadataPreInit<'info> {
    #[account(has_one = metadata_admin)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// CHECK: pubkey only used for canonical seeded bank PDA derivation.
    pub bank_mint: UncheckedAccount<'info>,

    /// CHECK: Canonical bank PDA from (group, bank_mint, bank_seed); bank may be uninitialized.
    #[account(
        seeds = [
            group.key().as_ref(),
            bank_mint.key().as_ref(),
            &bank_seed.to_le_bytes(),
        ],
        bump,
    )]
    pub bank: UncheckedAccount<'info>,

    #[account(mut)]
    pub metadata_admin: Signer<'info>,

    #[account(mut, has_one = bank)]
    pub metadata: AccountLoader<'info, BankMetadata>,
}
