use super::timelocked_utils::*;
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
        LIQUIDITY_VAULT_SEED, TIMELOCKED_OPERATION_SEED,
    },
    types::{
        operation_type, Bank, BankConfigCompact, FeeState, MarginfiGroup, TimelockedOperation,
    },
};

/// Step 3 of 3: Create bank after timelock. Requires validated=1.
pub fn lending_pool_finalize_timelocked_add_bank(
    ctx: Context<LendingPoolFinalizeTimelockedAddBank>,
    bank_config: BankConfigCompact,
) -> MarginfiResult {
    let timelocked_op = ctx.accounts.timelocked_operation.load()?;
    let mut marginfi_group = ctx.accounts.marginfi_group.load_mut()?;

    require!(
        timelocked_op.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidConfig
    );
    require!(
        timelocked_op.operation_type == operation_type::ADD_BANK,
        MarginfiError::InvalidConfig
    );
    require!(timelocked_op.executed == 0, MarginfiError::InvalidConfig);
    require!(timelocked_op.validated == 1, MarginfiError::InvalidConfig);

    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp >= timelocked_op.execution_available_at,
        MarginfiError::InvalidConfig
    );

    require!(
        ctx.accounts.signer.key() == timelocked_op.admin
            || ctx.accounts.signer.key() == marginfi_group.admin,
        MarginfiError::Unauthorized
    );
    require!(
        timelocked_op.bank_mint == ctx.accounts.bank_mint.key(),
        MarginfiError::InvalidConfig
    );

    assert_bank_config_matches_op(
        bank_config.deposit_limit,
        bank_config.borrow_limit,
        bank_config.risk_tier,
        bank_config.asset_tag,
        bank_config.total_asset_value_init_limit,
        &bank_config.asset_weight_init,
        &bank_config.asset_weight_maint,
        &bank_config.liability_weight_init,
        &bank_config.liability_weight_maint,
        &timelocked_op,
    )?;

    // Transfer the flat sol init fee to the global fee wallet
    let fee_state = ctx.accounts.fee_state.load()?;
    let bank_init_flat_sol_fee = fee_state.bank_init_flat_sol_fee;
    if bank_init_flat_sol_fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.fee_payer.to_account_info(),
                    to: ctx.accounts.global_fee_wallet.to_account_info(),
                },
            ),
            bank_init_flat_sol_fee as u64,
        )?;
    }

    let mut bank = ctx.accounts.bank.load_init()?;
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
        ctx.accounts.bank_mint.key(),
        ctx.accounts.bank_mint.decimals,
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

    log_pool_info(&bank);

    marginfi_group.add_bank()?;

    bank.config.validate()?;

    msg!(
        "Created bank from timelocked operation for mint: {:?}",
        ctx.accounts.bank_mint.key()
    );

    emit!(LendingPoolBankCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key())
        },
        bank: ctx.accounts.bank.key(),
        mint: ctx.accounts.bank_mint.key(),
    });

    drop(timelocked_op);
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_mut()?;
    timelocked_op.executed = 1;
    drop(timelocked_op);

    close_timelocked_account(
        &ctx.accounts.timelocked_operation,
        &ctx.accounts.fee_payer.to_account_info(),
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_config: BankConfigCompact)]
pub struct LendingPoolFinalizeTimelockedAddBank<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

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
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank_mint.key().as_ref(),
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<Bank>(),
        payer = fee_payer,
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

    pub signer: Signer<'info>,
}
