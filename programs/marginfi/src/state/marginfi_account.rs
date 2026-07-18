use super::price::{OraclePriceFeedAdapter, PriceAdapter};
use crate::{
    allocator::{heap_pos, heap_restore},
    check, check_eq, debug, live, math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::bank::BankImpl,
    utils::{is_integration_asset_tag, NumTraitsWithTolerance},
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
        ASSET_TAG_SOLEND, ASSET_TAG_STAKED, BANKRUPT_THRESHOLD, BANK_SAME_ASSET_EMODE_ELIGIBLE,
        CIRCUIT_BREAKER_ENABLED, EXP_10_I80F48, MAX_INTEGRATION_POSITIONS, ORDER_ACTIVE_TAGS,
        ZERO_AMOUNT_THRESHOLD,
    },
    types::{
        compute_same_asset_emode_weight, reconcile_emode_configs, u32_to_basis, Balance,
        BalanceSide, Bank, BankOperationalState, EmodeConfig, HealthCache, HealthPriceMode,
        LendingAccount, LiquidationPriceCache, MarginfiAccount, MarginfiGroup, OracleFeedFamily,
        OraclePriceType, OraclePriceWithConfidence, OracleSetup, PriceBias, ReconciledEmodeConfig,
        RequirementType, RiskTier, ACCOUNT_DISABLED, ACCOUNT_FROZEN, ACCOUNT_IN_FLASHLOAN,
        ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
    },
};
use std::{
    cmp::{max, min},
    collections::BTreeSet,
};

/// Returns the number of remaining accounts required for a bank (bank account + oracle/venue accounts).
///
/// Account counts by oracle setup and asset tag:
/// - `Fixed`: 1 (bank only)
/// - `FixedKamino`: 2 (bank + reserve)
/// - `FixedDrift`: 2 (bank + spot market)
/// - `FixedJuplend`: 2 (bank + lending state)
/// - `ASSET_TAG_STAKED`: 5 (bank + oracle + lst_mint + stake_pool + onramp)
/// - `ASSET_TAG_KAMINO` / `ASSET_TAG_DRIFT` / `ASSET_TAG_SOLEND` / `ASSET_TAG_JUPLEND`: 3 (bank + oracle + reserve)
/// - `ASSET_TAG_DEFAULT` / `ASSET_TAG_SOL`: 2 (bank + oracle)
pub fn get_remaining_accounts_per_bank(bank: &Bank) -> MarginfiResult<usize> {
    match bank.config.oracle_setup {
        OracleSetup::Fixed => Ok(1),
        // Fixed + Kamino: bank + reserve (no oracle)
        OracleSetup::FixedKamino => Ok(2),
        // Fixed + Drift: bank + spot market (no oracle)
        OracleSetup::FixedDrift => Ok(2),
        // Fixed + JupLend: bank + lending state (no oracle)
        OracleSetup::FixedJuplend => Ok(2),
        _ => get_remaining_accounts_per_asset_tag(bank.config.asset_tag),
    }
}

/// 5 for `ASSET_TAG_STAKED` (bank, oracle, lst mint, lst pool, onramp), 2 for most others (bank, oracle), 3
/// for Kamino (bank, oracle, reserve), 1 for Fixed
fn get_remaining_accounts_per_asset_tag(asset_tag: u8) -> MarginfiResult<usize> {
    match asset_tag {
        ASSET_TAG_DEFAULT | ASSET_TAG_SOL => Ok(2),
        ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND | ASSET_TAG_JUPLEND => Ok(3),
        ASSET_TAG_STAKED => Ok(5),
        _ => err!(MarginfiError::AssetTagMismatch),
    }
}

pub trait MarginfiAccountImpl {
    fn initialize(&mut self, group: Pubkey, authority: Pubkey, current_timestamp: u64);
    fn set_flag(&mut self, flag: u64, msg: bool);
    fn unset_flag(&mut self, flag: u64, msg: bool);
    fn get_flag(&self, flag: u64) -> bool;
    fn increment_active_orders(&mut self) -> MarginfiResult;
    fn decrement_active_orders(&mut self) -> MarginfiResult;
    fn can_be_closed(&self) -> bool;
    fn sync_indexer_flags(&mut self);
}

/// Checks if a signer is authorized to perform actions on a marginfi account.
///
/// Returns `true` if the signer is authorized, `false` otherwise.
///
/// Authorization rules (checked in order):
/// 1. If `allow_receivership` is true and the (NOT signer's) account is in receivership → `true`
/// 2. If `allow_order_execution` is true and the account is in order execution → `true`
/// 3. If the account is frozen → `true` only if signer is the group admin
/// 4. Otherwise → `true` only if signer is the account authority
pub fn is_signer_authorized(
    marginfi_account: &MarginfiAccount,
    group_admin: Pubkey,
    signer: Pubkey,
    allow_receivership: bool,
    allow_order_execution: bool,
) -> bool {
    if allow_receivership && marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP) {
        return marginfi_account.authority != signer; // forbidden to take receivership of your own account
    }

    if allow_order_execution && marginfi_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION) {
        return true;
    }

    if marginfi_account.get_flag(ACCOUNT_FROZEN) {
        return group_admin == signer;
    }

    marginfi_account.authority == signer
}

/// Checks if the account authority is allowed to act on their account based on frozen status.
///
/// Returns `true` if the action is allowed, `false` if blocked.
///
/// Returns `false` when both conditions are met:
/// - The account is frozen
/// - The signer is the account authority
///
/// This is intentionally separate from [`is_signer_authorized`] to return a distinct
/// `AccountFrozen` error in the instruction context  rather than `Unauthorized`.
pub fn account_not_frozen_for_authority(
    marginfi_account: &MarginfiAccount,
    signer: Pubkey,
) -> bool {
    !(marginfi_account.get_flag(ACCOUNT_FROZEN) && marginfi_account.authority == signer)
}

/// Returns `true` if any bank backing an active balance on `marginfi_account` is CB-halted.
/// `remaining_ais` must be the standard bank+oracle layout used by the health computation:
/// one bank account followed by `get_remaining_accounts_per_bank(bank) - 1` venue/oracle
/// accounts per active balance.
pub fn any_balance_bank_is_cb_halted<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<bool> {
    let now = Clock::get()?.unix_timestamp;
    let mut account_index = 0usize;
    for balance in marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
    {
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;
        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );
        // Both a temporal halt and the non-expiring `CircuitBroken` state count as halted.
        if bank.is_cb_halted(now)
            || bank.config.operational_state == BankOperationalState::CircuitBroken
        {
            return Ok(true);
        }
        let num_accounts = get_remaining_accounts_per_bank(&bank)?;
        account_index = account_index.saturating_add(num_accounts);
    }
    Ok(false)
}

/// Runs the inline circuit-breaker price gate (`BankImpl::cb_price_gate`) for every CB-enabled
/// bank backing an active balance on `marginfi_account`. Pure read — reverts with
/// `CircuitBreakerPriceJump` if any such bank's live oracle price has jumped past the breach
/// threshold. Non-CB banks are skipped, so the common case pays no extra oracle reads.
///
/// `remaining_ais` must be the standard bank+oracle layout used by the health computation.
pub fn run_cb_price_gate<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<()> {
    let clock = Clock::get()?;
    let mut account_index = 0usize;
    for balance in marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
    {
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;
        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );

        let num_accounts = get_remaining_accounts_per_bank(&bank)?;

        if bank.get_flag(CIRCUIT_BREAKER_ENABLED) {
            let oracle_start = account_index + 1;
            let oracle_end = oracle_start + num_accounts - 1;
            require_gte!(
                remaining_ais.len(),
                oracle_end,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            let oracle_ais = &remaining_ais[oracle_start..oracle_end];
            // The breaker reference tracks the raw (un-multiplied) oracle price.
            let (_, cache_price) =
                OraclePriceFeedAdapter::get_price_and_confidence_and_cache_of_type(
                    &bank,
                    oracle_ais,
                    &clock,
                    OraclePriceType::RealTime,
                )?;
            bank.cb_price_gate(cache_price.oracle_price)?;
        }

        account_index = account_index.saturating_add(num_accounts);
    }
    Ok(())
}

impl MarginfiAccountImpl for MarginfiAccount {
    /// Set the initial data for the marginfi account.
    fn initialize(&mut self, group: Pubkey, authority: Pubkey, current_timestamp: u64) {
        self.authority = authority;
        self.group = group;
        self.emissions_destination_account = Pubkey::default();
        self.migrated_from = Pubkey::default();
        self.last_update = current_timestamp;
        self.migrated_to = Pubkey::default();
        self.indexer_flags.is_empty = 1;
        // Seed activity flags so freshly-created accounts aren't immediately eligible for the
        // permissionless close path before the first pulse.
        self.indexer_flags.was_active_30d = 1;
        self.indexer_flags.was_active_60d = 1;
        self.active_orders = 0;
    }

    fn set_flag(&mut self, flag: u64, msg: bool) {
        if msg {
            msg!("Setting account flag {:b}", flag);
        }
        self.account_flags |= flag;
    }

    fn unset_flag(&mut self, flag: u64, msg: bool) {
        if msg {
            msg!("Unsetting account flag {:b}", flag);
        }
        self.account_flags &= !flag;
    }

    fn get_flag(&self, flag: u64) -> bool {
        self.account_flags & flag != 0
    }

    fn increment_active_orders(&mut self) -> MarginfiResult {
        // Note: Sanity check, expected to be unreachable, as this vastly exceeds max theoretical
        // orders one account can open.
        check!(
            self.active_orders < u8::MAX,
            MarginfiError::IllegalAction,
            "Too many active orders"
        );
        self.active_orders += 1;
        Ok(())
    }

    fn decrement_active_orders(&mut self) -> MarginfiResult {
        // Note: Sanity check, expected to be unreachable
        check!(
            self.active_orders > 0,
            MarginfiError::IllegalAction,
            "No active orders to close"
        );
        self.active_orders -= 1;
        Ok(())
    }

    fn can_be_closed(&self) -> bool {
        let is_disabled = self.get_flag(ACCOUNT_DISABLED);
        let is_in_flashloan = self.get_flag(ACCOUNT_IN_FLASHLOAN);
        let is_in_receivership = self.get_flag(ACCOUNT_IN_RECEIVERSHIP);
        let is_frozen = self.get_flag(ACCOUNT_FROZEN);
        let only_has_empty_balances = self.lending_account.balances.iter().all(|balance| {
            let liability_shares: I80F48 = balance.liability_shares.into();
            balance.get_side().is_none() && liability_shares <= I80F48::ZERO
        });

        !is_disabled
            && only_has_empty_balances
            && !is_in_flashloan
            && !is_in_receivership
            && !is_frozen
    }

    fn sync_indexer_flags(&mut self) {
        self.indexer_flags
            .sync_balance_derived(&self.lending_account.balances);
        self.indexer_flags.mark_active_now();
    }
}

#[derive(Debug)]
pub enum BalanceIncreaseType {
    Any,
    RepayOnly,
    DepositOnly,
    BypassDepositLimit,
}

#[derive(Debug)]
pub enum BalanceDecreaseType {
    WithdrawOnly,
    BorrowOnly,
    BypassBorrowLimit,
}

#[inline]
fn apply_price_bias(price: OraclePriceWithConfidence, bias: PriceBias) -> MarginfiResult<I80F48> {
    let price = match bias {
        PriceBias::Low => price
            .price
            .checked_sub(price.confidence)
            .ok_or_else(math_error!()),
        PriceBias::High => price
            .price
            .checked_add(price.confidence)
            .ok_or_else(math_error!()),
    }?;
    Ok(price)
}

pub struct BankAccountWithCache<'a, 'info> {
    bank: AccountLoader<'info, Bank>,
    balance: &'a Balance,
}

impl<'info> BankAccountWithCache<'_, 'info> {
    pub fn load<'a>(
        lending_account: &'a LendingAccount,
        remaining_ais: &'info [AccountInfo<'info>],
    ) -> MarginfiResult<Vec<BankAccountWithCache<'a, 'info>>> {
        let mut account_index = 0;
        let active_balances: Vec<&Balance> = lending_account
            .balances
            .iter()
            .filter(|balance| balance.is_active())
            .collect();
        let banks_only = remaining_ais.len() == active_balances.len();

        active_balances
            .into_iter()
            .map(|balance| {
                let bank_ai: Option<&AccountInfo<'info>> = remaining_ais.get(account_index);
                if bank_ai.is_none() {
                    msg!("Ran out of remaining accounts at {:?}", account_index);
                    return err!(MarginfiError::InvalidBankAccount);
                }
                let bank_ai = bank_ai.unwrap();
                let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
                let bank = bank_al.load()?;

                let num_accounts = if banks_only {
                    1
                } else {
                    get_remaining_accounts_per_bank(&bank)?
                };
                check_eq!(
                    balance.bank_pk,
                    *bank_ai.key,
                    MarginfiError::InvalidBankAccount
                );

                if !banks_only {
                    let end_idx = account_index + num_accounts;
                    require_gte!(
                        remaining_ais.len(),
                        end_idx,
                        MarginfiError::WrongNumberOfOracleAccounts
                    );
                }

                account_index += num_accounts;

                Ok(BankAccountWithCache {
                    bank: bank_al.clone(),
                    balance,
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    fn write_liquidation_price_cache_from(
        &self,
        liq_cache: &LiquidationPriceCache,
        index: usize,
    ) -> MarginfiResult<()> {
        let mut bank = self.bank.load_mut()?;
        let zero_price = OraclePriceWithConfidence {
            price: I80F48::ZERO,
            confidence: I80F48::ZERO,
            source_time: 0,
        };
        let price_rt = liq_cache
            .get_price(OraclePriceType::RealTime, index)
            .unwrap_or(zero_price);
        let price_twap = liq_cache
            .get_price(OraclePriceType::TimeWeighted, index)
            .unwrap_or(zero_price);

        bank.cache.liquidation_price_rt = price_rt.price.into();
        bank.cache.liquidation_price_rt_confidence = price_rt.confidence.into();
        bank.cache.liquidation_price_twap = price_twap.price.into();
        bank.cache.liquidation_price_twap_confidence = price_twap.confidence.into();
        bank.cache.set_liquidation_price_cache_locked();

        Ok(())
    }

    #[inline]
    pub fn is_empty(&self, side: BalanceSide) -> bool {
        self.balance.is_empty(side)
    }
}

pub(crate) fn write_liquidation_price_cache_from<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    liq_cache: &LiquidationPriceCache,
) -> MarginfiResult<()> {
    let bank_accounts_with_cache =
        BankAccountWithCache::load(&marginfi_account.lending_account, remaining_ais)?;
    for (i, bank_account) in bank_accounts_with_cache.iter().enumerate() {
        bank_account.write_liquidation_price_cache_from(liq_cache, i)?;
    }
    Ok(())
}

fn get_cached_price_with_confidence(
    bank: &Bank,
    requirement_type: RequirementType,
) -> OraclePriceWithConfidence {
    match requirement_type.get_oracle_price_type() {
        OraclePriceType::RealTime => OraclePriceWithConfidence {
            price: bank.cache.liquidation_price_rt.into(),
            confidence: bank.cache.liquidation_price_rt_confidence.into(),
            // Cached prices are used for risk-engine math, not CB detection — source_time is
            // meaningful only inside `update_circuit_breaker`.
            source_time: 0,
        },
        OraclePriceType::TimeWeighted => OraclePriceWithConfidence {
            price: bank.cache.liquidation_price_twap.into(),
            confidence: bank.cache.liquidation_price_twap_confidence.into(),
            source_time: 0,
        },
    }
}

fn get_same_asset_weight_for_balance(
    balance: &Balance,
    bank: &Bank,
    requirement_type: RequirementType,
    reconciled_emode_config: &ReconciledEmodeConfig,
) -> Option<I80F48> {
    if balance.is_empty(BalanceSide::Assets)
        || !reconciled_emode_config.same_asset.is_enabled()
        || bank.mint != reconciled_emode_config.same_asset.mint
        || bank.config.oracle_keys[0] != reconciled_emode_config.same_asset.oracle_key
        || bank.config.oracle_setup.feed_family() != reconciled_emode_config.same_asset.feed_family
        || !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE)
        || !matches!(bank.config.risk_tier, RiskTier::Collateral)
        || matches!(
            (bank.config.operational_state, requirement_type),
            (BankOperationalState::ReduceOnly, RequirementType::Initial)
        )
    {
        return None;
    }

    Some(reconciled_emode_config.same_asset.asset_weight)
}

#[inline(always)]
fn calc_weighted_asset_value_cached_standalone(
    balance: &Balance,
    bank: &Bank,
    requirement_type: RequirementType,
    reconciled_emode_config: &ReconciledEmodeConfig,
) -> MarginfiResult<(I80F48, I80F48)> {
    match bank.config.risk_tier {
        RiskTier::Collateral => {
            if matches!(
                (bank.config.operational_state, requirement_type),
                (BankOperationalState::ReduceOnly, RequirementType::Initial)
            ) {
                debug!("ReduceOnly bank assets worth 0 for Initial margin");
                return Ok((I80F48::ZERO, I80F48::ZERO));
            }

            let mut asset_weight = bank
                .config
                .get_weight(requirement_type, BalanceSide::Assets);
            if let Some(emode_entry) = reconciled_emode_config.find_with_tag(bank.emode.emode_tag) {
                asset_weight = max(asset_weight, emode_entry.asset_weight);
            }
            if let Some(same_asset_weight) = get_same_asset_weight_for_balance(
                balance,
                bank,
                requirement_type,
                reconciled_emode_config,
            ) {
                asset_weight = max(asset_weight, same_asset_weight);
            }

            let price_with_confidence = get_cached_price_with_confidence(bank, requirement_type);
            let lower_price = apply_price_bias(price_with_confidence, PriceBias::Low)?;

            if matches!(requirement_type, RequirementType::Initial) {
                if let Some(discount) = bank.maybe_get_asset_weight_init_discount(lower_price)? {
                    asset_weight = asset_weight
                        .checked_mul(discount)
                        .ok_or_else(math_error!())?;
                }
            }
            let value = calc_value(
                bank.get_asset_amount(balance.asset_shares.into())?,
                lower_price,
                bank.get_balance_decimals(),
                Some(asset_weight),
            )?;

            Ok((value, lower_price))
        }
        RiskTier::Isolated => Ok((I80F48::ZERO, I80F48::ZERO)),
    }
}

#[inline(always)]
fn calc_weighted_liab_value_cached_standalone(
    balance: &Balance,
    bank: &Bank,
    requirement_type: RequirementType,
) -> MarginfiResult<(I80F48, I80F48)> {
    let liability_weight = bank
        .config
        .get_weight(requirement_type, BalanceSide::Liabilities);

    let price_with_confidence = get_cached_price_with_confidence(bank, requirement_type);
    let higher_price = apply_price_bias(price_with_confidence, PriceBias::High)?;

    let value = calc_value(
        bank.get_liability_amount(balance.liability_shares.into())?,
        higher_price,
        bank.get_balance_decimals(),
        Some(liability_weight),
    )?;

    Ok((value, higher_price))
}

#[inline(always)]
fn calc_weighted_value_cached_for_balance(
    balance: &Balance,
    bank: &Bank,
    requirement_type: RequirementType,
    reconciled_emode_config: &ReconciledEmodeConfig,
) -> MarginfiResult<(I80F48, I80F48, I80F48)> {
    match balance.get_side() {
        Some(side) => match side {
            BalanceSide::Assets => {
                let (value, price) = calc_weighted_asset_value_cached_standalone(
                    balance,
                    bank,
                    requirement_type,
                    reconciled_emode_config,
                )?;
                Ok((value, I80F48::ZERO, price))
            }
            BalanceSide::Liabilities => {
                let (value, price) =
                    calc_weighted_liab_value_cached_standalone(balance, bank, requirement_type)?;
                Ok((I80F48::ZERO, value, price))
            }
        },
        None => Ok((I80F48::ZERO, I80F48::ZERO, I80F48::ZERO)),
    }
}
/// Calculate the value of an asset, given its quantity with a decimal exponent, and a price with a decimal exponent, and an optional weight.
#[inline]
pub fn calc_value(
    amount: I80F48,
    price: I80F48,
    mint_decimals: u8,
    weight: Option<I80F48>,
) -> MarginfiResult<I80F48> {
    if amount == I80F48::ZERO {
        return Ok(I80F48::ZERO);
    }

    let scaling_factor = EXP_10_I80F48[mint_decimals as usize];

    let weighted_asset_amount = if let Some(weight) = weight {
        amount.checked_mul(weight).unwrap()
    } else {
        amount
    };

    #[cfg(target_os = "solana")]
    debug!(
        "weighted_asset_qt: {}, price: {}, expo: {}",
        weighted_asset_amount, price, mint_decimals
    );

    let value = weighted_asset_amount
        .checked_mul(price)
        .ok_or_else(math_error!())?
        .checked_div(scaling_factor)
        .ok_or_else(math_error!())?;

    Ok(value)
}

#[inline]
pub fn calc_amount(value: I80F48, price: I80F48, mint_decimals: u8) -> MarginfiResult<I80F48> {
    let scaling_factor = EXP_10_I80F48[mint_decimals as usize];

    let qt = value
        .checked_mul(scaling_factor)
        .ok_or_else(math_error!())?
        .checked_div(price)
        .ok_or_else(math_error!())?;

    Ok(qt)
}

// =============================================================================
// RISK ENGINE - HEAP-EFFICIENT HEALTH CALCULATION
// =============================================================================
//
// These functions provide the core risk engine functionality for marginfi accounts.
// They calculate account health, validate liquidation conditions, and enforce
// risk constraints.
//
// ## Public API
//
// - `check_account_init_health`     - Validates health after risky actions (borrow/withdraw)
// - `check_pre_liquidation_condition_and_get_account_health` - Pre-liquidation validation
// - `check_post_liquidation_condition_and_get_account_health` - Post-liquidation validation
// - `check_account_bankrupt`        - Bankruptcy condition check
// - `get_health_components`         - Core health calculation (assets vs liabilities)
//
// ## Heap Reuse Optimization
//
// All functions use the custom allocator's heap reuse feature (heap_pos/heap_restore)
// to process positions one at a time, keeping peak heap usage low. This enables
// support for up to 16 positions (MAX_LENDING_ACCOUNT_BALANCES) without exceeding
// the default 32 KiB heap limit or requiring requestHeapFrame.
//
// See allocator.rs for details on the heap reuse mechanism.
// =============================================================================

// -----------------------------------------------------------------------------
// Internal Helpers
// -----------------------------------------------------------------------------

/// Iterator that yields each liability balance's `EmodeConfig` from a lending account while
/// folding the same-asset accumulators in a single pass. Each `EmodeConfig` is ~400 bytes, so
/// yielding one at a time keeps peak stack usage manageable across the 16-position limit.
///
/// When `same_asset_leverage` is `Some`, `next()` also tracks the shared liability mint and the
/// running lowest liability-side weight; the post-iteration `reconcile()` folds those into the
/// returned `ReconciledEmodeConfig`.
struct EmodeConfigIterator<'a, 'info> {
    lending_account: &'a LendingAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    balance_index: usize,
    account_index: usize,
    banks_only: bool,
    requirement_type: RequirementType,
    same_asset_leverage: Option<I80F48>,
    shared_mint: Option<Pubkey>,
    shared_oracle_key: Option<Pubkey>,
    shared_feed_family: Option<OracleFeedFamily>,
    lowest_liab_weight: Option<I80F48>,
    same_asset_invalid: bool,
}

impl<'a, 'info> EmodeConfigIterator<'a, 'info> {
    fn new(
        lending_account: &'a LendingAccount,
        remaining_ais: &'info [AccountInfo<'info>],
        banks_only: bool,
        requirement_type: RequirementType,
        same_asset_leverage: Option<I80F48>,
    ) -> Self {
        Self {
            lending_account,
            remaining_ais,
            balance_index: 0,
            account_index: 0,
            banks_only,
            requirement_type,
            same_asset_leverage,
            shared_mint: None,
            shared_oracle_key: None,
            shared_feed_family: None,
            lowest_liab_weight: None,
            same_asset_invalid: false,
        }
    }

    /// Drives the iterator to completion via `reconcile_emode_configs`, then folds any tracked
    /// same-asset state into the reconciled config when same-asset emode is active and all active
    /// liabilities shared a single mint.
    fn reconcile(mut self) -> ReconciledEmodeConfig {
        let requirement_type = self.requirement_type;
        let mut reconciled = reconcile_emode_configs(&mut self, requirement_type);
        if let (
            Some(leverage),
            false,
            Some(mint),
            Some(oracle_key),
            Some(feed_family),
            Some(liab_weight),
        ) = (
            self.same_asset_leverage,
            self.same_asset_invalid,
            self.shared_mint,
            self.shared_oracle_key,
            self.shared_feed_family,
            self.lowest_liab_weight,
        ) {
            reconciled.same_asset.mint = mint;
            reconciled.same_asset.oracle_key = oracle_key;
            reconciled.same_asset.feed_family = Some(feed_family);
            reconciled.same_asset.asset_weight =
                compute_same_asset_emode_weight(leverage, liab_weight);
        }
        reconciled
    }
}

impl<'a, 'info> Iterator for EmodeConfigIterator<'a, 'info> {
    type Item = EmodeConfig;

    fn next(&mut self) -> Option<Self::Item> {
        while self.balance_index < self.lending_account.balances.len() {
            let balance = &self.lending_account.balances[self.balance_index];

            if !balance.is_active() {
                self.balance_index += 1;
                continue;
            }

            let bank_ai = self.remaining_ais.get(self.account_index)?;
            let bank_al = AccountLoader::<Bank>::try_from(bank_ai).ok()?;
            let bank = bank_al.load().ok()?;

            if balance.bank_pk != *bank_ai.key {
                return None;
            }

            let num_accounts = if self.banks_only {
                1
            } else {
                get_remaining_accounts_per_bank(&bank).ok()?
            };

            self.account_index += num_accounts;
            self.balance_index += 1;

            if !balance.is_empty(BalanceSide::Liabilities) {
                if self.same_asset_leverage.is_some() && !self.same_asset_invalid {
                    let liab_weight = bank
                        .config
                        .get_weight(self.requirement_type, BalanceSide::Liabilities);
                    if !update_reconciled_same_asset_config(
                        &mut self.shared_mint,
                        &mut self.shared_oracle_key,
                        &mut self.shared_feed_family,
                        &mut self.lowest_liab_weight,
                        &bank,
                        bank.mint,
                        liab_weight,
                    ) {
                        self.same_asset_invalid = true;
                    }
                }
                return Some(bank.emode.emode_config);
            }
        }
        None
    }
}

fn same_asset_leverage_for_requirement(
    requirement_type: RequirementType,
    group: &MarginfiGroup,
) -> Option<I80F48> {
    let leverage = match requirement_type {
        RequirementType::Initial => u32_to_basis(group.same_asset_emode_init_leverage),
        RequirementType::Maintenance => u32_to_basis(group.same_asset_emode_maint_leverage),
        RequirementType::Equity => return None,
    };

    (leverage > I80F48::ONE).then_some(leverage)
}

/// Folds one liability mint/weight into the running same-asset accumulators.
/// Returns `false` when any liability bank is ineligible, lacks a feed family (fixed-price,
/// deprecated, or unset oracle setup), is missing an oracle key, or diverges from a previously
/// seen mint/oracle-key/feed-family triple. Callers must stop folding on `false`.
fn update_reconciled_same_asset_config(
    shared_mint: &mut Option<Pubkey>,
    shared_oracle_key: &mut Option<Pubkey>,
    shared_feed_family: &mut Option<OracleFeedFamily>,
    lowest_liab_weight: &mut Option<I80F48>,
    bank: &Bank,
    mint: Pubkey,
    liab_weight: I80F48,
) -> bool {
    let feed_family = match bank.config.oracle_setup.feed_family() {
        Some(family) if bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE) => family,
        _ => {
            *lowest_liab_weight = None;
            return false;
        }
    };
    if bank.config.oracle_keys[0] == Pubkey::default() {
        *lowest_liab_weight = None;
        return false;
    }

    let oracle_key = bank.config.oracle_keys[0];
    match shared_mint {
        Some(existing_mint)
            if *existing_mint != mint
                || shared_oracle_key.as_ref() != Some(&oracle_key)
                || shared_feed_family.as_ref() != Some(&feed_family) =>
        {
            *lowest_liab_weight = None;
            false
        }
        Some(_) => {
            if lowest_liab_weight.is_none_or(|existing| liab_weight < existing) {
                *lowest_liab_weight = Some(liab_weight);
            }
            true
        }
        None => {
            *shared_mint = Some(mint);
            *shared_oracle_key = Some(oracle_key);
            *shared_feed_family = Some(feed_family);
            *lowest_liab_weight = Some(liab_weight);
            true
        }
    }
}

// -----------------------------------------------------------------------------
// Public API - Risk Engine Functions
// -----------------------------------------------------------------------------

/// Calculates account health components with heap reuse optimization.
///
/// This function processes each balance position one at a time, using heap
/// checkpoints to recycle memory between positions. This keeps peak heap
/// usage low enough to handle up to 16 positions without `requestHeapFrame`.
///
/// ## Memory Pattern
///
/// Without heap reuse: O(N) heap where N = number of positions
/// With heap reuse: O(1) heap (memory recycled per position)
///
/// ## Parameters
///
/// - `marginfi_account`: The account to calculate health for
/// - `group`: The group whose same-asset auto-emode settings apply to this account
/// - `remaining_ais`: Remaining accounts containing banks and oracles
/// - `requirement_type`: Initial, Maintenance, or Equity requirement
/// - `health_cache`: Optional cache to populate with results
///
/// ## Returns
///
/// (total_assets, total_liabilities) weighted according to requirement_type
pub fn get_health_components<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    remaining_ais: &'info [AccountInfo<'info>],
    requirement_type: RequirementType,
    health_cache: &mut Option<&mut HealthCache>,
    price_mode: HealthPriceMode<'_>,
) -> MarginfiResult<(I80F48, I80F48)> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    let (is_cached, mut liq_cache, clock) = match price_mode {
        HealthPriceMode::Live { liq_cache } => (false, liq_cache, Some(Clock::get()?)),
        HealthPriceMode::Cached => (true, None, None),
        HealthPriceMode::Client(clock) => (false, None, Some(clock)),
    };

    let lending_account = &marginfi_account.lending_account;

    // =========================================================================
    // Phase 1: Reconcile emode configuration (incl. same-asset) with heap reuse
    // =========================================================================

    let same_asset_leverage = same_asset_leverage_for_requirement(requirement_type, group);
    let emode_checkpoint = heap_pos();
    let reconciled_emode_config = EmodeConfigIterator::new(
        lending_account,
        remaining_ais,
        is_cached,
        requirement_type,
        same_asset_leverage,
    )
    .reconcile();
    heap_restore(emode_checkpoint);

    // =========================================================================
    // Phase 2: Calculate health with heap reuse per position
    // =========================================================================

    let mut total_assets: I80F48 = I80F48::ZERO;
    let mut total_liabilities: I80F48 = I80F48::ZERO;
    const NO_INDEX_FOUND: usize = 255;
    let mut first_err_index = NO_INDEX_FOUND;
    let mut account_index = 0usize;

    for (position_index, balance) in lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
        .enumerate()
    {
        let heap_checkpoint = heap_pos();

        // Load bank
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;

        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );

        let num_accounts = if is_cached {
            check!(
                bank.cache.is_liquidation_price_cache_locked(),
                MarginfiError::InternalLogicError
            );
            1
        } else {
            get_remaining_accounts_per_bank(&bank)?
        };

        let (asset_val, liab_val, price, err_code) = if is_cached {
            let (asset_val, liab_val, price) = calc_weighted_value_cached_for_balance(
                balance,
                &bank,
                requirement_type,
                &reconciled_emode_config,
            )?;
            (asset_val, liab_val, price, 0)
        } else {
            // Load oracle (this is the heap-intensive operation)
            let oracle_ai_idx = account_index + 1;
            let end_idx = oracle_ai_idx + num_accounts - 1;
            require_gte!(
                remaining_ais.len(),
                end_idx,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            let oracle_ais = &remaining_ais[oracle_ai_idx..end_idx];

            // Create oracle adapter (heap allocation happens here)
            let price_adapter_result =
                OraclePriceFeedAdapter::try_from_bank(&bank, oracle_ais, clock.as_ref().unwrap());

            // Log heap usage per position for measurement/debugging
            // Measured results: Pyth ~64 bytes, Switchboard ~128 bytes per position
            #[cfg(target_os = "solana")]
            {
                let heap_after_oracle = heap_pos();
                let _heap_used = heap_after_oracle.saturating_sub(heap_checkpoint);
                debug!(
                    "HEAP_MEASURE: position={} heap_used={} bytes",
                    position_index, _heap_used
                );
            }

            // Calculate weighted value for this position
            calc_weighted_value_for_balance(
                balance,
                &bank,
                &price_adapter_result,
                requirement_type,
                &reconciled_emode_config,
                &mut liq_cache,
                position_index,
            )?
        };

        // Record error index if applicable
        if err_code != 0 && first_err_index == NO_INDEX_FOUND {
            first_err_index = position_index;
            if let Some(cache) = health_cache.as_mut() {
                cache.err_index = position_index as u8;
                cache.internal_err = err_code;
            }
        }

        // Update health cache with price
        if let Some(cache) = health_cache.as_mut() {
            if let RequirementType::Initial = requirement_type {
                cache.prices[position_index] = price.to_num::<f64>().to_le_bytes();
            }
        }

        debug!(
            "Balance {}, assets: {}, liabilities: {}",
            balance.bank_pk, asset_val, liab_val
        );

        // Accumulate totals (stack variables, survive heap restore)
        total_assets = total_assets
            .checked_add(asset_val)
            .ok_or_else(math_error!())?;
        total_liabilities = total_liabilities
            .checked_add(liab_val)
            .ok_or_else(math_error!())?;

        account_index += num_accounts;
        heap_restore(heap_checkpoint);
    }

    // Update health cache totals
    if let Some(cache) = health_cache.as_mut() {
        match requirement_type {
            RequirementType::Initial => {
                cache.asset_value = total_assets.into();
                cache.liability_value = total_liabilities.into();
            }
            RequirementType::Maintenance => {
                cache.asset_value_maint = total_assets.into();
                cache.liability_value_maint = total_liabilities.into();
            }
            RequirementType::Equity => {
                cache.asset_value_equity = total_assets.into();
                cache.liability_value_equity = total_liabilities.into();
            }
        }
    }

    Ok((total_assets, total_liabilities))
}

/// Returns the total assets and liabilities restricted to the provided set of balance tags.
/// Equivalent to computing the equity health of just the balances with matching tags.
/// * If tags are empty or not found, returns (0, 0, 0, 0)
pub fn get_tagged_account_health_components<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    balance_tags: &[u16],
) -> MarginfiResult<(I80F48, I80F48, usize, usize)> {
    if balance_tags.is_empty() {
        return Ok((I80F48::ZERO, I80F48::ZERO, 0, 0));
    }

    let lending_account = &marginfi_account.lending_account;
    let clock = Clock::get()?;

    let emode_checkpoint = heap_pos();
    let reconciled_emode_config = EmodeConfigIterator::new(
        lending_account,
        remaining_ais,
        false,
        RequirementType::Equity,
        None,
    )
    .reconcile();
    heap_restore(emode_checkpoint);

    let requirement_type = RequirementType::Equity;
    let mut total_assets: I80F48 = I80F48::ZERO;
    let mut total_liabilities: I80F48 = I80F48::ZERO;
    let mut asset_count = 0;
    let mut liab_count = 0;

    let mut account_index = 0usize;
    for (position_index, balance) in lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
        .enumerate()
    {
        let heap_checkpoint = heap_pos();

        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;

        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );

        let num_accounts = get_remaining_accounts_per_bank(&bank)?;

        if !balance_tags.contains(&balance.tag) {
            account_index += num_accounts;
            heap_restore(heap_checkpoint);
            continue;
        }

        let oracle_ai_idx = account_index + 1;
        let end_idx = oracle_ai_idx + num_accounts - 1;
        require_gte!(
            remaining_ais.len(),
            end_idx,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        let oracle_ais = &remaining_ais[oracle_ai_idx..end_idx];

        let (asset_val, liab_val) = {
            let price_adapter_result =
                OraclePriceFeedAdapter::try_from_bank(&bank, oracle_ais, &clock);

            let (asset_val, liab_val, _price, _err_code) = calc_weighted_value_for_balance(
                balance,
                &bank,
                &price_adapter_result,
                requirement_type,
                &reconciled_emode_config,
                &mut None,
                position_index,
            )?;
            (asset_val, liab_val)
        };

        match balance.get_side() {
            Some(BalanceSide::Assets) => asset_count += 1,
            Some(BalanceSide::Liabilities) => liab_count += 1,
            None => {}
        }

        total_assets = total_assets
            .checked_add(asset_val)
            .ok_or_else(math_error!())?;
        total_liabilities = total_liabilities
            .checked_add(liab_val)
            .ok_or_else(math_error!())?;

        account_index += num_accounts;
        heap_restore(heap_checkpoint);
    }

    Ok((total_assets, total_liabilities, asset_count, liab_count))
}

/// Check pre-liquidation condition with heap reuse optimization.
///
/// Uses heap reuse to process positions one at a time, enabling support for accounts
/// with up to 16 positions.
///
/// Returns (account_health, assets, liabilities) if the account is liquidatable.
pub fn check_pre_liquidation_condition_and_get_account_health<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    remaining_ais: &'info [AccountInfo<'info>],
    liability_bank_pk: Option<&Pubkey>,
    health_cache: &mut Option<&mut HealthCache>,
    price_mode: HealthPriceMode<'_>,
    ignore_healthy: bool,
) -> MarginfiResult<(I80F48, I80F48, I80F48)> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    if let Some(bank_pk) = liability_bank_pk {
        let lending_account = &marginfi_account.lending_account;
        let liability_balance = lending_account
            .balances
            .iter()
            .find(|b| b.is_active() && b.bank_pk == *bank_pk)
            .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

        check!(
            !liability_balance.is_empty(BalanceSide::Liabilities),
            MarginfiError::NoLiabilitiesInLiabilityBank
        );

        check!(
            liability_balance.is_empty(BalanceSide::Assets),
            MarginfiError::AssetsInLiabilityBank
        );
    }

    // Get health components using heap reuse
    let (assets, liabs) = get_health_components(
        marginfi_account,
        group,
        remaining_ais,
        RequirementType::Maintenance,
        health_cache,
        price_mode,
    )?;

    let account_health = assets.checked_sub(liabs).ok_or_else(math_error!())?;
    let healthy = account_health > I80F48::ZERO;

    if let Some(cache) = health_cache.as_mut() {
        cache.set_healthy(healthy);
    }

    if healthy && !ignore_healthy {
        msg!(
            "pre_liquidation_health: {} ({} - {})",
            account_health,
            assets,
            liabs
        );
        return err!(MarginfiError::HealthyAccount);
    }

    Ok((account_health, assets, liabs))
}

/// Check bankruptcy condition with heap reuse optimization.
///
/// Uses heap reuse to process positions one at a time.
pub fn check_account_bankrupt<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    remaining_ais: &'info [AccountInfo<'info>],
    health_cache: &mut Option<&mut HealthCache>,
) -> MarginfiResult {
    let (equity_assets, equity_liabs) = get_health_components(
        marginfi_account,
        group,
        remaining_ais,
        RequirementType::Equity,
        health_cache,
        HealthPriceMode::Live { liq_cache: None },
    )?;

    let has_liabilities = equity_liabs > I80F48::ZERO;
    let below_bankruptcy_threshold = equity_assets < BANKRUPT_THRESHOLD;
    let liabilities_exceed_assets = equity_liabs > equity_assets;
    let is_bankrupt = has_liabilities && below_bankruptcy_threshold && liabilities_exceed_assets;

    if !is_bankrupt {
        return err!(MarginfiError::AccountNotBankrupt);
    }

    Ok(())
}

/// Computes `indexer_flags.has_isolated` from live bank risk tiers and current liabilities.
///
/// Returns 1 iff the account has any isolated-tier liability.
pub fn compute_has_isolated_liability_flag<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<u8> {
    let mut has_isolated_liability = false;
    let mut account_index = 0usize;

    for balance in marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
    {
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;

        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );

        let num_accounts = get_remaining_accounts_per_bank(&bank)?;

        if !balance.is_empty(BalanceSide::Liabilities)
            && bank.config.risk_tier == RiskTier::Isolated
        {
            has_isolated_liability = true;
        }

        account_index += num_accounts;
    }

    Ok(has_isolated_liability as u8)
}

/// Check the isolated-risk-tier constraint (internal helper).
fn check_account_risk_tiers<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult {
    let mut isolated_risk_count = 0;
    let mut total_liability_balances = 0;

    let mut account_index = 0usize;
    for balance in marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
    {
        // Load bank to read risk tier and remaining account count
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;

        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );

        let num_accounts = get_remaining_accounts_per_bank(&bank)?;

        if !balance.is_empty(BalanceSide::Liabilities) {
            total_liability_balances += 1;
            if bank.config.risk_tier == RiskTier::Isolated {
                isolated_risk_count += 1;
                if isolated_risk_count > 1 {
                    break;
                }
            }
        }

        account_index += num_accounts;
    }

    check!(
        isolated_risk_count == 0 || total_liability_balances == 1,
        MarginfiError::IsolatedAccountIllegalState
    );

    Ok(())
}

pub fn clear_liquidation_price_cache_locks<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<()> {
    let bank_accounts_with_cache =
        BankAccountWithCache::load(&marginfi_account.lending_account, remaining_ais)?;

    for account in bank_accounts_with_cache.iter() {
        let mut bank = account.bank.load_mut()?;
        bank.cache.clear_liquidation_price_cache_locked();
    }
    Ok(())
}

/// Initial health check using the heap-reuse health calculator.
///
/// - Skips risk checks when the account is in a flashloan
/// - Enforces isolated-tier constraint
/// - Errors if initial health is negative
pub fn check_account_init_health<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    remaining_ais: &'info [AccountInfo<'info>],
    health_cache: &mut Option<&mut HealthCache>,
) -> MarginfiResult {
    if marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN) {
        // Risk checks are skipped during flashloans
        return Ok(());
    }

    let (assets, liabs) = get_health_components(
        marginfi_account,
        group,
        remaining_ais,
        RequirementType::Initial,
        health_cache,
        HealthPriceMode::Live { liq_cache: None },
    )?;

    let healthy = assets >= liabs;
    if let Some(cache) = health_cache.as_mut() {
        cache.set_healthy(healthy);
    }

    if !healthy {
        return err!(MarginfiError::RiskEngineInitRejected);
    }

    check_account_risk_tiers(marginfi_account, remaining_ais)
}

/// Post-liquidation invariant using the heap-reuse health calculator.
///
/// - Liability bank must still have outstanding liabilities and no assets
/// - Post-maintenance health must remain <= 0
/// - Post-maintenance health must improve relative to pre-liquidation health
pub fn check_post_liquidation_condition_and_get_account_health<'info>(
    marginfi_account: &MarginfiAccount,
    group: &MarginfiGroup,
    remaining_ais: &'info [AccountInfo<'info>],
    bank_pk: &Pubkey,
    pre_liquidation_health: I80F48,
) -> MarginfiResult<I80F48> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    let liability_balance = marginfi_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == *bank_pk)
        .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

    check!(
        !liability_balance.is_empty(BalanceSide::Liabilities),
        MarginfiError::ExhaustedLiability
    );

    check!(
        liability_balance.is_empty(BalanceSide::Assets),
        MarginfiError::TooSeverePayoff
    );

    let (assets, liabs) = get_health_components(
        marginfi_account,
        group,
        remaining_ais,
        RequirementType::Maintenance,
        &mut None,
        HealthPriceMode::Live { liq_cache: None },
    )?;

    let account_health = assets.checked_sub(liabs).ok_or_else(math_error!())?;

    check!(
        account_health <= I80F48::ZERO,
        MarginfiError::TooSevereLiquidation
    );

    if account_health <= pre_liquidation_health {
        msg!(
            "post_liquidation_health: {} ({} - {}), pre_liquidation_health: {}",
            account_health,
            assets,
            liabs,
            pre_liquidation_health
        );
        return err!(MarginfiError::WorseHealthPostLiquidation);
    };

    Ok(account_health)
}

/// Helper function to calculate weighted value for a single balance.
///
/// Calculates asset or liability value with appropriate weights and price biases.
#[inline(always)]
fn calc_weighted_value_for_balance(
    balance: &Balance,
    bank: &Bank,
    price_adapter_result: &MarginfiResult<OraclePriceFeedAdapter>,
    requirement_type: RequirementType,
    reconciled_emode_config: &ReconciledEmodeConfig,
    liq_cache: &mut Option<&mut LiquidationPriceCache>,
    position_index: usize,
) -> MarginfiResult<(I80F48, I80F48, I80F48, u32)> {
    match balance.get_side() {
        Some(side) => match side {
            BalanceSide::Assets => {
                let (value, price, err_code) = calc_weighted_asset_value_standalone(
                    balance,
                    bank,
                    price_adapter_result,
                    requirement_type,
                    reconciled_emode_config,
                    liq_cache,
                    position_index,
                )?;
                Ok((value, I80F48::ZERO, price, err_code))
            }
            BalanceSide::Liabilities => {
                let (value, price) = calc_weighted_liab_value_standalone(
                    balance,
                    bank,
                    price_adapter_result,
                    requirement_type,
                    liq_cache,
                    position_index,
                )?;
                Ok((I80F48::ZERO, value, price, 0))
            }
        },
        None => Ok((I80F48::ZERO, I80F48::ZERO, I80F48::ZERO, 0)),
    }
}

/// Calculate weighted asset value (standalone version for heap reuse).
#[inline(always)]
fn calc_weighted_asset_value_standalone(
    balance: &Balance,
    bank: &Bank,
    price_adapter_result: &MarginfiResult<OraclePriceFeedAdapter>,
    requirement_type: RequirementType,
    reconciled_emode_config: &ReconciledEmodeConfig,
    liq_cache: &mut Option<&mut LiquidationPriceCache>,
    position_index: usize,
) -> MarginfiResult<(I80F48, I80F48, u32)> {
    match bank.config.risk_tier {
        RiskTier::Collateral => {
            // ReduceOnly banks should not be counted as collateral for Initial checks
            if matches!(
                (bank.config.operational_state, requirement_type),
                (BankOperationalState::ReduceOnly, RequirementType::Initial)
            ) {
                debug!("ReduceOnly bank assets worth 0 for Initial margin");
                return Ok((I80F48::ZERO, I80F48::ZERO, 0));
            }

            // Extract error code if oracle failed
            let err_code = match price_adapter_result {
                Ok(_) => 0,
                Err(e) => match e {
                    anchor_lang::error::Error::AnchorError(inner) => {
                        inner.as_ref().error_code_number
                    }
                    anchor_lang::error::Error::ProgramError(inner) => {
                        match inner.as_ref().program_error {
                            ProgramError::Custom(code) => code,
                            _ => MarginfiError::InternalLogicError as u32,
                        }
                    }
                },
            };

            // Skip stale oracles for Initial requirement
            if matches!(
                (price_adapter_result, requirement_type),
                (&Err(_), RequirementType::Initial)
            ) {
                debug!("Skipping stale oracle");
                return Ok((I80F48::ZERO, I80F48::ZERO, err_code));
            }

            let price_feed = price_adapter_result
                .as_ref()
                .map_err(|_| error!(MarginfiError::from(err_code)))?;

            // Determine asset weight (emode or bank default)
            let mut asset_weight = bank
                .config
                .get_weight(requirement_type, BalanceSide::Assets);
            if let Some(emode_entry) = reconciled_emode_config.find_with_tag(bank.emode.emode_tag) {
                asset_weight = max(asset_weight, emode_entry.asset_weight);
            }
            if let Some(same_asset_weight) = get_same_asset_weight_for_balance(
                balance,
                bank,
                requirement_type,
                reconciled_emode_config,
            ) {
                asset_weight = max(asset_weight, same_asset_weight);
            }

            let lower_price = if let Some(cache) = liq_cache.as_mut() {
                let price_with_confidence = price_feed.get_price_and_confidence_of_type(
                    requirement_type.get_oracle_price_type(),
                    bank.config.oracle_max_confidence,
                )?;
                cache.record(requirement_type, position_index, price_with_confidence);
                apply_price_bias(price_with_confidence, PriceBias::Low)?
            } else {
                price_feed.get_price_of_type(
                    requirement_type.get_oracle_price_type(),
                    Some(PriceBias::Low),
                    bank.config.oracle_max_confidence,
                )?
            };

            // Apply initial discount if applicable
            if matches!(requirement_type, RequirementType::Initial) {
                if let Some(discount) = bank.maybe_get_asset_weight_init_discount(lower_price)? {
                    asset_weight = asset_weight
                        .checked_mul(discount)
                        .ok_or_else(math_error!())?;
                }
            }

            let value = calc_value(
                bank.get_asset_amount(balance.asset_shares.into())?,
                lower_price,
                bank.get_balance_decimals(),
                Some(asset_weight),
            )?;

            Ok((value, lower_price, 0))
        }
        RiskTier::Isolated => Ok((I80F48::ZERO, I80F48::ZERO, 0)),
    }
}

/// Calculate weighted liability value (standalone version for heap reuse).
#[inline(always)]
fn calc_weighted_liab_value_standalone(
    balance: &Balance,
    bank: &Bank,
    price_adapter_result: &MarginfiResult<OraclePriceFeedAdapter>,
    requirement_type: RequirementType,
    liq_cache: &mut Option<&mut LiquidationPriceCache>,
    position_index: usize,
) -> MarginfiResult<(I80F48, I80F48)> {
    // Propagate the original oracle error (e.g., PythPushStalePrice, SwitchboardStalePrice)
    let price_feed = match price_adapter_result {
        Ok(adapter) => adapter,
        Err(e) => {
            // Extract error code and re-create the error to propagate it
            let err_code = match e {
                anchor_lang::error::Error::AnchorError(inner) => inner.as_ref().error_code_number,
                anchor_lang::error::Error::ProgramError(inner) => {
                    match inner.as_ref().program_error {
                        ProgramError::Custom(code) => code,
                        _ => MarginfiError::InvalidOracleSetup as u32,
                    }
                }
            };
            return Err(error!(MarginfiError::from(err_code)));
        }
    };

    let liability_weight = bank
        .config
        .get_weight(requirement_type, BalanceSide::Liabilities);

    let higher_price = if let Some(cache) = liq_cache.as_mut() {
        let price_with_confidence = price_feed.get_price_and_confidence_of_type(
            requirement_type.get_oracle_price_type(),
            bank.config.oracle_max_confidence,
        )?;
        cache.record(requirement_type, position_index, price_with_confidence);
        apply_price_bias(price_with_confidence, PriceBias::High)?
    } else {
        price_feed.get_price_of_type(
            requirement_type.get_oracle_price_type(),
            Some(PriceBias::High),
            bank.config.oracle_max_confidence,
        )?
    };

    let value = calc_value(
        bank.get_liability_amount(balance.liability_shares.into())?,
        higher_price,
        bank.get_balance_decimals(),
        Some(liability_weight),
    )?;

    Ok((value, higher_price))
}

pub trait LendingAccountImpl {
    fn get_first_empty_balance(&self) -> Option<usize>;
    fn sort_balances(&mut self);
    fn reserve_n_tags(&mut self, n: usize) -> [u16; ORDER_ACTIVE_TAGS];
    fn get_balance_index(&self, bank_pk: &Pubkey) -> MarginfiResult<usize>;
    fn has_liabilities(&self) -> bool;
}

impl LendingAccountImpl for LendingAccount {
    fn get_first_empty_balance(&self) -> Option<usize> {
        self.balances.iter().position(|b| !b.is_active())
    }

    /// True if any active balance carries a liability. A withdraw from an account with no
    /// liabilities is risk-free and stays allowed during a circuit-breaker halt.
    fn has_liabilities(&self) -> bool {
        self.balances
            .iter()
            .any(|b| b.is_active() && !b.is_empty(BalanceSide::Liabilities))
    }

    fn sort_balances(&mut self) {
        // Sort all balances in descending order by bank_pk
        self.balances.sort_by(|a, b| b.bank_pk.cmp(&a.bank_pk));
    }

    /// Finds n free tags for new orders, starting with newer ones first
    /// n is expected to be <= [`ORDER_ACTIVE_TAGS`].
    /// It fills only the first n, leaving the rest as 0.
    fn reserve_n_tags(&mut self, n: usize) -> [u16; ORDER_ACTIVE_TAGS] {
        assert!(n <= ORDER_ACTIVE_TAGS, "Invalid tag count");

        let used: BTreeSet<u16> = self
            .balances
            .iter()
            .filter(|b| b.is_active() && b.tag != 0)
            .map(|b| b.tag)
            .collect();

        let mut tags = [0u16; ORDER_ACTIVE_TAGS];

        let mut next = self.last_tag_used.wrapping_add(1);

        let mut filled = 0;

        while filled < n {
            if next == 0 {
                next = 1;
            }

            if !used.contains(&next) {
                tags[filled] = next;
                filled += 1;
            }

            next = next.wrapping_add(1);
        }

        if n > 0 {
            self.last_tag_used = tags[n - 1];
        }

        tags
    }

    fn get_balance_index(&self, bank_pk: &Pubkey) -> MarginfiResult<usize> {
        let index = self
            .balances
            .binary_search_by(|balance| bank_pk.cmp(&balance.bank_pk))
            .ok()
            .and_then(|index| self.balances[index].is_active().then_some(index))
            .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

        Ok(index)
    }
}

pub trait BalanceImpl {
    fn change_asset_shares(&mut self, delta: I80F48) -> MarginfiResult;
    fn change_liability_shares(&mut self, delta: I80F48) -> MarginfiResult;
    fn close(&mut self) -> MarginfiResult;
}

impl BalanceImpl for Balance {
    fn change_asset_shares(&mut self, delta: I80F48) -> MarginfiResult {
        let asset_shares: I80F48 = self.asset_shares.into();
        self.asset_shares = asset_shares
            .checked_add(delta)
            .ok_or_else(math_error!())?
            .into();
        Ok(())
    }

    fn change_liability_shares(&mut self, delta: I80F48) -> MarginfiResult {
        let liability_shares: I80F48 = self.liability_shares.into();
        self.liability_shares = liability_shares
            .checked_add(delta)
            .ok_or_else(math_error!())?
            .into();
        Ok(())
    }

    fn close(&mut self) -> MarginfiResult {
        *self = Self::empty_deactivated();

        Ok(())
    }
}

pub struct BankAccountWrapper<'a> {
    pub balance: &'a mut Balance,
    pub bank: &'a mut Bank,
}

impl<'a> BankAccountWrapper<'a> {
    // Find existing user lending account balance by bank address.
    pub fn find(
        bank_pk: &Pubkey,
        bank: &'a mut Bank,
        lending_account: &'a mut LendingAccount,
    ) -> MarginfiResult<BankAccountWrapper<'a>> {
        let balance = lending_account
            .balances
            .iter_mut()
            .find(|balance| balance.is_active() && balance.bank_pk.eq(bank_pk))
            .ok_or_else(|| error!(MarginfiError::BankAccountNotFound))?;

        Ok(Self { balance, bank })
    }

    // Find existing user lending account balance by bank address.
    // Create it if not found.
    pub fn find_or_create(
        bank_pk: &Pubkey,
        bank: &'a mut Bank,
        lending_account: &'a mut LendingAccount,
    ) -> MarginfiResult<BankAccountWrapper<'a>> {
        let balance_index = lending_account
            .balances
            .iter()
            .position(|balance| balance.is_active() && balance.bank_pk.eq(bank_pk));

        match balance_index {
            Some(balance_index) => {
                let balance = lending_account
                    .balances
                    .get_mut(balance_index)
                    .ok_or_else(|| error!(MarginfiError::BankAccountNotFound))?;

                Ok(Self { balance, bank })
            }
            None => {
                // Enforce integration position limit before creating a new integration position
                if is_integration_asset_tag(bank.config.asset_tag) {
                    let integration_position_count = lending_account
                        .balances
                        .iter()
                        .filter(|b| b.is_active() && is_integration_asset_tag(b.bank_asset_tag))
                        .count();

                    // Note: this check is disabled in local integration tests so that we can measure the performance and
                    // eventually get rid of this limit altogether.
                    if live!() {
                        check!(
                            integration_position_count < MAX_INTEGRATION_POSITIONS,
                            MarginfiError::IntegrationPositionLimitExceeded
                        );
                    }
                }
                let empty_index = lending_account
                    .get_first_empty_balance()
                    .ok_or_else(|| error!(MarginfiError::LendingAccountBalanceSlotsFull))?;

                lending_account.balances[empty_index] = Balance {
                    active: 1,
                    bank_pk: *bank_pk,
                    bank_asset_tag: bank.config.asset_tag,
                    tag: 0,
                    _pad0: [0; 4],
                    asset_shares: I80F48::ZERO.into(),
                    liability_shares: I80F48::ZERO.into(),
                    emissions_outstanding: I80F48::ZERO.into(),
                    last_update: Clock::get()?.unix_timestamp as u64,
                    _padding: [0; 1],
                };

                Ok(Self {
                    balance: lending_account.balances.get_mut(empty_index).unwrap(),
                    bank,
                })
            }
        }
    }

    // ------------ Borrow / Lend primitives

    /// Deposit an asset, will error if this repays a liability instead of increasing a asset.
    /// Returns the asset share delta minted.
    pub fn deposit(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.increase_balance_internal(amount, BalanceIncreaseType::DepositOnly)
    }

    /// Deposit an asset, ignoring repayment of liabilities. Useful only for banks where borrowing is disabled.
    /// Returns the asset share delta minted.
    pub fn deposit_no_repay(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.increase_balance_internal(amount, BalanceIncreaseType::DepositOnly)
    }

    /// Repay a liability, will error if there is not enough liability - depositing is not allowed.
    /// Returns the liability share delta burned.
    pub fn repay(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.increase_balance_internal(amount, BalanceIncreaseType::RepayOnly)
    }

    /// Withdraw an asset, will error if there is not enough asset - borrowing is not allowed.
    /// Returns the asset share delta burned.
    pub fn withdraw(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.decrease_balance_internal(amount, BalanceDecreaseType::WithdrawOnly)
    }

    /// Incur a borrow, will error if this withdraws an asset instead of increasing a liability.
    /// Returns the liability share delta minted.
    pub fn borrow(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.decrease_balance_internal(amount, BalanceDecreaseType::BorrowOnly)
    }

    /// Deposit an asset, ignoring deposit caps, will error if this repays a liability instead of increasing a asset.
    /// Returns the asset share delta minted. Note: in the bypass/flip case (liability -> asset) only the
    /// asset side is reported, not any liability shares burned, so don't use this return value for events.
    pub fn deposit_ignore_deposit_cap(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.increase_balance_internal(amount, BalanceIncreaseType::BypassDepositLimit)
    }

    /// Incur a borrow, ignoring borrow caps, will error if this withdraws an asset instead of increasing a liability.
    /// Returns the liability share delta minted. Note: in the bypass/flip case (asset -> liability) only the
    /// liability side is reported, not any asset shares burned, so don't use this return value for events.
    pub fn withdraw_ignore_borrow_cap(&mut self, amount: I80F48) -> MarginfiResult<I80F48> {
        self.decrease_balance_internal(amount, BalanceDecreaseType::BypassBorrowLimit)
    }

    /// Withdraw existing asset in full - will error if there is no asset.
    /// When `in_receivership` is true, clears the bank's liquidation price cache lock
    /// so that banks whose balances are closed mid-liquidation don't stay permanently locked.
    /// Returns `(spl_withdraw_amount, asset_share_delta)`.
    pub fn withdraw_all(&mut self, in_receivership: bool) -> MarginfiResult<(u64, I80F48)> {
        let balance = &mut self.balance;
        let bank = &mut self.bank;

        let total_asset_shares: I80F48 = balance.asset_shares.into();
        let current_asset_amount = bank.get_asset_amount(total_asset_shares)?;
        let current_liability_amount =
            bank.get_liability_amount(balance.liability_shares.into())?;

        debug!("Withdrawing all: {}", current_asset_amount);

        check!(
            current_asset_amount.is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::NoAssetFound
        );

        check!(
            current_liability_amount.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::NoAssetFound
        );

        balance.close()?;

        // Only clear the lock when this account is actually in receivership.
        // The lock is bank-level global state, so clearing it unconditionally
        // would affect unrelated accounts sharing the same bank.
        if in_receivership {
            bank.cache.clear_liquidation_price_cache_locked();
        }

        bank.decrement_lending_position_count();
        bank.change_asset_shares(-total_asset_shares, false)?;
        bank.check_utilization_ratio()?;

        let spl_withdraw_amount = current_asset_amount
            .checked_floor()
            .ok_or_else(math_error!())?;

        bank.collected_insurance_fees_outstanding = {
            current_asset_amount
                .checked_sub(spl_withdraw_amount)
                .ok_or_else(math_error!())?
                .checked_add(bank.collected_insurance_fees_outstanding.into())
                .ok_or_else(math_error!())?
                .into()
        };

        let spl_withdraw_amount = spl_withdraw_amount
            .checked_to_num()
            .ok_or_else(math_error!())?;

        Ok((spl_withdraw_amount, total_asset_shares))
    }

    /// Repay existing liability in full - will error if there is no liability.
    /// When `in_receivership` is true, clears the bank's liquidation price cache lock
    /// so that banks whose balances are closed mid-liquidation don't stay permanently locked.
    /// Returns `(spl_repay_amount, liability_share_delta)`.
    pub fn repay_all(&mut self, in_receivership: bool) -> MarginfiResult<(u64, I80F48)> {
        let balance = &mut self.balance;
        let bank = &mut self.bank;

        let total_liability_shares: I80F48 = balance.liability_shares.into();
        let current_liability_amount = bank.get_liability_amount(total_liability_shares)?;
        let current_asset_amount = bank.get_asset_amount(balance.asset_shares.into())?;

        debug!("Repaying all: {}", current_liability_amount,);

        check!(
            current_liability_amount.is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::NoLiabilityFound
        );

        check!(
            current_asset_amount.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::NoLiabilityFound
        );

        balance.close()?;

        // Only clear the lock when this account is actually in receivership.
        // The lock is bank-level global state, so clearing it unconditionally
        // would affect unrelated accounts sharing the same bank.
        if in_receivership {
            bank.cache.clear_liquidation_price_cache_locked();
        }

        bank.decrement_borrowing_position_count();
        bank.change_liability_shares(-total_liability_shares, false)?;

        let spl_deposit_amount = current_liability_amount
            .checked_ceil()
            .ok_or_else(math_error!())?;

        bank.collected_insurance_fees_outstanding = {
            spl_deposit_amount
                .checked_sub(current_liability_amount)
                .ok_or_else(math_error!())?
                .checked_add(bank.collected_insurance_fees_outstanding.into())
                .ok_or_else(math_error!())?
                .into()
        };

        let spl_repay_amount = spl_deposit_amount
            .checked_to_num()
            .ok_or_else(math_error!())?;

        Ok((spl_repay_amount, total_liability_shares))
    }

    /// When `in_receivership` is true, clears the bank's liquidation price cache lock
    /// so that banks whose balances are closed mid-liquidation don't stay permanently locked.
    pub fn close_balance(&mut self, in_receivership: bool) -> MarginfiResult<()> {
        let balance = &mut self.balance;
        let bank = &mut self.bank;

        let current_liability_amount =
            bank.get_liability_amount(balance.liability_shares.into())?;
        let current_asset_amount = bank.get_asset_amount(balance.asset_shares.into())?;

        check!(
            current_liability_amount.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::IllegalBalanceState,
            "Balance has existing debt"
        );

        check!(
            current_asset_amount.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
            MarginfiError::IllegalBalanceState,
            "Balance has existing assets"
        );

        let asset_shares: I80F48 = balance.asset_shares.into();
        let liability_shares: I80F48 = balance.liability_shares.into();
        // Counters are incremented in `*_balance_internal` when shares cross
        // `ZERO_AMOUNT_THRESHOLD` upward; match that condition so we don't
        // double-decrement positions that already crossed downward earlier.
        let had_assets = asset_shares.is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let had_liabs = liability_shares.is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        balance.close()?;

        if in_receivership {
            bank.cache.clear_liquidation_price_cache_locked();
        }

        // Asset-side dust = real tokens still in the liquidity vault that the
        // user never withdrew. Route to `collected_insurance_fees_outstanding`
        // so vault content stays fully accounted for, mirroring the fractional-
        // remainder handling in `withdraw_all`.
        if current_asset_amount > I80F48::ZERO {
            bank.collected_insurance_fees_outstanding =
                I80F48::from(bank.collected_insurance_fees_outstanding)
                    .checked_add(current_asset_amount)
                    .ok_or_else(math_error!())?
                    .into();
        }

        bank.change_asset_shares(-asset_shares, false)?;
        // Liability-side dust = bad debt the borrower never repaid. Decrementing
        // here makes the loss explicit instead of leaving phantom shares in
        // `total_liability_shares` that would compound interest indefinitely.
        bank.change_liability_shares(-liability_shares, true)?;

        if had_assets {
            bank.decrement_lending_position_count();
        }
        if had_liabs {
            bank.decrement_borrowing_position_count();
        }

        Ok(())
    }

    // ------------ Internal accounting logic

    /// Note: in `BypassDepositLimit` mode, can flip a liability into an asset, a behavior that is used in liquidations.
    fn increase_balance_internal(
        &mut self,
        balance_delta: I80F48,
        operation_type: BalanceIncreaseType,
    ) -> MarginfiResult<I80F48> {
        debug!(
            "Balance increase: {} (type: {:?})",
            balance_delta, operation_type
        );

        let balance = &mut self.balance;
        let bank = &mut self.bank;
        // Record if the balance was an asset/liability beforehand
        let had_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let had_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        let current_liability_shares: I80F48 = balance.liability_shares.into();
        let current_liability_amount = bank.get_liability_amount(current_liability_shares)?;

        let (mut liability_amount_decrease, mut asset_amount_increase) = (
            min(current_liability_amount, balance_delta),
            max(
                balance_delta
                    .checked_sub(current_liability_amount)
                    .ok_or_else(math_error!())?,
                I80F48::ZERO,
            ),
        );

        match operation_type {
            BalanceIncreaseType::RepayOnly => {
                check!(
                    asset_amount_increase.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationRepayOnly
                );
                // Clamp tolerated dust to zero so it isn't booked as a new asset position.
                asset_amount_increase = I80F48::ZERO;
            }
            BalanceIncreaseType::DepositOnly => {
                check!(
                    liability_amount_decrease.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationDepositOnly
                );
                // Clamp tolerated dust to zero so it isn't consumed from an unrelated liability.
                liability_amount_decrease = I80F48::ZERO;
            }
            _ => {}
        }

        // Skip the no-op share updates when a side has no movement (e.g. a pure deposit has no
        // liability to repay, a pure repay adds no assets). The amounts are `max(_, 0)`, so `> 0`
        // captures exactly the cases where `change_*_shares(0)` would have been a no-op.
        let asset_shares_increase = if asset_amount_increase > I80F48::ZERO {
            let shares = bank.get_asset_shares(asset_amount_increase)?;
            balance.change_asset_shares(shares)?;
            bank.change_asset_shares(
                shares,
                matches!(operation_type, BalanceIncreaseType::BypassDepositLimit),
            )?;
            shares
        } else {
            I80F48::ZERO
        };

        let liability_shares_decrease = if liability_amount_decrease > I80F48::ZERO {
            let shares = bank.get_liability_shares(liability_amount_decrease)?;
            balance.change_liability_shares(-shares)?;
            bank.change_liability_shares(-shares, true)?;
            shares
        } else {
            I80F48::ZERO
        };

        // Record if the balance was an asset/liability after
        let has_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let has_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        // Increment position counts depending on the before/after state of the balance
        if !had_assets && has_assets {
            bank.increment_lending_position_count();
        }
        if had_assets && !has_assets {
            bank.decrement_lending_position_count();
        }
        if !had_liabs && has_liabs {
            bank.increment_borrowing_position_count();
        }
        if had_liabs && !has_liabs {
            bank.decrement_borrowing_position_count();
        }

        let share_amount = match operation_type {
            BalanceIncreaseType::RepayOnly => liability_shares_decrease,
            BalanceIncreaseType::Any
            | BalanceIncreaseType::DepositOnly
            | BalanceIncreaseType::BypassDepositLimit => asset_shares_increase,
        };

        Ok(share_amount)
    }

    /// Note: in `BypassBorrowLimit` mode, can flip a deposit into a liability, a behavior that is used in liquidations.
    /// It will also ignore the utilization ratio check in this case, so that the liquidation can continue even if
    /// if the bank is so bankrupt that assets < liabs.
    fn decrease_balance_internal(
        &mut self,
        balance_delta: I80F48,
        operation_type: BalanceDecreaseType,
    ) -> MarginfiResult<I80F48> {
        debug!(
            "Balance decrease: {} of (type: {:?})",
            balance_delta, operation_type
        );

        let balance = &mut self.balance;
        let bank = &mut self.bank;
        let had_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let had_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        let current_asset_shares: I80F48 = balance.asset_shares.into();
        let current_asset_amount = bank.get_asset_amount(current_asset_shares)?;

        let (mut asset_amount_decrease, mut liability_amount_increase) = (
            min(current_asset_amount, balance_delta),
            max(
                balance_delta
                    .checked_sub(current_asset_amount)
                    .ok_or_else(math_error!())?,
                I80F48::ZERO,
            ),
        );

        match operation_type {
            BalanceDecreaseType::WithdrawOnly => {
                check!(
                    liability_amount_increase.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationWithdrawOnly
                );
                // Clamp tolerated dust to zero so it isn't booked as a new liability position.
                liability_amount_increase = I80F48::ZERO;
            }
            BalanceDecreaseType::BorrowOnly => {
                check!(
                    asset_amount_decrease.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationBorrowOnly
                );
                // Clamp tolerated dust to zero so it isn't consumed from an unrelated asset.
                asset_amount_decrease = I80F48::ZERO;
            }
            _ => {}
        }

        // Skip the no-op share updates when a side has no movement (e.g. a pure withdraw adds no
        // liability, a pure borrow removes no assets). The amounts are `max(_, 0)`, so `> 0`
        // captures exactly the cases where `change_*_shares(0)` would have been a no-op.
        let asset_shares_decrease = if asset_amount_decrease > I80F48::ZERO {
            let shares = bank.get_asset_shares(asset_amount_decrease)?;
            balance.change_asset_shares(-shares)?;
            bank.change_asset_shares(-shares, false)?;
            shares
        } else {
            I80F48::ZERO
        };

        let liability_shares_increase = if liability_amount_increase > I80F48::ZERO {
            let shares = bank.get_liability_shares(liability_amount_increase)?;
            balance.change_liability_shares(shares)?;
            bank.change_liability_shares(
                shares,
                matches!(operation_type, BalanceDecreaseType::BypassBorrowLimit),
            )?;
            shares
        } else {
            I80F48::ZERO
        };

        // Only liquidation is allowed to bypass this check.
        if !matches!(operation_type, BalanceDecreaseType::BypassBorrowLimit) {
            bank.check_utilization_ratio()?;
        }

        let has_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let has_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        if !had_assets && has_assets {
            bank.increment_lending_position_count();
        }
        if had_assets && !has_assets {
            bank.decrement_lending_position_count();
        }
        if !had_liabs && has_liabs {
            bank.increment_borrowing_position_count();
        }
        if had_liabs && !has_liabs {
            bank.decrement_borrowing_position_count();
        }

        let share_amount = match operation_type {
            BalanceDecreaseType::BorrowOnly | BalanceDecreaseType::BypassBorrowLimit => {
                liability_shares_increase
            }
            BalanceDecreaseType::WithdrawOnly => asset_shares_decrease,
        };

        Ok(share_amount)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytemuck::Zeroable;
    use fixed_macro::types::I80F48;
    use marginfi_type_crate::types::basis_to_u32;

    fn same_asset_eligible_bank(mint: Pubkey, oracle_key: Pubkey, liab_weight: I80F48) -> Bank {
        let mut bank = Bank::zeroed();
        bank.mint = mint;
        bank.config.oracle_setup = OracleSetup::PythPushOracle;
        bank.config.oracle_keys[0] = oracle_key;
        bank.config.liability_weight_init = liab_weight.into();
        bank.config.liability_weight_maint = liab_weight.into();
        bank.update_flag(true, BANK_SAME_ASSET_EMODE_ELIGIBLE);
        bank
    }

    #[test]
    fn test_calc_asset_value() {
        assert_eq!(
            calc_value(I80F48!(10_000_000), I80F48!(1_000_000), 6, None).unwrap(),
            I80F48!(10_000_000)
        );

        assert_eq!(
            calc_value(I80F48!(1_000_000_000), I80F48!(10_000_000), 9, None).unwrap(),
            I80F48!(10_000_000)
        );

        assert_eq!(
            calc_value(I80F48!(1_000_000_000), I80F48!(10_000_000), 9, None).unwrap(),
            I80F48!(10_000_000)
        );
    }

    #[test]
    fn same_asset_weight_applies_to_matching_collateral_only() {
        let mint = Pubkey::new_unique();
        let mut bank = Bank::zeroed();
        bank.mint = mint;
        bank.config.risk_tier = RiskTier::Collateral;
        bank.config.operational_state = BankOperationalState::Operational;
        bank.config.oracle_setup = OracleSetup::PythPushOracle;
        bank.config.oracle_keys[0] = Pubkey::new_unique();
        bank.update_flag(true, BANK_SAME_ASSET_EMODE_ELIGIBLE);

        let mut balance = Balance::empty_deactivated();
        balance.set_active(true);
        balance.asset_shares = I80F48!(1).into();

        let mut reconciled = ReconciledEmodeConfig::default();
        reconciled.same_asset.mint = mint;
        reconciled.same_asset.oracle_key = bank.config.oracle_keys[0];
        reconciled.same_asset.feed_family = Some(OracleFeedFamily::PythPush);
        reconciled.same_asset.asset_weight = I80F48!(0.99);

        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            Some(I80F48!(0.99))
        );

        bank.mint = Pubkey::new_unique();
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            None
        );
    }

    #[test]
    fn same_asset_weight_requires_matching_feed_family() {
        let mint = Pubkey::new_unique();
        let mut bank = Bank::zeroed();
        bank.mint = mint;
        bank.config.risk_tier = RiskTier::Collateral;
        bank.config.operational_state = BankOperationalState::Operational;
        bank.config.oracle_setup = OracleSetup::KaminoPythPush;
        bank.config.oracle_keys[0] = Pubkey::new_unique();
        bank.update_flag(true, BANK_SAME_ASSET_EMODE_ELIGIBLE);

        let mut balance = Balance::empty_deactivated();
        balance.set_active(true);
        balance.asset_shares = I80F48!(1).into();

        let mut reconciled = ReconciledEmodeConfig::default();
        reconciled.same_asset.mint = mint;
        reconciled.same_asset.oracle_key = bank.config.oracle_keys[0];
        reconciled.same_asset.feed_family = Some(OracleFeedFamily::PythPush);
        reconciled.same_asset.asset_weight = I80F48!(0.99);

        // Integration setups in the same feed family qualify (kToken collateral vs native debt).
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            Some(I80F48!(0.99))
        );

        bank.config.oracle_setup = OracleSetup::SwitchboardPull;
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            None
        );

        bank.config.oracle_setup = OracleSetup::Fixed;
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            None
        );
    }

    #[test]
    fn same_asset_weight_disabled_when_reconciled_family_missing() {
        let mint = Pubkey::new_unique();
        let mut bank = Bank::zeroed();
        bank.mint = mint;
        bank.config.risk_tier = RiskTier::Collateral;
        bank.config.operational_state = BankOperationalState::Operational;
        bank.config.oracle_setup = OracleSetup::PythPushOracle;
        bank.config.oracle_keys[0] = Pubkey::new_unique();
        bank.update_flag(true, BANK_SAME_ASSET_EMODE_ELIGIBLE);

        let mut balance = Balance::empty_deactivated();
        balance.set_active(true);
        balance.asset_shares = I80F48!(1).into();

        let mut reconciled = ReconciledEmodeConfig::default();
        reconciled.same_asset.mint = mint;
        reconciled.same_asset.oracle_key = bank.config.oracle_keys[0];
        reconciled.same_asset.asset_weight = I80F48!(0.99);

        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            None
        );
    }

    #[test]
    fn same_asset_weight_respects_reduce_only_and_equity_disable_behavior() {
        let mint = Pubkey::new_unique();
        let mut bank = Bank::zeroed();
        bank.mint = mint;
        bank.config.risk_tier = RiskTier::Collateral;
        bank.config.operational_state = BankOperationalState::ReduceOnly;
        bank.config.oracle_setup = OracleSetup::PythPushOracle;
        bank.config.oracle_keys[0] = Pubkey::new_unique();
        bank.update_flag(true, BANK_SAME_ASSET_EMODE_ELIGIBLE);

        let mut balance = Balance::empty_deactivated();
        balance.set_active(true);
        balance.asset_shares = I80F48!(1).into();

        let mut reconciled = ReconciledEmodeConfig::default();
        reconciled.same_asset.mint = mint;
        reconciled.same_asset.oracle_key = bank.config.oracle_keys[0];
        reconciled.same_asset.feed_family = Some(OracleFeedFamily::PythPush);
        reconciled.same_asset.asset_weight = I80F48!(0.99);

        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Initial,
                &reconciled,
            ),
            None
        );
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Maintenance,
                &reconciled,
            ),
            Some(I80F48!(0.99))
        );
        assert_eq!(
            get_same_asset_weight_for_balance(
                &balance,
                &bank,
                RequirementType::Equity,
                &ReconciledEmodeConfig::default(),
            ),
            None
        );
    }

    #[test]
    fn same_asset_leverage_for_requirement_selects_enabled_non_equity_values() {
        let group = MarginfiGroup {
            same_asset_emode_init_leverage: basis_to_u32(I80F48::from_num(1.5)),
            same_asset_emode_maint_leverage: basis_to_u32(I80F48::from_num(2.5)),
            ..Default::default()
        };

        let init_leverage =
            same_asset_leverage_for_requirement(RequirementType::Initial, &group).unwrap();
        assert!(
            (init_leverage - I80F48::from_num(1.5)).abs() < I80F48::from_num(0.000001),
            "expected ~1.5, got {}",
            init_leverage
        );
        let maint_leverage =
            same_asset_leverage_for_requirement(RequirementType::Maintenance, &group).unwrap();
        assert!(
            (maint_leverage - I80F48::from_num(2.5)).abs() < I80F48::from_num(0.000001),
            "expected ~2.5, got {}",
            maint_leverage
        );
        assert_eq!(
            same_asset_leverage_for_requirement(RequirementType::Equity, &group),
            None
        );
        assert_eq!(
            same_asset_leverage_for_requirement(
                RequirementType::Initial,
                &MarginfiGroup {
                    same_asset_emode_init_leverage: basis_to_u32(I80F48::ONE),
                    same_asset_emode_maint_leverage: basis_to_u32(I80F48::from_num(2.5)),
                    ..Default::default()
                },
            ),
            None
        );
    }

    #[test]
    fn same_asset_leverage_treats_legacy_zero_as_disabled() {
        let group = MarginfiGroup {
            same_asset_emode_init_leverage: 0,
            same_asset_emode_maint_leverage: 0,
            ..Default::default()
        };

        assert_eq!(
            same_asset_leverage_for_requirement(RequirementType::Initial, &group),
            None
        );
    }

    #[test]
    fn same_asset_config_enables_when_all_liabilities_share_one_mint() {
        let mint = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let bank_a = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));
        let bank_b = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.05));

        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_a,
            bank_a.mint,
            I80F48!(1.00),
        ));
        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_b,
            bank_b.mint,
            I80F48!(1.05),
        ));

        assert_eq!(shared_mint, Some(mint));
        assert_eq!(shared_oracle_key, Some(oracle_key));
        assert_eq!(lowest_liab_weight, Some(I80F48!(1.00)));
        assert_eq!(
            compute_same_asset_emode_weight(I80F48::from_num(100), lowest_liab_weight.unwrap()),
            compute_same_asset_emode_weight(I80F48::from_num(100), I80F48!(1.00))
        );
    }

    #[test]
    fn same_asset_config_disables_when_liability_mints_diverge() {
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let bank_a = same_asset_eligible_bank(mint_a, oracle_key, I80F48!(1.00));
        let bank_b = same_asset_eligible_bank(mint_b, oracle_key, I80F48!(1.00));

        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_a,
            bank_a.mint,
            I80F48!(1.00),
        ));
        assert!(!update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_b,
            bank_b.mint,
            I80F48!(1.00),
        ));

        assert_eq!(shared_mint, Some(mint_a));
        assert_eq!(lowest_liab_weight, None);
    }

    #[test]
    fn same_asset_config_disables_when_liability_oracles_diverge() {
        let mint = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let bank_a = same_asset_eligible_bank(mint, Pubkey::new_unique(), I80F48!(1.00));
        let bank_b = same_asset_eligible_bank(mint, Pubkey::new_unique(), I80F48!(1.00));

        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_a,
            bank_a.mint,
            I80F48!(1.00),
        ));
        assert!(!update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_b,
            bank_b.mint,
            I80F48!(1.00),
        ));

        assert_eq!(shared_mint, Some(mint));
        assert_eq!(lowest_liab_weight, None);
    }

    #[test]
    fn same_asset_config_disables_when_liability_feed_families_diverge() {
        let mint = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let bank_a = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));
        let mut bank_b = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));
        bank_b.config.oracle_setup = OracleSetup::SwitchboardPull;

        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_a,
            bank_a.mint,
            I80F48!(1.00),
        ));
        assert!(!update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_b,
            bank_b.mint,
            I80F48!(1.00),
        ));

        assert_eq!(shared_feed_family, Some(OracleFeedFamily::PythPush));
        assert_eq!(lowest_liab_weight, None);
    }

    #[test]
    fn same_asset_config_disables_when_liability_setup_has_no_feed_family() {
        let mint = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let mut bank = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));
        bank.config.oracle_setup = OracleSetup::Fixed;

        assert!(!update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank,
            bank.mint,
            I80F48!(1.00),
        ));
        assert_eq!(shared_mint, None);
        assert_eq!(shared_feed_family, None);
        assert_eq!(lowest_liab_weight, None);
    }

    #[test]
    fn same_asset_config_disables_when_liability_bank_is_not_eligible() {
        let mint = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let mut bank = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));
        bank.update_flag(false, BANK_SAME_ASSET_EMODE_ELIGIBLE);

        assert!(!update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank,
            bank.mint,
            I80F48!(1.00),
        ));
        assert_eq!(shared_mint, None);
        assert_eq!(lowest_liab_weight, None);
    }

    #[test]
    fn same_asset_config_uses_least_favorable_liability_weight() {
        let mint = Pubkey::new_unique();
        let oracle_key = Pubkey::new_unique();
        let mut shared_mint = None;
        let mut shared_oracle_key = None;
        let mut shared_feed_family = None;
        let mut lowest_liab_weight = None;
        let bank_a = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.05));
        let bank_b = same_asset_eligible_bank(mint, oracle_key, I80F48!(1.00));

        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_a,
            bank_a.mint,
            I80F48!(1.05),
        ));
        assert!(update_reconciled_same_asset_config(
            &mut shared_mint,
            &mut shared_oracle_key,
            &mut shared_feed_family,
            &mut lowest_liab_weight,
            &bank_b,
            bank_b.mint,
            I80F48!(1.00),
        ));

        assert_eq!(lowest_liab_weight, Some(I80F48!(1.00)));
        assert_eq!(
            compute_same_asset_emode_weight(I80F48::from_num(100), lowest_liab_weight.unwrap()),
            compute_same_asset_emode_weight(I80F48::from_num(100), I80F48!(1.00))
        );
    }

    #[test]
    fn same_asset_requirement_decoded_leverage_at_or_below_one_is_treated_as_disabled() {
        let group = MarginfiGroup {
            same_asset_emode_init_leverage: basis_to_u32(I80F48::ONE),
            same_asset_emode_maint_leverage: basis_to_u32(I80F48::ONE),
            ..Default::default()
        };

        assert_eq!(
            same_asset_leverage_for_requirement(RequirementType::Initial, &group),
            None
        );
        assert_eq!(
            same_asset_leverage_for_requirement(RequirementType::Maintenance, &group),
            None
        );
    }

    /// Verify the operation-kind checks clamp sub-threshold dust to zero so
    /// it can't leak shares onto the prohibited side.
    mod tolerance_clamping {
        use super::*;
        use bytemuck::Zeroable;
        use marginfi_type_crate::types::{Balance, Bank};

        const SHARES: i128 = 10;

        /// Build a bank/balance pair where the user holds `(asset_shares,
        /// liability_shares)` and the bank's totals carry a healthy buffer
        /// from other (fictional) participants. Deposit and borrow caps are
        /// disabled (`u64::MAX`) so the test isolates the operation-type
        /// invariant rather than tripping on a downstream limit.
        fn make_bank_and_balance(
            asset_share_value: I80F48,
            liability_share_value: I80F48,
            asset_shares: I80F48,
            liability_shares: I80F48,
        ) -> (Bank, Balance) {
            let mut bank = Bank::zeroed();
            bank.asset_share_value = asset_share_value.into();
            bank.liability_share_value = liability_share_value.into();
            // Buffer the totals so utilization stays healthy after the call
            // and so dust on the prohibited side, if leaked, doesn't fail the
            // utilization-ratio check.
            let buffer = I80F48::from_num(1_000);
            bank.total_asset_shares = (asset_shares + buffer).into();
            bank.total_liability_shares = liability_shares.into();
            bank.config.deposit_limit = u64::MAX;
            bank.config.borrow_limit = u64::MAX;

            let mut balance = Balance::zeroed();
            balance.active = 1;
            balance.asset_shares = asset_shares.into();
            balance.liability_shares = liability_shares.into();
            (bank, balance)
        }

        /// `withdraw` on a bank with fractional `asset_share_value`. Choose an
        /// integer `amount` slightly above `current_asset_amount` so that
        /// `liability_amount_increase = amount - current_asset_amount` falls
        /// inside `(0, ZERO_AMOUNT_THRESHOLD)`. With the bug, this mints
        /// liability shares on a `WithdrawOnly` path.
        #[test]
        fn withdraw_only_does_not_mint_dust_liability() {
            // share_value < 1 by 5e-6 → current_asset_amount = 10 * 0.999995 = 9.99995.
            let asset_share_value = I80F48!(0.999995);
            let asset_shares = I80F48::from_num(SHARES);
            let (mut bank, mut balance) =
                make_bank_and_balance(asset_share_value, I80F48::ONE, asset_shares, I80F48::ZERO);
            let current_asset_amount = asset_shares * asset_share_value;
            // Withdraw exactly SHARES raw units → liability_amount_increase = 5e-5,
            // which is < ZERO_AMOUNT_THRESHOLD (1e-4) and bypasses the check.
            let withdraw_amount = I80F48::from_num(SHARES);
            let dust = withdraw_amount - current_asset_amount;
            assert!(dust > I80F48::ZERO && dust < ZERO_AMOUNT_THRESHOLD);
            let bank_total_liability_shares_before = I80F48::from(bank.total_liability_shares);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.withdraw(withdraw_amount).unwrap();

            // With the fix, the prohibited liability side stays at zero — no
            // dust is booked into shares on a `WithdrawOnly` path.
            assert_eq!(
                I80F48::from(balance.liability_shares),
                I80F48::ZERO,
                "WithdrawOnly leaked a dust liability into balance.liability_shares"
            );
            assert_eq!(
                I80F48::from(bank.total_liability_shares),
                bank_total_liability_shares_before,
                "WithdrawOnly leaked dust into bank.total_liability_shares"
            );
        }

        /// `repay` on a bank with fractional `liability_share_value`. Choose
        /// `amount` slightly above the user's `current_liability_amount` so
        /// that `asset_amount_increase = amount - current_liability_amount`
        /// falls inside `(0, ZERO_AMOUNT_THRESHOLD)`. With the bug, this mints
        /// asset shares on a `RepayOnly` path.
        #[test]
        fn repay_only_does_not_mint_dust_asset() {
            let liability_share_value = I80F48!(0.999995);
            let liability_shares = I80F48::from_num(SHARES);
            let (mut bank, mut balance) = make_bank_and_balance(
                I80F48::ONE,
                liability_share_value,
                I80F48::ZERO,
                liability_shares,
            );
            let current_liability_amount = liability_shares * liability_share_value;
            let repay_amount = I80F48::from_num(SHARES);
            let dust = repay_amount - current_liability_amount;
            assert!(dust > I80F48::ZERO && dust < ZERO_AMOUNT_THRESHOLD);
            let bank_total_asset_shares_before = I80F48::from(bank.total_asset_shares);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.repay(repay_amount).unwrap();

            assert_eq!(
                I80F48::from(balance.asset_shares),
                I80F48::ZERO,
                "RepayOnly leaked a dust asset into balance.asset_shares"
            );
            assert_eq!(
                I80F48::from(bank.total_asset_shares),
                bank_total_asset_shares_before,
                "RepayOnly leaked dust into bank.total_asset_shares"
            );
        }

        /// Sanity: a delta above the tolerance still errors with
        /// `OperationWithdrawOnly` (the guard is preserved by the fix).
        #[test]
        fn withdraw_only_still_rejects_above_threshold() {
            let (mut bank, mut balance) = make_bank_and_balance(
                I80F48!(0.9),
                I80F48::ONE,
                I80F48::from_num(SHARES),
                I80F48::ZERO,
            );
            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            let err = wrapper.withdraw(I80F48::from_num(SHARES)).unwrap_err();
            assert_eq!(err, MarginfiError::OperationWithdrawOnly.into());
        }

        /// `DepositOnly` clamp on the opposite (liability) side. A balance
        /// carries sub-threshold leftover liability shares; without the clamp
        /// a depositor would silently retire that dust.
        #[test]
        fn deposit_only_does_not_consume_dust_liability() {
            let liability_shares = I80F48!(0.00005);
            let (mut bank, mut balance) =
                make_bank_and_balance(I80F48::ONE, I80F48::ONE, I80F48::ZERO, liability_shares);
            let bank_total_liability_shares_before = I80F48::from(bank.total_liability_shares);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.deposit(I80F48::from_num(SHARES)).unwrap();

            assert_eq!(
                I80F48::from(balance.liability_shares),
                liability_shares,
                "DepositOnly consumed dust from balance.liability_shares"
            );
            assert_eq!(
                I80F48::from(bank.total_liability_shares),
                bank_total_liability_shares_before,
                "DepositOnly consumed dust from bank.total_liability_shares"
            );
        }

        /// `BorrowOnly` clamp on the opposite (asset) side. Mirror of the
        /// DepositOnly case — sub-threshold leftover asset shares must not
        /// be silently forfeited by a borrow.
        #[test]
        fn borrow_only_does_not_consume_dust_asset() {
            let asset_shares = I80F48!(0.00005);
            let (mut bank, mut balance) =
                make_bank_and_balance(I80F48::ONE, I80F48::ONE, asset_shares, I80F48::ZERO);
            let bank_total_asset_shares_before = I80F48::from(bank.total_asset_shares);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.borrow(I80F48::from_num(SHARES)).unwrap();

            assert_eq!(
                I80F48::from(balance.asset_shares),
                asset_shares,
                "BorrowOnly consumed dust from balance.asset_shares"
            );
            assert_eq!(
                I80F48::from(bank.total_asset_shares),
                bank_total_asset_shares_before,
                "BorrowOnly consumed dust from bank.total_asset_shares"
            );
        }
    }

    /// Verify `close_balance` decrements bank totals + position counters and
    /// routes asset-side residual to `collected_insurance_fees_outstanding`.
    mod close_balance_accounting {
        use super::*;
        use bytemuck::Zeroable;
        use marginfi_type_crate::types::{Balance, Bank};

        const BUFFER: i128 = 1_000;

        fn make_bank_and_balance(
            asset_share_value: I80F48,
            liability_share_value: I80F48,
            balance_asset_shares: I80F48,
            balance_liability_shares: I80F48,
            initial_lending_count: i32,
            initial_borrowing_count: i32,
        ) -> (Bank, Balance) {
            let mut bank = Bank::zeroed();
            bank.asset_share_value = asset_share_value.into();
            bank.liability_share_value = liability_share_value.into();
            // Bank holds the user's shares plus an external buffer so totals
            // stay positive after dust removal.
            let buffer = I80F48::from_num(BUFFER);
            bank.total_asset_shares = (balance_asset_shares + buffer).into();
            bank.total_liability_shares = balance_liability_shares.into();
            bank.config.deposit_limit = u64::MAX;
            bank.config.borrow_limit = u64::MAX;
            bank.lending_position_count = initial_lending_count;
            bank.borrowing_position_count = initial_borrowing_count;

            let mut balance = Balance::zeroed();
            balance.active = 1;
            balance.asset_shares = balance_asset_shares.into();
            balance.liability_shares = balance_liability_shares.into();
            (bank, balance)
        }

        /// Dust shares (`shares < ZERO_AMOUNT_THRESHOLD`) — represents a
        /// position that already crossed below threshold via a prior
        /// withdraw/repay, so the counter was decremented earlier. Closing
        /// must NOT decrement again, but must still unwind the dust from
        /// `bank.total_asset_shares` and route the dust amount to
        /// `collected_insurance_fees_outstanding`.
        #[test]
        fn asset_dust_unwinds_totals_and_routes_to_insurance_fees() {
            let asset_share_value = I80F48::ONE;
            let dust_shares = I80F48!(0.00005);
            let (mut bank, mut balance) = make_bank_and_balance(
                asset_share_value,
                I80F48::ONE,
                dust_shares,
                I80F48::ZERO,
                /* lending_count */ 5,
                /* borrowing_count */ 0,
            );
            let expected_dust_amount = dust_shares * asset_share_value;
            let total_asset_shares_before = I80F48::from(bank.total_asset_shares);
            let insurance_fees_before = I80F48::from(bank.collected_insurance_fees_outstanding);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.close_balance(false).unwrap();

            assert_eq!(balance.active, 0, "balance slot not freed");
            assert_eq!(
                I80F48::from(bank.total_asset_shares),
                total_asset_shares_before - dust_shares,
                "bank.total_asset_shares not decremented by dust"
            );
            assert_eq!(
                I80F48::from(bank.collected_insurance_fees_outstanding),
                insurance_fees_before + expected_dust_amount,
                "asset dust not routed to insurance fees"
            );
            // Counter was NOT incremented for this slot (shares < threshold),
            // so closing must NOT decrement it.
            assert_eq!(
                bank.lending_position_count, 5,
                "lending_position_count incorrectly decremented for sub-threshold shares"
            );
        }

        /// Liability-side dust — analogous to asset case, but no insurance-fees
        /// routing: the borrower kept the dust tokens (bad debt), and removing
        /// the phantom shares makes that loss explicit instead of leaving it
        /// to compound forever.
        #[test]
        fn liability_dust_unwinds_totals() {
            let (mut bank, mut balance) = make_bank_and_balance(
                I80F48::ONE,
                I80F48::ONE,
                I80F48::ZERO,
                I80F48!(0.00005),
                /* lending_count */ 0,
                /* borrowing_count */ 3,
            );
            let total_liability_shares_before = I80F48::from(bank.total_liability_shares);
            let insurance_fees_before = I80F48::from(bank.collected_insurance_fees_outstanding);

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.close_balance(false).unwrap();

            assert_eq!(
                I80F48::from(bank.total_liability_shares),
                total_liability_shares_before - I80F48!(0.00005),
                "bank.total_liability_shares not decremented by dust"
            );
            assert_eq!(
                I80F48::from(bank.collected_insurance_fees_outstanding),
                insurance_fees_before,
                "liability dust should not affect insurance fees"
            );
            assert_eq!(
                bank.borrowing_position_count, 3,
                "borrowing_position_count incorrectly decremented for sub-threshold shares"
            );
        }

        /// Counter-leak case: shares are ABOVE `ZERO_AMOUNT_THRESHOLD` but
        /// the corresponding amount is BELOW it because `share_value` has
        /// collapsed (e.g. after bad-debt socialization on the asset side).
        /// The check on `current_amount` passes, so `close_balance` is
        /// callable, and the position counter must be decremented here —
        /// no prior op ever crossed it downward.
        #[test]
        fn collapsed_share_value_decrements_position_counter() {
            // share_value tiny → amount = 0.01 * 0.001 = 1e-5 < threshold,
            // but shares = 0.01 > threshold.
            let asset_share_value = I80F48!(0.001);
            let asset_shares = I80F48!(0.01);
            let (mut bank, mut balance) = make_bank_and_balance(
                asset_share_value,
                I80F48::ONE,
                asset_shares,
                I80F48::ZERO,
                /* lending_count */ 1,
                /* borrowing_count */ 0,
            );
            assert!(asset_shares.is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD));
            let current_asset_amount = asset_shares * asset_share_value;
            assert!(current_asset_amount.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD));

            let mut wrapper = BankAccountWrapper {
                balance: &mut balance,
                bank: &mut bank,
            };
            wrapper.close_balance(false).unwrap();

            assert_eq!(
                bank.lending_position_count, 0,
                "lending_position_count not decremented for above-threshold shares"
            );
        }
    }
}
