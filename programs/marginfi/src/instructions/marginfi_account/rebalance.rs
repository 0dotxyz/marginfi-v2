//! Persistent same-mint auto-rebalance orders. A keeper relocates positions across banks of the
//! SAME mint within an allowlisted venue set — many source and many destination banks in a single
//! execution (up to `MAX_REBALANCE_MOVES` declared moves) — via a `start_rebalance`..`end_rebalance`
//! sandwich that reuses the existing per-venue withdraw/deposit instructions. The order is NOT
//! consumed on execution — it persists until cancelled.
//!
//! On-chain guarantees: every referenced bank holds the order's mint and is in the allowed set; each
//! declared move goes from a lower-rate bank to one beating it by `min_improvement` (pre-move) and
//! not inverted after the move's own market impact (post-move); the total value moved is capped by
//! the order's `amount` budget (uncapped when the order is unlimited); value is conserved per bank up
//! to a small dust tolerance; snapshotted non-referenced balances keep their side and shares; the
//! account stays healthy at the maintenance requirement if it borrows; and a per-order cooldown.
//!
//! Supports native, Kamino, Drift, and JupLend legs. Referenced banks arrive as a deduped, indexed
//! stream in the remaining accounts; a JupLend bank's `TokenReserve` is read from that stream
//! (validated against the bank's Lending state).

use crate::{
    check, check_eq,
    constants::PROGRAM_VERSION,
    events::{
        AccountEventHeader, KeeperCloseRebalanceOrderEvent,
        MarginfiAccountCloseRebalanceOrderEvent, MarginfiAccountPlaceRebalanceOrderEvent,
        MarginfiAccountUpdateRebalanceOrderEvent, RebalanceExecutedEvent,
        RebalanceFeePoolTopUpEvent, RebalanceFeePoolWithdrawEvent,
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
    utils::is_integration_asset_tag,
};
use anchor_lang::{
    prelude::*,
    solana_program::{
        program::{invoke, invoke_signed},
        system_instruction,
    },
    system_program,
};
use anchor_spl::token_interface::Mint;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_JUPLEND, REBALANCE_CONSERVATION_DUST_USD, REBALANCE_DEFAULT_COOLDOWN_SECONDS,
        REBALANCE_DEFAULT_MIN_IMPROVEMENT, REBALANCE_FEE_POOL_SEED, REBALANCE_ORDER_SEED,
        REBALANCE_RECORD_SEED,
    },
    types::{
        Bank, HealthCache, MarginfiAccount, MarginfiGroup, OraclePriceType, RebalanceMove,
        RebalanceOrder, RebalanceRecord, WrappedI80F48, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_REBALANCE, MAX_REBALANCE_BANKS, MAX_REBALANCE_MOVES, ORDER_BLOCKING_FLAGS,
    },
};

/// Equity (weight = 1) USD value of a raw native token amount in `bank`, priced from its oracle.
fn value_of_native<'info>(
    amount_native: I80F48,
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let adapter = OraclePriceFeedAdapter::try_from_bank(bank, oracle_ais, clock)?;
    let price = adapter.get_price_of_type(
        OraclePriceType::RealTime,
        None,
        bank.config.oracle_max_confidence,
    )?;
    calc_value(amount_native, price, bank.get_balance_decimals(), None)
}

/// Equity (weight = 1) USD value of the user's asset position in `bank`. Returns 0 if the user holds
/// no balance there (e.g. the source balance after a full move).
fn bank_asset_value<'info>(
    account: &MarginfiAccount,
    bank_key: &Pubkey,
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let balance = match account.lending_account.get_balance(bank_key) {
        Some(b) => b,
        None => return Ok(I80F48::ZERO),
    };
    let amount = bank.get_asset_amount(balance.asset_shares.into())?;
    value_of_native(amount, bank, oracle_ais, clock)
}

pub fn place_rebalance_order(
    ctx: Context<PlaceRebalanceOrder>,
    allowed_banks: Vec<Pubkey>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> MarginfiResult {
    // User-owned policy with sensible defaults: 5% min improvement, 24h cooldown, no budget cap,
    // no keeper tip.
    let min_improvement =
        min_improvement.unwrap_or_else(|| WrappedI80F48::from(REBALANCE_DEFAULT_MIN_IMPROVEMENT));
    let cooldown_seconds = cooldown_seconds.unwrap_or(REBALANCE_DEFAULT_COOLDOWN_SECONDS);
    let amount = amount.unwrap_or(0);
    let keeper_tip = keeper_tip.unwrap_or(0);

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
            keeper_tip,
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
        keeper_tip,
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

/// Close a rebalance order. The account authority may close their own order at any time (except
/// mid-rebalance). Permissionlessly, anyone may close a stale order once it can no longer act — the
/// account was closed, or it holds no position in any allowed venue. Rent goes to `fee_recipient`.
pub fn close_rebalance_order(ctx: Context<CloseRebalanceOrder>) -> MarginfiResult {
    let order = ctx.accounts.rebalance_order.load()?;
    let marginfi_account_info = ctx.accounts.marginfi_account.to_account_info();
    let signer = ctx.accounts.authority.as_ref().map(|a| a.key());

    // Manual owner check: only deserialize when the account is not already closed.
    let (authority_pk, group_pk, by_authority) =
        if marginfi_account_info.owner.eq(&system_program::ID)
            && marginfi_account_info.data_is_empty()
        {
            // The account is gone: the order is dead and anyone may reclaim it.
            (Pubkey::default(), Pubkey::default(), false)
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

            // The authority may close their own order anytime; anyone else may close it only once it
            // holds no position in any allowed venue.
            let by_authority = signer == Some(marginfi_account.authority);
            let allowed = &order.allowed_banks[..order.allowed_bank_count as usize];
            let has_allowed_position = marginfi_account
                .lending_account
                .balances
                .iter()
                .any(|b| b.is_active() && allowed.contains(&b.bank_pk));
            check!(
                by_authority || !has_allowed_position,
                MarginfiError::LiquidatorOrderCloseNotAllowed
            );
            if by_authority {
                check!(
                    !marginfi_account.get_flag(ACCOUNT_IN_REBALANCE),
                    MarginfiError::IllegalAction
                );
            }
            marginfi_account.decrement_active_orders()?;
            (
                marginfi_account.authority,
                marginfi_account.group,
                by_authority,
            )
        };

    let header = AccountEventHeader {
        signer: if by_authority { signer } else { None },
        marginfi_account: marginfi_account_info.key(),
        marginfi_account_authority: authority_pk,
        marginfi_group: group_pk,
    };
    let rebalance_order = ctx.accounts.rebalance_order.key();
    if by_authority {
        emit!(MarginfiAccountCloseRebalanceOrderEvent {
            header,
            rebalance_order,
        });
    } else {
        emit!(KeeperCloseRebalanceOrderEvent {
            header,
            rebalance_order,
        });
    }
    Ok(())
}

#[derive(Accounts)]
pub struct CloseRebalanceOrder<'info> {
    /// CHECK: unchecked so the ix works even when the marginfi account was closed; ownership and type
    /// are validated in the handler.
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,
    /// Signs to close an order that still holds a position; omitted for the permissionless close of a
    /// dead order.
    pub authority: Option<Signer<'info>>,
    /// CHECK: no checks; receives the order's rent.
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,
    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        close = fee_recipient
    )]
    pub rebalance_order: AccountLoader<'info, RebalanceOrder>,
}

/// Modify an existing order's policy in place: venue allowlist, min improvement, cooldown, amount
/// budget, and/or keeper tip. `None` fields are left unchanged.
pub fn update_rebalance_order(
    ctx: Context<UpdateRebalanceOrder>,
    allowed_banks: Option<Vec<Pubkey>>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> MarginfiResult {
    let account = ctx.accounts.marginfi_account.load()?;
    check!(
        !account.get_flag(ACCOUNT_IN_REBALANCE),
        MarginfiError::IllegalAction
    );

    let (allowed, min_imp, cooldown, amount, tip) = {
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
        if let Some(t) = keeper_tip {
            order.keeper_tip = t;
        }
        (
            order.allowed_banks[..order.allowed_bank_count as usize].to_vec(),
            order.min_improvement,
            order.cooldown_seconds,
            order.amount,
            order.keeper_tip,
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
        keeper_tip: tip,
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

/// Transfer `amount` lamports out of a marginfi account's fee-pool PDA, which signs via its seeds.
/// No-op for a zero amount.
fn pay_from_fee_pool<'info>(
    fee_pool: &SystemAccount<'info>,
    to: &AccountInfo<'info>,
    system_program: &Program<'info, System>,
    marginfi_account: &Pubkey,
    bump: u8,
    amount: u64,
) -> MarginfiResult {
    if amount == 0 {
        return Ok(());
    }
    let ix = system_instruction::transfer(&fee_pool.key(), to.key, amount);
    invoke_signed(
        &ix,
        &[
            fee_pool.to_account_info(),
            to.clone(),
            system_program.to_account_info(),
        ],
        &[&[
            REBALANCE_FEE_POOL_SEED.as_bytes(),
            marginfi_account.as_ref(),
            &[bump],
        ]],
    )?;
    Ok(())
}

/// Fund an account's rebalance fee pool. Permissionless: anyone may top up any account's pool (the
/// authority, a keeper, or a third party), since the funds can only ever pay keeper tips or be
/// withdrawn by the account authority. The first top-up also seeds the pool's rent-exempt reserve, so
/// the pool is always rent-exempt and `amount` is the spendable tip budget added above the reserve.
pub fn top_up_rebalance_fee_pool(
    ctx: Context<TopUpRebalanceFeePool>,
    amount: u64,
) -> MarginfiResult {
    let seed = if ctx.accounts.fee_pool.lamports() == 0 {
        Rent::get()?.minimum_balance(0)
    } else {
        0
    };
    let transfer = amount.checked_add(seed).ok_or_else(math_error!())?;
    let ix = system_instruction::transfer(
        &ctx.accounts.payer.key(),
        &ctx.accounts.fee_pool.key(),
        transfer,
    );
    invoke(
        &ix,
        &[
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.fee_pool.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;
    let account = ctx.accounts.marginfi_account.load()?;
    emit!(RebalanceFeePoolTopUpEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.payer.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        fee_pool: ctx.accounts.fee_pool.key(),
        amount,
        new_balance: ctx.accounts.fee_pool.lamports(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct TopUpRebalanceFeePool<'info> {
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    #[account(
        mut,
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub fee_pool: SystemAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Withdraw lamports from an account's rebalance fee pool back to the authority. Caps at the pool
/// balance; only the account authority may withdraw. The pool is a rent-exempt system PDA, so a
/// withdrawal that would leave it rent-paying (0 < balance < exempt) instead closes it and returns
/// the full balance.
pub fn withdraw_rebalance_fee_pool(
    ctx: Context<WithdrawRebalanceFeePool>,
    amount: u64,
) -> MarginfiResult {
    let balance = ctx.accounts.fee_pool.lamports();
    let amount = amount.min(balance);
    let amount = if balance.saturating_sub(amount) < Rent::get()?.minimum_balance(0) {
        balance
    } else {
        amount
    };
    pay_from_fee_pool(
        &ctx.accounts.fee_pool,
        &ctx.accounts.destination.to_account_info(),
        &ctx.accounts.system_program,
        &ctx.accounts.marginfi_account.key(),
        ctx.bumps.fee_pool,
        amount,
    )?;
    let account = ctx.accounts.marginfi_account.load()?;
    emit!(RebalanceFeePoolWithdrawEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        fee_pool: ctx.accounts.fee_pool.key(),
        amount,
        new_balance: ctx.accounts.fee_pool.lamports(),
    });
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawRebalanceFeePool<'info> {
    #[account(has_one = authority @ MarginfiError::Unauthorized)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
    pub authority: Signer<'info>,
    #[account(
        mut,
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub fee_pool: SystemAccount<'info>,
    /// CHECK: recipient of the withdrawn lamports.
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// A bank parsed from the rebalance remaining-accounts stream, with its pricing accounts.
struct ParsedBank<'info> {
    key: Pubkey,
    loader: AccountLoader<'info, Bank>,
    token_reserve: Option<&'info AccountInfo<'info>>,
    oracles: &'info [AccountInfo<'info>],
}

/// Parse the referenced-bank prefix of the rebalance remaining-accounts stream: exactly `bank_count`
/// blocks, each `[bank] [token_reserve (JupLend only)] [oracles]`, deduped (each referenced bank
/// appears once and moves reference it by index). Returns the parsed banks and the untouched tail
/// (empty for `start`; the post-move health observation set for `end`).
fn parse_rebalance_banks<'info>(
    remaining: &'info [AccountInfo<'info>],
    group: &Pubkey,
    bank_count: usize,
) -> MarginfiResult<(Vec<ParsedBank<'info>>, &'info [AccountInfo<'info>])> {
    let mut cursor = 0usize;
    let mut banks: Vec<ParsedBank> = Vec::with_capacity(bank_count);
    while banks.len() < bank_count {
        require_gt!(
            remaining.len(),
            cursor,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        let bank_ai = &remaining[cursor];
        // Reject a bank appearing more than once — indices must be unambiguous.
        check!(
            !banks.iter().any(|b| b.key == bank_ai.key()),
            MarginfiError::SameAssetAndLiabilityBanks
        );
        let loader = AccountLoader::<Bank>::try_from(bank_ai)
            .map_err(|_| error!(MarginfiError::InvalidBankAccount))?;
        cursor += 1;
        let (tag, oracle_n) = {
            let b = loader.load()?;
            require_keys_eq!(b.group, *group, MarginfiError::InvalidGroup);
            (
                b.config.asset_tag,
                get_remaining_accounts_per_bank(&b)?.saturating_sub(1),
            )
        };
        let token_reserve = if tag == ASSET_TAG_JUPLEND {
            require_gt!(
                remaining.len(),
                cursor,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            let t = &remaining[cursor];
            cursor += 1;
            Some(t)
        } else {
            None
        };
        require_gte!(
            remaining.len(),
            cursor + oracle_n,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        let oracles = &remaining[cursor..cursor + oracle_n];
        cursor += oracle_n;
        banks.push(ParsedBank {
            key: bank_ai.key(),
            loader,
            token_reserve,
            oracles,
        });
    }
    Ok((banks, &remaining[cursor..]))
}

/// Derive the referenced-bank count from a keeper move list: the highest index used, plus one. Every
/// referenced bank must be touched by some move, so this equals the number of banks to parse.
fn referenced_bank_count(moves: &[RebalanceMove]) -> usize {
    moves
        .iter()
        .map(|m| m.src_index.max(m.dst_index) as usize)
        .max()
        .map(|max_idx| max_idx + 1)
        .unwrap_or(0)
}

/// Freshen a native bank's cached supply rate in place (accrue interest + recompute cache), so the
/// improvement gate reads a current rate rather than a lagged one. No-op for integration banks, whose
/// rate comes from the venue reserve (refreshed by the keeper's crank + the staleness check).
fn accrue_native_bank(
    parsed: &ParsedBank,
    group: &AccountLoader<MarginfiGroup>,
    clock: &Clock,
) -> MarginfiResult {
    let is_integration = { is_integration_asset_tag(parsed.loader.load()?.config.asset_tag) };
    if is_integration {
        return Ok(());
    }
    let group = group.load()?;
    let mut bank = parsed.loader.load_mut()?;
    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        parsed.key,
    )?;
    bank.update_bank_cache(&group)?;
    Ok(())
}

pub fn start_rebalance<'info>(
    ctx: Context<'info, StartRebalance<'info>>,
    moves: Vec<RebalanceMove>,
) -> MarginfiResult {
    let clock = Clock::get()?;
    let group_key = ctx.accounts.group.key();
    let remaining = ctx.remaining_accounts;

    check!(
        !moves.is_empty() && moves.len() <= MAX_REBALANCE_MOVES,
        MarginfiError::IllegalBalanceState
    );
    let bank_count = referenced_bank_count(&moves);
    check!(
        (2..=MAX_REBALANCE_BANKS).contains(&bank_count),
        MarginfiError::RebalanceBankNotAllowed
    );

    let (banks, tail) = parse_rebalance_banks(remaining, &group_key, bank_count)?;
    check!(tail.is_empty(), MarginfiError::WrongNumberOfOracleAccounts);

    let order = ctx.accounts.rebalance_order.load()?;
    check!(
        (clock.unix_timestamp as u64)
            >= order
                .last_exec_timestamp
                .checked_add(order.cooldown_seconds)
                .ok_or_else(math_error!())?,
        MarginfiError::RebalanceCooldown
    );
    let allowed = &order.allowed_banks[..order.allowed_bank_count as usize];
    let min_imp = I80F48::from(order.min_improvement);

    // Freshen native banks before reading their rates (integration banks were refreshed by the
    // keeper's venue crank, enforced by the staleness check inside `rate_of`).
    for parsed in banks.iter() {
        accrue_native_bank(parsed, &ctx.accounts.group, &clock)?;
    }

    // Price every referenced bank once: validate allowlist + mint, compute its supply rate and its
    // pre-move position value.
    let account = ctx.accounts.marginfi_account.load()?;
    let mut rates: Vec<I80F48> = Vec::with_capacity(banks.len());
    let mut ref_banks: Vec<(Pubkey, I80F48)> = Vec::with_capacity(banks.len());
    for parsed in banks.iter() {
        check!(
            allowed.contains(&parsed.key),
            MarginfiError::RebalanceBankNotAllowed
        );
        let bank = parsed.loader.load()?;
        check!(
            bank.mint == order.mint,
            MarginfiError::RebalanceMintMismatch
        );
        let rate = rate_of(&bank, parsed.oracles, parsed.token_reserve, &clock)?;
        let pre = bank_asset_value(&account, &parsed.key, &bank, parsed.oracles, &clock)?;
        rates.push(rate);
        ref_banks.push((parsed.key, pre));
    }

    // Every declared move must go from a lower-rate bank to one that beats it by the margin.
    for m in moves.iter() {
        let src_rate = rates[m.src_index as usize];
        let dst_rate = rates[m.dst_index as usize];
        check!(
            dst_rate > src_rate.checked_add(min_imp).ok_or_else(math_error!())?,
            MarginfiError::RebalanceNotImproving
        );
    }

    {
        let mut record = ctx.accounts.rebalance_record.load_init()?;
        record.initialize(
            ctx.accounts.rebalance_order.key(),
            ctx.accounts.executor.key(),
            &ref_banks,
            &moves,
            &account,
        )?;
    }

    drop(account);
    drop(order);
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
    // Referenced banks follow in remaining_accounts, one block each (deduped, indexed by the moves):
    // [bank, (JupLend reserve), oracles]...
}

impl<'info> Hashable for StartRebalance<'info> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_start_rebalance")
    }
}

pub fn end_rebalance<'info>(ctx: Context<'info, EndRebalance<'info>>) -> MarginfiResult {
    validate_not_cpi_by_stack_height()?;
    let clock = Clock::get()?;
    let group_key = ctx.accounts.group.key();
    let remaining = ctx.remaining_accounts;

    let (ref_keys, order_amount, keeper_tip) = {
        let record = ctx.accounts.rebalance_record.load()?;
        let order = ctx.accounts.rebalance_order.load()?;
        let n = record.ref_bank_count as usize;
        (
            record.ref_banks[..n]
                .iter()
                .map(|r| r.bank)
                .collect::<Vec<_>>(),
            order.amount,
            order.keeper_tip,
        )
    };

    // Remaining layout: [referenced bank blocks][post-move health observation set]. Parse exactly the
    // recorded banks (order and identity must match the record's indices); the tail is the health set.
    let (banks, health_obs) = parse_rebalance_banks(remaining, &group_key, ref_keys.len())?;
    for (parsed, key) in banks.iter().zip(ref_keys.iter()) {
        require_keys_eq!(parsed.key, *key, MarginfiError::InvalidBankAccount);
    }

    let dust = REBALANCE_CONSERVATION_DUST_USD;
    let mut health_cache = HealthCache::zeroed();
    let (value_moved, tip_paid) = {
        let account = ctx.accounts.marginfi_account.load()?;

        // Price every referenced bank once: current supply rate (for the per-move overshoot check) and
        // post-move position value (for reconciliation).
        let mut post_rates: Vec<I80F48> = Vec::with_capacity(banks.len());
        let mut post_values: Vec<I80F48> = Vec::with_capacity(banks.len());
        for parsed in banks.iter() {
            let bank = parsed.loader.load()?;
            post_rates.push(rate_of(
                &bank,
                parsed.oracles,
                parsed.token_reserve,
                &clock,
            )?);
            post_values.push(bank_asset_value(
                &account,
                &parsed.key,
                &bank,
                parsed.oracles,
                &clock,
            )?);
        }

        // Every move must not have inverted its rate advantage (the destination still beats the source
        // after the move's own market impact).
        let record = ctx.accounts.rebalance_record.load()?;
        for m in record.active_moves() {
            check!(
                post_rates[m.dst_index as usize] >= post_rates[m.src_index as usize],
                MarginfiError::RebalanceOvershoot
            );
        }

        // Reconcile the declared moves against the real per-bank value deltas. This proves value
        // conservation (each bank's delta matches its declared net flow within dust) and, with the
        // per-move improvement check, that every dollar moved to a strictly better venue.
        let (total_moved, total_source_pre) = record.reconcile(&post_values, dust)?;
        check!(
            total_moved > I80F48::ZERO,
            MarginfiError::RebalanceIncompleteMove
        );

        // Per-withdraw health checks are skipped while ACCOUNT_IN_REBALANCE is set, so recompute
        // health once here over the post-move balance set. A rebalance moves an existing position
        // between same-mint venues rather than opening new risk, so the MAINTENANCE requirement
        // applies: the account need only stay non-liquidatable, not pass the stricter initial bar.
        check_account_maint_health(&account, health_obs, &mut Some(&mut health_cache))?;
        health_cache.program_version = PROGRAM_VERSION;
        health_cache.set_engine_ok(true);

        record.verify_others_unchanged(&account)?;

        // `order.amount` (native) is a per-execution TOTAL value budget: the move may relocate at most
        // its value across all banks. Unlimited (0) means no cap. Priced via any referenced bank
        // (all same-mint).
        let amount_value = if order_amount == 0 {
            None
        } else {
            let bank0 = banks[0].loader.load()?;
            Some(value_of_native(
                I80F48::from_num(order_amount),
                &bank0,
                banks[0].oracles,
                &clock,
            )?)
        };
        if let Some(cap) = amount_value {
            check!(
                total_moved <= cap.checked_add(dust).ok_or_else(math_error!())?,
                MarginfiError::RebalanceExceedsAmount
            );
        }

        // Proportional tip over a stable denominator: `keeper_tip * (moved / target)`. `target` is the
        // order's `amount` value (stable across executions) or the full source position when unlimited.
        // A fixed target makes the tip invariant to how the move is split across banks — fragmenting
        // earns no more. The tip is drawn only from lamports above the pool's rent-exempt reserve, so
        // the reserve is never paid out and the pool is never left in a rent-paying state.
        let spendable = ctx
            .accounts
            .fee_pool
            .lamports()
            .saturating_sub(Rent::get()?.minimum_balance(0));
        let target_value = amount_value
            .map(|cap| cap.min(total_source_pre))
            .unwrap_or(total_source_pre);
        let tip_paid = if keeper_tip == 0 || target_value <= I80F48::ZERO {
            0
        } else {
            let fraction = total_moved
                .checked_div(target_value)
                .ok_or_else(math_error!())?
                .min(I80F48::from_num(1));
            let owed = I80F48::from_num(keeper_tip)
                .checked_mul(fraction)
                .ok_or_else(math_error!())?;
            owed.floor().to_num::<u64>().min(spendable)
        };
        (total_moved, tip_paid)
    };

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

    pay_from_fee_pool(
        &ctx.accounts.fee_pool,
        &ctx.accounts.executor.to_account_info(),
        &ctx.accounts.system_program,
        &ctx.accounts.marginfi_account.key(),
        ctx.bumps.fee_pool,
        tip_paid,
    )?;

    let (authority, group) = {
        let account = ctx.accounts.marginfi_account.load()?;
        (account.authority, account.group)
    };
    emit!(RebalanceExecutedEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.executor.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: authority,
            marginfi_group: group,
        },
        rebalance_order: ctx.accounts.rebalance_order.key(),
        executor: ctx.accounts.executor.key(),
        bank_count: ref_keys.len() as u8,
        value_moved: value_moved.into(),
        tip_paid,
    });
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
    #[account(
        mut,
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub fee_pool: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
    // Referenced banks then the health set follow in remaining_accounts:
    // [bank, (JupLend reserve), oracles]...[bank, oracle per active balance].
}

impl<'info> Hashable for EndRebalance<'info> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_end_rebalance")
    }
}
