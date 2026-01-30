use crate::constants::{
    MIN_PYTH_PUSH_VERIFICATION_LEVEL, NATIVE_STAKE_ID, PYTH_ID, SPL_SINGLE_POOL_ID,
    SWITCHBOARD_PULL_ID,
};
use crate::state::bank_config::BankConfigImpl;
use crate::{check, check_eq, debug, live, math_error, prelude::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{borsh1::try_from_slice_unchecked, stake::state::StakeStateV2};
use anchor_spl::token::Mint;
use enum_dispatch::enum_dispatch;
use fixed::types::I80F48;
use kamino_mocks::state::MinimalReserve;
use marginfi_type_crate::{
    constants::{
        CONF_INTERVAL_MULTIPLE, EXP_10_I80F48, MAX_CONF_INTERVAL, STD_DEV_MULTIPLE, U32_MAX,
        U32_MAX_DIV_10,
    },
    types::{adjust_i128, adjust_i64, adjust_u64, Bank, BankConfig, OracleSetup},
};
use pyth_solana_receiver_sdk::price_update::{self, FeedId, PriceUpdateV2};
use pyth_solana_receiver_sdk::PYTH_PUSH_ORACLE_ID;
use std::{cell::Ref, cmp::min};
use switchboard_on_demand::{
    CurrentResult, Discriminator, PullFeedAccountData, SPL_TOKEN_PROGRAM_ID,
};

#[derive(Copy, Clone, Debug)]
pub enum PriceBias {
    Low,
    High,
}

#[derive(Copy, Clone, Debug)]
pub struct OraclePriceWithConfidence {
    pub price: I80F48,
    pub confidence: I80F48,
}

#[derive(Copy, Clone, Debug)]
pub enum OraclePriceType {
    /// Time weighted price
    /// EMA for PythEma
    TimeWeighted,
    /// Real time price
    RealTime,
}

#[enum_dispatch]
pub trait PriceAdapter {
    fn get_price_and_confidence_of_type(
        &self,
        oracle_price_type: OraclePriceType,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<OraclePriceWithConfidence>;

    fn get_price_of_type(
        &self,
        oracle_price_type: OraclePriceType,
        bias: Option<PriceBias>,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<I80F48>;

    fn get_price_of_type_ignore_conf(
        &self,
        t: OraclePriceType,
        b: Option<PriceBias>,
    ) -> MarginfiResult<I80F48> {
        self.get_price_of_type(t, b, u32::MAX)
    }
}

#[enum_dispatch(PriceAdapter)]
#[cfg_attr(feature = "client", derive(Clone))]
pub enum OraclePriceFeedAdapter {
    PythPushOracle(PythPushOraclePriceFeed),
    SwitchboardPull(SwitchboardPullPriceFeed),
    Fixed(FixedPriceFeed),
}

impl OraclePriceFeedAdapter {
    pub fn try_from_bank<'info>(
        bank: &Bank,
        ais: &'info [AccountInfo<'info>],
        clock: &Clock,
    ) -> MarginfiResult<Self> {
        Self::try_from_bank_with_max_age(bank, ais, clock, bank.config.get_oracle_max_age())
    }

    pub fn try_from_bank_with_max_age<'info>(
        bank: &Bank,
        ais: &'info [AccountInfo<'info>],
        clock: &Clock,
        max_age: u64,
    ) -> MarginfiResult<Self> {
        let bank_config = &bank.config;
        match bank_config.oracle_setup {
            OracleSetup::None => Err(MarginfiError::OracleNotSetup.into()),
            OracleSetup::PythLegacy => {
                panic!("pyth legacy is deprecated");
            }
            OracleSetup::SwitchboardV2 => {
                panic!("swb v2 is deprecated");
            }
            OracleSetup::PythPushOracle => {
                check!(ais.len() == 1, MarginfiError::WrongNumberOfOracleAccounts);

                let account_info = &ais[0];

                if live!() {
                    check_eq!(
                        *account_info.owner,
                        pyth_solana_receiver_sdk::id(),
                        MarginfiError::PythPushWrongAccountOwner
                    );
                } else {
                    // On localnet, allow the mock program ID -OR- the real one
                    let owner_ok = account_info.owner.eq(&PYTH_ID)
                        || account_info.owner.eq(&pyth_solana_receiver_sdk::id());
                    check!(owner_ok, MarginfiError::PythPushWrongAccountOwner);
                }

                require_keys_eq!(
                    *account_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                Ok(OraclePriceFeedAdapter::PythPushOracle(
                    PythPushOraclePriceFeed::load_checked(account_info, clock, max_age)?,
                ))
            }
            OracleSetup::SwitchboardPull => {
                check!(ais.len() == 1, MarginfiError::WrongNumberOfOracleAccounts);
                if ais[0].key != &bank_config.oracle_keys[0] {
                    msg!(
                        "Expected oracle key: {:?}, got: {:?}",
                        bank_config.oracle_keys[0],
                        ais[0].key
                    );
                    return Err(error!(MarginfiError::WrongOracleAccountKeys));
                }

                Ok(OraclePriceFeedAdapter::SwitchboardPull(
                    SwitchboardPullPriceFeed::load_checked(&ais[0], clock.unix_timestamp, max_age)?,
                ))
            }
            OracleSetup::StakedWithPythPush => {
                check!(ais.len() == 3, MarginfiError::WrongNumberOfOracleAccounts);

                if ais[1].key != &bank_config.oracle_keys[1]
                    || ais[2].key != &bank_config.oracle_keys[2]
                {
                    msg!(
                        "Expected oracle keys: [1] {:?}, [2] {:?}, got: [1] {:?}, [2] {:?}",
                        bank_config.oracle_keys[1],
                        bank_config.oracle_keys[2],
                        ais[1].key,
                        ais[2].key
                    );
                    return Err(error!(MarginfiError::WrongOracleAccountKeys));
                }

                let lst_mint = Account::<'info, Mint>::try_from(&ais[1]).unwrap();
                let lst_supply = lst_mint.supply;
                check!(lst_supply > 0, MarginfiError::ZeroSupplyInStakePool);
                let stake_state = try_from_slice_unchecked::<StakeStateV2>(&ais[2].data.borrow())?;
                let (_, stake) = match stake_state {
                    StakeStateV2::Stake(meta, stake, _) => (meta, stake),
                    _ => panic!("unsupported stake state"), // TODO emit more specific error
                };
                let sol_pool_balance = stake.delegation.stake;
                // Note: When the pool is fresh, it has 1 SOL in it (an initial and non-refundable
                // balance that will stay in the pool forever). We don't want to include that
                // balance when reading the quantity of SOL that has been staked from actual
                // depositors (i.e. the amount that can actually be redeemed again).
                let lamports_per_sol: u64 = 1_000_000_000;
                let sol_pool_adjusted_balance = sol_pool_balance
                    .checked_sub(lamports_per_sol)
                    .ok_or_else(math_error!())?;
                // Note: exchange rate is `sol_pool_balance / lst_supply`, but we will do the
                // division last to avoid precision loss. Division does not need to be
                // decimal-adjusted because both SOL and stake positions use 9 decimals

                let account_info = &ais[0];
                require_keys_eq!(
                    *account_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                if live!() {
                    check_eq!(
                        account_info.owner,
                        &pyth_solana_receiver_sdk::id(),
                        MarginfiError::StakedPythPushWrongAccountOwner
                    );
                } else {
                    // On localnet, allow the mock program ID OR the real one (for regression tests against
                    // actual mainnet accounts).
                    // * Note: Typically price updates are owned by `pyth_solana_receiver_sdk` and the oracle
                    // feed account itself is owned by PYTH ID. On localnet, the mock program may own both for
                    // simplicity.
                    let owner_ok = account_info.owner.eq(&PYTH_ID)
                        || account_info.owner.eq(&pyth_solana_receiver_sdk::id());
                    check!(owner_ok, MarginfiError::StakedPythPushWrongAccountOwner);
                }

                let mut feed = PythPushOraclePriceFeed::load_checked(account_info, clock, max_age)?;

                let adjusted_price = (feed.price.price as i128)
                    .checked_mul(sol_pool_adjusted_balance as i128)
                    .ok_or_else(math_error!())?
                    .checked_div(lst_supply as i128)
                    .ok_or_else(math_error!())?;
                feed.price.price = adjusted_price.try_into().unwrap();

                let adjusted_ema_price = (feed.ema_price.price as i128)
                    .checked_mul(sol_pool_adjusted_balance as i128)
                    .ok_or_else(math_error!())?
                    .checked_div(lst_supply as i128)
                    .ok_or_else(math_error!())?;
                feed.ema_price.price = adjusted_ema_price.try_into().unwrap();

                let price = OraclePriceFeedAdapter::PythPushOracle(feed);
                Ok(price)
            }
            OracleSetup::KaminoPythPush => {
                // (1) Pyth oracle (for price)_and (2) Kamino reserve (for exchange rate)
                check!(ais.len() == 2, MarginfiError::WrongNumberOfOracleAccounts);

                let account_info = &ais[0];
                let reserve_info = &ais[1];

                // Validate oracle account matches expected key (new pattern)
                require_keys_eq!(
                    *account_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                check_eq!(
                    *reserve_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::KaminoReserveValidationFailed
                );

                // Verifies owner + discriminator automatically
                let reserve_loader: AccountLoader<MinimalReserve> =
                    AccountLoader::try_from(reserve_info)
                        .map_err(|_| MarginfiError::KaminoReserveValidationFailed)?;
                let reserve = reserve_loader.load()?;
                let is_stale = reserve.is_stale(clock.slot);
                if is_stale {
                    // msg!(
                    //     "stale. slot now: {:?} but has: {:?}, stale flag: {:?}",
                    //     clock.slot,
                    //     reserve.slot,
                    //     reserve.stale
                    // );
                    return err!(MarginfiError::ReserveStale);
                }

                if live!() {
                    require_keys_eq!(
                        *account_info.owner,
                        pyth_solana_receiver_sdk::id(),
                        MarginfiError::PythPushWrongAccountOwner
                    );
                } else {
                    // Localnet only
                    // On localnet, allow the mock program ID -OR- the real one
                    let owner_ok = account_info.owner.eq(&PYTH_ID)
                        || account_info.owner.eq(&pyth_solana_receiver_sdk::id());
                    check!(owner_ok, MarginfiError::PythPushWrongAccountOwner);
                };

                // Use new pattern: no feed_id parameter needed
                let mut price_feed =
                    PythPushOraclePriceFeed::load_checked(account_info, clock, max_age)?;

                let (total_liq, total_col) = reserve.scaled_supplies()?;
                if total_col > I80F48::ZERO {
                    let liq_to_col_ratio = total_liq / total_col;

                    // Adjust prices & confidence in place
                    price_feed.price.price = adjust_i64(price_feed.price.price, liq_to_col_ratio)
                        .ok_or_else(math_error!())?;
                    price_feed.ema_price.price =
                        adjust_i64(price_feed.ema_price.price, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                    price_feed.price.conf = adjust_u64(price_feed.price.conf, liq_to_col_ratio)
                        .ok_or_else(math_error!())?;
                    price_feed.ema_price.conf =
                        adjust_u64(price_feed.ema_price.conf, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                }

                Ok(OraclePriceFeedAdapter::PythPushOracle(price_feed))
            }
            OracleSetup::KaminoSwitchboardPull => {
                // (1) Switchboard oracle (for price) and (2) Kamino reserve (for exchange rate)
                check!(ais.len() == 2, MarginfiError::WrongNumberOfOracleAccounts);

                let oracle_info = &ais[0];
                let reserve_info = &ais[1];

                require_keys_eq!(
                    *oracle_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                require_keys_eq!(
                    *reserve_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::KaminoReserveValidationFailed
                );

                // Verifies owner + discriminator automatically
                let reserve_loader: AccountLoader<MinimalReserve> =
                    AccountLoader::try_from(reserve_info)
                        .map_err(|_| MarginfiError::KaminoReserveValidationFailed)?;
                let reserve = reserve_loader.load()?;
                let is_stale = reserve.is_stale(clock.slot);
                if is_stale {
                    return err!(MarginfiError::ReserveStale);
                }

                let mut price_feed = SwitchboardPullPriceFeed::load_checked(
                    oracle_info,
                    clock.unix_timestamp,
                    max_age,
                )?;

                let (total_liq, total_col) = reserve.scaled_supplies()?;
                if total_col > I80F48::ZERO {
                    let liq_to_col_ratio = total_liq / total_col;

                    // Adjust Switchboard value & std_dev (i128 with 1e18 precision)
                    price_feed.feed.result.value =
                        adjust_i128(price_feed.feed.result.value, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                    price_feed.feed.result.std_dev =
                        adjust_i128(price_feed.feed.result.std_dev, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                }

                Ok(OraclePriceFeedAdapter::SwitchboardPull(price_feed))
            }
            OracleSetup::Fixed => {
                check!(ais.is_empty(), MarginfiError::WrongNumberOfOracleAccounts);

                let price: I80F48 = bank.config.fixed_price.into();
                check!(
                    price >= I80F48::ZERO,
                    MarginfiError::FixedOraclePriceNegative
                );

                Ok(OraclePriceFeedAdapter::Fixed(FixedPriceFeed { price }))
            }
            OracleSetup::DriftPythPull => {
                // (1) Pyth oracle (for price) and (2) Drift spot market (for exchange rate)
                require_eq!(ais.len(), 2, MarginfiError::WrongNumberOfOracleAccounts);

                let account_info = &ais[0];
                let spot_market_info = &ais[1];

                // Validate oracle account matches expected key (Kamino pattern)
                require_keys_eq!(
                    *account_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                require_keys_eq!(
                    *spot_market_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::DriftSpotMarketValidationFailed
                );

                // Verifies owner + discriminator automatically
                let spot_market_loader: AccountLoader<drift_mocks::state::MinimalSpotMarket> =
                    AccountLoader::try_from(spot_market_info)
                        .map_err(|_| MarginfiError::DriftSpotMarketValidationFailed)?;
                let spot_market = spot_market_loader.load()?;

                // Check if spot market interest is stale
                require!(
                    !spot_market.is_stale(clock.unix_timestamp),
                    MarginfiError::DriftSpotMarketStale
                );

                if live!() {
                    require_keys_eq!(
                        *account_info.owner,
                        pyth_solana_receiver_sdk::id(),
                        MarginfiError::PythPushWrongAccountOwner
                    );
                } else {
                    // Localnet only
                    // On localnet, allow the mock program ID -OR- the real one
                    let owner_ok = account_info.owner.eq(&PYTH_ID)
                        || account_info.owner.eq(&pyth_solana_receiver_sdk::id());
                    check!(owner_ok, MarginfiError::PythPushWrongAccountOwner);
                };

                // Use Kamino pattern: no feed_id parameter needed
                let mut price_feed =
                    PythPushOraclePriceFeed::load_checked(account_info, clock, max_age)?;

                // Adjust Pyth prices & confidence in place
                price_feed.price.price = spot_market.adjust_i64(price_feed.price.price)?;
                price_feed.ema_price.price = spot_market.adjust_i64(price_feed.ema_price.price)?;
                price_feed.price.conf = spot_market.adjust_u64(price_feed.price.conf)?;
                price_feed.ema_price.conf = spot_market.adjust_u64(price_feed.ema_price.conf)?;

                Ok(OraclePriceFeedAdapter::PythPushOracle(price_feed))
            }
            OracleSetup::DriftSwitchboardPull => {
                // (1) Switchboard oracle (for price) and (2) Drift spot market (for exchange rate)
                check!(ais.len() == 2, MarginfiError::WrongNumberOfOracleAccounts);

                let oracle_info = &ais[0];
                let spot_market_info = &ais[1];

                require_keys_eq!(
                    *oracle_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                require_keys_eq!(
                    *spot_market_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::DriftSpotMarketValidationFailed
                );

                // Verifies owner + discriminator automatically
                let spot_market_loader: AccountLoader<drift_mocks::state::MinimalSpotMarket> =
                    AccountLoader::try_from(spot_market_info)
                        .map_err(|_| MarginfiError::DriftSpotMarketValidationFailed)?;
                let spot_market = spot_market_loader.load()?;

                // Check if spot market interest is stale
                require!(
                    !spot_market.is_stale(clock.unix_timestamp),
                    MarginfiError::DriftSpotMarketStale
                );

                let mut price_feed = SwitchboardPullPriceFeed::load_checked(
                    oracle_info,
                    clock.unix_timestamp,
                    max_age,
                )?;

                // Adjust Switchboard value & std_dev (i128 with 1e18 precision)
                price_feed.feed.result.value =
                    spot_market.adjust_i128(price_feed.feed.result.value)?;
                price_feed.feed.result.std_dev =
                    spot_market.adjust_i128(price_feed.feed.result.std_dev)?;

                Ok(OraclePriceFeedAdapter::SwitchboardPull(price_feed))
            }
            OracleSetup::SolendPythPull => {
                // (1) Pyth oracle (for price) and (2) Solend reserve (for exchange rate)
                require_eq!(ais.len(), 2, MarginfiError::WrongNumberOfOracleAccounts);
                let reserve_info = &ais[1];
                require_keys_eq!(
                    *reserve_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::SolendReserveValidationFailed
                );

                // Verifies owner + discriminator automatically
                let reserve_loader: AccountLoader<solend_mocks::state::SolendMinimalReserve> =
                    AccountLoader::try_from(reserve_info)
                        .map_err(|_| MarginfiError::SolendReserveValidationFailed)?;
                let reserve = reserve_loader.load()?;

                // Check reserve has been refreshed this slot
                require!(!reserve.is_stale()?, MarginfiError::SolendReserveStale);

                let account_info = &ais[0];

                // Validate oracle account matches expected key (Kamino pattern)
                require_keys_eq!(
                    *account_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                if live!() {
                    require_keys_eq!(
                        *account_info.owner,
                        pyth_solana_receiver_sdk::id(),
                        MarginfiError::PythPushWrongAccountOwner
                    );
                } else {
                    // Localnet only
                    // On localnet, allow the mock program ID -OR- the real one
                    let owner_ok = account_info.owner.eq(&PYTH_ID)
                        || account_info.owner.eq(&pyth_solana_receiver_sdk::id());
                    check!(owner_ok, MarginfiError::PythPushWrongAccountOwner);
                };

                // Use Kamino pattern: no feed_id parameter needed
                let mut price_feed =
                    PythPushOraclePriceFeed::load_checked(account_info, clock, max_age)?;

                let (total_liq, total_col) = reserve.scaled_supplies()?;
                if total_col > I80F48::ZERO {
                    let liq_to_col_ratio = total_liq / total_col;

                    // Adjust Pyth prices & confidence in place
                    price_feed.price.price = adjust_i64(price_feed.price.price, liq_to_col_ratio)
                        .ok_or_else(math_error!())?;
                    price_feed.ema_price.price =
                        adjust_i64(price_feed.ema_price.price, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                    price_feed.price.conf = adjust_u64(price_feed.price.conf, liq_to_col_ratio)
                        .ok_or_else(math_error!())?;
                    price_feed.ema_price.conf =
                        adjust_u64(price_feed.ema_price.conf, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                }
                Ok(OraclePriceFeedAdapter::PythPushOracle(price_feed))
            }
            OracleSetup::SolendSwitchboardPull => {
                // (1) Switchboard oracle (for price) and (2) Solend reserve (for exchange rate)
                require_eq!(ais.len(), 2, MarginfiError::WrongNumberOfOracleAccounts);

                let oracle_info = &ais[0];
                let reserve_info = &ais[1];

                require_keys_eq!(
                    *oracle_info.key,
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                require_keys_eq!(
                    *reserve_info.key,
                    bank_config.oracle_keys[1],
                    MarginfiError::SolendReserveValidationFailed
                );

                // Verifies owner + discriminator automatically
                let reserve_loader: AccountLoader<solend_mocks::state::SolendMinimalReserve> =
                    AccountLoader::try_from(reserve_info)
                        .map_err(|_| MarginfiError::SolendReserveValidationFailed)?;
                let reserve = reserve_loader.load()?;

                // Check reserve has been refreshed this slot
                require!(!reserve.is_stale()?, MarginfiError::SolendReserveStale);

                let mut price_feed = SwitchboardPullPriceFeed::load_checked(
                    oracle_info,
                    clock.unix_timestamp,
                    max_age,
                )?;

                let (total_liq, total_col) = reserve.scaled_supplies()?;
                if total_col > I80F48::ZERO {
                    let liq_to_col_ratio = total_liq / total_col;

                    // Adjust Switchboard value & std_dev (i128 with 1e18 precision)
                    price_feed.feed.result.value =
                        adjust_i128(price_feed.feed.result.value, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                    price_feed.feed.result.std_dev =
                        adjust_i128(price_feed.feed.result.std_dev, liq_to_col_ratio)
                            .ok_or_else(math_error!())?;
                }

                Ok(OraclePriceFeedAdapter::SwitchboardPull(price_feed))
            }
        }
    }

    /// * lst_mint, stake_pool, sol_pool - required only if configuring
    ///   `OracleSetup::StakedWithPythPush` initially. (subsequent validations of staked banks can
    ///   omit these)
    pub fn validate_bank_config(
        bank_config: &BankConfig,
        oracle_ais: &[AccountInfo],
        lst_mint: Option<Pubkey>,
        stake_pool: Option<Pubkey>,
        sol_pool: Option<Pubkey>,
    ) -> MarginfiResult {
        match bank_config.oracle_setup {
            OracleSetup::None => Err(MarginfiError::OracleNotSetup.into()),
            OracleSetup::KaminoPythPush => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                // Validate oracle account matches expected key (new pattern)
                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                // Validate it's a valid Pyth Push oracle account
                load_price_update_v2_checked(&oracle_ais[0])?;

                require_keys_eq!(
                    *oracle_ais[1].key,
                    bank_config.oracle_keys[1],
                    MarginfiError::KaminoReserveValidationFailed
                );
                Ok(())
            }
            OracleSetup::KaminoSwitchboardPull => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                SwitchboardPullPriceFeed::check_ais(&oracle_ais[0])?;

                require_keys_eq!(
                    *oracle_ais[1].key,
                    bank_config.oracle_keys[1],
                    MarginfiError::KaminoReserveValidationFailed
                );
                Ok(())
            }
            OracleSetup::PythLegacy => {
                panic!("pyth legacy is deprecated");
            }
            OracleSetup::SwitchboardV2 => {
                panic!("swb v2 is deprecated");
            }
            OracleSetup::PythPushOracle => {
                check!(
                    oracle_ais.len() == 1,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );
                load_price_update_v2_checked(&oracle_ais[0])?;
                Ok(())
            }
            OracleSetup::SwitchboardPull => {
                check!(
                    oracle_ais.len() == 1,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                if oracle_ais[0].key != &bank_config.oracle_keys[0] {
                    msg!(
                        "Expected oracle key: {:?}, got: {:?}",
                        bank_config.oracle_keys[0],
                        oracle_ais[0].key
                    );
                    return Err(error!(MarginfiError::WrongOracleAccountKeys));
                }

                SwitchboardPullPriceFeed::check_ais(&oracle_ais[0])?;

                Ok(())
            }
            OracleSetup::StakedWithPythPush => {
                if lst_mint.is_some() && stake_pool.is_some() && sol_pool.is_some() {
                    check!(
                        oracle_ais.len() == 3,
                        MarginfiError::WrongNumberOfOracleAccounts
                    );

                    require_keys_eq!(
                        oracle_ais[0].key(),
                        bank_config.oracle_keys[0],
                        MarginfiError::WrongOracleAccountKeys
                    );
                    load_price_update_v2_checked(&oracle_ais[0])?;

                    let lst_mint = lst_mint.unwrap();
                    let stake_pool = stake_pool.unwrap();
                    let sol_pool = sol_pool.unwrap();

                    let program_id = &SPL_SINGLE_POOL_ID;
                    let stake_pool_bytes = &stake_pool.to_bytes();
                    // Validate the given stake_pool derives the same lst_mint, proving stake_pool is correct
                    let (exp_mint, _) =
                        Pubkey::find_program_address(&[b"mint", stake_pool_bytes], program_id);
                    check_eq!(exp_mint, lst_mint, MarginfiError::StakePoolValidationFailed);
                    // Validate the now-proven stake_pool derives the given sol_pool
                    let (exp_pool, _) =
                        Pubkey::find_program_address(&[b"stake", stake_pool_bytes], program_id);
                    check_eq!(exp_pool, sol_pool, MarginfiError::StakePoolValidationFailed);

                    // Sanity check the mint. Note: spl-single-pool uses a classic Token, never Token22
                    check!(
                        oracle_ais[1].owner == &SPL_TOKEN_PROGRAM_ID,
                        MarginfiError::StakePoolValidationFailed
                    );
                    check_eq!(
                        oracle_ais[1].key(),
                        lst_mint,
                        MarginfiError::StakePoolValidationFailed
                    );
                    // Sanity check the pool is a native stake pool. Note: the native staking program is
                    // written in vanilla Solana and has no Anchor discriminator.
                    check!(
                        oracle_ais[2].owner == &NATIVE_STAKE_ID,
                        MarginfiError::StakePoolValidationFailed
                    );
                    check_eq!(
                        oracle_ais[2].key(),
                        sol_pool,
                        MarginfiError::StakePoolValidationFailed
                    );

                    Ok(())
                } else {
                    // light validation (after initial setup, only the Pyth oracle needs to be validated)
                    check!(
                        oracle_ais.len() == 1,
                        MarginfiError::WrongNumberOfOracleAccounts
                    );

                    require_keys_eq!(
                        oracle_ais[0].key(),
                        bank_config.oracle_keys[0],
                        MarginfiError::WrongOracleAccountKeys
                    );
                    load_price_update_v2_checked(&oracle_ais[0])?;

                    Ok(())
                }
            }
            OracleSetup::Fixed => {
                check!(
                    oracle_ais.is_empty(),
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                Ok(())
            }
            OracleSetup::DriftPythPull => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                load_price_update_v2_checked(&oracle_ais[0])?;

                require_keys_eq!(
                    *oracle_ais[1].key,
                    bank_config.oracle_keys[1],
                    MarginfiError::DriftSpotMarketValidationFailed
                );
                Ok(())
            }
            OracleSetup::DriftSwitchboardPull => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                SwitchboardPullPriceFeed::check_ais(&oracle_ais[0])?;

                require_keys_eq!(
                    *oracle_ais[1].key,
                    bank_config.oracle_keys[1],
                    MarginfiError::DriftSpotMarketValidationFailed
                );
                Ok(())
            }
            OracleSetup::SolendPythPull => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                // First account is the pyth push oracle
                // Validate oracle account matches expected key (Kamino pattern)
                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                // Validate it's a valid Pyth Push oracle account
                load_price_update_v2_checked(&oracle_ais[0])?;

                // Second account is the solend reserve
                require_keys_eq!(
                    *oracle_ais[1].key,
                    bank_config.oracle_keys[1],
                    MarginfiError::SolendReserveValidationFailed
                );
                Ok(())
            }
            OracleSetup::SolendSwitchboardPull => {
                require_eq!(
                    oracle_ais.len(),
                    2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );

                require_keys_eq!(
                    oracle_ais[0].key(),
                    bank_config.oracle_keys[0],
                    MarginfiError::WrongOracleAccountKeys
                );

                SwitchboardPullPriceFeed::check_ais(&oracle_ais[0])?;

                require_keys_eq!(
                    oracle_ais[1].key(),
                    bank_config.oracle_keys[1],
                    MarginfiError::SolendReserveValidationFailed
                );
                Ok(())
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FixedPriceFeed {
    pub price: I80F48,
}

impl PriceAdapter for FixedPriceFeed {
    fn get_price_of_type(
        &self,
        _oracle_price_type: OraclePriceType,
        _bias: Option<PriceBias>,
        _oracle_max_confidence: u32,
    ) -> MarginfiResult<I80F48> {
        Ok(self.price)
    }
    fn get_price_and_confidence_of_type(
        &self,
        oracle_price_type: OraclePriceType,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<OraclePriceWithConfidence> {
        Ok(OraclePriceWithConfidence {
            price: self.get_price_of_type(oracle_price_type, None, oracle_max_confidence)?,
            confidence: I80F48::ZERO,
        })
    }
}

#[cfg_attr(feature = "client", derive(Clone, Debug))]
pub struct SwitchboardPullPriceFeed {
    pub feed: Box<LitePullFeedAccountData>,
}

impl SwitchboardPullPriceFeed {
    pub fn load_checked(
        ai: &AccountInfo,
        current_timestamp: i64,
        max_age: u64,
    ) -> MarginfiResult<Self> {
        let ai_data = ai.data.borrow();

        check!(
            ai.owner.eq(&SWITCHBOARD_PULL_ID),
            MarginfiError::SwitchboardWrongAccountOwner
        );

        let feed: PullFeedAccountData = parse_swb_ignore_alignment(ai_data)?;
        let lite_feed = LitePullFeedAccountData::from(&feed);
        // TODO restore when swb fixes alignment issue in crate.
        // let feed = PullFeedAccountData::parse(ai_data)
        //     .map_err(|_| MarginfiError::SwitchboardInvalidAccount)?;

        // Check staleness
        let last_updated = feed.last_update_timestamp;
        if current_timestamp.saturating_sub(last_updated) > max_age as i64 {
            return err!(MarginfiError::SwitchboardStalePrice);
        }

        Ok(Self {
            feed: Box::new(lite_feed),
        })
    }

    fn check_ais(ai: &AccountInfo) -> MarginfiResult {
        let ai_data = ai.data.borrow();

        check!(
            ai.owner.eq(&SWITCHBOARD_PULL_ID),
            MarginfiError::SwitchboardWrongAccountOwner
        );

        let _feed = parse_swb_ignore_alignment(ai_data)?;
        // TODO restore when swb fixes alignment issue in crate.
        // PullFeedAccountData::parse(ai_data)
        //     .map_err(|_| MarginfiError::SwitchboardInvalidAccount)?;

        Ok(())
    }

    fn get_price(&self) -> MarginfiResult<I80F48> {
        let sw_result = self.feed.result;
        // Note: Pull oracles support mean (result.mean) or median (result.value)
        let price: I80F48 = I80F48::from_num(sw_result.value)
            .checked_div(EXP_10_I80F48[switchboard_on_demand::PRECISION as usize])
            .ok_or_else(math_error!())?;
        Ok(price)
    }

    fn get_confidence_interval(&self, oracle_max_confidence: u32) -> MarginfiResult<I80F48> {
        let conf_interval: I80F48 = I80F48::from_num(self.feed.result.std_dev)
            .checked_div(EXP_10_I80F48[switchboard_on_demand::PRECISION as usize])
            .ok_or_else(math_error!())?
            .checked_mul(STD_DEV_MULTIPLE)
            .ok_or_else(math_error!())?;

        let price = self.get_price()?;

        // Fail the price fetch if confidence > price * oracle_max_confidence
        let oracle_max_confidence = if oracle_max_confidence > 0 {
            I80F48::from_num(oracle_max_confidence)
        } else {
            // The default max confidence is 10%
            U32_MAX_DIV_10
        };
        let max_conf = price
            .checked_mul(oracle_max_confidence)
            .ok_or_else(math_error!())?
            .checked_div(U32_MAX)
            .ok_or_else(math_error!())?;
        if conf_interval > max_conf {
            let conf_interval = conf_interval.to_num::<f64>();
            let max_conf = max_conf.to_num::<f64>();
            msg!("conf was {:?}, but max is {:?}", conf_interval, max_conf);
            return err!(MarginfiError::OracleMaxConfidenceExceeded);
        }

        // Clamp confidence to 5% of the price regardless
        let max_conf_interval = price
            .checked_mul(MAX_CONF_INTERVAL)
            .ok_or_else(math_error!())?;

        assert!(
            max_conf_interval >= I80F48::ZERO,
            "Negative max confidence interval"
        );

        assert!(
            conf_interval >= I80F48::ZERO,
            "Negative confidence interval"
        );

        Ok(min(conf_interval, max_conf_interval))
    }
}

impl PriceAdapter for SwitchboardPullPriceFeed {
    fn get_price_of_type(
        &self,
        _price_type: OraclePriceType,
        bias: Option<PriceBias>,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<I80F48> {
        let price = self.get_price()?;

        match bias {
            Some(price_bias) => {
                let confidence_interval = self.get_confidence_interval(oracle_max_confidence)?;

                match price_bias {
                    PriceBias::Low => Ok(price
                        .checked_sub(confidence_interval)
                        .ok_or_else(math_error!())?),
                    PriceBias::High => Ok(price
                        .checked_add(confidence_interval)
                        .ok_or_else(math_error!())?),
                }
            }
            None => Ok(price),
        }
    }

    fn get_price_and_confidence_of_type(
        &self,
        price_type: OraclePriceType,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<OraclePriceWithConfidence> {
        let confidence_interval = self.get_confidence_interval(oracle_max_confidence)?;
        let price = self.get_price_of_type(price_type, None, oracle_max_confidence)?;

        Ok(OraclePriceWithConfidence {
            price,
            confidence: confidence_interval,
        })
    }
}

// TODO remove when swb fixes the alignment issue in their crate
// (TargetAlignmentGreaterAndInputNotAligned) when bytemuck::from_bytes executes on any local system
// (including bpf next-test) where the struct is "properly" aligned 16
/// The same as PullFeedAccountData::parse but completely ignores input alignment.
pub fn parse_swb_ignore_alignment(data: Ref<&mut [u8]>) -> MarginfiResult<PullFeedAccountData> {
    if data.len() < 8 {
        return err!(MarginfiError::SwitchboardInvalidAccount);
    }

    if data[..8] != PullFeedAccountData::DISCRIMINATOR {
        return err!(MarginfiError::SwitchboardInvalidAccount);
    }

    let feed = bytemuck::try_pod_read_unaligned::<PullFeedAccountData>(
        &data[8..8 + std::mem::size_of::<PullFeedAccountData>()],
    )
    .map_err(|_| MarginfiError::SwitchboardInvalidAccount)?;

    Ok(feed)
}

pub fn load_price_update_v2_checked(ai: &AccountInfo) -> MarginfiResult<PriceUpdateV2> {
    if live!() {
        check_eq!(
            *ai.owner,
            pyth_solana_receiver_sdk::id(),
            MarginfiError::PythPushWrongAccountOwner
        );
    } else {
        // On localnet, allow the mock program ID OR the real one (for regression tests against
        // actual mainnet accounts).
        // * Note: Typically price updates are owned by `pyth_solana_receiver_sdk` and the oracle
        // feed account itself is owned by PYTH ID. On localnet, the mock program may own both for
        // simplicity.
        let owner_ok = ai.owner.eq(&PYTH_ID) || ai.owner.eq(&pyth_solana_receiver_sdk::id());
        check!(owner_ok, MarginfiError::PythPushWrongAccountOwner);
    }

    let price_feed_data = ai.try_borrow_data()?;
    let discriminator = &price_feed_data[0..8];
    let expected_discrim = <PriceUpdateV2 as anchor_lang::Discriminator>::DISCRIMINATOR;

    check_eq!(
        discriminator,
        expected_discrim,
        MarginfiError::PythPushInvalidAccount
    );

    Ok(PriceUpdateV2::deserialize(
        &mut &price_feed_data.as_ref()[8..],
    )?)
}

#[cfg_attr(feature = "client", derive(Clone, Debug))]
pub struct PythPushOraclePriceFeed {
    ema_price: Box<pyth_solana_receiver_sdk::price_update::Price>,
    price: Box<pyth_solana_receiver_sdk::price_update::Price>,
}

impl PythPushOraclePriceFeed {
    /// Pyth push oracles are update using crosschain messages from pythnet There can be multiple
    /// pyth push oracles for a given feed_id. Marginfi allows using any pyth push oracle with a
    /// sufficient verification level and price age.
    ///
    /// Security assumptions:
    /// - The pyth-push-oracle account is owned by the pyth-solana-receiver program, checked in
    ///   `load_price_update_v2_checked`
    /// - The pyth-push-oracle account is a PriceUpdateV2 account, checked in
    ///   `load_price_update_v2_checked`
    /// - The pyth-push-oracle account has a minimum verification level, checked in
    ///   `get_price_no_older_than_with_custom_verification_level`
    /// - The pyth-push-oracle account has a valid feed_id, the pyth-solana-receiver program
    ///   enforces that the feed_id matches the pythnet feed_id, checked in
    ///     - pyth-push-oracle asserts that a valid price update has a matching feed_id with the
    ///       existing pyth-push-oracle update
    ///       https://github.com/pyth-network/pyth-crosschain/blob/94f1bd54612adc3e186eaf0bb0f1f705880f20a6/target_chains/solana/programs/pyth-push-oracle/src/lib.rs#L101
    ///     - pyth-solana-receiver set the feed_id directly from a pythnet verified price_update
    ///       message
    ///       https://github.com/pyth-network/pyth-crosschain/blob/94f1bd54612adc3e186eaf0bb0f1f705880f20a6/target_chains/solana/programs/pyth-solana-receiver/src/lib.rs#L437
    /// - The pyth-push-oracle account is not older than the max_age, checked in
    ///   `get_price_no_older_than_with_custom_verification_level`
    pub fn load_checked(ai: &AccountInfo, clock: &Clock, max_age: u64) -> MarginfiResult<Self> {
        let price_feed_account = load_price_update_v2_checked(ai)?;
        let feed_id = &price_feed_account.price_message.feed_id;

        let price = price_feed_account
            .get_price_no_older_than_with_custom_verification_level(
                clock,
                max_age,
                feed_id,
                MIN_PYTH_PUSH_VERIFICATION_LEVEL,
            )
            .map_err(|e| {
                debug!("Pyth push oracle error: {:?}", e);
                let error: MarginfiError = e.into();
                error
            })?;

        let ema_price = {
            let price_update::PriceFeedMessage {
                exponent,
                publish_time,
                ema_price,
                ema_conf,
                ..
            } = price_feed_account.price_message;

            pyth_solana_receiver_sdk::price_update::Price {
                price: ema_price,
                conf: ema_conf,
                exponent,
                publish_time,
            }
        };

        Ok(Self {
            price: Box::new(price),
            ema_price: Box::new(ema_price),
        })
    }

    #[cfg(feature = "client")]
    pub fn load_unchecked(ai: &AccountInfo) -> MarginfiResult<Self> {
        let price_feed_account = load_price_update_v2_checked(ai)?;

        let price = price_feed_account
            .get_price_unchecked(&price_feed_account.price_message.feed_id)
            .map_err(|e| {
                println!("Pyth push oracle error: {:?}", e);
                let error: MarginfiError = e.into();
                error
            })?;

        let ema_price = {
            let price_update::PriceFeedMessage {
                exponent,
                publish_time,
                ema_price,
                ema_conf,
                ..
            } = price_feed_account.price_message;

            pyth_solana_receiver_sdk::price_update::Price {
                price: ema_price,
                conf: ema_conf,
                exponent,
                publish_time,
            }
        };

        Ok(Self {
            price: Box::new(price),
            ema_price: Box::new(ema_price),
        })
    }

    #[cfg(feature = "client")]
    pub fn peek_feed_id(ai: &AccountInfo) -> MarginfiResult<FeedId> {
        let price_feed_account = load_price_update_v2_checked(ai)?;

        Ok(price_feed_account.price_message.feed_id)
    }

    fn get_confidence_interval(
        &self,
        use_ema: bool,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<I80F48> {
        let price = if use_ema {
            &self.ema_price
        } else {
            &self.price
        };

        let conf_interval =
            pyth_price_components_to_i80f48(I80F48::from_num(price.conf), price.exponent)?
                .checked_mul(CONF_INTERVAL_MULTIPLE)
                .ok_or_else(math_error!())?;

        let price = pyth_price_components_to_i80f48(I80F48::from_num(price.price), price.exponent)?;

        // Fail the price fetch if confidence > price * oracle_max_confidence
        let oracle_max_confidence = if oracle_max_confidence > 0 {
            I80F48::from_num(oracle_max_confidence)
        } else {
            // The default max confidence is 10%
            U32_MAX_DIV_10
        };
        let max_conf = price
            .checked_mul(oracle_max_confidence)
            .ok_or_else(math_error!())?
            .checked_div(U32_MAX)
            .ok_or_else(math_error!())?;
        if conf_interval > max_conf {
            let price = price.to_num::<f64>();
            let conf_interval = conf_interval.to_num::<f64>();
            let max_conf = max_conf.to_num::<f64>();
            msg!(
                "oracle price: {:?}, conf was {:?}, but max is {:?}",
                price,
                conf_interval,
                max_conf
            );
            return err!(MarginfiError::OracleMaxConfidenceExceeded);
        }

        // Cap confidence interval to 5% of price regardless
        let capped_conf_interval = price
            .checked_mul(MAX_CONF_INTERVAL)
            .ok_or_else(math_error!())?;

        assert!(
            capped_conf_interval >= I80F48::ZERO,
            "Negative max confidence interval"
        );

        assert!(
            conf_interval >= I80F48::ZERO,
            "Negative confidence interval"
        );

        Ok(min(conf_interval, capped_conf_interval))
    }

    #[inline(always)]
    fn get_ema_price(&self) -> MarginfiResult<I80F48> {
        pyth_price_components_to_i80f48(
            I80F48::from_num(self.ema_price.price),
            self.ema_price.exponent,
        )
    }

    #[inline(always)]
    fn get_unweighted_price(&self) -> MarginfiResult<I80F48> {
        pyth_price_components_to_i80f48(I80F48::from_num(self.price.price), self.price.exponent)
    }

    /// Find PDA address of a pyth push oracle given a shard_id and feed_id
    ///
    /// Pyth sponsored feed id
    /// `constants::PYTH_PUSH_PYTH_SPONSORED_SHARD_ID = 0`
    ///
    /// Marginfi sponsored feed id
    /// `constants::PYTH_PUSH_MARGINFI_SPONSORED_SHARD_ID = 3301`
    pub fn find_oracle_address(shard_id: u16, feed_id: &FeedId) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[&shard_id.to_le_bytes(), feed_id], &PYTH_PUSH_ORACLE_ID)
    }
}

impl PriceAdapter for PythPushOraclePriceFeed {
    fn get_price_of_type(
        &self,
        price_type: OraclePriceType,
        bias: Option<PriceBias>,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<I80F48> {
        let price = match price_type {
            OraclePriceType::TimeWeighted => self.get_ema_price()?,
            OraclePriceType::RealTime => self.get_unweighted_price()?,
        };

        match bias {
            None => Ok(price),
            Some(price_bias) => {
                let confidence_interval = self.get_confidence_interval(
                    matches!(price_type, OraclePriceType::TimeWeighted),
                    oracle_max_confidence,
                )?;

                let biased_price = match price_bias {
                    PriceBias::Low => price
                        .checked_sub(confidence_interval)
                        .ok_or_else(math_error!())?,
                    PriceBias::High => price
                        .checked_add(confidence_interval)
                        .ok_or_else(math_error!())?,
                };

                Ok(biased_price)
            }
        }
    }

    fn get_price_and_confidence_of_type(
        &self,
        price_type: OraclePriceType,
        oracle_max_confidence: u32,
    ) -> MarginfiResult<OraclePriceWithConfidence> {
        let confidence_interval = self.get_confidence_interval(
            matches!(price_type, OraclePriceType::TimeWeighted),
            oracle_max_confidence,
        )?;
        let price = self.get_price_of_type(price_type, None, oracle_max_confidence)?;

        Ok(OraclePriceWithConfidence {
            price,
            confidence: confidence_interval,
        })
    }
}

/// A slimmed down version of the PullFeedAccountData struct copied from the
/// switchboard-on-demand/src/pull_feed.rs
#[cfg_attr(feature = "client", derive(Clone, Debug))]
pub struct LitePullFeedAccountData {
    pub result: CurrentResult,
    #[cfg(feature = "client")]
    pub feed_hash: [u8; 32],
    #[cfg(feature = "client")]
    pub last_update_timestamp: i64,
}

impl From<&PullFeedAccountData> for LitePullFeedAccountData {
    fn from(feed: &PullFeedAccountData) -> Self {
        Self {
            result: feed.result,
            #[cfg(feature = "client")]
            feed_hash: feed.feed_hash,
            #[cfg(feature = "client")]
            last_update_timestamp: feed.last_update_timestamp,
        }
    }
}

impl From<Ref<'_, PullFeedAccountData>> for LitePullFeedAccountData {
    fn from(feed: Ref<'_, PullFeedAccountData>) -> Self {
        Self {
            result: feed.result,
            #[cfg(feature = "client")]
            feed_hash: feed.feed_hash,
            #[cfg(feature = "client")]
            last_update_timestamp: feed.last_update_timestamp,
        }
    }
}

#[inline(always)]
fn pyth_price_components_to_i80f48(price: I80F48, exponent: i32) -> MarginfiResult<I80F48> {
    let scaling_factor = EXP_10_I80F48[exponent.unsigned_abs() as usize];

    let price = if exponent == 0 {
        price
    } else if exponent < 0 {
        price
            .checked_div(scaling_factor)
            .ok_or_else(math_error!())?
    } else {
        price
            .checked_mul(scaling_factor)
            .ok_or_else(math_error!())?
    };

    Ok(price)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::utils::hex_to_bytes;

    use super::*;

    use anchor_lang::solana_program::account_info::AccountInfo;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn swb_pull_get_price_1() {
        // From mainnet: https://solana.fm/address/BSzfJs4d1tAkSDqkepnfzEVcx2WtDVnwwXa2giy9PLeP
        // Actual price $155.59404527
        // conf/Std_dev ~$0.47
        let bytes = hex_to_bytes("c41b6cc40ad7db286f5e7566ac000a9530e56b1db49585772719aeaaeeadb4d9bd8c2357b88e9e782e53d81000000000000000000000000000985f538057856308000000000000005cba953f3f15356b17703e554d3983801916531d7976aa424ad64348ec50e4224650d81000000000000000000000000000a0d5a780cc7f580800000000000000a20b742cedab55efd1faf60aef2cb872a092d24dfba8a48c8b953a5e90ac7bbf874ed81000000000000000000000000000c04958360093580800000000000000e7ef024ea756f8beec2eaa40234070da356754a8eeb2ac6a17c32d17c3e99f8ddc50d81000000000000000000000000000bc8739b45d215b0800000000000000e3e5130902c3e9c27917789769f1ae05de15cf504658beafeed2c598a949b3b7bf53d810000000000000000000000000007cec168c94d667080000000000000020e270b743473d87eff321663e267ba1c9a151f7969cef8147f625e9a2af7287ea54d81000000000000000000000000000dc65eccc174d6f0800000000000000ab605484238ac93f225c65f24d7705bb74b00cdb576555c3995e196691a4de5f484ed8100000000000000000000000000088f28dc9271d59080000000000000015196392573dc9043242716f629d4c0fb93bc0cff7a1a10ede24281b0e98fb7d5454d810000000000000000000000000000441a10ca4a268080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000048ac38271f28ab1b12e49439bddf54871094e4832a56c7a8ec57bd18d357980086807068432f186a147cf0b13a30067d386204ea9d6c8b04743ac2ef010b07524c935636f2523f6aeeb6dc7b7dab0e86a13ff2c794f7895fc78851d69fdb593bdccdb36600000000000000000000000000e40b540200000001000000534f4c2f55534400000000000000000000000000000000000000000000000000000000019e9eb66600000000fca3d11000000000000000000000000000000000000000000000000000000000000000000000000000dc65eccc174d6f0800000000000000006c9225e039550300000000000000000070d3c6ecddf76b080000000000000000d8244bc073aa060000000000000000000441a10ca4a268080000000000000000dc65eccc174d6f08000000000000000200000000000000ea54d810000000005454d81000000000ea54d81000000000fa0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
        let key = pubkey!("BSzfJs4d1tAkSDqkepnfzEVcx2WtDVnwwXa2giy9PLeP");
        let mut lamports = 1_000_000u64;
        let mut data = bytes.clone();

        let ai = AccountInfo {
            key: &key,
            lamports: Rc::new(RefCell::new(&mut lamports)),
            data: Rc::new(RefCell::new(&mut data[..])),
            owner: &SWITCHBOARD_PULL_ID,
            rent_epoch: 361,
            is_signer: false,
            is_writable: true,
            executable: false,
        };

        let ai_check = SwitchboardPullPriceFeed::check_ais(&ai);
        assert!(ai_check.is_ok());

        let current_timestamp = 42;
        let max_age = 100;
        let feed: SwitchboardPullPriceFeed =
            SwitchboardPullPriceFeed::load_checked(&ai, current_timestamp, max_age).unwrap();
        let price: I80F48 = feed.get_price().unwrap();
        let conf: I80F48 = feed.get_confidence_interval(0).unwrap();

        let target_price: I80F48 = I80F48::from_num(155.59);
        let price_tolerance: I80F48 = target_price * I80F48::from_num(0.0001);
        let min_price: I80F48 = target_price.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price.checked_add(price_tolerance).unwrap();
        assert!(price >= min_price && price <= max_price);

        let max_conf: I80F48 = target_price * I80F48::from_num(0.05);
        assert!(conf <= max_conf);

        let exp_conf: I80F48 = I80F48::from_num(0.47);
        let min_exp_conf: I80F48 = exp_conf - exp_conf * I80F48::from_num(0.01);
        let max_exp_conf: I80F48 = exp_conf + exp_conf * I80F48::from_num(0.01);
        assert!(exp_conf >= min_exp_conf && exp_conf <= max_exp_conf);

        let price_bias_none: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, None, 0)
            .unwrap();
        assert_eq!(price, price_bias_none);

        let price_bias_low: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::Low), 0)
            .unwrap();
        let target_price_low: I80F48 = target_price.checked_sub(exp_conf).unwrap();
        let min_price: I80F48 = target_price_low.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_low.checked_add(price_tolerance).unwrap();
        assert!(price_bias_low >= min_price && price_bias_low <= max_price);

        let price_bias_high: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::High), 0)
            .unwrap();
        let target_price_high: I80F48 = target_price.checked_add(exp_conf).unwrap();
        let min_price: I80F48 = target_price_high.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_high.checked_add(price_tolerance).unwrap();
        assert!(price_bias_high >= min_price && price_bias_high <= max_price);
    }

    #[test]
    fn swb_pull_get_price_2() {
        // From mainnet: https://solscan.io/account/HX5WM3qzogAfRCjBUWwnniLByMfFrjm1b5yo4KoWGR27
        // Actual price ~$177.351466043
        // conf/Std_dev ~$0.0046528305
        let bytes = hex_to_bytes("c41b6cc40ad7db281dd702ec182a223f272559ae7f7edad00455866aeca0870c8945f34227ac1847a637a31400000000fd37a31400000000de3d13f28e491a9c0900000000000000acb413831a0babf917c781324d0f1b31d42dac362f47a80da94daba626bb13dbf02ba31400000000fa2ba314000000005498dc173c82caaa090000000000000071b847ccf77337d6d9a6eb3d6297d30eb951c938c185605535bbaf28235fc62bcb31a31400000000d831a314000000009aff8957d335f3ab090000000000000049519d81046597f86c4cc529195900eab598b817a61759bc54660924a84522ea263ba31400000000323ba314000000002a87d2f2ce1b209d090000000000000023548094c33bcf8fb772e2ccb7721b54c11eb4a704a7d02aae49b8462b8636ef613aa314000000006f3aa31400000000c091d3bfb7fa3e9d09000000000000004509ff024fc1c9fff436b81fbd989517adb137ccf48e5261905b827eaf825d30613aa314000000006f3aa31400000000c091d3bfb7fa3e9d09000000000000009dc7e97afd0f2e10190798c4aaf6e77740f5d2dc9c1bd32620752bfd825c93b28ac19d14000000009ac19d140000000016aafbd81d87fec20900000000000000c1197acd5a9eefb91b03ca87916fe89e9cf5c027e578ca940921c265863e4452032aa31400000000122aa31400000000965ccffa333e8ca40900000000000000405a6ee0581e9bb6037232cfc7318590752f05f769821aa7c18bcd2edf291e89a32ea31400000000b42ea31400000000e2489952eda59ba609000000000000002e97cc55ff354de72464e80ce0e6a8337e0de64083ce41463366f603225bded6ea1fa31400000000f51fa31400000000683786a4afa269b109000000000000008f7b60b96800d5624794438f00854421f4660c6766f842c444f059b4d4c6e780f5409f14000000000f419f1400000000fa95533b11a896ff0900000000000000960fa06ef8fcadb3d46549a15e429bce810862c7e7a0f34fab5affafe4bc86d4263ba31400000000323ba31400000000e2385d26a2fc1b9d09000000000000004b145dd321b1561f624910735c2440c2e4348d82ae2ec18fb9b65fb4b219bad5cb31a31400000000d831a31400000000ec683a62afd9fcab0900000000000000ef3b9b69b40f2d5529727f09f1954412eb0f0b5c9a2658864160d8095c6384eb702fa31400000000792fa31400000000c81795f15ed4a4a809000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004c82aa3ee2e57bee96f6a7efeb329250801c57db0bf44ff15c1e8e992ab12ff386807068432f186a147cf0b13a30067d386204ea9d6c8b04743ac2ef010b0752c02f22d47b20b43bafde474328ac027283dbd7bb443660f5ec414c93faec56dc7012cd6600000000000000000000000000e40b5402000000010000006a7570534f4c202f2055534400000000000000000000000000000000000000000000100116624a68000000000e900c11000000000000000000000000000000000000000000000000000000000000000000000000c091d3bfb7fa3e9d09000000000000007e3deb83b98710000000000000000000e2b8352678832e9d0900000000000000de58769915fe22000000000000000000e2385d26a2fc1b9d0900000000000000c091d3bfb7fa3e9d09000000000000000404000000000000613aa31400000000613aa31400000000263ba31400000000fa000000000000000000000000000000ba251c3b45a63243bd1ea31400000000ae7c443acbcd3243ea1fa31400000000618a5b3bb0ae32431a21a3140000000000000000278e3243a822a31400000000c3aff93cf49532430c23a31400000000f44d1c3bff2b32435725a3140000000000000000e8ea3143d528a31400000000e9dc5e3ae3e03143032aa31400000000fcdff13a5f503243262ba3140000000065bb083dc24432438c2ba3140000000000000000ac063243a32ea31400000000c4998c3d841f3243092fa31400000000f9d3b13a90693243cb31a31400000000bdb3213aaf5032435d33a3140000000000000000d70632434f35a3140000000000000000e2443143a637a31400000000c576983bca583143613aa31400000000bbc5533da2b432436504a314000000006aa7ee3cdbae32439a05a31400000000000000004ba93243c906a3140000000047abd93b9cb132437a09a3140000000057f2603ddaa13243de09a314000000000000000089743243c90ba314000000000000000091523243eb0ea31400000000f3664d3bcd4b32430a10a3140000000000000000198e3243ce12a314000000004b405b3a108132430615a314000000006751df3ce5ae32436815a314000000003b293a3ba5a832439b16a31400000000db42e23c84933243f816a3140000000055875c3b55e232433e1ba3140000000082582f3b674a3243341da314000000000000000000000000000000000000000000000000000000000000000000000000cb604a6800000000fd5b4a6800000000575e4a680000000016624a6800000000c8614a6800000000c8614a68000000000031486800000000385b4a6800000000175d4a680000000025574a680000000024ca48680000000016624a6800000000575e4a6800000000655d4a6800000000c9140368000000000d9d026800000000dec0d867000000006b75f76700000000fb9c02680000000086d00168000000000665f5670000000001120368000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
        let key = pubkey!("HX5WM3qzogAfRCjBUWwnniLByMfFrjm1b5yo4KoWGR27");
        let mut lamports = 1_000_000u64;
        let mut data = bytes.clone();

        let ai = AccountInfo {
            key: &key,
            lamports: Rc::new(RefCell::new(&mut lamports)),
            data: Rc::new(RefCell::new(&mut data[..])),
            owner: &SWITCHBOARD_PULL_ID,
            rent_epoch: 361,
            is_signer: false,
            is_writable: true,
            executable: false,
        };

        let ai_check = SwitchboardPullPriceFeed::check_ais(&ai);
        assert!(ai_check.is_ok());

        let current_timestamp = 42;
        let max_age = 100;
        let feed: SwitchboardPullPriceFeed =
            SwitchboardPullPriceFeed::load_checked(&ai, current_timestamp, max_age).unwrap();
        let price: I80F48 = feed.get_price().unwrap();
        let conf: I80F48 = feed.get_confidence_interval(0).unwrap();

        let target_price: I80F48 = I80F48::from_num(177.351466043);
        let price_tolerance: I80F48 = target_price * I80F48::from_num(0.0001);
        let min_price: I80F48 = target_price.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price.checked_add(price_tolerance).unwrap();
        assert!(price >= min_price && price <= max_price);

        let max_conf: I80F48 = target_price * I80F48::from_num(0.05);
        assert!(conf <= max_conf);

        let exp_conf: I80F48 = I80F48::from_num(0.0046528305);
        let min_exp_conf: I80F48 = exp_conf - exp_conf * I80F48::from_num(0.01);
        let max_exp_conf: I80F48 = exp_conf + exp_conf * I80F48::from_num(0.01);
        assert!(exp_conf >= min_exp_conf && exp_conf <= max_exp_conf);

        let price_bias_none: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, None, 0)
            .unwrap();
        assert_eq!(price, price_bias_none);

        let price_bias_low: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::Low), 0)
            .unwrap();
        let target_price_low: I80F48 = target_price.checked_sub(exp_conf).unwrap();
        let min_price: I80F48 = target_price_low.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_low.checked_add(price_tolerance).unwrap();
        assert!(price_bias_low >= min_price && price_bias_low <= max_price);

        let price_bias_high: I80F48 = feed
            .get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::High), 0)
            .unwrap();
        let target_price_high: I80F48 = target_price.checked_add(exp_conf).unwrap();
        let min_price: I80F48 = target_price_high.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_high.checked_add(price_tolerance).unwrap();
        assert!(price_bias_high >= min_price && price_bias_high <= max_price);
    }

    #[test]
    fn pyth_pull_get_price() {
        // From mainnet: https://solana.fm/address/DBE3N8uNjhKPRHfANdwGvCZghWXyLPdqdSbEW2XFwBiX
        // Actual price ~$3.4987e-5
        // conf/Std_dev ~$8.1619e-8
        let bytes = hex_to_bytes("22f123639d7ef4cdb4eacbe402ae9165c2ab7dfcdbe5044d27f284106f88a90bfddefa5fbff60ca00172b021217ca3fe68922a19aaf990109cb9d84e9ad004b4d2025ad6f529314419fd510500000000006501000000000000f6ffffff29bc80680000000029bc806800000000af56050000000000810100000000000058d32b150000000000");
        let key = pubkey!("DBE3N8uNjhKPRHfANdwGvCZghWXyLPdqdSbEW2XFwBiX");
        let mut lamports = 1_000_000u64;
        let mut data = bytes.clone();

        let ai = AccountInfo {
            key: &key,
            lamports: Rc::new(RefCell::new(&mut lamports)),
            data: Rc::new(RefCell::new(&mut data[..])),
            owner: &pyth_solana_receiver_sdk::id(),
            rent_epoch: 361,
            is_signer: false,
            is_writable: true,
            executable: false,
        };

        let max_age = 100;
        let feed: PythPushOraclePriceFeed =
            PythPushOraclePriceFeed::load_checked(&ai, &Clock::default(), max_age).unwrap();
        let price: I80F48 = feed.get_ema_price().unwrap();
        let conf: I80F48 = feed.get_confidence_interval(true, 0).unwrap();

        let target_price: I80F48 = I80F48::from_num(0.00003498);
        let price_tolerance: I80F48 = I80F48::from_num(0.00000001);
        let min_price: I80F48 = target_price.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price.checked_add(price_tolerance).unwrap();
        assert!(price >= min_price && price <= max_price);

        let max_conf: I80F48 = target_price * I80F48::from_num(0.05);
        assert!(conf <= max_conf);

        let exp_conf: I80F48 = I80F48::from_num(0.0000000816);
        let min_exp_conf: I80F48 = exp_conf - exp_conf * I80F48::from_num(0.01);
        let max_exp_conf: I80F48 = exp_conf + exp_conf * I80F48::from_num(0.01);
        assert!(exp_conf >= min_exp_conf && exp_conf <= max_exp_conf);

        let price_bias_none: I80F48 = feed
            .get_price_of_type(OraclePriceType::TimeWeighted, None, 0)
            .unwrap();
        assert_eq!(price, price_bias_none);

        let price_bias_low: I80F48 = feed
            .get_price_of_type(OraclePriceType::TimeWeighted, Some(PriceBias::Low), 0)
            .unwrap();
        let target_price_low: I80F48 = target_price.checked_sub(exp_conf).unwrap();
        let min_price: I80F48 = target_price_low.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_low.checked_add(price_tolerance).unwrap();
        assert!(price_bias_low >= min_price && price_bias_low <= max_price);

        let price_bias_high: I80F48 = feed
            .get_price_of_type(OraclePriceType::TimeWeighted, Some(PriceBias::High), 0)
            .unwrap();
        let target_price_high: I80F48 = target_price.checked_add(exp_conf).unwrap();
        let min_price: I80F48 = target_price_high.checked_sub(price_tolerance).unwrap();
        let max_price: I80F48 = target_price_high.checked_add(price_tolerance).unwrap();
        assert!(price_bias_high >= min_price && price_bias_high <= max_price);
    }
}
