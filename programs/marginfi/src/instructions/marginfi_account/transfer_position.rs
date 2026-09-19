use crate::{
    check, check_eq,
    constants::{
        DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS, DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS,
        PROGRAM_VERSION,
    },
    events::{AccountEventHeader, LendingAccountTransferPositionEvent},
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, calc_value, check_account_init_health,
            deposit_is_halt_safe, is_signer_authorized, run_cb_price_gate, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        premium::{MarginfiAccountPremiumImpl, PremiumScratch},
    },
    utils::{
        fetch_asset_price_for_bank_low_bias, validate_asset_tags, validate_bank_state,
        InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{
        is_marginfi_asset_tag, Bank, FeeState, HealthCache, MarginfiAccount, MarginfiGroup,
        ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_RECEIVERSHIP, ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED,
        ACCOUNT_POSITION_TRANSFER_SEND_DISABLED,
    },
};

/// Moves `transfer_amount` native units of the source's asset position in `bank` to the
/// destination; both authorities sign and the destination authority pays the flat protocol fee.
/// Remaining accounts: the source's observation set, then the destination's
/// (`destination_accounts` long), each `[bank, oracles...]` per active balance in balance order.
pub fn lending_account_transfer_position<'info>(
    ctx: Context<'info, LendingAccountTransferPosition<'info>>,
    transfer_amount: u64,
    destination_accounts: u8,
) -> MarginfiResult {
    check!(
        ctx.accounts.source_marginfi_account.key()
            != ctx.accounts.destination_marginfi_account.key(),
        MarginfiError::PositionTransferIdenticalAccounts
    );
    check!(
        transfer_amount > 0,
        MarginfiError::InvalidPositionTransferAmount
    );

    let clock = Clock::get()?;
    let bank_key = ctx.accounts.bank.key();
    let mut source_account = ctx.accounts.source_marginfi_account.load_mut()?;
    let mut destination_account = ctx.accounts.destination_marginfi_account.load_mut()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    check_eq!(
        ctx.accounts.global_fee_wallet.key(),
        group.fee_state_cache.global_fee_wallet,
        MarginfiError::InvalidGlobalFeeWallet
    );

    for account in [&*source_account, &*destination_account] {
        check!(
            !account.get_flag(ACCOUNT_DISABLED),
            MarginfiError::AccountDisabled
        );
        check!(
            !account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::AccountInFlashloan
        );
        check!(
            !account.get_flag(
                ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_DELEVERAGE
            ),
            MarginfiError::ForbiddenIx
        );
    }
    check!(
        !source_account.get_flag(ACCOUNT_POSITION_TRANSFER_SEND_DISABLED),
        MarginfiError::PositionTransferSendDisabled
    );
    check!(
        !destination_account.get_flag(ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED),
        MarginfiError::PositionTransferDisabled
    );

    validate_asset_tags(&bank, &destination_account)?;
    // Halt-safe only when both legs are: the source sheds collateral like a withdraw, the
    // destination gains it like a deposit.
    validate_bank_state(
        &bank,
        InstructionKind::FailsIfPausedOrReduceState,
        !source_account.lending_account.has_liabilities()
            && deposit_is_halt_safe(&destination_account, &bank_key),
    )?;

    let source_shares: I80F48 = source_account
        .lending_account
        .get_balance(&bank_key)
        .ok_or(MarginfiError::LendingAccountBalanceNotFound)?
        .asset_shares
        .into();
    check!(source_shares > I80F48::ZERO, MarginfiError::NoAssetFound);

    let fee_state = ctx.accounts.fee_state.load()?;
    let min_value_usd_cents = match fee_state.position_transfer_min_value_usd_cents {
        0 => DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS,
        cents => cents,
    };
    let position_transfer_fee = match fee_state.position_transfer_fee {
        0 => DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS,
        fee => fee,
    };

    let price =
        fetch_asset_price_for_bank_low_bias(&bank_key, &bank, &clock, ctx.remaining_accounts)?;
    let transfer_usd_value = calc_value(
        I80F48::from_num(transfer_amount),
        price,
        bank.get_balance_decimals(),
        None,
    )?;
    check!(
        transfer_usd_value >= I80F48::from_num(min_value_usd_cents) / I80F48::from_num(100),
        MarginfiError::InvalidPositionTransferAmount
    );

    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_key,
    )?;
    check!(
        bank.get_asset_amount(source_shares)? >= I80F48::from_num(transfer_amount),
        MarginfiError::PositionTransferInsufficientFunds
    );

    let share_amount =
        BankAccountWrapper::find(&bank_key, &mut bank, &mut source_account.lending_account)?
            .withdraw(I80F48::from_num(transfer_amount))?;
    let asset_amount = bank.get_asset_amount(share_amount)?;
    BankAccountWrapper::find_or_create(
        &bank_key,
        &mut bank,
        &mut destination_account.lending_account,
    )?
    .deposit(asset_amount)?;

    anchor_lang::system_program::transfer(
        ctx.accounts.transfer_fee(),
        u64::from(position_transfer_fee),
    )?;

    for account in [&mut *source_account, &mut *destination_account] {
        account.last_update = clock.unix_timestamp as u64;
        account.lending_account.sort_balances();
        account.sync_indexer_flags();
    }
    bank.update_bank_cache(&group)?;
    let bank_mint = bank.mint;
    drop(bank);

    let split = ctx
        .remaining_accounts
        .len()
        .checked_sub(destination_accounts as usize)
        .ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    let (source_obs, destination_obs) = ctx.remaining_accounts.split_at(split);
    check_health_and_refresh_premium(&mut source_account, &group, source_obs, &clock)?;
    if source_account.lending_account.has_liabilities() {
        run_cb_price_gate(&source_account, source_obs)?;
    }
    check_health_and_refresh_premium(&mut destination_account, &group, destination_obs, &clock)?;

    emit!(LendingAccountTransferPositionEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.source_marginfi_account.key(),
            marginfi_account_authority: source_account.authority,
            marginfi_group: source_account.group,
        },
        source_account: ctx.accounts.source_marginfi_account.key(),
        source_account_authority: source_account.authority,
        destination_account: ctx.accounts.destination_marginfi_account.key(),
        destination_account_authority: destination_account.authority,
        bank: bank_key,
        mint: bank_mint,
        transfer_amount,
        transfer_share_amount: share_amount.into(),
        protocol_fee_lamports: position_transfer_fee,
    });

    Ok(())
}

/// Non-inlined so each account's `PremiumScratch` gets its own frame within the SBF stack budget.
#[inline(never)]
fn check_health_and_refresh_premium<'info>(
    account: &mut MarginfiAccount,
    group: &MarginfiGroup,
    observation_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult {
    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    let mut premium_scratch = PremiumScratch::default();
    check_account_init_health(
        account,
        group,
        observation_ais,
        &mut Some(&mut health_cache),
        &mut Some(&mut premium_scratch),
    )?;
    health_cache.program_version = PROGRAM_VERSION;
    health_cache.set_engine_ok(true);
    account.health_cache = health_cache;
    account.update_premium_snapshots(group, &premium_scratch, clock.unix_timestamp as u64)
}

#[derive(Accounts)]
pub struct LendingAccountTransferPosition<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = source_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = source_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub source_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = destination_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, destination_authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = destination_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, destination_authority.key(), false, false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub destination_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    /// Pays the flat protocol fee.
    #[account(mut)]
    pub destination_authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated against `group.fee_state_cache.global_fee_wallet` in the handler.
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    // Note: there is just one FeeState per program. Read here for the configurable transfer fee.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    pub system_program: Program<'info, System>,
}

impl<'info> LendingAccountTransferPosition<'info> {
    fn transfer_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.key(),
            anchor_lang::system_program::Transfer {
                from: self.destination_authority.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}
