use crate::{
    check,
    constants::{LOCALNET_ID, MAINNET_PROGRAM_ID, STAGING_ID},
    events::{GroupEventHeader, LendingPoolBankCreateEvent},
    log_pool_info,
    state::{bank::BankImpl, marginfi_group::MarginfiGroupImpl},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;
use bytemuck::from_bytes;
use marginfi_type_crate::{
    constants::{
        BANK_SEED_KNOWN, FEE_VAULT_AUTHORITY_SEED, FEE_VAULT_SEED, INSURANCE_VAULT_AUTHORITY_SEED,
        INSURANCE_VAULT_SEED, IS_T22, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
    },
    types::{Bank, MarginfiGroup},
};

pub fn lending_pool_clone_bank(
    ctx: Context<LendingPoolCloneBank>,
    bank_seed: u64,
) -> MarginfiResult {
    if crate::ID != STAGING_ID && crate::ID != LOCALNET_ID {
        panic!("Staging or localnet only!");
    }

    // Sanity check
    if crate::ID == MAINNET_PROGRAM_ID || *ctx.program_id == MAINNET_PROGRAM_ID {
        panic!("clone bank cannot run on mainnet deployment");
    }

    // Note: We don't bother to pay the flat init fee, this ix only runs on staging.

    let _ = bank_seed;

    let (
        source_mint,
        source_mint_decimals,
        source_config,
        source_flags,
        source_emissions_rate,
        source_emissions_mint,
        source_emode,
        source_fees_destination_account,
    ) = {
        let source_bank_info = ctx.accounts.source_bank.as_ref();

        check!(
            *source_bank_info.owner == MAINNET_PROGRAM_ID,
            MarginfiError::InvalidBankAccount
        );

        let data = source_bank_info
            .try_borrow_data()
            .map_err(|_| error!(MarginfiError::InvalidBankAccount))?;

        check!(
            data.len() >= 8 + std::mem::size_of::<Bank>(),
            MarginfiError::InvalidBankAccount
        );
        check!(
            data[..8] == Bank::DISCRIMINATOR,
            MarginfiError::InvalidBankAccount
        );

        let bank_data = &data[8..8 + std::mem::size_of::<Bank>()];
        let source_bank = from_bytes::<Bank>(bank_data);

        (
            source_bank.mint,
            source_bank.mint_decimals,
            source_bank.config,
            source_bank.flags,
            source_bank.emissions_rate,
            source_bank.emissions_mint,
            source_bank.emode,
            source_bank.fees_destination_account,
        )
    };

    // Sanity checks to make sure it's the same mint
    check!(
        ctx.accounts.bank_mint.key() == source_mint,
        MarginfiError::InvalidBankAccount
    );
    check!(
        ctx.accounts.bank_mint.decimals == source_mint_decimals,
        MarginfiError::InvalidBankAccount
    );

    let liquidity_vault_bump = ctx.bumps.liquidity_vault;
    let liquidity_vault_authority_bump = ctx.bumps.liquidity_vault_authority;
    let insurance_vault_bump = ctx.bumps.insurance_vault;
    let insurance_vault_authority_bump = ctx.bumps.insurance_vault_authority;
    let fee_vault_bump = ctx.bumps.fee_vault;
    let fee_vault_authority_bump = ctx.bumps.fee_vault_authority;

    let mut bank = ctx.accounts.bank.load_init()?;
    *bank = Bank::new(
        ctx.accounts.marginfi_group.key(),
        source_config,
        source_mint,
        source_mint_decimals,
        ctx.accounts.liquidity_vault.key(),
        ctx.accounts.insurance_vault.key(),
        ctx.accounts.fee_vault.key(),
        Clock::get().unwrap().unix_timestamp,
        liquidity_vault_bump,
        liquidity_vault_authority_bump,
        insurance_vault_bump,
        insurance_vault_authority_bump,
        fee_vault_bump,
        fee_vault_authority_bump,
        bank_seed,
    );

    bank.flags = source_flags;
    bank.flags |= BANK_SEED_KNOWN;
    if ctx.accounts.bank_mint.to_account_info().owner == &anchor_spl::token_2022::ID {
        bank.flags |= IS_T22;
    }
    bank.emissions_rate = source_emissions_rate;
    bank.emissions_mint = source_emissions_mint;
    bank.emode = source_emode;
    bank.fees_destination_account = source_fees_destination_account;

    log_pool_info(&bank);

    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    group.add_bank()?;

    // Note: we don't bother to validate, if the bank is in an invalid state, we'll copy that too.

    emit!(LendingPoolBankCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        bank: ctx.accounts.bank.key(),
        mint: ctx.accounts.bank_mint.key(),
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_seed: u64)]
pub struct LendingPoolCloneBank<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Source bank to clone from mainnet program
    ///
    /// CHECK: Validate only that it belongs to the mainnet program and is the correct length.
    pub source_bank: UncheckedAccount<'info>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<Bank>(),
        payer = fee_payer,
        seeds = [
            marginfi_group.key().as_ref(),
            bank_mint.key().as_ref(),
            &bank_seed.to_le_bytes(),
        ],
        bump,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: ⋐ ͡⋄ ω ͡⋄ ⋑
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump
    )]
    pub liquidity_vault_authority: AccountInfo<'info>,

    #[account(
        init,
        payer = fee_payer,
        token::mint = bank_mint,
        token::authority = liquidity_vault_authority,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: ⋐ ͡⋄ ω ͡⋄ ⋑
    #[account(
        seeds = [
            INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump
    )]
    pub insurance_vault_authority: AccountInfo<'info>,

    #[account(
        init,
        payer = fee_payer,
        token::mint = bank_mint,
        token::authority = insurance_vault_authority,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: ⋐ ͡⋄ ω ͡⋄ ⋑
    #[account(
        seeds = [
            FEE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump
    )]
    pub fee_vault_authority: AccountInfo<'info>,

    #[account(
        init,
        payer = fee_payer,
        token::mint = bank_mint,
        token::authority = fee_vault_authority,
        seeds = [
            FEE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
