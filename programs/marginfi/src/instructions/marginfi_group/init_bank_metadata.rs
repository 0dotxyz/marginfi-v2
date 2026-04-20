use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use marginfi_type_crate::{
    constants::METADATA_SEED,
    types::{BankMetadata, MarginfiGroup},
};

pub fn init_bank_metadata(ctx: Context<InitBankMetadata>, _bank_seed: u64) -> Result<()> {
    let mut metadata = ctx.accounts.metadata.load_init()?;

    metadata.bank = ctx.accounts.bank.key();
    metadata.bump = ctx.bumps.metadata;

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_seed: u64)]
pub struct InitBankMetadata<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Canonical bank PDA; may not be initialized yet.
    #[account(
        seeds = [
            group.key().as_ref(),
            bank_mint.key().as_ref(),
            &bank_seed.to_le_bytes(),
        ],
        bump,
    )]
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
