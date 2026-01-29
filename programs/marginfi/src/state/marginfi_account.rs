use super::price::{
    OraclePriceFeedAdapter, OraclePriceType, OraclePriceWithConfidence, PriceAdapter, PriceBias,
};
use crate::{
    check, check_eq, debug, math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::{bank::BankImpl, bank_config::BankConfigImpl},
    utils::{is_integration_asset_tag, NumTraitsWithTolerance},
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_KAMINO, ASSET_TAG_SOL, ASSET_TAG_SOLEND,
        ASSET_TAG_STAKED, BANKRUPT_THRESHOLD, EMISSIONS_FLAG_BORROW_ACTIVE,
        EMISSIONS_FLAG_LENDING_ACTIVE, EXP_10_I80F48, MAX_INTEGRATION_POSITIONS,
        MIN_EMISSIONS_START_TIME, SECONDS_PER_YEAR, ZERO_AMOUNT_THRESHOLD,
    },
    types::{
        reconcile_emode_configs, Balance, BalanceSide, Bank, BankOperationalState, EmodeConfig,
        HealthCache, LendingAccount, MarginfiAccount, OracleSetup, RiskTier, ACCOUNT_DISABLED,
        ACCOUNT_FROZEN, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_RECEIVERSHIP,
    },
};
use std::cmp::{max, min};

/// Returns the number of remaining accounts required for a bank (bank account + oracle/venue accounts).
///
/// Account counts by oracle setup and asset tag:
/// - `Fixed`: 1 (bank only)
/// - `FixedStakedWithPythPush`: 3 (bank + lst_mint + stake_pool)
/// - `FixedKaminoPythPush` / `FixedKaminoSwitchboardPull`: 2 (bank + reserve)
/// - `ASSET_TAG_STAKED`: 4 (bank + oracle + lst_mint + stake_pool)
/// - `ASSET_TAG_KAMINO` / `ASSET_TAG_DRIFT` / `ASSET_TAG_SOLEND`: 3 (bank + oracle + reserve)
/// - `ASSET_TAG_DEFAULT` / `ASSET_TAG_SOL`: 2 (bank + oracle)
pub fn get_remaining_accounts_per_bank(bank: &Bank) -> MarginfiResult<usize> {
    match bank.config.oracle_setup {
        OracleSetup::Fixed => Ok(1),
        // Fixed + staking: bank + lst_mint + stake_pool (no oracle)
        OracleSetup::FixedStakedWithPythPush => Ok(3),
        // Fixed + Kamino: bank + reserve (no oracle)
        OracleSetup::FixedKaminoPythPush | OracleSetup::FixedKaminoSwitchboardPull => Ok(2),
        _ => get_remaining_accounts_per_asset_tag(bank.config.asset_tag),
    }
}

/// 4 for `ASSET_TAG_STAKED` (bank, oracle, lst mint, lst pool), 2 for most others (bank, oracle), 3
/// for Kamino (bank, oracle, reserve), 1 for Fixed
fn get_remaining_accounts_per_asset_tag(asset_tag: u8) -> MarginfiResult<usize> {
    match asset_tag {
        ASSET_TAG_DEFAULT | ASSET_TAG_SOL => Ok(2),
        ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND => Ok(3),
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
/// 2. If the account is frozen → `true` only if signer is the group admin
/// 3. Otherwise → `true` only if signer is the account authority
pub fn is_signer_authorized(
    marginfi_account: &MarginfiAccount,
    group_admin: Pubkey,
    signer: Pubkey,
    allow_receivership: bool,
) -> bool {
    if allow_receivership && marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP) {
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

pub struct BankAccountWithPriceFeed<'a, 'info> {
    bank: AccountLoader<'info, Bank>,
    price_feed: Box<MarginfiResult<OraclePriceFeedAdapter>>,
    balance: &'a Balance,
}

impl<'info> BankAccountWithPriceFeed<'_, 'info> {
    pub fn load<'a>(
        lending_account: &'a LendingAccount,
        remaining_ais: &'info [AccountInfo<'info>],
    ) -> MarginfiResult<Vec<BankAccountWithPriceFeed<'a, 'info>>> {
        let clock = Clock::get()?;
        let mut account_index = 0;

        lending_account
            .balances
            .iter()
            .filter(|balance| balance.is_active())
            .map(|balance| {
                // Get the bank
                let bank_ai: Option<&AccountInfo<'info>> = remaining_ais.get(account_index);
                if bank_ai.is_none() {
                    msg!("Ran out of remaining accounts at {:?}", account_index);
                    return err!(MarginfiError::InvalidBankAccount);
                }
                let bank_ai = bank_ai.unwrap();
                let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
                let bank = bank_al.load()?;

                // Determine number of accounts to process for this bank
                let num_accounts = get_remaining_accounts_per_bank(&bank)?;
                check_eq!(
                    balance.bank_pk,
                    *bank_ai.key,
                    MarginfiError::InvalidBankAccount
                );

                // Get the oracle, and the LST mint and sol pool if applicable (staked only)
                let oracle_ai_idx = account_index + 1;
                let end_idx = oracle_ai_idx + num_accounts - 1;
                require_gte!(
                    remaining_ais.len(),
                    end_idx,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                let oracle_ais = &remaining_ais[oracle_ai_idx..end_idx];

                let price_adapter = Box::new(OraclePriceFeedAdapter::try_from_bank(
                    &bank, oracle_ais, &clock,
                ));

                account_index += num_accounts;

                Ok(BankAccountWithPriceFeed {
                    bank: bank_al.clone(),
                    price_feed: price_adapter,
                    balance,
                })
            })
            .collect::<Result<Vec<_>>>()
    }

    #[inline(always)]
    /// Calculate the value of the balance, which is either an asset or a liability. If it is an
    /// asset, returns (asset_value, 0, price, 0), and if it is a liability, returns (0, liabilty
    /// value, price, 0), where price is the actual oracle price used to determine the value after
    /// bias adjustments, etc.
    ///
    /// err_code is an internal oracle error code in the event the oracle did not load. This applies
    /// only to assets and the return type will always be (0, 0, 0, err_code).
    ///
    /// Nuances:
    /// 1. Maintenance requirement is calculated using the real time price feed.
    /// 2. Initial requirement is calculated using the time weighted price feed, if available.
    /// 3. Initial requirement is discounted by the initial discount, if enabled and the usd limit
    ///    is exceeded.
    /// 4. Assets are only calculated for collateral risk tier.
    /// 5. Oracle errors are ignored for deposits in isolated risk tier.
    fn calc_weighted_value(
        &self,
        requirement_type: RequirementType,
        emode_config: &EmodeConfig,
    ) -> MarginfiResult<(I80F48, I80F48, I80F48, u32)> {
        match self.balance.get_side() {
            Some(side) => {
                let bank = &self.bank.load()?;

                match side {
                    BalanceSide::Assets => {
                        let (value, price, err_code) =
                            self.calc_weighted_asset_value(requirement_type, bank, emode_config)?;
                        Ok((value, I80F48::ZERO, price, err_code))
                    }

                    BalanceSide::Liabilities => {
                        let (value, price) =
                            self.calc_weighted_liab_value(requirement_type, bank)?;
                        Ok((I80F48::ZERO, value, price, 0))
                    }
                }
            }
            None => Ok((I80F48::ZERO, I80F48::ZERO, I80F48::ZERO, 0)),
        }
    }

    /// Returns value, the price used to determine that value, and an error code if applicable. In
    /// most cases, returns (value, price, 0). If there was an error loading the price feed, treats
    /// the price as zero, and passes the u32 argument that contains the error code, i.e. the return
    /// type is (0, 0, err_code). Other types of errors (e.g. math) will still throw.
    #[inline(always)]
    fn calc_weighted_asset_value(
        &self,
        requirement_type: RequirementType,
        bank: &Bank,
        emode_config: &EmodeConfig,
    ) -> MarginfiResult<(I80F48, I80F48, u32)> {
        match bank.config.risk_tier {
            RiskTier::Collateral => {
                // ReduceOnly banks should not be counted as collateral for new loans (Initial checks)
                // but should maintain their value for existing positions (Maintenance checks)
                if matches!(
                    (bank.config.operational_state, requirement_type),
                    (BankOperationalState::ReduceOnly, RequirementType::Initial)
                ) {
                    debug!("ReduceOnly bank assets worth 0 for Initial margin");
                    return Ok((I80F48::ZERO, I80F48::ZERO, 0));
                }

                let (price_feed, err_code) = self.try_get_price_feed();

                if matches!(
                    (&price_feed, requirement_type),
                    (&Err(_), RequirementType::Initial)
                ) {
                    debug!("Skipping stale oracle");
                    return Ok((I80F48::ZERO, I80F48::ZERO, err_code));
                }

                let price_feed = price_feed?;

                // If an emode entry exists for this bank's emode tag in the reconciled config of
                // all borrowing banks, use its weight, otherwise use the weight designated on the
                // collateral bank itself. If the bank's weight is higher, always use that weight.
                let mut asset_weight =
                    if let Some(emode_entry) = emode_config.find_with_tag(bank.emode.emode_tag) {
                        let bank_weight = bank
                            .config
                            .get_weight(requirement_type, BalanceSide::Assets);
                        let emode_weight = match requirement_type {
                            RequirementType::Initial => I80F48::from(emode_entry.asset_weight_init),
                            RequirementType::Maintenance => {
                                I80F48::from(emode_entry.asset_weight_maint)
                            }
                            // Note: For equity (which is only used for bankruptcies) emode does not
                            // apply, as the asset weight is always 1
                            RequirementType::Equity => I80F48::ONE,
                        };
                        max(bank_weight, emode_weight)
                    } else {
                        bank.config
                            .get_weight(requirement_type, BalanceSide::Assets)
                    };

                let lower_price = price_feed.get_price_of_type(
                    requirement_type.get_oracle_price_type(),
                    Some(PriceBias::Low),
                    bank.config.oracle_max_confidence,
                )?;

                if matches!(requirement_type, RequirementType::Initial) {
                    if let Some(discount) =
                        bank.maybe_get_asset_weight_init_discount(lower_price)?
                    {
                        asset_weight = asset_weight
                            .checked_mul(discount)
                            .ok_or_else(math_error!())?;
                    }
                }
                let value = calc_value(
                    bank.get_asset_amount(self.balance.asset_shares.into())?,
                    lower_price,
                    bank.get_balance_decimals(),
                    Some(asset_weight),
                )?;

                Ok((value, lower_price, 0))
            }
            RiskTier::Isolated => Ok((I80F48::ZERO, I80F48::ZERO, 0)),
        }
    }

    /// Returns value, the net liability value in $, and the price used to determine that value.
    /// Note that an error in liability value always throws, unlike assets which simply return 0.
    #[inline(always)]
    fn calc_weighted_liab_value(
        &self,
        requirement_type: RequirementType,
        bank: &Bank,
    ) -> MarginfiResult<(I80F48, I80F48)> {
        let (price_feed, _) = self.try_get_price_feed();
        let price_feed = price_feed?;
        let liability_weight = bank
            .config
            .get_weight(requirement_type, BalanceSide::Liabilities);

        let higher_price = price_feed.get_price_of_type(
            requirement_type.get_oracle_price_type(),
            Some(PriceBias::High),
            bank.config.oracle_max_confidence,
        )?;

        // If `ASSET_TAG_STAKED` assets can ever be borrowed, accomodate for that here...

        let value = calc_value(
            bank.get_liability_amount(self.balance.liability_shares.into())?,
            higher_price,
            bank.get_balance_decimals(),
            Some(liability_weight),
        )?;

        Ok((value, higher_price))
    }

    fn try_get_price_feed(&self) -> (MarginfiResult<&OraclePriceFeedAdapter>, u32) {
        match self.price_feed.as_ref() {
            Ok(a) => (Ok(a), 0),
            #[allow(unused_variables)]
            Err(e) => match e {
                anchor_lang::error::Error::AnchorError(inner) => {
                    let error_code = inner.as_ref().error_code_number;
                    let custom_error = MarginfiError::from(error_code);
                    (Err(error!(custom_error)), error_code)
                }
                anchor_lang::error::Error::ProgramError(inner) => {
                    match inner.as_ref().program_error {
                        ProgramError::Custom(error_code) => {
                            let custom_error = MarginfiError::from(error_code);
                            (Err(error!(custom_error)), error_code)
                        }
                        _ => (
                            Err(error!(MarginfiError::InternalLogicError)),
                            MarginfiError::InternalLogicError as u32,
                        ),
                    }
                }
            },
        }
    }

    #[inline]
    pub fn is_empty(&self, side: BalanceSide) -> bool {
        self.balance.is_empty(side)
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

pub struct RiskEngine<'a, 'info> {
    marginfi_account: &'a MarginfiAccount,
    bank_accounts_with_price: Vec<BankAccountWithPriceFeed<'a, 'info>>,
    emode_config: EmodeConfig,
}

impl<'info> RiskEngine<'_, 'info> {
    pub fn new<'a>(
        marginfi_account: &'a MarginfiAccount,
        remaining_ais: &'info [AccountInfo<'info>],
    ) -> MarginfiResult<RiskEngine<'a, 'info>> {
        check!(
            !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::AccountInFlashloan
        );

        Self::new_no_flashloan_check(marginfi_account, remaining_ais)
    }

    /// Internal constructor used either after manually checking account is not in a flashloan,
    /// or explicity checking health for flashloan enabled actions.
    fn new_no_flashloan_check<'a>(
        marginfi_account: &'a MarginfiAccount,
        remaining_ais: &'info [AccountInfo<'info>],
    ) -> MarginfiResult<RiskEngine<'a, 'info>> {
        let bank_accounts_with_price =
            BankAccountWithPriceFeed::load(&marginfi_account.lending_account, remaining_ais)?;

        // Load the reconciled Emode configuration for all banks where the user has borrowed
        let reconciled_emode_config = reconcile_emode_configs(
            bank_accounts_with_price
                .iter()
                .filter(|b| !b.balance.is_empty(BalanceSide::Liabilities))
                .map(|b| b.bank.load().unwrap().emode.emode_config),
        );

        Ok(RiskEngine {
            marginfi_account,
            bank_accounts_with_price,
            emode_config: reconciled_emode_config,
        })
    }

    /// Checks account is healthy after performing actions that increase risk (removing liquidity).
    ///
    /// `ACCOUNT_IN_FLASHLOAN` behavior.
    /// - Health check is skipped.
    /// - `remaining_ais` can be an empty vec.
    /// * Returns a Some(RiskEngine) if creating the engine didn't error, even if the risk check itself did error.
    pub fn check_account_init_health<'a>(
        marginfi_account: &'a MarginfiAccount,
        remaining_ais: &'info [AccountInfo<'info>],
        health_cache: &mut Option<&mut HealthCache>,
    ) -> (MarginfiResult, Option<RiskEngine<'a, 'info>>) {
        if marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN) {
            // Note: All risk, including the health cache, is not applicable during flashloans
            return (Ok(()), None);
        }

        let risk_engine = match Self::new_no_flashloan_check(marginfi_account, remaining_ais) {
            Ok(engine) => engine,
            Err(e) => return (Err(e), None),
        };
        let requirement_type = RiskRequirementType::Initial;
        let risk_engine_result = risk_engine.check_account_health(requirement_type, health_cache);

        (risk_engine_result, Some(risk_engine))
    }

    /// Returns the total assets and liabilities of the account in the form of (assets, liabilities)
    pub fn get_account_health_components(
        &self,
        requirement_type: RiskRequirementType,
        health_cache: &mut Option<&mut HealthCache>,
    ) -> MarginfiResult<(I80F48, I80F48)> {
        let mut total_assets: I80F48 = I80F48::ZERO;
        let mut total_liabilities: I80F48 = I80F48::ZERO;
        const NO_INDEX_FOUND: usize = 255;
        let mut first_err_index = NO_INDEX_FOUND;

        for (i, bank_account) in self.bank_accounts_with_price.iter().enumerate() {
            let requirement_type = requirement_type.to_weight_type();
            let (asset_val, liab_val, price, err_code) =
                bank_account.calc_weighted_value(requirement_type, &self.emode_config)?;
            if err_code != 0 && first_err_index == NO_INDEX_FOUND {
                first_err_index = i;
                if let Some(cache) = health_cache {
                    cache.err_index = i as u8;
                    cache.internal_err = err_code;
                };
            }

            if let Some(health_cache) = health_cache {
                // Note: We only record the Initial weighted price in cache, at some point we may
                // record others.
                if let RequirementType::Initial = requirement_type {
                    health_cache.prices[i] = price.to_num::<f64>().to_le_bytes();
                }
            }

            debug!(
                "Balance {}, assets: {}, liabilities: {}",
                bank_account.balance.bank_pk, asset_val, liab_val
            );

            total_assets = total_assets
                .checked_add(asset_val)
                .ok_or_else(math_error!())?;
            total_liabilities = total_liabilities
                .checked_add(liab_val)
                .ok_or_else(math_error!())?;
        }

        if let Some(health_cache) = health_cache {
            match requirement_type {
                RiskRequirementType::Initial => {
                    health_cache.asset_value = total_assets.into();
                    health_cache.liability_value = total_liabilities.into();
                }
                RiskRequirementType::Maintenance => {
                    health_cache.asset_value_maint = total_assets.into();
                    health_cache.liability_value_maint = total_liabilities.into();
                }
                RiskRequirementType::Equity => {
                    health_cache.asset_value_equity = total_assets.into();
                    health_cache.liability_value_equity = total_liabilities.into();
                }
            }
        }

        Ok((total_assets, total_liabilities))
    }

    pub fn get_unbiased_price_for_bank(
        &self,
        bank_pk: &Pubkey,
    ) -> MarginfiResult<OraclePriceWithConfidence> {
        let bank_account = self
            .bank_accounts_with_price
            .iter()
            .find(|b| b.balance.bank_pk == *bank_pk)
            .ok_or_else(|| error!(MarginfiError::BankAccountNotFound))?;

        let bank = bank_account.bank.load()?;
        let (price_feed_res, _) = bank_account.try_get_price_feed();
        let price_feed = price_feed_res?;

        let price = price_feed.get_price_and_confidence_of_type(
            OraclePriceType::RealTime,
            bank.config.oracle_max_confidence,
        )?;

        Ok(price)
    }

    /// Errors if risk account's liabilities exceed their assets.
    fn check_account_health(
        &self,
        requirement_type: RiskRequirementType,
        health_cache: &mut Option<&mut HealthCache>,
    ) -> MarginfiResult {
        let (total_weighted_assets, total_weighted_liabilities) =
            self.get_account_health_components(requirement_type, health_cache)?;

        let healthy = total_weighted_assets >= total_weighted_liabilities;

        if healthy {
            debug!(
                "check_health: assets {} - liabs: {}",
                total_weighted_assets, total_weighted_liabilities
            );
        } else {
            let assets_f64: f64 = total_weighted_assets.to_num();
            let liabs_f64: f64 = total_weighted_liabilities.to_num();
            msg!("check_health: assets {} - liabs: {}", assets_f64, liabs_f64);
        }

        if let Some(cache) = health_cache {
            cache.set_healthy(healthy);
        }

        if !healthy {
            return err!(MarginfiError::RiskEngineInitRejected);
        }

        self.check_account_risk_tiers()?;

        Ok(())
    }

    // TODO rename to something more appropriate (we do this pre and post liquidation in
    // receivership liquidation, as we no longer care about the per-position check in post)
    /// Checks
    /// 1. Account is liquidatable
    /// 2. Account has an outstanding liability for the provided liability bank. This check is
    ///    ignored if passing None.
    /// * returns - account health (assets - liabs), asset, liabs.
    pub fn check_pre_liquidation_condition_and_get_account_health(
        &self,
        bank_pk: Option<&Pubkey>,
        health_cache: &mut Option<&mut HealthCache>,
        ignore_healthy: bool,
    ) -> MarginfiResult<(I80F48, I80F48, I80F48)> {
        check!(
            !self.marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::AccountInFlashloan
        );

        if bank_pk.is_some() {
            let liability_bank_balance = self
                .bank_accounts_with_price
                .iter()
                .find(|a| a.balance.bank_pk == *bank_pk.unwrap())
                .ok_or(MarginfiError::LendingAccountBalanceNotFound)?;

            check!(
                !liability_bank_balance.is_empty(BalanceSide::Liabilities),
                MarginfiError::NoLiabilitiesInLiabilityBank
            );

            check!(
                liability_bank_balance.is_empty(BalanceSide::Assets),
                MarginfiError::AssetsInLiabilityBank
            );
        }

        let (assets, liabs) =
            self.get_account_health_components(RiskRequirementType::Maintenance, health_cache)?;

        let account_health = assets.checked_sub(liabs).ok_or_else(math_error!())?;
        let healthy = account_health > I80F48::ZERO;

        if let Some(cache) = health_cache {
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

    /// Check that the account is at most at the maintenance requirement level post liquidation.
    /// This check is used to ensure two things in the liquidation process:
    /// 1. We check that the liquidatee's remaining liability is not empty
    /// 2. Liquidatee account was below the maintenance requirement level before liquidation (as health can only increase, because liquidations always pay down liabilities)
    /// 3. Liquidator didn't liquidate too many assets that would result in unnecessary loss for the liquidatee.
    ///
    /// This check works on the assumption that the liquidation always results in a reduction of risk.
    ///
    /// 1. We check that the paid off liability is not zero. Assuming the liquidation always pays off some liability, this ensures that the liquidation was not too large.
    /// 2. We check that the account is still at most at the maintenance requirement level. This ensures that the liquidation was not too large overall.
    pub fn check_post_liquidation_condition_and_get_account_health(
        &self,
        bank_pk: &Pubkey,
        pre_liquidation_health: I80F48,
    ) -> MarginfiResult<I80F48> {
        check!(
            !self.marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::AccountInFlashloan
        );

        let liability_bank_balance = self
            .bank_accounts_with_price
            .iter()
            .find(|a| a.balance.bank_pk == *bank_pk)
            .unwrap();

        check!(
            !liability_bank_balance.is_empty(BalanceSide::Liabilities),
            MarginfiError::ExhaustedLiability
        );

        check!(
            liability_bank_balance.is_empty(BalanceSide::Assets),
            MarginfiError::TooSeverePayoff
        );

        let (assets, liabs) =
            self.get_account_health_components(RiskRequirementType::Maintenance, &mut None)?;

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

    /// Check that the account is in a bankrupt state. Account needs to be insolvent and total value
    /// of assets need to be below the bankruptcy threshold.
    ///
    /// * returns assets, liabilities in EQUITY value terms.
    pub fn check_account_bankrupt(
        &self,
        health_cache: &mut Option<&mut HealthCache>,
    ) -> MarginfiResult<(I80F48, I80F48)> {
        let (total_assets, total_liabilities) =
            self.get_account_health_components(RiskRequirementType::Equity, health_cache)?;

        // TODO remove this check here and raise it to the top-level instruction
        check!(
            !self.marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::AccountInFlashloan
        );

        msg!(
            "check_bankrupt: assets {} - liabs: {}",
            total_assets,
            total_liabilities
        );

        check!(
            total_assets < total_liabilities,
            MarginfiError::AccountNotBankrupt
        );
        check!(
            total_assets < BANKRUPT_THRESHOLD && total_liabilities > ZERO_AMOUNT_THRESHOLD,
            MarginfiError::AccountNotBankrupt
        );

        Ok((total_assets, total_liabilities))
    }

    fn check_account_risk_tiers(&self) -> MarginfiResult {
        let mut isolated_risk_count = 0;
        let mut total_liability_balances = 0;

        for account in self.bank_accounts_with_price.iter() {
            if account.balance.is_empty(BalanceSide::Liabilities) {
                continue;
            }
            total_liability_balances += 1;

            let bank = account.bank.load()?;
            if bank.config.risk_tier == RiskTier::Isolated {
                isolated_risk_count += 1;
                // Early exit if we find more than one isolated risk tier with liabilities
                if isolated_risk_count > 1 {
                    break;
                }
            }
        }

        check!(
            isolated_risk_count == 0 || total_liability_balances == 1,
            MarginfiError::IsolatedAccountIllegalState
        );

        Ok(())
    }
}

pub trait LendingAccountImpl {
    fn get_first_empty_balance(&self) -> Option<usize>;
    fn sort_balances(&mut self);
}

impl LendingAccountImpl for LendingAccount {
    fn get_first_empty_balance(&self) -> Option<usize> {
        self.balances.iter().position(|b| !b.is_active())
    }

    fn sort_balances(&mut self) {
        // Sort all balances in descending order by bank_pk
        self.balances.sort_by(|a, b| b.bank_pk.cmp(&a.bank_pk));
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

                    check!(
                        integration_position_count < MAX_INTEGRATION_POSITIONS,
                        MarginfiError::IntegrationPositionLimitExceeded
                    );
                }

                let empty_index = lending_account
                    .get_first_empty_balance()
                    .ok_or_else(|| error!(MarginfiError::LendingAccountBalanceSlotsFull))?;

                lending_account.balances[empty_index] = Balance {
                    active: 1,
                    bank_pk: *bank_pk,
                    bank_asset_tag: bank.config.asset_tag,
                    _pad0: [0; 6],
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
