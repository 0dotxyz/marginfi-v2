// Adds a Solend type bank to a group with sane defaults. Used to integrate with Solend
// allowing users to interact with Solend pools through marginfi
use crate::{
    constants::SOLEND_OBLIGATION_SEED,
    events::{GroupEventHeader, LendingPoolBankCreateEvent},
    log_pool_info,
    state::{
        bank::BankImpl, bank_config::BankConfigImpl, marginfi_group::MarginfiGroupImpl,
        solend::SolendConfigCompact,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;
use marginfi_type_crate::constants::{
    FEE_VAULT_AUTHORITY_SEED, FEE_VAULT_SEED, INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED,
    LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
};
use marginfi_type_crate::types::{Bank, MarginfiGroup, OracleSetup};
use solend_mocks::state::SolendMinimalReserve;

/// Add a Solend bank to the marginfi lending pool
pub fn lending_pool_add_bank_solend(
    ctx: Context<LendingPoolAddBankSolend>,
    bank_config: SolendConfigCompact,
    _bank_seed: u64,
) -> MarginfiResult {
    // Note: Solend banks don't need to debit the flat SOL fee because these will always be
    // first-party pools owned by mrgn and never permissionless pools
    let LendingPoolAddBankSolend {
        bank_mint,
        bank: bank_loader,
        integration_acc_1: reserve_loader,
        integration_acc_2: obligation_loader,
        ..
    } = ctx.accounts;

    let mut bank = bank_loader.load_init()?;
    let mut group = ctx.accounts.group.load_mut()?;
    let reserve_key = reserve_loader.key();
    let obligation_key = obligation_loader.key();

    // Validate that we're using a supported Solend oracle setup type
    require!(
        matches!(
            bank_config.oracle_setup,
            OracleSetup::SolendPythPull | OracleSetup::SolendSwitchboardPull
        ),
        MarginfiError::SolendInvalidOracleSetup
    );

    let config = bank_config.to_bank_config(reserve_key);

    let liquidity_vault_bump = ctx.bumps.liquidity_vault;
    let liquidity_vault_authority_bump = ctx.bumps.liquidity_vault_authority;
    let insurance_vault_bump = ctx.bumps.insurance_vault;
    let insurance_vault_authority_bump = ctx.bumps.insurance_vault_authority;
    let fee_vault_bump = ctx.bumps.fee_vault;
    let fee_vault_authority_bump = ctx.bumps.fee_vault_authority;

    *bank = Bank::new(
        ctx.accounts.group.key(),
        config, // Use the modified BankConfig directly instead of converting from BankConfigCompact
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

    // Set Solend-specific fields
    bank.integration_acc_1 = reserve_key;
    bank.integration_acc_2 = obligation_key;

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
#[instruction(bank_config: SolendConfigCompact, bank_seed: u64)]
pub struct LendingPoolAddBankSolend<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Must match the mint used by `integration_acc_1`, Solend calls this the `liquidity.mint_pubkey`
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

    /// Solend reserve account that must match the bank mint
    #[account(
        constraint = integration_acc_1.load()?.liquidity_mint_pubkey == bank_mint.key()
            @ MarginfiError::SolendReserveMintMismatch,
    )]
    pub integration_acc_1: AccountLoader<'info, SolendMinimalReserve>,

    /// Obligation PDA for this bank in Solend
    /// Will be initialized and transferred to Solend in init_obligation instruction
    #[account(
        seeds = [
            SOLEND_OBLIGATION_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub integration_acc_2: SystemAccount<'info>,

    /// Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// For Solend banks, the `liquidity_vault` never holds assets, but is instead used as an
    /// intermediary when depositing/withdrawing, e.g., withdrawn funds move from Solend -> here ->
    /// the user's token account.
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

    /// Note: Currently does nothing.
    #[account(
        seeds = [
            FEE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub fee_vault_authority: SystemAccount<'info>,

    /// Note: Currently does nothing.
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

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
