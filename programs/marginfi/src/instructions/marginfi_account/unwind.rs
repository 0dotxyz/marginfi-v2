use anchor_lang::prelude::*;

mod quote;
mod settle;
#[cfg(test)]
mod tests;

use quote::*;
pub use settle::settle_unwind_ticket;
#[cfg(test)]
use settle::{book_unwind_settlement, require_stake_deactivation_elapsed};

use anchor_lang::solana_program::{
    program::{invoke, invoke_signed},
    system_instruction,
};
use anchor_spl::token::{self, Token, TokenAccount};
use borsh::BorshDeserialize;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use solana_stake_interface::{
    instruction as stake_ix, program::ID as STAKE_PROGRAM_ID, state::StakeStateV2,
    sysvar::stake_history::ID as STAKE_HISTORY_SYSVAR_ID,
};
use spl_stake_pool::instruction as pool_ix;
use spl_stake_pool::state::StakePool;

use crate::state::marginfi_account::run_cb_price_gate;
use crate::{
    check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, UnwindLstEvent},
    state::{
        bank::BankImpl,
        marginfi_account::{
            check_account_init_health_and_clear_tag, BankAccountWrapper, LendingAccountImpl,
            MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        premium::{MarginfiAccountPremiumImpl, PremiumScratch},
        rate_limiter::GroupRateLimiterImpl,
    },
    utils::{
        general::fetch_asset_price_for_bank_low_bias, record_withdrawal_outflow,
        validate_bank_state, InstructionKind,
    },
    MarginfiError,
};
use marginfi_type_crate::constants::{
    LIQUIDITY_VAULT_AUTHORITY_SEED, UNWIND_CONFIG_SEED, UNWIND_TICKET_SEED,
};
use marginfi_type_crate::types::{
    is_marginfi_asset_tag, Bank, HealthCache, MarginfiAccount, MarginfiGroup, OracleSetup,
    UnwindConfig, UnwindTicket, ACCOUNT_DISABLED, ACCOUNT_FROZEN, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_REBALANCE,
    ACCOUNT_IN_RECEIVERSHIP,
};

const MIN_UNWIND_FEE_MULTIPLIER_BPS: u32 = 10_000;
const MAX_UNWIND_FEE_MULTIPLIER_BPS: u32 = 30_000;
const NATIVE_MINT_ID: Pubkey = token::spl_token::native_mint::ID;

fn validate_unwind_config_params(fee_multiplier_bps: u32) -> Result<()> {
    require!(
        (MIN_UNWIND_FEE_MULTIPLIER_BPS..=MAX_UNWIND_FEE_MULTIPLIER_BPS)
            .contains(&fee_multiplier_bps),
        MarginfiError::InvalidConfig
    );
    Ok(())
}

fn load_and_validate_stake_pool(accounts: &UnwindLst<'_>, clock: &Clock) -> Result<StakePool> {
    let pool: StakePool = {
        let pool_data = accounts.stake_pool.try_borrow_data()?;
        BorshDeserialize::deserialize(&mut &pool_data[..])
            .map_err(|_| MarginfiError::StakePoolValidationFailed)?
    };

    require!(pool.is_valid(), MarginfiError::StakePoolValidationFailed);
    require_keys_eq!(
        pool.pool_mint,
        accounts.pool_mint.key(),
        MarginfiError::StakePoolValidationFailed
    );
    require_keys_eq!(
        pool.manager_fee_account,
        accounts.manager_fee_account.key(),
        MarginfiError::StakePoolValidationFailed
    );
    require_keys_eq!(
        pool.validator_list,
        accounts.validator_list.key(),
        MarginfiError::StakePoolValidationFailed
    );
    require_keys_eq!(
        pool.token_program_id,
        token::ID,
        MarginfiError::StakePoolValidationFailed
    );
    require_keys_neq!(
        accounts.pool_source_stake_account.key(),
        pool.reserve_stake,
        MarginfiError::StakePoolValidationFailed
    );
    require!(
        pool.last_update_epoch == clock.epoch,
        MarginfiError::StakePoolStale
    );
    Ok(pool)
}

#[derive(Accounts)]
pub struct SettleUnwindTicket<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(mut, has_one = group)]
    pub sol_bank: AccountLoader<'info, Bank>,
    #[account(
        mut,
        close = rent_recipient,
        seeds = [UNWIND_TICKET_SEED, stake_account.key().as_ref()],
        bump = unwind_ticket.bump,
        has_one = sol_bank @ MarginfiError::InvalidBankAccount,
        has_one = rent_recipient @ MarginfiError::Unauthorized
    )]
    pub unwind_ticket: Box<Account<'info, UnwindTicket>>,
    #[account(mut, owner = STAKE_PROGRAM_ID @ MarginfiError::StakePoolValidationFailed)]
    /// CHECK: Owner is constrained to the Stake Program.
    pub stake_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Writable account receives the stake rent prefund.
    pub rent_recipient: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = sol_liquidity_vault.key() == sol_bank.load()?.liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = sol_liquidity_vault.mint == NATIVE_MINT_ID @ MarginfiError::InvalidMint
    )]
    pub sol_liquidity_vault: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(), sol_bank.key().as_ref()], bump)]
    /// CHECK: Validated against the SOL bank's vault authority PDA.
    pub sol_liquidity_vault_authority: UncheckedAccount<'info>,
    #[account(address = STAKE_PROGRAM_ID)]
    /// CHECK: Address is constrained to the Stake Program.
    pub stake_program: UncheckedAccount<'info>,
    pub clock: Sysvar<'info, Clock>,
    #[account(address = STAKE_HISTORY_SYSVAR_ID)]
    /// CHECK: Address is constrained to the Stake History sysvar.
    pub stake_history: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitUnwindConfig<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, has_one = group)]
    pub sol_bank: AccountLoader<'info, Bank>,
    #[account(
        init,
        payer = admin,
        space = 8 + UnwindConfig::INIT_SPACE,
        seeds = [UNWIND_CONFIG_SEED, sol_bank.key().as_ref()],
        bump
    )]
    pub unwind_config: Account<'info, UnwindConfig>,
    pub system_program: Program<'info, System>,
}

pub fn init_unwind_config(
    ctx: Context<InitUnwindConfig>,
    fee_multiplier_bps: u32,
    max_pending_unwind_bps: u32,
    min_unwind_lamports: u64,
) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.group.load()?.admin,
        ctx.accounts.admin.key(),
        MarginfiError::Unauthorized
    );

    validate_unwind_config_params(fee_multiplier_bps)?;
    {
        let mut bank = ctx.accounts.sol_bank.load_mut()?;
        require_keys_eq!(bank.mint, NATIVE_MINT_ID, MarginfiError::InvalidMint);
        bank.set_max_pending_unwind_bps(max_pending_unwind_bps)?;
    }

    let config = &mut ctx.accounts.unwind_config;
    config.fee_multiplier_bps = fee_multiplier_bps;
    config.min_unwind_lamports = min_unwind_lamports;
    config.bump = ctx.bumps.unwind_config;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateUnwindConfig<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(mut, has_one = group)]
    pub sol_bank: AccountLoader<'info, Bank>,
    #[account(
        mut,
        seeds = [UNWIND_CONFIG_SEED, sol_bank.key().as_ref()],
        bump = unwind_config.bump
    )]
    pub unwind_config: Account<'info, UnwindConfig>,
}

pub fn update_unwind_config(
    ctx: Context<UpdateUnwindConfig>,
    fee_multiplier_bps: u32,
    max_pending_unwind_bps: u32,
    min_unwind_lamports: u64,
) -> Result<()> {
    require_keys_eq!(
        ctx.accounts.group.load()?.admin,
        ctx.accounts.admin.key(),
        MarginfiError::Unauthorized
    );

    validate_unwind_config_params(fee_multiplier_bps)?;
    let mut sol_bank = ctx.accounts.sol_bank.load_mut()?;
    require_keys_eq!(sol_bank.mint, NATIVE_MINT_ID, MarginfiError::InvalidMint);

    sol_bank.set_max_pending_unwind_bps(max_pending_unwind_bps)?;

    let config = &mut ctx.accounts.unwind_config;
    config.fee_multiplier_bps = fee_multiplier_bps;
    config.min_unwind_lamports = min_unwind_lamports;
    Ok(())
}

#[derive(Accounts)]
pub struct UnwindLst<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut, has_one = group, has_one = authority @ MarginfiError::Unauthorized)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    pub authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut, has_one = group, constraint = lst_bank.key() != sol_bank.key() @ MarginfiError::IllegalAction)]
    pub lst_bank: AccountLoader<'info, Bank>,
    #[account(mut, has_one = group, constraint = sol_bank.load()?.mint == NATIVE_MINT_ID @ MarginfiError::InvalidMint)]
    pub sol_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        constraint = lst_liquidity_vault.key() == lst_bank.load()?.liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = lst_liquidity_vault.mint == lst_bank.load()?.mint @ MarginfiError::InvalidMint
    )]
    pub lst_liquidity_vault: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(), lst_bank.key().as_ref()], bump)]
    /// CHECK: Validated against the bank's vault authority PDA.
    pub lst_liquidity_vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = sol_liquidity_vault.key() == sol_bank.load()?.liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = sol_liquidity_vault.mint == NATIVE_MINT_ID @ MarginfiError::InvalidMint
    )]
    pub sol_liquidity_vault: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(), sol_bank.key().as_ref()], bump)]
    /// CHECK: Validated against the bank's vault authority PDA.
    pub sol_liquidity_vault_authority: UncheckedAccount<'info>,

    #[account(
        seeds = [UNWIND_CONFIG_SEED, sol_bank.key().as_ref()],
        bump = unwind_config.bump
    )]
    pub unwind_config: Box<Account<'info, UnwindConfig>>,

    #[account(
        init,
        payer = payer,
        space = 8 + UnwindTicket::INIT_SPACE,
        seeds = [UNWIND_TICKET_SEED, unwind_stake_account.key().as_ref()],
        bump
    )]
    pub unwind_ticket: Box<Account<'info, UnwindTicket>>,

    #[account(mut, signer)]
    /// CHECK: Fresh signer; the system CPI creates it as stake-program-owned.
    pub unwind_stake_account: UncheckedAccount<'info>,

    #[account(mut, owner = spl_stake_pool::id() @ MarginfiError::StakePoolValidationFailed)]
    /// CHECK: Owner is constrained to the SPL Stake Pool program.
    pub stake_pool: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Validated against the stake pool's validator-list key.
    pub validator_list: UncheckedAccount<'info>,
    /// CHECK: Validated by the SPL Stake Pool CPI.
    pub pool_withdraw_authority: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Validated by the SPL Stake Pool CPI.
    pub pool_source_stake_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Validated against the pool's manager fee account.
    pub manager_fee_account: UncheckedAccount<'info>,
    #[account(mut, constraint = lst_bank.load()?.mint == pool_mint.key() @ MarginfiError::InvalidMint)]
    /// CHECK: Constrained to the LST bank mint.
    pub pool_mint: UncheckedAccount<'info>,
    #[account(address = spl_stake_pool::id())]
    /// CHECK: Address is constrained to the SPL Stake Pool program.
    pub stake_pool_program: UncheckedAccount<'info>,
    #[account(address = STAKE_PROGRAM_ID)]
    /// CHECK: Address is constrained to the Stake Program.
    pub stake_program: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn unwind_lst<'info>(ctx: Context<'info, UnwindLst<'info>>, max_lst_in: u64) -> Result<()> {
    let clock = Clock::get()?;
    let ticket_bump = ctx.bumps.unwind_ticket;
    let remaining = ctx.remaining_accounts;
    let accounts: &UnwindLst<'info> = ctx.accounts;
    let sol_bank_key = accounts.sol_bank.key();
    #[cfg(not(feature = "client"))]
    let lst_bank_key = accounts.lst_bank.key();
    let unwind_config = &accounts.unwind_config;

    let group = accounts.group.load()?;
    let mut marginfi_account = accounts.marginfi_account.load_mut()?;

    marginfi_account.lending_account.sort_balances();

    let (plan, lst_vault_authority_bump, sol_vault_authority_bump) = {
        let mut lst_bank = accounts.lst_bank.load_mut()?;
        let mut sol_bank = accounts.sol_bank.load_mut()?;

        lst_bank.accrue_interest(
            clock.unix_timestamp,
            &group,
            #[cfg(not(feature = "client"))]
            lst_bank_key,
        )?;
        sol_bank.accrue_interest(
            clock.unix_timestamp,
            &group,
            #[cfg(not(feature = "client"))]
            sol_bank_key,
        )?;

        validate_unwind_preconditions(&group, &marginfi_account, &lst_bank, &sol_bank)?;
        let epoch_seconds_remaining = seconds_until_epoch_end(&clock)?;
        let quote = quote_unwind_debt_and_fee(
            &sol_bank,
            &sol_bank_key,
            &group,
            unwind_config,
            &marginfi_account,
            epoch_seconds_remaining,
        )?;
        require!(quote.debt_lamports > 0, MarginfiError::NoLiabilityFound);
        require!(
            quote.debt_lamports >= unwind_config.min_unwind_lamports,
            MarginfiError::BelowMinimum
        );
        let stake_pool = load_and_validate_stake_pool(accounts, &clock)?;
        let redemption = quote_pool_redemption(
            &stake_pool,
            &accounts.stake_pool.key(),
            &lst_bank,
            &accounts.manager_fee_account.to_account_info(),
            &accounts.lst_liquidity_vault.key(),
            &quote,
            max_lst_in,
        )?;
        let plan = UnwindPlan {
            quote,
            redemption,
            epoch_seconds_remaining,
        };

        apply_unwind_account_and_bank_updates(
            accounts,
            remaining,
            &clock,
            &group,
            &mut marginfi_account,
            &mut lst_bank,
            &mut sol_bank,
            &plan,
        )?;

        (
            plan,
            lst_bank.liquidity_vault_authority_bump,
            sol_bank.liquidity_vault_authority_bump,
        )
    };

    drop(marginfi_account);
    drop(group);

    {
        let mut marginfi_account_for_health = accounts.marginfi_account.load_mut()?;
        let group_for_health = accounts.group.load()?;
        check_health_and_refresh_premium(
            &mut marginfi_account_for_health,
            &group_for_health,
            remaining,
            &clock,
        )?;
    }

    let stake_rent_prefund_lamports = redeem_lst_into_deactivating_stake(
        accounts,
        &plan,
        lst_vault_authority_bump,
        sol_vault_authority_bump,
    )?;
    create_unwind_ticket_and_emit(
        ctx.accounts,
        ticket_bump,
        &plan,
        stake_rent_prefund_lamports,
        &clock,
    )
}

fn validate_unwind_preconditions(
    group: &MarginfiGroup,
    marginfi_account: &MarginfiAccount,
    lst_bank: &Bank,
    sol_bank: &Bank,
) -> Result<()> {
    check!(
        matches!(lst_bank.config.oracle_setup, OracleSetup::PythLST),
        MarginfiError::IllegalAction
    );
    check!(
        is_marginfi_asset_tag(lst_bank.config.asset_tag),
        MarginfiError::WrongAssetTagForStandardInstructions
    );
    check!(
        is_marginfi_asset_tag(sol_bank.config.asset_tag),
        MarginfiError::WrongAssetTagForStandardInstructions
    );

    validate_bank_state(lst_bank, InstructionKind::FailsInPausedState, false)?;
    validate_bank_state(sol_bank, InstructionKind::FailsInPausedState, false)?;
    check!(!group.is_protocol_paused(), MarginfiError::ProtocolPaused);

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    check!(
        !marginfi_account.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );
    check!(
        !marginfi_account.get_flag(
            ACCOUNT_IN_RECEIVERSHIP
                | ACCOUNT_IN_ORDER_EXECUTION
                | ACCOUNT_IN_REBALANCE
                | ACCOUNT_IN_FLASHLOAN
                | ACCOUNT_IN_DELEVERAGE
        ),
        MarginfiError::IllegalAction
    );
    Ok(())
}

fn move_sol_debt_to_pending_unwind(
    sol_bank_key: &Pubkey,
    sol_bank: &mut Bank,
    marginfi_account: &mut MarginfiAccount,
    debt_lamports: u64,
) -> Result<()> {
    let mut sol_bank_account = BankAccountWrapper::find(
        sol_bank_key,
        sol_bank,
        &mut marginfi_account.lending_account,
    )?;

    let (repaid_lamports, _) = sol_bank_account.repay_all_without_premium(false)?;
    require!(
        repaid_lamports <= debt_lamports && debt_lamports - repaid_lamports <= 1,
        MarginfiError::MathError
    );

    sol_bank.add_pending_unwind(debt_lamports)
}

#[allow(clippy::too_many_arguments)]
fn apply_unwind_account_and_bank_updates<'info, 'r>(
    accounts: &UnwindLst<'info>,
    remaining_accounts: &'r [AccountInfo<'r>],
    clock: &Clock,
    group: &MarginfiGroup,
    marginfi_account: &mut MarginfiAccount,
    lst_bank: &mut Bank,
    sol_bank: &mut Bank,
    plan: &UnwindPlan,
) -> Result<()> {
    let (lst_bank_key, sol_bank_key) = (accounts.lst_bank.key(), accounts.sol_bank.key());
    let total_lst_debit = plan.redemption.total_lst_required;
    let unwind_fee_lst = plan.redemption.unwind_fee_lst;

    move_sol_debt_to_pending_unwind(
        &sol_bank_key,
        sol_bank,
        marginfi_account,
        plan.quote.debt_lamports,
    )?;

    let rate_limiter_enabled = group.rate_limiter.is_enabled();
    let lst_price_low_bias = if rate_limiter_enabled {
        fetch_asset_price_for_bank_low_bias(
            &lst_bank_key,
            &*lst_bank,
            clock,
            remaining_accounts,
            false,
        )?
    } else {
        I80F48::ZERO
    };

    BankAccountWrapper::find(
        &lst_bank_key,
        lst_bank,
        &mut marginfi_account.lending_account,
    )?
    .withdraw(I80F48::from_num(total_lst_debit))?;

    if unwind_fee_lst > 0 {
        lst_bank.collected_program_fees_outstanding =
            I80F48::from(lst_bank.collected_program_fees_outstanding)
                .checked_add(I80F48::from_num(unwind_fee_lst))
                .ok_or(MarginfiError::MathError)?
                .into();
    }

    record_withdrawal_outflow(
        rate_limiter_enabled,
        total_lst_debit,
        total_lst_debit,
        lst_price_low_bias,
        lst_bank,
        group,
        accounts.group.key(),
        lst_bank_key,
        &*marginfi_account,
        clock,
    )?;

    sol_bank.update_bank_cache(group)?;
    lst_bank.update_bank_cache(group)?;
    Ok(())
}

fn check_health_and_refresh_premium<'info>(
    marginfi_account: &mut MarginfiAccount,
    group: &MarginfiGroup,
    remaining: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> Result<()> {
    marginfi_account.lending_account.sort_balances();
    marginfi_account.sync_indexer_flags();
    marginfi_account.last_update = clock.unix_timestamp as u64;

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    let mut premium_scratch = PremiumScratch::default();
    check_account_init_health_and_clear_tag(
        marginfi_account,
        group,
        remaining,
        &mut Some(&mut health_cache),
        &mut Some(&mut premium_scratch),
    )?;
    health_cache.program_version = PROGRAM_VERSION;
    health_cache.set_engine_ok(true);
    marginfi_account.health_cache = health_cache;

    if marginfi_account.lending_account.has_liabilities() {
        run_cb_price_gate(marginfi_account, remaining)?;
    }
    marginfi_account.update_premium_snapshots(
        group,
        &premium_scratch,
        clock.unix_timestamp as u64,
        true,
    )?;
    Ok(())
}

fn redeem_lst_into_deactivating_stake<'info>(
    accounts: &UnwindLst<'info>,
    plan: &UnwindPlan,
    lst_vault_authority_bump: u8,
    sol_vault_authority_bump: u8,
) -> Result<u64> {
    let (lst_bank_key, sol_bank_key) = (accounts.lst_bank.key(), accounts.sol_bank.key());
    let stake_rent_prefund_lamports = Rent::get()?.minimum_balance(StakeStateV2::size_of());

    invoke(
        &system_instruction::create_account(
            &accounts.payer.key(),
            &accounts.unwind_stake_account.key(),
            stake_rent_prefund_lamports,
            StakeStateV2::size_of() as u64,
            &STAKE_PROGRAM_ID,
        ),
        &[
            accounts.payer.to_account_info(),
            accounts.unwind_stake_account.to_account_info(),
            accounts.system_program.to_account_info(),
        ],
    )?;

    let lst_vault_authority_seeds: &[&[u8]] = &[
        LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
        lst_bank_key.as_ref(),
        &[lst_vault_authority_bump],
    ];
    let sol_vault_authority_seeds: &[&[u8]] = &[
        LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
        sol_bank_key.as_ref(),
        &[sol_vault_authority_bump],
    ];

    let withdraw_ix = pool_ix::withdraw_stake_with_slippage(
        &accounts.stake_pool_program.key(),
        &accounts.stake_pool.key(),
        &accounts.validator_list.key(),
        &accounts.pool_withdraw_authority.key(),
        &accounts.pool_source_stake_account.key(),
        &accounts.unwind_stake_account.key(),
        &accounts.sol_liquidity_vault_authority.key(),
        &accounts.lst_liquidity_vault_authority.key(),
        &accounts.lst_liquidity_vault.key(),
        &accounts.manager_fee_account.key(),
        &accounts.pool_mint.key(),
        &token::ID,
        plan.redemption.pool_tokens_in,
        plan.quote.debt_lamports,
    );
    invoke_signed(
        &withdraw_ix,
        &[
            accounts.stake_pool.to_account_info(),
            accounts.validator_list.to_account_info(),
            accounts.pool_withdraw_authority.to_account_info(),
            accounts.pool_source_stake_account.to_account_info(),
            accounts.unwind_stake_account.to_account_info(),
            accounts.sol_liquidity_vault_authority.to_account_info(),
            accounts.lst_liquidity_vault_authority.to_account_info(),
            accounts.lst_liquidity_vault.to_account_info(),
            accounts.manager_fee_account.to_account_info(),
            accounts.pool_mint.to_account_info(),
            accounts.clock.to_account_info(),
            accounts.token_program.to_account_info(),
            accounts.stake_program.to_account_info(),
            accounts.stake_pool_program.to_account_info(),
        ],
        &[lst_vault_authority_seeds],
    )?;

    invoke_signed(
        &stake_ix::deactivate_stake(
            &accounts.unwind_stake_account.key(),
            &accounts.sol_liquidity_vault_authority.key(),
        ),
        &[
            accounts.unwind_stake_account.to_account_info(),
            accounts.clock.to_account_info(),
            accounts.sol_liquidity_vault_authority.to_account_info(),
            accounts.stake_program.to_account_info(),
        ],
        &[sol_vault_authority_seeds],
    )?;

    Ok(stake_rent_prefund_lamports)
}

fn create_unwind_ticket_and_emit(
    accounts: &mut UnwindLst<'_>,
    ticket_bump: u8,
    plan: &UnwindPlan,
    stake_rent_prefund_lamports: u64,
    clock: &Clock,
) -> Result<()> {
    let redemption = &plan.redemption;
    let event = UnwindLstEvent {
        header: AccountEventHeader {
            signer: Some(accounts.authority.key()),
            marginfi_account: accounts.marginfi_account.key(),
            marginfi_account_authority: accounts.authority.key(),
            marginfi_group: accounts.group.key(),
        },
        lst_bank: accounts.lst_bank.key(),
        sol_bank: accounts.sol_bank.key(),
        stake_account: accounts.unwind_stake_account.key(),
        debt_repaid_lamports: plan.quote.debt_lamports,
        unwind_fee_lamports: plan.quote.fee_lamports,
        unwind_fee_lst: redemption.unwind_fee_lst,
        lst_debited_total: redemption.total_lst_required,
        lst_burned: redemption
            .pool_tokens_in
            .checked_sub(redemption.pool_withdrawal_fee_lst)
            .ok_or(MarginfiError::MathError)?,
        pool_withdrawal_fee_lst: redemption.pool_withdrawal_fee_lst,
        epoch_seconds_remaining: plan.epoch_seconds_remaining,
    };

    let unwind_ticket = &mut accounts.unwind_ticket;
    unwind_ticket.sol_bank = event.sol_bank;
    unwind_ticket.stake_account = event.stake_account;
    unwind_ticket.rent_recipient = accounts.payer.key();
    unwind_ticket.principal_lamports = plan.quote.debt_lamports;
    unwind_ticket.stake_rent_prefund_lamports = stake_rent_prefund_lamports;
    unwind_ticket.created_epoch = clock.epoch;
    unwind_ticket.bump = ticket_bump;

    emit!(event);
    Ok(())
}
