//! Persistent same-mint auto-rebalance orders. A keeper moves one asset from its current bank into
//! a higher-yield bank of the SAME mint within an allowlisted venue set, via a start/end sandwich
//! that reuses the existing per-venue withdraw/deposit instructions (a `start_rebalance`..
//! `end_rebalance` sandwich). The order is NOT consumed on execution — it persists until cancelled.
//!
//! On-chain guarantees: same mint, dst in the allowed set, dst supply APR > src + min_improvement
//! (pre-move) and dst >= src (post-move), the source moved by the order's `amount` (the full
//! position when the order is unlimited), value conserved up to exactly the flat fee, untouched
//! balances unchanged, account stays healthy if it borrows, and a per-order cooldown.
//!
//! Supports native, Kamino, Drift, and JupLend legs. JupLend reads its `TokenReserve` via the
//! optional `*_token_reserve` accounts (validated against the bank's Lending state).

use crate::{
    check, check_eq,
    constants::PROGRAM_VERSION,
    events::{
        AccountEventHeader, KeeperCloseRebalanceOrderEvent,
        MarginfiAccountCloseRebalanceOrderEvent, MarginfiAccountPlaceRebalanceOrderEvent,
        MarginfiAccountUpdateRebalanceOrderEvent,
    },
    ix_utils::{
        get_discrim_hash, validate_not_cpi_by_stack_height, validate_rebalance_instructions,
        Hashable,
    },
    math_error,
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            calc_value, check_account_maint_health, get_remaining_accounts_per_bank,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        price::{OraclePriceFeedAdapter, PriceAdapter},
        rate::rate_of,
        rebalance::{RebalanceOrderImpl, RebalanceRecordImpl},
    },
};
use anchor_lang::{prelude::*, system_program};
use anchor_spl::token_interface::Mint;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        REBALANCE_DEFAULT_COOLDOWN_SECONDS, REBALANCE_DEFAULT_MIN_IMPROVEMENT,
        REBALANCE_FLAT_FEE_USD, REBALANCE_ORDER_SEED, REBALANCE_RECORD_SEED,
    },
    types::{
        BalanceSide, Bank, HealthCache, MarginfiAccount, MarginfiGroup, OnRampTransition,
        OraclePriceType, RebalanceOrder, RebalanceRecord, WrappedI80F48,
        ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_REBALANCE, ORDER_BLOCKING_FLAGS,
    },
};

/// Equity (weight-1) USD value of a raw native token amount in `bank`, priced from its oracle.
fn value_of_native<'info>(
    amount_native: I80F48,
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
    on_ramp_transition: OnRampTransition,
) -> MarginfiResult<I80F48> {
    let adapter =
        OraclePriceFeedAdapter::try_from_bank(bank, oracle_ais, clock, on_ramp_transition)?;
    let price = adapter.get_price_of_type(
        OraclePriceType::RealTime,
        None,
        bank.config.oracle_max_confidence,
    )?;
    calc_value(amount_native, price, bank.get_balance_decimals(), None)
}

/// Equity (weight-1) USD value of the user's asset position in `bank`. Returns 0 if the user holds
/// no balance there (e.g. the source balance after a full move).
fn bank_asset_value<'info>(
    account: &MarginfiAccount,
    bank_key: &Pubkey,
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
    on_ramp_transition: OnRampTransition,
) -> MarginfiResult<I80F48> {
    let balance = match account.lending_account.get_balance(bank_key) {
        Some(b) => b,
        None => return Ok(I80F48::ZERO),
    };
    let amount = bank.get_asset_amount(balance.asset_shares.into())?;
    value_of_native(amount, bank, oracle_ais, clock, on_ramp_transition)
}

pub fn place_rebalance_order(
    ctx: Context<PlaceRebalanceOrder>,
    allowed_banks: Vec<Pubkey>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
) -> MarginfiResult {
    // User-owned policy with sensible defaults: 5% min improvement, 24h cooldown, full-position move.
    let min_improvement =
        min_improvement.unwrap_or_else(|| WrappedI80F48::from(REBALANCE_DEFAULT_MIN_IMPROVEMENT));
    let cooldown_seconds = cooldown_seconds.unwrap_or(REBALANCE_DEFAULT_COOLDOWN_SECONDS);
    let amount = amount.unwrap_or(0);

    let mut account = ctx.accounts.marginfi_account.load_mut()?;
    {
        let mut order = ctx.accounts.rebalance_order.load_init()?;
        order.initialize(
            ctx.accounts.marginfi_account.key(),
            ctx.accounts.authority.key(),
            ctx.accounts.mint.key(),
            &allowed_banks,
            min_improvement,
            cooldown_seconds,
            amount,
            ctx.bumps.rebalance_order,
        )?;
    }
    account.increment_active_orders()?;

    emit!(MarginfiAccountPlaceRebalanceOrderEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        rebalance_order: ctx.accounts.rebalance_order.key(),
        mint: ctx.accounts.mint.key(),
        allowed_banks,
        min_improvement,
        cooldown_seconds,
        amount,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct PlaceRebalanceOrder<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = authority @ MarginfiError::Unauthorized,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
        ) @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    pub authority: Signer<'info>,
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        init,
        payer = fee_payer,
        space = 8 + RebalanceOrder::LEN,
        seeds = [
            REBALANCE_ORDER_SEED.as_bytes(),
            marginfi_account.key().as_ref(),
            mint.key().as_ref(),
        ],
        bump,
    )]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn close_rebalance_order(ctx: Context<CloseRebalanceOrder>) -> MarginfiResult {
    let mut account = ctx.accounts.marginfi_account.load_mut()?;
    check!(
        !account.get_flag(ACCOUNT_IN_REBALANCE),
        MarginfiError::IllegalAction
    );
    account.decrement_active_orders()?;

    emit!(MarginfiAccountCloseRebalanceOrderEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        rebalance_order: ctx.accounts.rebalance_order.key(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CloseRebalanceOrder<'info> {
    #[account(
        mut,
        has_one = authority @ MarginfiError::Unauthorized,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    pub authority: Signer<'info>,
    #[account(
        mut,
        close = authority,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = authority @ MarginfiError::Unauthorized,
    )]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
}

/// Modify an existing order's policy in place: venue allowlist, min improvement, and/or cooldown.
/// `None` fields are left unchanged.
pub fn update_rebalance_order(
    ctx: Context<UpdateRebalanceOrder>,
    allowed_banks: Option<Vec<Pubkey>>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
) -> MarginfiResult {
    let account = ctx.accounts.marginfi_account.load()?;
    check!(
        !account.get_flag(ACCOUNT_IN_REBALANCE),
        MarginfiError::IllegalAction
    );

    let (allowed, min_imp, cooldown, amount) = {
        let mut order = ctx.accounts.rebalance_order.load_mut()?;
        if let Some(banks) = allowed_banks {
            order.set_allowed_banks(&banks)?;
        }
        if let Some(mi) = min_improvement {
            check!(
                I80F48::from(mi) >= I80F48::ZERO,
                MarginfiError::RebalanceInvalidMinImprovement
            );
            order.min_improvement = mi;
        }
        if let Some(cs) = cooldown_seconds {
            order.cooldown_seconds = cs;
        }
        if let Some(a) = amount {
            order.amount = a;
        }
        (
            order.allowed_banks[..order.allowed_bank_count as usize].to_vec(),
            order.min_improvement,
            order.cooldown_seconds,
            order.amount,
        )
    };

    emit!(MarginfiAccountUpdateRebalanceOrderEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        rebalance_order: ctx.accounts.rebalance_order.key(),
        allowed_banks: allowed,
        min_improvement: min_imp,
        cooldown_seconds: cooldown,
        amount,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateRebalanceOrder<'info> {
    #[account(has_one = authority @ MarginfiError::Unauthorized)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = authority @ MarginfiError::Unauthorized,
    )]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
}

/// Permissionless: a keeper reclaims the rent of a stale rebalance order once it can no longer act —
/// the account was closed, or it holds no position in any allowed venue (nothing left to rebalance).
pub fn keeper_close_rebalance_order(ctx: Context<KeeperCloseRebalanceOrder>) -> MarginfiResult {
    let order = ctx.accounts.rebalance_order.load()?;
    let marginfi_account_info = ctx.accounts.marginfi_account.to_account_info();

    // Manual owner check: only deserialize when the account is not already closed.
    let (authority_pk, group_pk, can_close) = if marginfi_account_info.owner.eq(&system_program::ID)
        && marginfi_account_info.data_is_empty()
    {
        (Pubkey::default(), Pubkey::default(), true)
    } else {
        require_keys_eq!(
            *marginfi_account_info.owner,
            crate::ID,
            MarginfiError::InternalLogicError
        );
        let mut data = marginfi_account_info.try_borrow_mut_data()?;
        require!(
            data.len() >= 8 + std::mem::size_of::<MarginfiAccount>(),
            MarginfiError::InternalLogicError
        );
        let disc = &data[..8];
        check_eq!(
            disc,
            MarginfiAccount::DISCRIMINATOR,
            MarginfiError::InternalLogicError
        );
        let marginfi_account: &mut MarginfiAccount =
            bytemuck::from_bytes_mut(&mut data[8..8 + std::mem::size_of::<MarginfiAccount>()]);

        let allowed = &order.allowed_banks[..order.allowed_bank_count as usize];
        let can_close = !marginfi_account
            .lending_account
            .balances
            .iter()
            .any(|b| b.is_active() && allowed.contains(&b.bank_pk));
        if can_close {
            marginfi_account.decrement_active_orders()?;
        }
        (
            marginfi_account.authority,
            marginfi_account.group,
            can_close,
        )
    };

    check!(can_close, MarginfiError::LiquidatorOrderCloseNotAllowed);

    emit!(KeeperCloseRebalanceOrderEvent {
        header: AccountEventHeader {
            signer: None,
            marginfi_account: marginfi_account_info.key(),
            marginfi_account_authority: authority_pk,
            marginfi_group: group_pk,
        },
        rebalance_order: ctx.accounts.rebalance_order.key(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct KeeperCloseRebalanceOrder<'info> {
    /// CHECK: unchecked so the ix works even when the marginfi account was closed; ownership and
    /// type are validated in the handler.
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,
    /// CHECK: no checks; the keeper keeps the rent.
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,
    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        close = fee_recipient
    )]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
}

pub fn start_rebalance<'info>(ctx: Context<'info, StartRebalance<'info>>) -> MarginfiResult {
    let clock = Clock::get()?;
    let src_key = ctx.accounts.src_bank.key();
    let dst_key = ctx.accounts.dst_bank.key();
    let remaining = ctx.remaining_accounts;

    {
        let order = ctx.accounts.rebalance_order.load()?;
        let allowed = &order.allowed_banks[..order.allowed_bank_count as usize];
        check!(
            allowed.contains(&src_key) && allowed.contains(&dst_key),
            MarginfiError::RebalanceBankNotAllowed
        );
        check!(
            src_key != dst_key,
            MarginfiError::SameAssetAndLiabilityBanks
        );
        check!(
            clock.unix_timestamp as u64
                >= order
                    .last_exec_timestamp
                    .checked_add(order.cooldown_seconds)
                    .ok_or_else(math_error!())?,
            MarginfiError::RebalanceCooldown
        );
        let src_bank = ctx.accounts.src_bank.load()?;
        let dst_bank = ctx.accounts.dst_bank.load()?;
        check!(
            src_bank.mint == order.mint,
            MarginfiError::RebalanceMintMismatch
        );
        check!(
            dst_bank.mint == order.mint,
            MarginfiError::RebalanceMintMismatch
        );
    }

    let (src_oracle, dst_oracle) = {
        let src_bank = ctx.accounts.src_bank.load()?;
        let dst_bank = ctx.accounts.dst_bank.load()?;
        let src_n = get_remaining_accounts_per_bank(&src_bank)?.saturating_sub(1);
        let dst_n = get_remaining_accounts_per_bank(&dst_bank)?.saturating_sub(1);
        require_gte!(
            remaining.len(),
            src_n + dst_n,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        (&remaining[0..src_n], &remaining[src_n..src_n + dst_n])
    };

    {
        let order = ctx.accounts.rebalance_order.load()?;
        let src_bank = ctx.accounts.src_bank.load()?;
        let dst_bank = ctx.accounts.dst_bank.load()?;
        let src_tr = ctx.accounts.src_token_reserve.as_deref();
        let dst_tr = ctx.accounts.dst_token_reserve.as_deref();
        let src_rate = rate_of(&src_bank, src_oracle, src_tr, &clock)?;
        let dst_rate = rate_of(&dst_bank, dst_oracle, dst_tr, &clock)?;
        let min_imp = I80F48::from(order.min_improvement);
        check!(
            dst_rate > src_rate.checked_add(min_imp).ok_or_else(math_error!())?,
            MarginfiError::RebalanceNotImproving
        );

        let on_ramp_transition = ctx.accounts.group.load()?.on_ramp_transition();
        let account = ctx.accounts.marginfi_account.load()?;
        let pre_src_value = bank_asset_value(
            &account,
            &src_key,
            &src_bank,
            src_oracle,
            &clock,
            on_ramp_transition,
        )?;
        let pre_dst_value = bank_asset_value(
            &account,
            &dst_key,
            &dst_bank,
            dst_oracle,
            &clock,
            on_ramp_transition,
        )?;

        let mut record = ctx.accounts.rebalance_record.load_init()?;
        record.initialize(
            ctx.accounts.rebalance_order.key(),
            ctx.accounts.executor.key(),
            src_key,
            dst_key,
            pre_src_value,
            pre_dst_value,
            src_rate,
            dst_rate,
            &account,
        )?;
    }

    {
        let mut account = ctx.accounts.marginfi_account.load_mut()?;
        account.set_flag(ACCOUNT_IN_REBALANCE, false);
    }
    validate_rebalance_instructions(&ctx.accounts.instruction_sysvar)?;
    Ok(())
}

#[derive(Accounts)]
pub struct StartRebalance<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
        ) @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub src_bank: AccountLoader<'info, Bank>,
    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub dst_bank: AccountLoader<'info, Bank>,
    /// CHECK: JupLend src TokenReserve, validated in-handler against the Lending state; None otherwise.
    pub src_token_reserve: Option<UncheckedAccount<'info>>,
    /// CHECK: JupLend dst TokenReserve, validated in-handler against the Lending state; None otherwise.
    pub dst_token_reserve: Option<UncheckedAccount<'info>>,
    #[account(has_one = marginfi_account @ MarginfiError::Unauthorized)]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
    /// CHECK: the keeper; gains temporary withdraw/deposit authority for the sandwich.
    pub executor: UncheckedAccount<'info>,
    #[account(
        init,
        payer = fee_payer,
        space = 8 + RebalanceRecord::LEN,
        seeds = [REBALANCE_RECORD_SEED.as_bytes(), rebalance_order.key().as_ref()],
        bump,
    )]
    pub rebalance_record: AccountLoader<'info, RebalanceRecord>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    /// CHECK: validated by address.
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Hashable for StartRebalance<'info> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_start_rebalance")
    }
}

pub fn end_rebalance<'info>(ctx: Context<'info, EndRebalance<'info>>) -> MarginfiResult {
    validate_not_cpi_by_stack_height()?;
    let clock = Clock::get()?;
    let remaining = ctx.remaining_accounts;

    let (src_key, dst_key, pre_src_value, pre_dst_value) = {
        let record = ctx.accounts.rebalance_record.load()?;
        (
            record.src_bank,
            record.dst_bank,
            I80F48::from(record.pre_src_value),
            I80F48::from(record.pre_dst_value),
        )
    };
    require_keys_eq!(
        ctx.accounts.src_bank.key(),
        src_key,
        MarginfiError::InvalidBankAccount
    );
    require_keys_eq!(
        ctx.accounts.dst_bank.key(),
        dst_key,
        MarginfiError::InvalidBankAccount
    );

    // Remaining layout: [src venue/oracle accounts][dst venue/oracle accounts][post-move health
    // observation set]. The first two segments price the {src,dst} rate gate; the tail is the full
    // observation set (bank+oracle per active balance, after the move) for the init-health recheck.
    let (src_oracle, dst_oracle, health_obs) = {
        let src_bank = ctx.accounts.src_bank.load()?;
        let dst_bank = ctx.accounts.dst_bank.load()?;
        let src_n = get_remaining_accounts_per_bank(&src_bank)?.saturating_sub(1);
        let dst_n = get_remaining_accounts_per_bank(&dst_bank)?.saturating_sub(1);
        require_gte!(
            remaining.len(),
            src_n + dst_n,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        (
            &remaining[0..src_n],
            &remaining[src_n..src_n + dst_n],
            &remaining[src_n + dst_n..],
        )
    };

    let mut health_cache = HealthCache::zeroed();
    {
        let src_bank = ctx.accounts.src_bank.load()?;
        let dst_bank = ctx.accounts.dst_bank.load()?;
        let src_tr = ctx.accounts.src_token_reserve.as_deref();
        let dst_tr = ctx.accounts.dst_token_reserve.as_deref();
        let src_post = rate_of(&src_bank, src_oracle, src_tr, &clock)?;
        let dst_post = rate_of(&dst_bank, dst_oracle, dst_tr, &clock)?;
        check!(dst_post >= src_post, MarginfiError::RebalanceOvershoot);

        let on_ramp_transition = ctx.accounts.group.load()?.on_ramp_transition();
        let account = ctx.accounts.marginfi_account.load()?;
        let post_src_value = bank_asset_value(
            &account,
            &src_key,
            &src_bank,
            src_oracle,
            &clock,
            on_ramp_transition,
        )?;
        let post_dst_value = bank_asset_value(
            &account,
            &dst_key,
            &dst_bank,
            dst_oracle,
            &clock,
            on_ramp_transition,
        )?;

        let order_amount = ctx.accounts.rebalance_order.load()?.amount;
        if order_amount == 0 {
            // Unlimited order: the full source balance must move — the source asset slot must be
            // emptied. Checked in share space via the canonical `is_empty` (asset_shares <
            // EMPTY_BALANCE_THRESHOLD), which is oracle- and decimals-independent.
            let src_emptied = match account.lending_account.get_balance(&src_key) {
                None => true,
                Some(b) => b.is_empty(BalanceSide::Assets),
            };
            check!(src_emptied, MarginfiError::RebalanceIncompleteMove);
        } else {
            // Bounded order: cap the move at `amount` and require the destination to actually receive
            // it. The keeper's fee is the slice it withholds from the deposit, so a falling source
            // balance is not enough — value must land in dst. The upper bound caps the withdrawal at
            // the ordered amount; the delivery bound forces dst to gain ~that amount net of at most the
            // flat fee, so the keeper cannot be paid without performing the move (checking source
            // reduction instead would let a keeper withdraw and pocket without ever depositing). The
            // target is capped at the position so an `amount` exceeding the holding cleanly degrades to
            // a full move.
            let amount_value = value_of_native(
                I80F48::from_num(order_amount),
                &src_bank,
                src_oracle,
                &clock,
                on_ramp_transition,
            )?;
            let src_moved = pre_src_value
                .checked_sub(post_src_value)
                .ok_or_else(math_error!())?;
            let dst_gained = post_dst_value
                .checked_sub(pre_dst_value)
                .ok_or_else(math_error!())?;
            let target = amount_value.min(pre_src_value);
            check!(
                src_moved <= amount_value,
                MarginfiError::RebalanceExceedsAmount
            );
            check!(
                dst_gained
                    >= target
                        .checked_sub(REBALANCE_FLAT_FEE_USD)
                        .ok_or_else(math_error!())?,
                MarginfiError::RebalanceIncompleteMove
            );
        }

        // Value conservation: per-move user loss is bounded by EXACTLY the flat fee — no dust slack.
        // Any sub-unit venue rounding is the keeper's burden, not extra user loss, so the move is
        // exactly conservative: `post_total + keeper_fee == pre_total`, keeper_fee in [0, flat_fee].
        let pre_total = pre_src_value
            .checked_add(pre_dst_value)
            .ok_or_else(math_error!())?;
        let post_total = post_src_value
            .checked_add(post_dst_value)
            .ok_or_else(math_error!())?;
        let floor = pre_total
            .checked_sub(REBALANCE_FLAT_FEE_USD)
            .ok_or_else(math_error!())?;
        check!(post_total >= floor, MarginfiError::RebalanceValueLeak);

        // Per-withdraw health checks are skipped while ACCOUNT_IN_REBALANCE is set, so recompute
        // health once here over the post-move balance set. A rebalance moves an existing position
        // between same-mint venues rather than opening new risk, so the MAINTENANCE requirement
        // applies: the account need only stay non-liquidatable, not pass the stricter initial bar. A
        // same-mint, value-conserving move can still shift health via venue weights/oracles, emode,
        // risk tiers, and the flat-fee skim, so a full health check is needed
        check_account_maint_health(
            &account,
            health_obs,
            &mut Some(&mut health_cache),
            on_ramp_transition,
        )?;
        health_cache.program_version = PROGRAM_VERSION;
        health_cache.set_engine_ok(true);

        let record = ctx.accounts.rebalance_record.load()?;
        record.verify_others_unchanged(&account)?;
    }

    {
        let mut account = ctx.accounts.marginfi_account.load_mut()?;
        account.health_cache = health_cache;
        account.unset_flag(ACCOUNT_IN_REBALANCE, false);
        account.lending_account.sort_balances();
        account.sync_indexer_flags();
        account.last_update = clock.unix_timestamp as u64;
    }
    {
        let mut order = ctx.accounts.rebalance_order.load_mut()?;
        order.last_exec_timestamp = clock.unix_timestamp as u64;
    }
    Ok(())
}

#[derive(Accounts)]
pub struct EndRebalance<'info> {
    #[account(
        constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.get_flag(ACCOUNT_IN_REBALANCE) && !acc.get_flag(ORDER_BLOCKING_FLAGS)
        } @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    #[account(mut, has_one = marginfi_account @ MarginfiError::Unauthorized)]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
    #[account(
        mut,
        close = executor,
        has_one = executor @ MarginfiError::Unauthorized,
        constraint = rebalance_record.load()?.order == rebalance_order.key()
            @ MarginfiError::Unauthorized,
    )]
    pub rebalance_record: AccountLoader<'info, RebalanceRecord>,
    pub executor: Signer<'info>,
    pub src_bank: AccountLoader<'info, Bank>,
    pub dst_bank: AccountLoader<'info, Bank>,
    /// CHECK: JupLend src TokenReserve, validated in-handler against the Lending state; None otherwise.
    pub src_token_reserve: Option<UncheckedAccount<'info>>,
    /// CHECK: JupLend dst TokenReserve, validated in-handler against the Lending state; None otherwise.
    pub dst_token_reserve: Option<UncheckedAccount<'info>>,
}

impl<'info> Hashable for EndRebalance<'info> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_end_rebalance")
    }
}
