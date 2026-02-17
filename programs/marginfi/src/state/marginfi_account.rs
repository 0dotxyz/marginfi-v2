use super::price::{
    OraclePriceFeedAdapter, OraclePriceType, OraclePriceWithConfidence, PriceAdapter, PriceBias,
};
use crate::{
    allocator::{heap_pos, heap_restore},
    check, check_eq, debug, live, math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::{bank::BankImpl, bank_config::BankConfigImpl},
    utils::{is_integration_asset_tag, NumTraitsWithTolerance},
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
        ASSET_TAG_SOLEND, ASSET_TAG_STAKED, BANKRUPT_THRESHOLD, EMISSIONS_FLAG_BORROW_ACTIVE,
        EMISSIONS_FLAG_LENDING_ACTIVE, EXP_10_I80F48, MAX_INTEGRATION_POSITIONS,
        MIN_EMISSIONS_START_TIME, ORDER_ACTIVE_TAGS, SECONDS_PER_YEAR, ZERO_AMOUNT_THRESHOLD,
    },
    types::{
        reconcile_emode_configs, Balance, BalanceSide, Bank, BankOperationalState, EmodeConfig,
        HealthCache, LendingAccount, MarginfiAccount, OracleSetup, RiskTier, ACCOUNT_DISABLED,
        ACCOUNT_FROZEN, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
        MAX_LENDING_ACCOUNT_BALANCES,
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
/// - `ASSET_TAG_STAKED`: 4 (bank + oracle + lst_mint + stake_pool)
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

/// Validate remaining accounts for active balances, requiring the closing bank group to appear
/// after all other active balances.
///
/// This is used for withdraw_all where the caller must include the closing bank's risk accounts,
/// but post-close health checks should still see active balances first.
pub fn validate_remaining_accounts_for_balances_close_last<'info>(
    lending_account: &LendingAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    close_bank: &Pubkey,
) -> MarginfiResult<()> {
    let mut account_index = 0;

    for balance in lending_account
        .balances
        .iter()
        .filter(|balance| balance.is_active())
        .filter(|balance| balance.bank_pk != *close_bank)
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
        let end_idx = account_index + num_accounts;
        require_gte!(
            remaining_ais.len(),
            end_idx,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        account_index = end_idx;
    }

    let bank_ai = remaining_ais
        .get(account_index)
        .ok_or(MarginfiError::InvalidBankAccount)?;
    check_eq!(*bank_ai.key, *close_bank, MarginfiError::InvalidBankAccount);
    let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
    let bank = bank_al.load()?;
    let num_accounts = get_remaining_accounts_per_bank(&bank)?;
    let end_idx = account_index + num_accounts;
    require_gte!(
        remaining_ais.len(),
        end_idx,
        MarginfiError::WrongNumberOfOracleAccounts
    );

    Ok(())
}

/// Validate that remaining accounts include the risk accounts for each active balance,
/// without enforcing any specific ordering between balance groups.
///
/// This is useful for repay_all where the instruction does not rely on ordered accounts.
pub fn validate_remaining_accounts_for_balances_unordered<'info>(
    lending_account: &LendingAccount,
    remaining_ais: &'info [AccountInfo<'info>],
) -> MarginfiResult<()> {
    for balance in lending_account
        .balances
        .iter()
        .filter(|balance| balance.is_active())
    {
        let bank_idx = remaining_ais
            .iter()
            .position(|ai| ai.key == &balance.bank_pk)
            .ok_or(MarginfiError::InvalidBankAccount)?;

        let bank_ai = remaining_ais
            .get(bank_idx)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        let bank = bank_al.load()?;

        let num_accounts = get_remaining_accounts_per_bank(&bank)?;
        let end_idx = bank_idx + num_accounts;
        require_gte!(
            remaining_ais.len(),
            end_idx,
            MarginfiError::WrongNumberOfOracleAccounts
        );
    }

    Ok(())
}

/// 4 for `ASSET_TAG_STAKED` (bank, oracle, lst mint, lst pool), 2 for most others (bank, oracle), 3
/// for Kamino (bank, oracle, reserve), 1 for Fixed
fn get_remaining_accounts_per_asset_tag(asset_tag: u8) -> MarginfiResult<usize> {
    match asset_tag {
        ASSET_TAG_DEFAULT | ASSET_TAG_SOL => Ok(2),
        ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND | ASSET_TAG_JUPLEND => Ok(3),
        ASSET_TAG_STAKED => Ok(4),
        _ => err!(MarginfiError::AssetTagMismatch),
    }
}

pub trait MarginfiAccountImpl {
    fn initialize(&mut self, group: Pubkey, authority: Pubkey, current_timestamp: u64);
    fn set_flag(&mut self, flag: u64, msg: bool);
    fn unset_flag(&mut self, flag: u64, msg: bool);
    fn get_flag(&self, flag: u64) -> bool;
    fn can_be_closed(&self) -> bool;
}

/// Checks if a signer is authorized to perform actions on a marginfi account.
///
/// Returns `true` if the signer is authorized, `false` otherwise.
///
/// Authorization rules (checked in order):
/// 1. If `allow_receivership` is true and the account is in receivership → `true`
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
        return true;
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

impl MarginfiAccountImpl for MarginfiAccount {
    /// Set the initial data for the marginfi account.
    fn initialize(&mut self, group: Pubkey, authority: Pubkey, current_timestamp: u64) {
        self.authority = authority;
        self.group = group;
        self.emissions_destination_account = Pubkey::default();
        self.migrated_from = Pubkey::default();
        self.last_update = current_timestamp;
        self.migrated_to = Pubkey::default();
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

    fn can_be_closed(&self) -> bool {
        let is_disabled = self.get_flag(ACCOUNT_DISABLED);
        let is_in_flashloan = self.get_flag(ACCOUNT_IN_FLASHLOAN);
        let is_in_receivership = self.get_flag(ACCOUNT_IN_RECEIVERSHIP);
        let only_has_empty_balances = self
            .lending_account
            .balances
            .iter()
            .all(|balance| balance.get_side().is_none());

        !is_disabled && only_has_empty_balances && !is_in_flashloan && !is_in_receivership
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

#[derive(Copy, Clone)]
pub enum RequirementType {
    Initial,
    Maintenance,
    Equity,
}

impl RequirementType {
    /// Get oracle price type for the requirement type.
    ///
    /// Initial and equity requirements use the time weighted price feed.
    /// Maintenance requirement uses the real time price feed, as its more accurate for triggering liquidations.
    pub fn get_oracle_price_type(&self) -> OraclePriceType {
        match self {
            RequirementType::Initial | RequirementType::Equity => OraclePriceType::TimeWeighted,
            RequirementType::Maintenance => OraclePriceType::RealTime,
        }
    }
}

/// Temporary struct used to store prices during receivership liquidation, these price will
/// ultimately populate the respective Bank's BankCache, and then be loaded at End Liqudation.
#[derive(Default)]
pub struct LiquidationPriceCache {
    real_time: [Option<OraclePriceWithConfidence>; MAX_LENDING_ACCOUNT_BALANCES],
    time_weighted: [Option<OraclePriceWithConfidence>; MAX_LENDING_ACCOUNT_BALANCES],
}

impl LiquidationPriceCache {
    pub fn record(
        &mut self,
        requirement_type: RequirementType,
        index: usize,
        price: OraclePriceWithConfidence,
    ) {
        match requirement_type.get_oracle_price_type() {
            OraclePriceType::RealTime => self.real_time[index] = Some(price),
            OraclePriceType::TimeWeighted => self.time_weighted[index] = Some(price),
        }
    }

    pub fn get_price(
        &self,
        price_type: OraclePriceType,
        index: usize,
    ) -> Option<OraclePriceWithConfidence> {
        match price_type {
            OraclePriceType::RealTime => self.real_time[index],
            OraclePriceType::TimeWeighted => self.time_weighted[index],
        }
    }
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
        },
        OraclePriceType::TimeWeighted => OraclePriceWithConfidence {
            price: bank.cache.liquidation_price_twap.into(),
            confidence: bank.cache.liquidation_price_twap_confidence.into(),
        },
    }
}

#[inline(always)]
fn calc_weighted_asset_value_cached_standalone(
    balance: &Balance,
    bank: &Bank,
    requirement_type: RequirementType,
    emode_config: &EmodeConfig,
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

            let mut asset_weight = if let Some(emode_entry) =
                emode_config.find_with_tag(bank.emode.emode_tag)
            {
                let bank_weight = bank
                    .config
                    .get_weight(requirement_type, BalanceSide::Assets);
                let emode_weight = match requirement_type {
                    RequirementType::Initial => I80F48::from(emode_entry.asset_weight_init),
                    RequirementType::Maintenance => I80F48::from(emode_entry.asset_weight_maint),
                    RequirementType::Equity => I80F48::ONE,
                };
                max(bank_weight, emode_weight)
            } else {
                bank.config
                    .get_weight(requirement_type, BalanceSide::Assets)
            };

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
    emode_config: &EmodeConfig,
) -> MarginfiResult<(I80F48, I80F48, I80F48)> {
    match balance.get_side() {
        Some(side) => match side {
            BalanceSide::Assets => {
                let (value, price) = calc_weighted_asset_value_cached_standalone(
                    balance,
                    bank,
                    requirement_type,
                    emode_config,
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

pub enum RiskRequirementType {
    Initial,
    Maintenance,
    Equity,
}

impl RiskRequirementType {
    pub fn to_weight_type(&self) -> RequirementType {
        match self {
            RiskRequirementType::Initial => RequirementType::Initial,
            RiskRequirementType::Maintenance => RequirementType::Maintenance,
            RiskRequirementType::Equity => RequirementType::Equity,
        }
    }
}

pub enum HealthPriceMode<'a> {
    Live {
        liq_cache: Option<&'a mut LiquidationPriceCache>,
    },
    Cached,
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

/// Iterator that yields EmodeConfig for each liability balance in a lending account.
///
/// This avoids allocating a large array of EmodeConfig on the stack by yielding
/// one config at a time. Each EmodeConfig is ~400 bytes, so storing 16 of them
/// would use ~6.4 KiB of stack space, which is problematic.
struct EmodeConfigIterator<'a, 'info> {
    lending_account: &'a LendingAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    balance_index: usize,
    account_index: usize,
    banks_only: bool,
}

impl<'a, 'info> EmodeConfigIterator<'a, 'info> {
    fn new(
        lending_account: &'a LendingAccount,
        remaining_ais: &'info [AccountInfo<'info>],
        banks_only: bool,
    ) -> Self {
        Self {
            lending_account,
            remaining_ais,
            balance_index: 0,
            account_index: 0,
            banks_only,
        }
    }
}
impl<'a, 'info> Iterator for EmodeConfigIterator<'a, 'info> {
    type Item = EmodeConfig;

    fn next(&mut self) -> Option<Self::Item> {
        // Find next active balance with liabilities
        while self.balance_index < self.lending_account.balances.len() {
            let balance = &self.lending_account.balances[self.balance_index];

            if !balance.is_active() {
                self.balance_index += 1;
                continue;
            }

            // Try to load bank to get account count and emode config
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

            // Advance indices
            self.account_index += num_accounts;
            self.balance_index += 1;

            // Only yield emode config if this balance has liabilities
            if !balance.is_empty(BalanceSide::Liabilities) {
                return Some(bank.emode.emode_config);
            }
        }
        None
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
/// - `remaining_ais`: Remaining accounts containing banks and oracles
/// - `requirement_type`: Initial, Maintenance, or Equity requirement
/// - `health_cache`: Optional cache to populate with results
///
/// ## Returns
///
/// (total_assets, total_liabilities) weighted according to requirement_type
pub fn get_health_components<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    requirement_type: RiskRequirementType,
    health_cache: &mut Option<&mut HealthCache>,
    price_mode: HealthPriceMode<'_>,
) -> MarginfiResult<(I80F48, I80F48)> {
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    let (is_cached, mut liq_cache) = match price_mode {
        HealthPriceMode::Live { liq_cache } => (false, liq_cache),
        HealthPriceMode::Cached => (true, None),
    };

    let clock = if is_cached { None } else { Some(Clock::get()?) };
    let lending_account = &marginfi_account.lending_account;

    // =========================================================================
    // Phase 1: Load emode configuration with heap reuse
    // =========================================================================

    let emode_checkpoint = heap_pos();
    let reconciled_emode_config = {
        let emode_iter = EmodeConfigIterator::new(lending_account, remaining_ais, is_cached);
        reconcile_emode_configs(emode_iter)
    };
    let reconciled_emode_config: EmodeConfig = reconciled_emode_config;
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
                requirement_type.to_weight_type(),
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
                requirement_type.to_weight_type(),
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
            if let RequirementType::Initial = requirement_type.to_weight_type() {
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
            RiskRequirementType::Initial => {
                cache.asset_value = total_assets.into();
                cache.liability_value = total_liabilities.into();
            }
            RiskRequirementType::Maintenance => {
                cache.asset_value_maint = total_assets.into();
                cache.liability_value_maint = total_liabilities.into();
            }
            RiskRequirementType::Equity => {
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
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    if balance_tags.is_empty() {
        return Ok((I80F48::ZERO, I80F48::ZERO, 0, 0));
    }

    let lending_account = &marginfi_account.lending_account;
    let clock = Clock::get()?;

    let emode_checkpoint = heap_pos();
    let reconciled_emode_config = {
        let emode_iter = EmodeConfigIterator::new(lending_account, remaining_ais, false);
        reconcile_emode_configs(emode_iter)
    };
    let reconciled_emode_config: EmodeConfig = reconciled_emode_config;
    heap_restore(emode_checkpoint);

    let requirement_type = RiskRequirementType::Equity.to_weight_type();
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

        if !balance_tags.iter().any(|tag| *tag == balance.tag) {
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
        remaining_ais,
        RiskRequirementType::Maintenance,
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
    remaining_ais: &'info [AccountInfo<'info>],
    health_cache: &mut Option<&mut HealthCache>,
) -> MarginfiResult {
    // TODO remove this check here and raise it to the top-level instruction
    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    let (equity_assets, equity_liabs) = get_health_components(
        marginfi_account,
        remaining_ais,
        RiskRequirementType::Equity,
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
    remaining_ais: &'info [AccountInfo<'info>],
    health_cache: &mut Option<&mut HealthCache>,
) -> MarginfiResult {
    if marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN) {
        // Risk checks are skipped during flashloans
        return Ok(());
    }

    let (assets, liabs) = get_health_components(
        marginfi_account,
        remaining_ais,
        RiskRequirementType::Initial,
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
        remaining_ais,
        RiskRequirementType::Maintenance,
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
    emode_config: &EmodeConfig,
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
                    emode_config,
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
    emode_config: &EmodeConfig,
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
            let mut asset_weight = if let Some(emode_entry) =
                emode_config.find_with_tag(bank.emode.emode_tag)
            {
                let bank_weight = bank
                    .config
                    .get_weight(requirement_type, BalanceSide::Assets);
                let emode_weight = match requirement_type {
                    RequirementType::Initial => I80F48::from(emode_entry.asset_weight_init),
                    RequirementType::Maintenance => I80F48::from(emode_entry.asset_weight_maint),
                    RequirementType::Equity => I80F48::ONE,
                };
                max(bank_weight, emode_weight)
            } else {
                bank.config
                    .get_weight(requirement_type, BalanceSide::Assets)
            };

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
}

impl LendingAccountImpl for LendingAccount {
    fn get_first_empty_balance(&self) -> Option<usize> {
        self.balances.iter().position(|b| !b.is_active())
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
}

pub trait BalanceImpl {
    fn change_asset_shares(&mut self, delta: I80F48) -> MarginfiResult;
    fn change_liability_shares(&mut self, delta: I80F48) -> MarginfiResult;
    fn close(&mut self, check_emissions: bool) -> MarginfiResult;
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

    fn close(&mut self, check_emissions: bool) -> MarginfiResult {
        if check_emissions {
            check!(
                I80F48::from(self.emissions_outstanding) < I80F48::ONE,
                MarginfiError::CannotCloseOutstandingEmissions
            );
        }

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

    /// Deposit an asset, will error if this repays a liability instead of increasing a asset
    pub fn deposit(&mut self, amount: I80F48) -> MarginfiResult {
        self.increase_balance_internal(amount, BalanceIncreaseType::DepositOnly)
    }

    /// Deposit an asset, ignoring repayment of liabilities. Useful only for banks where borrowing is disabled.
    pub fn deposit_no_repay(&mut self, amount: I80F48) -> MarginfiResult {
        self.increase_balance_internal(amount, BalanceIncreaseType::DepositOnly)
    }

    /// Repay a liability, will error if there is not enough liability - depositing is not allowed.
    pub fn repay(&mut self, amount: I80F48) -> MarginfiResult {
        self.increase_balance_internal(amount, BalanceIncreaseType::RepayOnly)
    }

    /// Withdraw an asset, will error if there is not enough asset - borrowing is not allowed.
    pub fn withdraw(&mut self, amount: I80F48) -> MarginfiResult {
        self.decrease_balance_internal(amount, BalanceDecreaseType::WithdrawOnly)
    }

    /// Incur a borrow, will error if this withdraws an asset instead of increasing a liability
    pub fn borrow(&mut self, amount: I80F48) -> MarginfiResult {
        self.decrease_balance_internal(amount, BalanceDecreaseType::BorrowOnly)
    }

    /// Deposit an asset, ignoring deposit caps, will error if this repays a liability instead of increasing a asset
    pub fn deposit_ignore_deposit_cap(&mut self, amount: I80F48) -> MarginfiResult {
        self.increase_balance_internal(amount, BalanceIncreaseType::BypassDepositLimit)
    }

    /// Incur a borrow, ignoring borrow caps, will error if this withdraws an asset instead of increasing a liability
    pub fn withdraw_ignore_borrow_cap(&mut self, amount: I80F48) -> MarginfiResult {
        self.decrease_balance_internal(amount, BalanceDecreaseType::BypassBorrowLimit)
    }

    /// Withdraw existing asset in full - will error if there is no asset.
    pub fn withdraw_all(&mut self) -> MarginfiResult<u64> {
        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

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

        balance.close(true)?;

        // Note: We do this so that banks in the receivership/deleverage flow are unlocked, as
        // closed-position banks will not make it to the "end" instruction to be unlocked there
        bank.cache.clear_liquidation_price_cache_locked();

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

        Ok(spl_withdraw_amount
            .checked_to_num()
            .ok_or_else(math_error!())?)
    }

    /// Repay existing liability in full - will error if there is no liability.
    pub fn repay_all(&mut self) -> MarginfiResult<u64> {
        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

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

        balance.close(true)?;

        // Note: We do this so that banks in the receivership/deleverage flow are unlocked, as
        // closed-position banks will not make it to the "end" instruction to be unlocked there
        bank.cache.clear_liquidation_price_cache_locked();

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

        Ok(spl_deposit_amount
            .checked_to_num()
            .ok_or_else(math_error!())?)
    }

    pub fn close_balance(&mut self) -> MarginfiResult<()> {
        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

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

        balance.close(true)?;

        Ok(())
    }

    // ------------ Internal accounting logic

    /// Note: in `BypassDepositLimit` mode, can flip a liability into an asset, a behavior that is used in liquidations.
    fn increase_balance_internal(
        &mut self,
        balance_delta: I80F48,
        operation_type: BalanceIncreaseType,
    ) -> MarginfiResult {
        debug!(
            "Balance increase: {} (type: {:?})",
            balance_delta, operation_type
        );

        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

        let balance = &mut self.balance;
        let bank = &mut self.bank;
        // Record if the balance was an asset/liability beforehand
        let had_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let had_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        let current_liability_shares: I80F48 = balance.liability_shares.into();
        let current_liability_amount = bank.get_liability_amount(current_liability_shares)?;

        let (liability_amount_decrease, asset_amount_increase) = (
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
            }
            BalanceIncreaseType::DepositOnly => {
                check!(
                    liability_amount_decrease.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationDepositOnly
                );
            }
            _ => {}
        }

        let asset_shares_increase = bank.get_asset_shares(asset_amount_increase)?;
        balance.change_asset_shares(asset_shares_increase)?;
        bank.change_asset_shares(
            asset_shares_increase,
            matches!(operation_type, BalanceIncreaseType::BypassDepositLimit),
        )?;

        let liability_shares_decrease = bank.get_liability_shares(liability_amount_decrease)?;
        // TODO: Use `IncreaseType` to skip certain balance updates, and save on compute.
        balance.change_liability_shares(-liability_shares_decrease)?;
        bank.change_liability_shares(-liability_shares_decrease, true)?;

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

        Ok(())
    }

    /// Note: in `BypassBorrowLimit` mode, can flip a deposit into a liability, a behavior that is used in liquidations.
    /// It will also ignore the utilization ratio check in this case, so that the liquidation can continue even if
    /// if the bank is so bankrupt that assets < liabs.
    fn decrease_balance_internal(
        &mut self,
        balance_delta: I80F48,
        operation_type: BalanceDecreaseType,
    ) -> MarginfiResult {
        debug!(
            "Balance decrease: {} of (type: {:?})",
            balance_delta, operation_type
        );

        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

        let balance = &mut self.balance;
        let bank = &mut self.bank;
        let had_assets =
            I80F48::from(balance.asset_shares).is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);
        let had_liabs = I80F48::from(balance.liability_shares)
            .is_positive_with_tolerance(ZERO_AMOUNT_THRESHOLD);

        let current_asset_shares: I80F48 = balance.asset_shares.into();
        let current_asset_amount = bank.get_asset_amount(current_asset_shares)?;

        let (asset_amount_decrease, liability_amount_increase) = (
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
            }
            BalanceDecreaseType::BorrowOnly => {
                check!(
                    asset_amount_decrease.is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
                    MarginfiError::OperationBorrowOnly
                );
            }
            _ => {}
        }

        let asset_shares_decrease = bank.get_asset_shares(asset_amount_decrease)?;
        balance.change_asset_shares(-asset_shares_decrease)?;
        bank.change_asset_shares(-asset_shares_decrease, false)?;

        let liability_shares_increase = bank.get_liability_shares(liability_amount_increase)?;
        balance.change_liability_shares(liability_shares_increase)?;
        bank.change_liability_shares(
            liability_shares_increase,
            matches!(operation_type, BalanceDecreaseType::BypassBorrowLimit),
        )?;

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

        Ok(())
    }

    /// Claim any unclaimed emissions and add them to the outstanding emissions amount.
    pub fn claim_emissions(&mut self, current_timestamp: u64) -> MarginfiResult {
        if let Some(balance_amount) = match (
            self.balance.get_side(),
            self.bank.get_flag(EMISSIONS_FLAG_LENDING_ACTIVE),
            self.bank.get_flag(EMISSIONS_FLAG_BORROW_ACTIVE),
        ) {
            (Some(BalanceSide::Assets), true, _) => Some(
                self.bank
                    .get_asset_amount(self.balance.asset_shares.into())?,
            ),
            (Some(BalanceSide::Liabilities), _, true) => Some(
                self.bank
                    .get_liability_amount(self.balance.liability_shares.into())?,
            ),
            _ => None,
        } {
            let last_update = if self.balance.last_update < MIN_EMISSIONS_START_TIME {
                current_timestamp
            } else {
                self.balance.last_update
            };
            let period = I80F48::from_num(
                current_timestamp
                    .checked_sub(last_update)
                    .ok_or_else(math_error!())?,
            );
            let emissions_rate = I80F48::from_num(self.bank.emissions_rate);
            let emissions = calc_emissions(
                period,
                balance_amount,
                self.bank.get_balance_decimals() as usize,
                emissions_rate,
            )?;

            let emissions_real = min(emissions, I80F48::from(self.bank.emissions_remaining));

            if emissions != emissions_real {
                msg!(
                    "Emissions capped: {} ({} calculated) for period {}s",
                    emissions_real,
                    emissions,
                    period
                );
            }

            debug!(
                "Outstanding emissions: {}",
                I80F48::from(self.balance.emissions_outstanding)
            );

            self.balance.emissions_outstanding = {
                I80F48::from(self.balance.emissions_outstanding)
                    .checked_add(emissions_real)
                    .ok_or_else(math_error!())?
            }
            .into();
            self.bank.emissions_remaining = {
                I80F48::from(self.bank.emissions_remaining)
                    .checked_sub(emissions_real)
                    .ok_or_else(math_error!())?
            }
            .into();
        }

        self.balance.last_update = current_timestamp;

        Ok(())
    }

    /// Claim any outstanding emissions, and return the max amount that can be withdrawn.
    pub fn settle_emissions_and_get_transfer_amount(&mut self) -> MarginfiResult<u64> {
        self.claim_emissions(Clock::get()?.unix_timestamp as u64)?;

        let outstanding_emissions_floored = I80F48::from(self.balance.emissions_outstanding)
            .checked_floor()
            .ok_or_else(math_error!())?;
        let new_outstanding_amount = I80F48::from(self.balance.emissions_outstanding)
            .checked_sub(outstanding_emissions_floored)
            .ok_or_else(math_error!())?;

        self.balance.emissions_outstanding = new_outstanding_amount.into();

        Ok(outstanding_emissions_floored
            .checked_to_num::<u64>()
            .ok_or_else(math_error!())?)
    }
}

/// Calculates the emissions based on the given period, balance amount, mint decimals,
/// emissions rate, and seconds per year.
///
/// Formula:
/// emissions = period * balance_amount / (10 ^ mint_decimals) * emissions_rate
///
/// # Arguments
///
/// * `period` - The period for which emissions are calculated.
/// * `balance_amount` - The balance amount used in the calculation.
/// * `mint_decimals` - The number of decimal places for the mint.
/// * `emissions_rate` - The emissions rate used in the calculation.
///
/// # Returns
///
/// The calculated emissions value.
fn calc_emissions(
    period: I80F48,
    balance_amount: I80F48,
    mint_decimals: usize,
    emissions_rate: I80F48,
) -> MarginfiResult<I80F48> {
    let exponent = EXP_10_I80F48[mint_decimals];
    let balance_amount_ui = balance_amount
        .checked_div(exponent)
        .ok_or_else(math_error!())?;

    let emissions = period
        .checked_mul(balance_amount_ui)
        .ok_or_else(math_error!())?
        .checked_div(SECONDS_PER_YEAR)
        .ok_or_else(math_error!())?
        .checked_mul(emissions_rate)
        .ok_or_else(math_error!())?;

    Ok(emissions)
}

#[cfg(test)]
mod test {
    use super::*;
    use fixed_macro::types::I80F48;

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
    fn test_calc_emissions() {
        let balance_amount: u64 = 106153222432271169;
        let emissions_rate = 1.5;

        // 1 second
        let period = 1;
        let emissions = calc_emissions(
            I80F48::from_num(period),
            I80F48::from_num(balance_amount),
            9,
            I80F48::from_num(emissions_rate),
        );
        assert!(emissions.is_ok());
        assert_eq!(emissions.unwrap(), I80F48::from_num(5.049144902600414));

        // 126 days
        let period = 126 * 24 * 60 * 60;
        let emissions = calc_emissions(
            I80F48::from_num(period),
            I80F48::from_num(balance_amount),
            9,
            I80F48::from_num(emissions_rate),
        );
        assert!(emissions.is_ok());

        // 2 years
        let period = 2 * 365 * 24 * 60 * 60;
        let emissions = calc_emissions(
            I80F48::from_num(period),
            I80F48::from_num(balance_amount),
            9,
            I80F48::from_num(emissions_rate),
        );
        assert!(emissions.is_ok());

        {
            // 10x balance amount
            let balance_amount = balance_amount * 10;
            let emissions = calc_emissions(
                I80F48::from_num(period),
                I80F48::from_num(balance_amount),
                9,
                I80F48::from_num(emissions_rate),
            );
            assert!(emissions.is_ok());
        }

        // 20 years + 100x emissions rate
        let period = 20 * 365 * 24 * 60 * 60;
        let emissions_rate = emissions_rate * 100.0;
        let emissions = calc_emissions(
            I80F48::from_num(period),
            I80F48::from_num(balance_amount),
            9,
            I80F48::from_num(emissions_rate),
        );
        assert!(emissions.is_ok());

        {
            // u64::MAX deposit amount
            let balance_amount = u64::MAX;
            let emissions_rate = emissions_rate;
            let emissions = calc_emissions(
                I80F48::from_num(period),
                I80F48::from_num(balance_amount),
                9,
                I80F48::from_num(emissions_rate),
            );
            assert!(emissions.is_ok());
        }

        {
            // 10000x emissions rate
            let balance_amount = u64::MAX;
            let emissions_rate = emissions_rate * 10000.;
            let emissions = calc_emissions(
                I80F48::from_num(period),
                I80F48::from_num(balance_amount),
                9,
                I80F48::from_num(emissions_rate),
            );
            assert!(emissions.is_ok());
        }

        {
            let balance_amount = I80F48::from_num(10000000);
            let emissions_rate = I80F48::from_num(1.5);
            let period = I80F48::from_num(10 * 24 * 60 * 60);

            let emissions = period
                .checked_mul(balance_amount)
                .unwrap()
                .checked_div(EXP_10_I80F48[9])
                .unwrap()
                .checked_mul(emissions_rate)
                .unwrap()
                .checked_div(SECONDS_PER_YEAR)
                .unwrap();

            let emissions_new = calc_emissions(period, balance_amount, 9, emissions_rate).unwrap();

            assert!(emissions_new - emissions < I80F48::from_num(0.00000001));
        }
    }
}
