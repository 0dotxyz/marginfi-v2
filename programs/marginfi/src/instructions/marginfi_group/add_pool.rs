use crate::{
    events::{GroupEventHeader, LendingPoolBankCreateEvent},
    log_pool_info,
    state::{bank::BankImpl, bank_config::BankConfigImpl, marginfi_group::MarginfiGroupImpl},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_SOL, FEE_STATE_SEED, FEE_VAULT_AUTHORITY_SEED, FEE_VAULT_SEED,
        INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED,
        LIQUIDITY_VAULT_SEED,
    },
    types::{Bank, BankConfigCompact, FeeState, MarginfiGroup},
};

/// Add a bank to the lending pool
///
/// Admin only
pub fn lending_pool_add_bank(
    ctx: Context<LendingPoolAddBank>,
    bank_config: BankConfigCompact,
) -> MarginfiResult {
    // Transfer the flat sol init fee to the global fee wallet
    let fee_state = ctx.accounts.fee_state.load()?;
    let bank_init_flat_sol_fee = fee_state.bank_init_flat_sol_fee;
    if bank_init_flat_sol_fee > 0 {
        anchor_lang::system_program::transfer(
            ctx.accounts.transfer_flat_fee(),
            bank_init_flat_sol_fee as u64,
        )?;
    }

    let LendingPoolAddBank {
        bank_mint,
        liquidity_vault,
        insurance_vault,
        fee_vault,
        bank: bank_loader,
        ..
    } = ctx.accounts;

    let mut bank = bank_loader.load_init()?;
    require!(
        bank_config.asset_tag == ASSET_TAG_DEFAULT || bank_config.asset_tag == ASSET_TAG_SOL,
        MarginfiError::WrongAssetTagForStandardInstructions
    );

    let liquidity_vault_bump = ctx.bumps.liquidity_vault;
    let liquidity_vault_authority_bump: u8 = ctx.bumps.liquidity_vault_authority;
    let insurance_vault_bump = ctx.bumps.insurance_vault;
    let insurance_vault_authority_bump = ctx.bumps.insurance_vault_authority;
    let fee_vault_bump = ctx.bumps.fee_vault;
    let fee_vault_authority_bump = ctx.bumps.fee_vault_authority;

    *bank = Bank::new(
        ctx.accounts.marginfi_group.key(),
        bank_config.into(),
        bank_mint.key(),
        bank_mint.decimals,
        liquidity_vault.key(),
        insurance_vault.key(),
        fee_vault.key(),
        Clock::get().unwrap().unix_timestamp,
        liquidity_vault_bump,
        liquidity_vault_authority_bump,
        insurance_vault_bump,
        insurance_vault_authority_bump,
        fee_vault_bump,
        fee_vault_authority_bump,
    );

    log_pool_info(&bank);

    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    group.add_bank()?;

    bank.config.validate()?;

    emit!(LendingPoolBankCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        bank: bank_loader.key(),
        mint: bank_mint.key(),
    });

    Ok(())
}

/*
. Aligns line spacing for easier comparison against with_seed
.
*/
#[derive(Accounts)]
#[instruction(bank_config: BankConfigCompact)]
pub struct LendingPoolAddBank<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    /// Pays to init accounts and pays `fee_state.bank_init_flat_sol_fee` lamports to the protocol
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_wallet @ MarginfiError::InvalidFeeWallet
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: The fee admin's native SOL wallet, validated against fee state
    #[account(mut)]
    pub global_fee_wallet: AccountInfo<'info>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<Bank>(),
        payer = fee_payer,
        /*
        In the "with seed" version of this ix, the seed is defined here
        .
        .
        .
        */
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

impl<'info> LendingPoolAddBank<'info> {
    fn transfer_flat_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: self.fee_payer.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}
