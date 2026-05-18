use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{BANK_SEED_KNOWN, METADATA_SEED},
    types::{Bank, BankMetadata},
};

use crate::{check_eq, MarginfiError};

/// (permissionless) Pay rent to create the metadata PDA for `bank`.
///
/// When the bank is already initialized and its seed is on-chain (`BANK_SEED_KNOWN`), the
/// canonical bank PDA is recomputed from `(group, mint, bank_seed)` and verified against the
/// passed `bank` key. Otherwise — account not yet created, or a legacy keypair-based bank
/// without a known seed — the check is skipped and the caller is trusted to pass the right
/// pubkey (the only cost of getting it wrong is the caller's own rent).
pub fn init_bank_metadata(ctx: Context<InitBankMetadata>) -> Result<()> {
    let bank_ai = &ctx.accounts.bank;
    let bank_key = bank_ai.key();

    if !bank_ai.data_is_empty() && bank_ai.owner == &crate::ID {
        let data = bank_ai.try_borrow_data()?;
        if data.len() >= 8 + std::mem::size_of::<Bank>() && data[..8] == Bank::DISCRIMINATOR {
            let bank: &Bank = bytemuck::from_bytes(&data[8..8 + std::mem::size_of::<Bank>()]);
            if (bank.flags & BANK_SEED_KNOWN) != 0 {
                let (expected, _) = Pubkey::find_program_address(
                    &[
                        bank.group.as_ref(),
                        bank.mint.as_ref(),
                        &bank.bank_seed.to_le_bytes(),
                    ],
                    &crate::ID,
                );
                check_eq!(expected, bank_key, MarginfiError::InvalidBankAccount);
            }
        }
    }

    let mut metadata = ctx.accounts.metadata.load_init()?;
    metadata.bank = bank_key;
    metadata.bump = ctx.bumps.metadata;
    Ok(())
}

#[derive(Accounts)]
pub struct InitBankMetadata<'info> {
    /// CHECK: bank may be uninitialized (pre-create metadata for an upcoming PDA) or a legacy
    /// keypair-based bank. When initialized with a known seed, the PDA is verified in the handler.
    pub bank: UncheckedAccount<'info>,

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
