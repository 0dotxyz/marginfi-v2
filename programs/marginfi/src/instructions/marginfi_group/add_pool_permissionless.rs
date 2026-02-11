// Adds a ASSET_TAG_STAKED type bank to a group with sane defaults. Used by validators to add their
// stake pool to a group so users can borrow SOL against it
use crate::{
    check,
    constants::SPL_SINGLE_POOL_ID,
    events::{GroupEventHeader, LendingPoolBankCreateEvent},
    log_pool_info,
    state::{bank::BankImpl, bank_config::BankConfigImpl, marginfi_group::MarginfiGroupImpl},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;
use fixed_macro::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_STAKED, FEE_VAULT_AUTHORITY_SEED, FEE_VAULT_SEED, INSURANCE_VAULT_AUTHORITY_SEED,
        INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
        PYTH_PUSH_MIGRATED_DEPRECATED,
    },
    types::{
        make_points, Bank, BankConfigCompact, BankOperationalState, InterestRateConfig,
        MarginfiGroup, OracleSetup, RatePoint, StakedSettings, INTEREST_CURVE_SEVEN_POINT,
    },
};

pub fn lending_pool_add_bank_permissionless(
    ctx: Context<LendingPoolAddBankPermissionless>,
    _bank_seed: u64,
) -> MarginfiResult {
    let LendingPoolAddBankPermissionless {
        bank_mint,
        liquidity_vault,
        insurance_vault,
        fee_vault,
        bank: bank_loader,
        stake_pool,
        sol_pool,
        ..
    } = ctx.accounts;

    let mut bank = bank_loader.load_init()?;
    let settings = ctx.accounts.staked_settings.load()?;
    let mut group = ctx.accounts.marginfi_group.load_mut()?;

    let liquidity_vault_bump = ctx.bumps.liquidity_vault;
    let liquidity_vault_authority_bump = ctx.bumps.liquidity_vault_authority;
    let insurance_vault_bump = ctx.bumps.insurance_vault;
    let insurance_vault_authority_bump = ctx.bumps.insurance_vault_authority;
    let fee_vault_bump = ctx.bumps.fee_vault;
    let fee_vault_authority_bump = ctx.bumps.fee_vault_authority;

    // These are placeholder values: staked collateral positions do not support borrowing and likely
    // never will, thus they will earn no interest.

    // Note: Some placeholder values are non-zero to handle downstream validation checks.
    let default_ir_config = InterestRateConfig {
        protocol_fixed_fee_apr: I80F48!(0.01).into(),
        insurance_ir_fee: I80F48!(0.1).into(),

        zero_util_rate: 0,
        hundred_util_rate: 1234567,
        points: make_points(&[RatePoint::new(12345, 123456)]),
        curve_type: INTEREST_CURVE_SEVEN_POINT,

        ..Default::default()
    };

    let default_config: BankConfigCompact = BankConfigCompact {
        asset_weight_init: settings.asset_weight_init,
        asset_weight_maint: settings.asset_weight_maint,
        liability_weight_init: I80F48!(1.5).into(), // placeholder
        liability_weight_maint: I80F48!(1.25).into(), // placeholder
        deposit_limit: settings.deposit_limit,
        interest_rate_config: default_ir_config.into(), // placeholder
        operational_state: BankOperationalState::Operational,
        borrow_limit: 0,
        risk_tier: settings.risk_tier,
        asset_tag: ASSET_TAG_STAKED,
        config_flags: PYTH_PUSH_MIGRATED_DEPRECATED,
        _pad0: [0; 5],
        total_asset_value_init_limit: settings.total_asset_value_init_limit,
        oracle_max_age: settings.oracle_max_age,
        // Note: this will use the default of 10%. SOL oracle confidence is generally fine.
        oracle_max_confidence: 0,
    };

    let now = Clock::get().unwrap().unix_timestamp;

    *bank = Bank::new(
        ctx.accounts.marginfi_group.key(),
        default_config.into(),
        bank_mint.key(),
        bank_mint.decimals,
        liquidity_vault.key(),
        insurance_vault.key(),
        fee_vault.key(),
        now,
        liquidity_vault_bump,
        liquidity_vault_authority_bump,
        insurance_vault_bump,
        insurance_vault_authority_bump,
        fee_vault_bump,
        fee_vault_authority_bump,
    );
    bank.config.oracle_setup = OracleSetup::StakedWithPythPush;
    bank.config.oracle_keys[0] = settings.oracle;

    log_pool_info(&bank);

    group.add_bank()?;

    bank.config.validate()?;

    check!(
        stake_pool.owner == &SPL_SINGLE_POOL_ID,
        MarginfiError::StakePoolValidationFailed
    );
    let lst_mint = bank_mint.key();
    let stake_pool = stake_pool.key();
    let sol_pool = sol_pool.key();
    // The mint (for supply) and stake pool (for sol balance) are recorded for price calculation
    bank.config.oracle_keys[1] = lst_mint;
    bank.config.oracle_keys[2] = sol_pool;
    bank.config.validate_oracle_setup(
        ctx.remaining_accounts,
        Some(lst_mint),
        Some(stake_pool),
        Some(sol_pool),
    )?;

    emit!(LendingPoolBankCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(group.admin)
        },
        bank: bank_loader.key(),
        mint: bank_mint.key(),
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_seed: u64)]
pub struct LendingPoolAddBankPermissionless<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        has_one = marginfi_group @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Mint of the spl-single-pool LST (a PDA derived from `stake_pool`)
    ///
    /// CHECK: passing a mint here that is not actually a staked collateral LST is not possible
    /// because the sol_pool and stake_pool will not derive to a valid PDA which is also owned by
    /// the staking program and spl-single-pool program.
    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Validated using `stake_pool`
    pub sol_pool: AccountInfo<'info>,

    /// CHECK: We validate this is correct backwards, by deriving the PDA of the `bank_mint` using
    /// this key.
    ///
    /// If derives the same `bank_mint`, then this must be the correct stake pool for that mint, and
    /// we can subsequently use it to validate the `sol_pool`
    pub stake_pool: AccountInfo<'info>,

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
