// Adds a JupLend type bank to a group with sane defaults. Used to integrate with JupLend
// allowing users to interact with JupLend lending pools through marginfi.
use crate::{
    events::{GroupEventHeader, LendingPoolBankCreateEvent},
    log_pool_info,
    state::{
        bank::BankImpl, bank_config::BankConfigImpl, juplend::JuplendConfigCompact,
        marginfi_group::MarginfiGroupImpl,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_interface::*;
use juplend_mocks::state::Lending as JuplendLending;
use marginfi_type_crate::constants::{
    FEE_VAULT_AUTHORITY_SEED, FEE_VAULT_SEED, INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED,
    IS_T22, JUPLEND_F_TOKEN_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
};
use marginfi_type_crate::types::{Bank, MarginfiGroup, OracleSetup};

/// Add a JupLend bank to the marginfi lending pool.
///
/// Bank starts `Paused` because once the fToken vault exists, the bank can be interacted
/// with even without a seed deposit. Call `juplend_init_position` to activate.
///
/// Remaining accounts: 0. oracle feed, 1. JupLend `Lending` state
pub fn lending_pool_add_bank_juplend(
    ctx: Context<LendingPoolAddBankJuplend>,
    bank_config: JuplendConfigCompact,
    _bank_seed: u64,
) -> MarginfiResult {
    // Note: JupLend banks don't need to debit the flat SOL fee because these will always be
    // first-party pools owned by mrgn and never permissionless pools
    let LendingPoolAddBankJuplend {
        bank_mint,
        bank: bank_loader,
        integration_acc_1,
        integration_acc_2,
        ..
    } = ctx.accounts;

    let mut bank = bank_loader.load_init()?;
    let mut group = ctx.accounts.group.load_mut()?;
    let lending_key = integration_acc_1.key();
    let f_token_vault_key = integration_acc_2.key();

    // Validate that we're using a supported Juplend oracle setup type
    require!(
        matches!(
            bank_config.oracle_setup,
            OracleSetup::JuplendPythPull | OracleSetup::JuplendSwitchboardPull
        ),
        MarginfiError::JuplendInvalidOracleSetup
    );

    let config = bank_config.to_bank_config(lending_key);

    let liquidity_vault_bump = ctx.bumps.liquidity_vault;
    let liquidity_vault_authority_bump = ctx.bumps.liquidity_vault_authority;
    let insurance_vault_bump = ctx.bumps.insurance_vault;
    let insurance_vault_authority_bump = ctx.bumps.insurance_vault_authority;
    let fee_vault_bump = ctx.bumps.fee_vault;
    let fee_vault_authority_bump = ctx.bumps.fee_vault_authority;

    *bank = Bank::new(
        ctx.accounts.group.key(),
        config,
        bank_mint.key(),
        bank_mint.decimals,
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
    );
    if bank_mint.to_account_info().owner == &anchor_spl::token_2022::ID {
        bank.flags |= IS_T22;
    }

    // Set JupLend-specific fields
    // - integration_acc_2: protocol fToken vault (PDA token account)
    // - integration_acc_3: withdraw intermediary ATA expected by JupLend's withdraw constraints
    bank.integration_acc_1 = lending_key;
    bank.integration_acc_2 = f_token_vault_key;
    bank.integration_acc_3 = get_associated_token_address_with_program_id(
        &ctx.accounts.liquidity_vault_authority.key(),
        &bank_mint.key(),
        &ctx.accounts.token_program.key(),
    );

    log_pool_info(&bank);

    group.add_bank()?;

    bank.config.validate()?;
    bank.config
        .validate_oracle_setup(ctx.remaining_accounts, None, None, None)?;

    emit!(LendingPoolBankCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(group.admin)
        },
        bank: bank_loader.key(),
        mint: bank_mint.key(),
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_config: JuplendConfigCompact, bank_seed: u64)]
pub struct LendingPoolAddBankJuplend<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Must match the mint used by the JupLend lending state.
    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<Bank>(),
        payer = fee_payer,
        seeds = [
            group.key().as_ref(),
            bank_mint.key().as_ref(),
            &bank_seed.to_le_bytes(),
        ],
        bump,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// JupLend lending state account that must match the bank mint.
    #[account(
        constraint = integration_acc_1.load()?.mint == bank_mint.key()
            @ MarginfiError::JuplendLendingMintMismatch,
        has_one = f_token_mint @ MarginfiError::InvalidJuplendLending,
    )]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    /// Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws.
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// For JupLend banks, the `liquidity_vault` is used as an intermediary when depositing/
    /// withdrawing, e.g., withdrawn funds move from JupLend -> here -> the user's token account.
    #[account(
        init,
        payer = fee_payer,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
        token::mint = bank_mint,
        token::authority = liquidity_vault_authority,
        token::token_program = token_program,
    )]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Note: Currently does nothing.
    #[account(
        seeds = [
            INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub insurance_vault_authority: SystemAccount<'info>,

    /// Note: Currently does nothing.
    #[account(
        init,
        payer = fee_payer,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
        token::mint = bank_mint,
        token::authority = insurance_vault_authority,
    )]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        seeds = [
            FEE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub fee_vault_authority: SystemAccount<'info>,

    #[account(
        init,
        payer = fee_payer,
        seeds = [
            FEE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
        token::mint = bank_mint,
        token::authority = fee_vault_authority,
    )]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The bank's fToken vault holds the fTokens received when depositing into JupLend.
    ///
    #[account(
        init,
        payer = fee_payer,
        seeds = [
            JUPLEND_F_TOKEN_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
        token::mint = f_token_mint,
        token::authority = liquidity_vault_authority,
        token::token_program = token_program,
    )]
    pub integration_acc_2: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Token program for both underlying mint and fToken mint (SPL Token or Token-2022).
    /// JupLend creates fToken mints using the same token program as the underlying.
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
