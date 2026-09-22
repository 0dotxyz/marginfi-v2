use crate::{
    check, check_eq,
    constants::{
        DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS, DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS,
        PROGRAM_VERSION,
    },
    events::{AccountEventHeader, LendingAccountTransferPositionEvent},
    math_error,
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
        fetch_asset_price_for_bank_low_bias, fetch_unbiased_price_for_bank_cache,
        validate_asset_tags, validate_bank_state, InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{EMPTY_BALANCE_THRESHOLD, FEE_STATE_SEED, TOKENLESS_REPAYMENTS_ALLOWED},
    types::{
        is_marginfi_asset_tag, BalanceSide, Bank, FeeState, HealthCache, MarginfiAccount,
        MarginfiGroup, RiskTier, ACCOUNT_DISABLED, ACCOUNT_FROZEN, ACCOUNT_IN_DELEVERAGE,
        ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_REBALANCE,
        ACCOUNT_IN_RECEIVERSHIP, ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED,
    },
};

/// Moves `transfer_amount` of the source's position in `bank` to the destination, on the side it
/// holds. Remaining accounts: the source's observation set, then the destination's.
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
                ACCOUNT_IN_RECEIVERSHIP
                    | ACCOUNT_IN_ORDER_EXECUTION
                    | ACCOUNT_IN_DELEVERAGE
                    | ACCOUNT_IN_REBALANCE
            ),
            MarginfiError::ForbiddenIx
        );
    }
    check!(
        !destination_account.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );
    check!(
        !destination_account.get_flag(ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED),
        MarginfiError::PositionTransferReceiveDisabled
    );

    let source_balance = source_account
        .lending_account
        .get_balance(&bank_key)
        .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;
    let (is_liability, source_shares): (bool, I80F48) = match source_balance.get_side() {
        Some(BalanceSide::Liabilities) => (true, source_balance.liability_shares.into()),
        Some(BalanceSide::Assets) => (false, source_balance.asset_shares.into()),
        None => return err!(MarginfiError::NoAssetFound),
    };
    if is_liability {
        let receiver_consents = destination_account.authority == ctx.accounts.authority.key()
            || ctx
                .accounts
                .destination_authority
                .as_ref()
                .is_some_and(|signer| signer.key() == destination_account.authority);
        check!(
            receiver_consents,
            MarginfiError::PositionTransferDebtConsentRequired
        );
    }

    validate_asset_tags(&bank, &destination_account)?;
    let halt_safe = !is_liability
        && !source_account.lending_account.has_liabilities()
        && deposit_is_halt_safe(&destination_account, &bank_key);
    validate_bank_state(
        &bank,
        InstructionKind::FailsIfPausedOrReduceState,
        halt_safe,
    )?;

    let fee_state = ctx.accounts.fee_state.load()?;
    let min_value_usd_cents = match fee_state.position_transfer_min_value_usd_cents {
        0 => DEFAULT_POSITION_TRANSFER_MIN_VALUE_USD_CENTS,
        cents => cents,
    };
    let position_transfer_fee = match fee_state.position_transfer_fee {
        0 => DEFAULT_POSITION_TRANSFER_FEE_LAMPORTS,
        fee => fee,
    };

    let amount = I80F48::from_num(transfer_amount);
    let price =
        fetch_asset_price_for_bank_low_bias(&bank_key, &bank, &clock, ctx.remaining_accounts)?;
    let transfer_usd_value = calc_value(amount, price, bank.get_balance_decimals(), None)?;
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
    // A debt move is capped at the debt itself, so an over-ask moves all of it as interest accrues.
    let amount = if is_liability {
        amount.min(bank.get_liability_amount(source_shares)?)
    } else {
        check!(
            bank.get_asset_amount(source_shares)? >= amount,
            MarginfiError::PositionTransferInsufficientFunds
        );
        amount
    };

    // Repaying the last of a debt writes its premium receivable off, so it is read first and moved
    // with an emptied source.
    let mut carried_premium = I80F48::ZERO;
    let share_amount = {
        let mut source_position =
            BankAccountWrapper::find(&bank_key, &mut bank, &mut source_account.lending_account)?;
        if is_liability {
            source_position.claim_premium()?;
            let receivable: I80F48 = source_position.balance.premium_outstanding.into();
            let shares = source_position.repay(amount)?;
            if source_position.balance.is_empty(BalanceSide::Liabilities) {
                carried_premium = receivable;
            }
            shares
        } else {
            source_position.transfer_out(amount)?
        }
    };
    let moved_amount = if is_liability {
        bank.get_liability_amount(share_amount)?
    } else {
        bank.get_asset_amount(share_amount)?
    };
    {
        let mut destination_position = BankAccountWrapper::find_or_create(
            &bank_key,
            &mut bank,
            &mut destination_account.lending_account,
        )?;
        if is_liability {
            destination_position.debt_transfer_in(moved_amount)?;
            check!(
                I80F48::from(destination_position.balance.liability_shares)
                    >= EMPTY_BALANCE_THRESHOLD,
                MarginfiError::IllegalBalanceState,
                "Transfer would leave positive liability shares below the empty balance threshold"
            );
            destination_position.balance.premium_outstanding =
                I80F48::from(destination_position.balance.premium_outstanding)
                    .checked_add(carried_premium)
                    .ok_or_else(math_error!())?
                    .into();
        } else {
            destination_position.transfer_in(moved_amount)?;
        }
    }
    if is_liability && bank.config.risk_tier == RiskTier::Isolated {
        destination_account.indexer_flags.has_isolated = 1;
    }

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
    // A source shedding debt is not re-checked, as with repay.
    if !is_liability {
        let source_carries_risk = source_account.lending_account.has_liabilities();
        check_health_and_refresh_premium(
            &mut source_account,
            &group,
            source_obs,
            &clock,
            source_carries_risk,
            false,
        )?;
    }
    check_health_and_refresh_premium(
        &mut destination_account,
        &group,
        destination_obs,
        &clock,
        is_liability,
        is_liability,
    )?;

    let maybe_price = fetch_unbiased_price_for_bank_cache(
        &bank_key,
        &*ctx.accounts.bank.load()?,
        &clock,
        ctx.remaining_accounts,
    )
    .ok();
    ctx.accounts
        .bank
        .load_mut()?
        .update_cache_price(maybe_price)?;

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
        is_liability,
        transfer_amount: amount.checked_to_num().ok_or_else(math_error!())?,
        transfer_share_amount: share_amount.into(),
        premium_carried: carried_premium.into(),
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
    carries_risk: bool,
    takes_on_debt: bool,
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
    if takes_on_debt {
        check!(
            !premium_scratch.refresh_unavailable(),
            MarginfiError::PremiumSnapshotUnavailable
        );
    }
    if carries_risk {
        run_cb_price_gate(account, observation_ais)?;
    }
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

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub destination_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    /// Signs only to consent to receiving debt.
    pub destination_authority: Option<Signer<'info>>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
        constraint = !bank.load()?.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
            @ MarginfiError::BankReduceOnly,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated against `group.fee_state_cache.global_fee_wallet` in the handler.
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

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
                from: self.fee_payer.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}
