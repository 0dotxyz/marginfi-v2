#[cfg(not(feature = "client"))]
use crate::events::{GroupEventHeader, LendingPoolBankAccrueInterestEvent};
use crate::{
    check, debug,
    errors::MarginfiError,
    math_error,
    prelude::MarginfiResult,
    set_if_some,
    state::{
        bank_cache::update_interest_rates,
        bank_config::BankConfigImpl,
        interest_rate::{
            calc_interest_rate_accrual_state_changes, InterestRateConfigImpl,
            InterestRateStateChanges,
        },
        marginfi_account::calc_value,
        price::OraclePriceWithMultiplier,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::{
    err,
    prelude::{AccountInfo, CpiContext, InterfaceAccount},
    ToAccountInfo,
};
use anchor_spl::{
    token::{transfer, Transfer},
    token_2022::spl_token_2022,
    token_interface::Mint,
};
use bytemuck::Zeroable;
use drift_mocks::constants::scale_drift_deposit_limit;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DRIFT, CIRCUIT_BREAKER_ENABLED, CLOSE_ENABLED_FLAG, FEE_VAULT_AUTHORITY_SEED,
        FEE_VAULT_SEED, FREEZE_SETTINGS, GROUP_FLAGS, INSURANCE_VAULT_AUTHORITY_SEED,
        INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
        PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG, TOKENLESS_REPAYMENTS_ALLOWED,
    },
    types::{
        Bank, BankConfig, BankConfigOpt, BankOperationalState, EmodeSettings, MarginfiGroup,
        OraclePriceWithConfidence,
    },
};

/// Minimum Solana-slot gap between counted CB pulses. Rate-limits how fast the EMA can be
/// nudged and how fast the breach counter can accumulate.
pub const CB_MIN_PULSE_SLOT_GAP: u64 = 2;

/// Floor for the EMA reference used in deviation math. Below this, reseed instead of dividing.
pub const CB_MIN_REF_PRICE: I80F48 = I80F48::from_bits(1 << 20);

/// Consecutive tier-3 trips before the bank is forced to `CircuitBroken`.
pub const CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK: u8 = 3;

/// Maximum age (seconds) of `cache.last_oracle_price_timestamp` accepted when enabling CB.
/// Forces admin to bundle a fresh pulse with `configure_bank` so the EMA can't be seeded
/// from an attacker-controlled stale price.
pub const CB_ENABLE_MAX_PRICE_AGE_SECONDS: i64 = 30;

/// Per-pulse cap on EMA reference movement, in basis points of the current reference. Bounds
/// how fast a sustained attacker can reanchor the EMA even at maximum α.
pub const CB_MAX_EMA_SHIFT_BPS_PER_PULSE: u64 = 500;

/// Long-window cap that catches slow oracle walking under the per-observation threshold.
pub const CB_WINDOW_SECONDS: i64 = 4 * 60 * 60;
pub const CB_WINDOW_MAX_UP_BPS: u64 = 2_000;
pub const CB_WINDOW_MAX_DOWN_BPS: u64 = 4_000;

fn cb_confidence_adjusted_deviation_bps(
    current: I80F48,
    reference: I80F48,
    confidence: I80F48,
) -> u64 {
    let effective_delta =
        ((current - reference).abs() - confidence.max(I80F48::ZERO)).max(I80F48::ZERO);
    cb_deviation_bps(effective_delta, reference)
}

fn cb_deviation_bps(delta: I80F48, reference: I80F48) -> u64 {
    (delta * I80F48::from_num(10_000u64) / reference).to_num::<u64>()
}

fn cb_tier_for_deviation(tiers: &[u16; 3], deviation_bps: u64) -> u8 {
    if tiers[2] > 0 && deviation_bps >= tiers[2] as u64 {
        3
    } else if tiers[1] > 0 && deviation_bps >= tiers[1] as u64 {
        2
    } else if tiers[0] > 0 && deviation_bps >= tiers[0] as u64 {
        1
    } else {
        0
    }
}

pub trait BankImpl {
    const LEN: usize = std::mem::size_of::<Bank>();

    #[allow(clippy::too_many_arguments)]
    fn new(
        marginfi_group_pk: Pubkey,
        config: BankConfig,
        mint: Pubkey,
        mint_decimals: u8,
        liquidity_vault: Pubkey,
        insurance_vault: Pubkey,
        fee_vault: Pubkey,
        current_timestamp: i64,
        liquidity_vault_bump: u8,
        liquidity_vault_authority_bump: u8,
        insurance_vault_bump: u8,
        insurance_vault_authority_bump: u8,
        fee_vault_bump: u8,
        fee_vault_authority_bump: u8,
        bank_seed: u64,
    ) -> Self;
    fn get_liability_amount(&self, shares: I80F48) -> MarginfiResult<I80F48>;
    fn get_asset_amount(&self, shares: I80F48) -> MarginfiResult<I80F48>;
    fn get_liability_shares(&self, value: I80F48) -> MarginfiResult<I80F48>;
    fn get_asset_shares(&self, value: I80F48) -> MarginfiResult<I80F48>;
    fn get_remaining_deposit_capacity(&self) -> MarginfiResult<u64>;
    fn change_asset_shares(&mut self, shares: I80F48, bypass_deposit_limit: bool)
        -> MarginfiResult;
    fn maybe_get_asset_weight_init_discount(&self, price: I80F48)
        -> MarginfiResult<Option<I80F48>>;
    fn change_liability_shares(
        &mut self,
        shares: I80F48,
        bypass_borrow_limit: bool,
    ) -> MarginfiResult;
    fn check_utilization_ratio(&self) -> MarginfiResult;
    fn configure(&mut self, config: &BankConfigOpt) -> MarginfiResult;
    fn configure_unfrozen_fields_only(&mut self, config: &BankConfigOpt) -> MarginfiResult;
    fn accrue_interest(
        &mut self,
        current_timestamp: i64,
        group: &MarginfiGroup,
        #[cfg(not(feature = "client"))] bank: Pubkey,
    ) -> MarginfiResult<()>;
    fn update_bank_cache(&mut self, group: &MarginfiGroup) -> MarginfiResult<()>;
    fn update_cache_price(
        &mut self,
        oracle_price: Option<OraclePriceWithMultiplier>,
    ) -> MarginfiResult<()>;
    fn is_cb_halted(&self, now: i64) -> bool;
    fn cb_price_gate(&self, oracle: OraclePriceWithConfidence) -> MarginfiResult<()>;
    fn cb_effective_operational_state(&self) -> BankOperationalState;
    fn update_circuit_breaker(
        &mut self,
        now: i64,
        slot: u64,
        oracle: OraclePriceWithConfidence,
    ) -> MarginfiResult<()>;
    fn reset_cb_runtime_state(&mut self);
    fn trip_cb_halt(&mut self, now: i64, deviation_bps: u64, breached_tier: u8);
    fn deposit_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult;
    fn withdraw_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        signer_seeds: &[&[&[u8]]],
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult;
    fn socialize_loss(&mut self, loss_amount: I80F48) -> MarginfiResult<bool>;
    fn get_flag(&self, flag: u64) -> bool;
    fn update_flag(&mut self, value: bool, flag: u64);
    fn verify_group_flags(flags: u64) -> bool;
    fn increment_lending_position_count(&mut self);
    fn decrement_lending_position_count(&mut self);
    fn increment_borrowing_position_count(&mut self);
    fn decrement_borrowing_position_count(&mut self);
}

impl BankImpl for Bank {
    #[allow(clippy::too_many_arguments)]
    fn new(
        marginfi_group_pk: Pubkey,
        config: BankConfig,
        mint: Pubkey,
        mint_decimals: u8,
        liquidity_vault: Pubkey,
        insurance_vault: Pubkey,
        fee_vault: Pubkey,
        current_timestamp: i64,
        liquidity_vault_bump: u8,
        liquidity_vault_authority_bump: u8,
        insurance_vault_bump: u8,
        insurance_vault_authority_bump: u8,
        fee_vault_bump: u8,
        fee_vault_authority_bump: u8,
        bank_seed: u64,
    ) -> Self {
        Self {
            mint,
            mint_decimals,
            group: marginfi_group_pk,
            asset_share_value: I80F48::ONE.into(),
            liability_share_value: I80F48::ONE.into(),
            liquidity_vault,
            liquidity_vault_bump,
            liquidity_vault_authority_bump,
            insurance_vault,
            insurance_vault_bump,
            insurance_vault_authority_bump,
            collected_insurance_fees_outstanding: I80F48::ZERO.into(),
            fee_vault,
            fee_vault_bump,
            fee_vault_authority_bump,
            collected_group_fees_outstanding: I80F48::ZERO.into(),
            total_liability_shares: I80F48::ZERO.into(),
            total_asset_shares: I80F48::ZERO.into(),
            last_update: current_timestamp,
            config,
            flags: CLOSE_ENABLED_FLAG,
            emissions_rate: 0,
            emissions_remaining: I80F48::ZERO.into(),
            emissions_mint: Pubkey::default(),
            collected_program_fees_outstanding: I80F48::ZERO.into(),
            emode: EmodeSettings::zeroed(),
            fees_destination_account: Pubkey::default(),
            lending_position_count: 0,
            borrowing_position_count: 0,
            _padding_0: [0; 16],
            integration_acc_1: Pubkey::default(),
            integration_acc_2: Pubkey::default(),
            bank_seed,
            ..Default::default()
        }
    }

    fn get_liability_amount(&self, shares: I80F48) -> MarginfiResult<I80F48> {
        Ok(shares
            .checked_mul(self.liability_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_asset_amount(&self, shares: I80F48) -> MarginfiResult<I80F48> {
        Ok(shares
            .checked_mul(self.asset_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_liability_shares(&self, value: I80F48) -> MarginfiResult<I80F48> {
        Ok(value
            .checked_div(self.liability_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_asset_shares(&self, value: I80F48) -> MarginfiResult<I80F48> {
        if self.asset_share_value == I80F48::ZERO.into() {
            return Ok(I80F48::ZERO);
        }
        Ok(value
            .checked_div(self.asset_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_remaining_deposit_capacity(&self) -> MarginfiResult<u64> {
        if !self.config.is_deposit_limit_active() {
            return Ok(u64::MAX);
        }

        let current_assets = self.get_asset_amount(self.total_asset_shares.into())?;

        let limit = if self.config.asset_tag == ASSET_TAG_DRIFT {
            scale_drift_deposit_limit(self.config.deposit_limit, self.mint_decimals)?
        } else {
            I80F48::from_num(self.config.deposit_limit)
        };

        if current_assets >= limit {
            return Ok(0);
        }

        let remaining = limit
            .checked_sub(current_assets)
            .ok_or_else(math_error!())?
            .checked_sub(I80F48::ONE) // Subtract 1 to ensure we stay under limit
            .ok_or_else(math_error!())?
            .checked_floor()
            .ok_or_else(math_error!())?
            .checked_to_num::<u64>()
            .ok_or_else(math_error!())?;

        Ok(remaining)
    }

    fn change_asset_shares(
        &mut self,
        shares: I80F48,
        bypass_deposit_limit: bool,
    ) -> MarginfiResult {
        let total_asset_shares: I80F48 = self.total_asset_shares.into();
        self.total_asset_shares = total_asset_shares
            .checked_add(shares)
            .ok_or_else(math_error!())?
            .into();

        if shares.is_positive() && self.config.is_deposit_limit_active() && !bypass_deposit_limit {
            let total_deposits_amount = self.get_asset_amount(self.total_asset_shares.into())?;

            // For Drift banks, deposit_limit is in native decimals but total_deposits_amount
            // is in 9-decimal (DRIFT_SCALED_BALANCE_DECIMALS). We Scale deposit_limit to match.
            let deposit_limit = if self.config.asset_tag == ASSET_TAG_DRIFT {
                scale_drift_deposit_limit(self.config.deposit_limit, self.mint_decimals)?
            } else {
                I80F48::from_num(self.config.deposit_limit)
            };

            if total_deposits_amount >= deposit_limit {
                let deposits_num: f64 = total_deposits_amount.to_num();
                let limit_num: f64 = deposit_limit.to_num();
                msg!("deposits: {:?} deposit lim: {:?}", deposits_num, limit_num);
                return err!(MarginfiError::BankAssetCapacityExceeded);
            }
        }

        Ok(())
    }

    fn maybe_get_asset_weight_init_discount(
        &self,
        price: I80F48,
    ) -> MarginfiResult<Option<I80F48>> {
        if self.config.usd_init_limit_active() {
            let bank_total_assets_value = calc_value(
                self.get_asset_amount(self.total_asset_shares.into())?,
                price,
                self.get_balance_decimals(),
                None,
            )?;

            let total_asset_value_init_limit =
                I80F48::from_num(self.config.total_asset_value_init_limit);

            #[cfg(target_os = "solana")]
            debug!(
                "Init limit active, limit: {}, total_assets: {}",
                total_asset_value_init_limit, bank_total_assets_value
            );

            if bank_total_assets_value > total_asset_value_init_limit {
                let discount = total_asset_value_init_limit
                    .checked_div(bank_total_assets_value)
                    .ok_or_else(math_error!())?;

                #[cfg(target_os = "solana")]
                debug!(
                    "Discounting assets by {:.2} because of total deposits {} over {} usd cap",
                    discount, bank_total_assets_value, total_asset_value_init_limit
                );

                Ok(Some(discount))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn change_liability_shares(
        &mut self,
        shares: I80F48,
        bypass_borrow_limit: bool,
    ) -> MarginfiResult {
        let total_liability_shares: I80F48 = self.total_liability_shares.into();
        self.total_liability_shares = total_liability_shares
            .checked_add(shares)
            .ok_or_else(math_error!())?
            .into();

        if !bypass_borrow_limit && shares.is_positive() && self.config.is_borrow_limit_active() {
            let total_liability_amount =
                self.get_liability_amount(self.total_liability_shares.into())?;
            let borrow_limit = I80F48::from_num(self.config.borrow_limit);

            if total_liability_amount >= borrow_limit {
                let liab_num: f64 = total_liability_amount.to_num();
                let borrow_num: f64 = borrow_limit.to_num();
                msg!("amt: {:?} borrow lim: {:?}", liab_num, borrow_num);
                return err!(MarginfiError::BankLiabilityCapacityExceeded);
            }
        }

        Ok(())
    }

    fn check_utilization_ratio(&self) -> MarginfiResult {
        let total_assets = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities = self.get_liability_amount(self.total_liability_shares.into())?;

        if total_assets < total_liabilities {
            let assets_num: f64 = total_assets.to_num();
            let liabs_num: f64 = total_liabilities.to_num();
            msg!("assets: {:?} liabs: {:?}", assets_num, liabs_num);
            return err!(MarginfiError::IllegalUtilizationRatio);
        }

        Ok(())
    }

    fn configure(&mut self, config: &BankConfigOpt) -> MarginfiResult {
        set_if_some!(self.config.asset_weight_init, config.asset_weight_init);
        set_if_some!(self.config.asset_weight_maint, config.asset_weight_maint);
        set_if_some!(
            self.config.liability_weight_init,
            config.liability_weight_init
        );
        set_if_some!(
            self.config.liability_weight_maint,
            config.liability_weight_maint
        );
        set_if_some!(self.config.deposit_limit, config.deposit_limit);

        set_if_some!(self.config.borrow_limit, config.borrow_limit);

        if let Some(new_state) = config.operational_state {
            // `KilledByBankruptcy` and `CircuitBroken` are not admin-settable: the former is
            // terminal, the latter is breaker-controlled.
            check!(
                new_state != BankOperationalState::KilledByBankruptcy
                    && new_state != BankOperationalState::CircuitBroken,
                MarginfiError::Unauthorized
            );
            // A circuit-broken bank only leaves that state via `clear_circuit_breaker`, which
            // restores the pre-break state and resets the breaker atomically.
            check!(
                self.config.operational_state != BankOperationalState::CircuitBroken,
                MarginfiError::BankCircuitBreakerHalted
            );
            // Log operational state change
            let old_state = self.config.operational_state;
            self.config.operational_state = new_state;
            msg!(
                "Operational state changed from {:?} to {:?}",
                old_state,
                new_state
            );
        }

        if let Some(ir_config) = &config.interest_rate_config {
            self.config.interest_rate_config.update(ir_config);
        }

        // Log risk tier change
        if let Some(new_risk_tier) = config.risk_tier {
            let old_risk_tier = self.config.risk_tier;
            self.config.risk_tier = new_risk_tier;
            msg!(
                "Risk tier changed from {:?} to {:?}",
                old_risk_tier,
                new_risk_tier
            );
        }

        set_if_some!(self.config.asset_tag, config.asset_tag);

        set_if_some!(
            self.config.total_asset_value_init_limit,
            config.total_asset_value_init_limit
        );

        set_if_some!(
            self.config.oracle_max_confidence,
            config.oracle_max_confidence
        );

        set_if_some!(self.config.oracle_max_age, config.oracle_max_age);

        if let Some(flag) = config.permissionless_bad_debt_settlement {
            msg!(
                "setting bad debt settlement: {:?}",
                config.permissionless_bad_debt_settlement.unwrap()
            );
            self.update_flag(flag, PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG);
        }

        if let Some(flag) = config.freeze_settings {
            msg!(
                "setting freeze settings: {:?}",
                config.freeze_settings.unwrap()
            );
            self.update_flag(flag, FREEZE_SETTINGS);
        }

        if let Some(flag) = config.tokenless_repayments_allowed {
            msg!(
                "setting tokenless repayments allowed: {:?}",
                config.tokenless_repayments_allowed.unwrap()
            );
            self.update_flag(flag, TOKENLESS_REPAYMENTS_ALLOWED);
        }

        if let Some(flag) = config.circuit_breaker_enabled {
            msg!("setting circuit breaker enabled: {:?}", flag);
            let was_enabled = self.get_flag(CIRCUIT_BREAKER_ENABLED);
            // Seed the EMA reference from the cached oracle price (must be non-zero and recent):
            // a warm-cache sanity gate that also stops an attacker sandwiching the enable tx to
            // pick the seed. NOT a defense against sustained manipulation — if the feed is
            // already compromised, so is the cache. CB is meant to be enabled proactively under
            // normal conditions; `clear_circuit_breaker` reseeds the reference if it ever ends
            // up anchored badly.
            if flag && !was_enabled {
                let last: I80F48 = self.cache.last_oracle_price.into();
                check!(
                    last > I80F48::ZERO,
                    MarginfiError::CircuitBreakerRequiresWarmCache
                );
                let now = Clock::get()?.unix_timestamp;
                let age = now.saturating_sub(self.cache.last_oracle_price_timestamp);
                check!(
                    (0..=CB_ENABLE_MAX_PRICE_AGE_SECONDS).contains(&age),
                    MarginfiError::CircuitBreakerRequiresWarmCache
                );
                let ref_price: I80F48 = self.cb_reference_price.into();
                if ref_price == I80F48::ZERO {
                    self.cb_reference_price = last.into();
                }
                let window_ref_price: I80F48 = self.cb_window_reference_price.into();
                if window_ref_price == I80F48::ZERO {
                    self.cb_window_reference_price = last.into();
                    self.cb_window_started_at = now;
                }
            }
            // Disable: zero halt + dedup state so a later re-enable starts clean.
            // `cb_reference_price` is preserved; `clear_circuit_breaker` offers a reseed path.
            if !flag && was_enabled {
                self.reset_cb_runtime_state();
            }
            self.update_flag(flag, CIRCUIT_BREAKER_ENABLED);
        }
        set_if_some!(
            self.config.cb_deviation_bps_tiers,
            config.cb_deviation_bps_tiers
        );
        set_if_some!(
            self.config.cb_tier_durations_seconds,
            config.cb_tier_durations_seconds
        );
        set_if_some!(
            self.config.cb_escalation_window_mult,
            config.cb_escalation_window_mult
        );
        set_if_some!(self.config.cb_ema_alpha_bps, config.cb_ema_alpha_bps);

        self.config.validate()?;
        if self.get_flag(CIRCUIT_BREAKER_ENABLED) {
            self.config.validate_circuit_breaker()?;
        }

        Ok(())
    }

    /// Configures just the borrow and deposit limits, ignoring all other values
    fn configure_unfrozen_fields_only(&mut self, config: &BankConfigOpt) -> MarginfiResult {
        set_if_some!(self.config.deposit_limit, config.deposit_limit);
        set_if_some!(self.config.borrow_limit, config.borrow_limit);
        // weights didn't change so no validation is needed
        Ok(())
    }

    /// Calculate the interest rate accrual state changes for a given time period
    ///
    /// Collected protocol and insurance fees are stored in state.
    /// A separate instruction is required to withdraw these fees.
    fn accrue_interest(
        &mut self,
        current_timestamp: i64,
        group: &MarginfiGroup,
        #[cfg(not(feature = "client"))] bank: Pubkey,
    ) -> MarginfiResult<()> {
        #[cfg(all(not(feature = "client"), feature = "debug"))]
        anchor_lang::solana_program::log::sol_log_compute_units();

        let time_delta: u64 = (current_timestamp - self.last_update).try_into().unwrap();
        if time_delta == 0 {
            return Ok(());
        }

        // Freeze interest accrual during a halt. Deposit/repay remain open while borrow/withdraw
        // are blocked, so unfrozen accrual would let new depositors free-ride on borrower
        // interest they can no longer escape. Advance `last_update` so post-halt accrual starts
        // from the current point instead of catching up on the frozen interval.
        if self.is_cb_halted(current_timestamp) {
            self.last_update = current_timestamp;
            return Ok(());
        }

        let total_assets = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities = self.get_liability_amount(self.total_liability_shares.into())?;

        self.last_update = current_timestamp;

        if (total_assets == I80F48::ZERO) || (total_liabilities == I80F48::ZERO) {
            #[cfg(not(feature = "client"))]
            emit!(LendingPoolBankAccrueInterestEvent {
                header: GroupEventHeader {
                    marginfi_group: self.group,
                    signer: None
                },
                bank,
                mint: self.mint,
                delta: time_delta,
                fees_collected: 0.,
                insurance_collected: 0.,
            });

            return Ok(());
        }
        let ir_calc = self
            .config
            .interest_rate_config
            .create_interest_rate_calculator(group);

        let InterestRateStateChanges {
            new_asset_share_value: asset_share_value,
            new_liability_share_value: liability_share_value,
            insurance_fees_collected,
            group_fees_collected,
            protocol_fees_collected,
        } = calc_interest_rate_accrual_state_changes(
            time_delta,
            total_assets,
            total_liabilities,
            &ir_calc,
            self.asset_share_value.into(),
            self.liability_share_value.into(),
        )?;

        debug!("deposit share value: {}\nliability share value: {}\nfees collected: {}\ninsurance collected: {}",
            asset_share_value, liability_share_value, group_fees_collected, insurance_fees_collected);

        self.cache.accumulated_since_last_update = asset_share_value
            .checked_sub(I80F48::from(self.asset_share_value))
            .and_then(|v| v.checked_mul(I80F48::from(self.total_asset_shares)))
            .ok_or_else(math_error!())?
            .into();
        self.cache.interest_accumulated_for = time_delta.min(u32::MAX as u64) as u32;
        self.asset_share_value = asset_share_value.into();
        self.liability_share_value = liability_share_value.into();

        if group_fees_collected > I80F48::ZERO {
            self.collected_group_fees_outstanding = {
                group_fees_collected
                    .checked_add(self.collected_group_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }

        if insurance_fees_collected > I80F48::ZERO {
            self.collected_insurance_fees_outstanding = {
                insurance_fees_collected
                    .checked_add(self.collected_insurance_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }
        if protocol_fees_collected > I80F48::ZERO {
            self.collected_program_fees_outstanding = {
                protocol_fees_collected
                    .checked_add(self.collected_program_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }

        #[cfg(not(feature = "client"))]
        {
            #[cfg(feature = "debug")]
            anchor_lang::solana_program::log::sol_log_compute_units();

            emit!(LendingPoolBankAccrueInterestEvent {
                header: GroupEventHeader {
                    marginfi_group: self.group,
                    signer: None
                },
                bank,
                mint: self.mint,
                delta: time_delta,
                fees_collected: group_fees_collected.to_num::<f64>(),
                insurance_collected: insurance_fees_collected.to_num::<f64>(),
            });
        }

        Ok(())
    }

    /// Updates bank cache with the actual values for interest/fee rates.
    ///
    /// Should be called in the end of each instruction calling `accrue_interest` to ensure the cache is up to date.
    ///
    /// # Arguments
    /// * `group` - The marginfi group
    fn update_bank_cache(&mut self, group: &MarginfiGroup) -> MarginfiResult<()> {
        if self.cache.is_liquidation_price_cache_locked() {
            return Ok(());
        }
        let total_assets_amount: I80F48 = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities_amount: I80F48 =
            self.get_liability_amount(self.total_liability_shares.into())?;

        if (total_assets_amount == I80F48::ZERO) || (total_liabilities_amount == I80F48::ZERO) {
            self.cache.reset_preserving_oracle_state();
            return Ok(());
        }

        let ir_calc = self
            .config
            .interest_rate_config
            .create_interest_rate_calculator(group);

        let utilization_rate: I80F48 = total_liabilities_amount
            .checked_div(total_assets_amount)
            .ok_or_else(math_error!())?;
        let interest_rates = ir_calc.calc_interest_rate(utilization_rate)?;

        update_interest_rates(&mut self.cache, &interest_rates);

        // Update banks last update timestamp
        self.last_update = Clock::get()?.unix_timestamp;
        Ok(())
    }

    /// Records the live oracle price on the cache and feeds it into the circuit breaker.
    /// Called from every path that consumes a fresh oracle reading (borrow/withdraw/liquidate,
    /// adapter integrations, and the explicit pulse crank); CB dedup makes redundant calls a
    /// no-op so callers don't need to coordinate.
    fn update_cache_price(
        &mut self,
        oracle_price: Option<OraclePriceWithMultiplier>,
    ) -> MarginfiResult<()> {
        if self.cache.is_liquidation_price_cache_locked() {
            return Ok(());
        }
        if let Some(price_with_multiplier) = oracle_price {
            let clock = Clock::get()?;
            self.cache.last_oracle_price = price_with_multiplier.oracle_price.price.into();
            self.cache.last_oracle_price_confidence =
                price_with_multiplier.oracle_price.confidence.into();
            self.cache.price_multiplier = price_with_multiplier.price_multiplier.into();
            self.cache.last_oracle_price_timestamp = clock.unix_timestamp;
            self.update_circuit_breaker(
                clock.unix_timestamp,
                clock.slot,
                price_with_multiplier.oracle_price,
            )?;
        }

        Ok(())
    }

    fn is_cb_halted(&self, now: i64) -> bool {
        self.get_flag(CIRCUIT_BREAKER_ENABLED) && self.cb_tier > 0 && now < self.cb_halt_ended_at
    }

    /// The operational state whose deposit/repay/withdraw rules currently apply. For a
    /// `CircuitBroken` bank this is the pre-break state (`cb_pre_break_state`) — so e.g. a bank
    /// that was `ReduceOnly` before breaking does not silently re-enable deposits. For any
    /// other bank it is just `operational_state`.
    fn cb_effective_operational_state(&self) -> BankOperationalState {
        if self.config.operational_state == BankOperationalState::CircuitBroken {
            match self.cb_pre_break_state {
                x if x == BankOperationalState::KilledByBankruptcy as u8 => {
                    BankOperationalState::KilledByBankruptcy
                }
                x if x == BankOperationalState::Paused as u8 => BankOperationalState::Paused,
                x if x == BankOperationalState::ReduceOnly as u8 => {
                    BankOperationalState::ReduceOnly
                }
                x if x == BankOperationalState::Operational as u8 => {
                    BankOperationalState::Operational
                }
                _ => BankOperationalState::Operational,
            }
        } else {
            self.config.operational_state
        }
    }

    /// Read-only guard that rejects the current instruction when the live oracle price deviates
    /// from `cb_reference_price` by at least the first breach tier (`cb_deviation_bps_tiers[0]`).
    ///
    /// A pure read — it never updates the reference or halt state, so callers can run it for
    /// every involved bank without those banks being writable. No-op when the breaker is
    /// disabled or the reference is not yet seeded. The deviation math mirrors
    /// `update_circuit_breaker` so both agree on what counts as a breach.
    fn cb_price_gate(&self, oracle: OraclePriceWithConfidence) -> MarginfiResult<()> {
        if !self.get_flag(CIRCUIT_BREAKER_ENABLED) {
            return Ok(());
        }
        // Without a usable reference there is nothing to compare against. The breaker seeds it
        // at enable-time (warm-cache gate) or on its first observation, so this only skips
        // genuinely cold banks.
        let ref_price: I80F48 = self.cb_reference_price.into();
        if ref_price <= CB_MIN_REF_PRICE {
            return Ok(());
        }
        let threshold = self.config.cb_deviation_bps_tiers[0];
        if threshold == 0 {
            return Ok(());
        }

        let deviation_bps =
            cb_confidence_adjusted_deviation_bps(oracle.price, ref_price, oracle.confidence);

        if deviation_bps >= threshold as u64 {
            msg!(
                "CB price gate tripped: deviation {} bps >= threshold {} bps",
                deviation_bps,
                threshold
            );
            return err!(MarginfiError::CircuitBreakerPriceJump);
        }
        Ok(())
    }

    /// Observe an oracle price and update the circuit breaker.
    ///
    /// Tier 0 (Operational): the first counted breach halts the bank at the breached tier. Halt
    /// lasts for the tier's duration, then enters an escalation window of
    /// `duration * escalation_mult` where any re-breach bumps to the next tier (capped at 3). A
    /// clean escalation window returns the bank to tier 0.
    ///
    /// Pulses are deduped by Solana slot, by `CB_MIN_PULSE_SLOT_GAP`, and by oracle publish-time;
    /// confidence is subtracted from raw delta before tier comparison. The breaker also checks a
    /// fixed long-window anchor to catch slow oracle walking under the per-observation threshold.
    /// After `CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK` consecutive tier-3 trips the bank is forced to
    /// `CircuitBroken`.
    fn update_circuit_breaker(
        &mut self,
        now: i64,
        slot: u64,
        oracle: OraclePriceWithConfidence,
    ) -> MarginfiResult<()> {
        if !self.get_flag(CIRCUIT_BREAKER_ENABLED) {
            return Ok(());
        }

        // `CircuitBroken` is the non-expiring end state — the breaker stays frozen (no
        // detection, no EMA, no escalation expiry) until an admin clears it.
        if self.config.operational_state == BankOperationalState::CircuitBroken {
            return Ok(());
        }

        // Halted: freeze detection and EMA until the tier timer expires.
        if self.cb_tier > 0 && now < self.cb_halt_ended_at {
            return Ok(());
        }

        if self.cb_tier > 0 {
            let tier_dur =
                self.config.cb_tier_durations_seconds[(self.cb_tier - 1) as usize] as i64;
            let escalation_deadline = self.cb_halt_ended_at.saturating_add(
                tier_dur.saturating_mul(self.config.cb_escalation_window_mult as i64),
            );
            // Escalation window expired without a re-breach: fall back to operational.
            if now >= escalation_deadline {
                let prior_tier = self.cb_tier;
                self.cb_tier = 0;
                self.cb_halt_started_at = 0;
                self.cb_halt_ended_at = 0;
                self.cb_tier3_consecutive_trips = 0;

                #[cfg(not(test))]
                emit!(crate::events::CircuitBreakerClearedEvent {
                    prior_tier,
                    reason: crate::events::CB_CLEAR_REASON_ESCALATION_EXPIRED,
                    current_timestamp: now,
                });
                #[cfg(test)]
                let _ = prior_tier;
            }
        }

        let mut ref_price: I80F48 = self.cb_reference_price.into();
        let current = oracle.price;

        // First-ever observation: seed the EMA and record dedup cursors. Normally already seeded
        // at enable-time, but this path also covers banks that were never pulsed before enable.
        if ref_price == I80F48::ZERO {
            self.cb_reference_price = current.into();
            self.cb_window_reference_price = current.into();
            self.cb_window_started_at = now;
            self.cb_last_observed_slot = slot;
            if oracle.source_time != 0 {
                self.cb_last_oracle_source_time = oracle.source_time;
            }
            return Ok(());
        }

        // Dedup gates: same-slot replay, min-slot-gap rate limit, then strictly-advancing oracle
        // publish-time (skipped when the adapter reports `source_time == 0`, e.g. Fixed feeds).
        if slot
            < self
                .cb_last_observed_slot
                .saturating_add(CB_MIN_PULSE_SLOT_GAP)
        {
            return Ok(());
        }
        if oracle.source_time != 0 && oracle.source_time <= self.cb_last_oracle_source_time {
            return Ok(());
        }
        self.cb_last_observed_slot = slot;
        if oracle.source_time != 0 {
            self.cb_last_oracle_source_time = oracle.source_time;
        }

        // Guard against a corrupted/decayed reference: reseed and defer detection rather than
        // dividing by a near-zero value and producing a megabps deviation.
        if ref_price <= CB_MIN_REF_PRICE {
            self.cb_reference_price = current.into();
            self.cb_window_reference_price = current.into();
            self.cb_window_started_at = now;
            return Ok(());
        }

        let confidence = oracle.confidence.max(I80F48::ZERO);
        let deviation_bps = cb_confidence_adjusted_deviation_bps(current, ref_price, confidence);
        let tiers = &self.config.cb_deviation_bps_tiers;

        let mut roll_window_anchor = false;
        let mut window_ref_price: I80F48 = self.cb_window_reference_price.into();
        if window_ref_price == I80F48::ZERO || window_ref_price <= CB_MIN_REF_PRICE {
            self.cb_window_reference_price = ref_price.into();
            self.cb_window_started_at = now;
            window_ref_price = ref_price;
        }
        if self.cb_window_started_at == 0 {
            self.cb_window_started_at = now;
        }

        let up_delta = (current - window_ref_price - confidence).max(I80F48::ZERO);
        let down_delta = (window_ref_price - current - confidence).max(I80F48::ZERO);
        let window_up_bps = cb_deviation_bps(up_delta, window_ref_price);
        let window_down_bps = cb_deviation_bps(down_delta, window_ref_price);
        let window_deviation_bps = window_up_bps.max(window_down_bps);

        if window_up_bps >= CB_WINDOW_MAX_UP_BPS || window_down_bps >= CB_WINDOW_MAX_DOWN_BPS {
            let breached = cb_tier_for_deviation(tiers, window_deviation_bps).max(1);
            msg!(
                "CB long-window cap tripped: up {} bps / down {} bps",
                window_up_bps,
                window_down_bps
            );
            self.trip_cb_halt(now, window_deviation_bps, breached);
            return Ok(());
        }

        if now >= self.cb_window_started_at.saturating_add(CB_WINDOW_SECONDS) {
            roll_window_anchor = true;
        }

        let breached: u8 = cb_tier_for_deviation(tiers, deviation_bps);

        if breached > 0 {
            self.trip_cb_halt(now, deviation_bps, breached);
            return Ok(());
        }

        if roll_window_anchor {
            self.cb_window_reference_price = current.into();
            self.cb_window_started_at = now;
        }

        // Clean observation: advance the EMA reference. The per-pulse shift is clipped so a
        // high-α config can't reanchor the reference in a handful of attacker-controlled
        // publications.
        let alpha = I80F48::from_num(self.config.cb_ema_alpha_bps) / I80F48::from_num(10_000u64);
        let new_ref = alpha * current + (I80F48::ONE - alpha) * ref_price;
        let max_shift = ref_price * I80F48::from_num(CB_MAX_EMA_SHIFT_BPS_PER_PULSE)
            / I80F48::from_num(10_000u64);
        let clipped_shift = (new_ref - ref_price).max(-max_shift).min(max_shift);
        ref_price += clipped_shift;
        self.cb_reference_price = ref_price.into();

        Ok(())
    }

    fn deposit_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult {
        check!(
            to.key.eq(&self.liquidity_vault),
            MarginfiError::InvalidTransfer
        );

        debug!(
            "deposit_spl_transfer: amount: {} from {} to {}, auth {}",
            amount, from.key, to.key, authority.key
        );

        if let Some(mint) = maybe_mint {
            spl_token_2022::onchain::invoke_transfer_checked(
                program.key,
                from,
                mint.to_account_info(),
                to,
                authority,
                remaining_accounts,
                amount,
                mint.decimals,
                &[],
            )?;
        } else {
            #[allow(deprecated)]
            transfer(
                CpiContext::new_with_signer(
                    program,
                    Transfer {
                        from,
                        to,
                        authority,
                    },
                    &[],
                ),
                amount,
            )?;
        }

        Ok(())
    }

    fn withdraw_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        signer_seeds: &[&[&[u8]]],
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult {
        debug!(
            "withdraw_spl_transfer: amount: {} from {} to {}, auth {}",
            amount, from.key, to.key, authority.key
        );

        if let Some(mint) = maybe_mint {
            spl_token_2022::onchain::invoke_transfer_checked(
                program.key,
                from,
                mint.to_account_info(),
                to,
                authority,
                remaining_accounts,
                amount,
                mint.decimals,
                signer_seeds,
            )?;
        } else {
            // `transfer_checked` and `transfer` does the same thing, the additional `_checked` logic
            // is only to assert the expected attributes by the user (mint, decimal scaling),
            //
            // Security of `transfer` is equal to `transfer_checked`.
            #[allow(deprecated)]
            transfer(
                CpiContext::new_with_signer(
                    program,
                    Transfer {
                        from,
                        to,
                        authority,
                    },
                    signer_seeds,
                ),
                amount,
            )?;
        }

        Ok(())
    }

    /// Socialize a loss of `loss_amount` among depositors, the `total_deposit_shares` stays the
    /// same, but total value of deposits is reduced by `loss_amount`;
    ///
    /// In cases where assets < liabilities, the asset share value will be set to zero, but cannot
    /// go negative. Effectively, depositors forfeit their entire deposit AND all earned interest in
    /// this case.
    fn socialize_loss(&mut self, loss_amount: I80F48) -> MarginfiResult<bool> {
        let mut kill_bank = false;
        let total_asset_shares: I80F48 = self.total_asset_shares.into();
        let old_asset_share_value: I80F48 = self.asset_share_value.into();

        // Compute total "old" value of shares
        let total_value: I80F48 = total_asset_shares
            .checked_mul(old_asset_share_value)
            .ok_or_else(math_error!())?;

        // Subtract loss, clamping at zero (i.e. assets < liabilities, the bank is wiped out)
        if total_value <= loss_amount {
            self.asset_share_value = I80F48::ZERO.into();
            // This state is irrecoverable, the bank is dead.
            kill_bank = true;
        } else {
            // otherwise subtract then redistribute
            let new_share_value: I80F48 = (total_value - loss_amount)
                .checked_div(total_asset_shares)
                .ok_or_else(math_error!())?;
            self.asset_share_value = new_share_value.into();
            // Sanity check: should be unreachable.
            if new_share_value == I80F48::ZERO {
                kill_bank = true;
            }
        }

        Ok(kill_bank)
    }

    fn get_flag(&self, flag: u64) -> bool {
        (self.flags & flag) == flag
    }

    fn update_flag(&mut self, value: bool, flag: u64) {
        assert!(Self::verify_group_flags(flag));

        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn verify_group_flags(flags: u64) -> bool {
        flags & GROUP_FLAGS == flags
    }

    fn increment_lending_position_count(&mut self) {
        self.lending_position_count = self.lending_position_count.saturating_add(1);
    }

    fn decrement_lending_position_count(&mut self) {
        self.lending_position_count = self.lending_position_count.saturating_sub(1);
    }

    fn increment_borrowing_position_count(&mut self) {
        self.borrowing_position_count = self.borrowing_position_count.saturating_add(1);
    }

    fn decrement_borrowing_position_count(&mut self) {
        self.borrowing_position_count = self.borrowing_position_count.saturating_sub(1);
    }

    fn reset_cb_runtime_state(&mut self) {
        // If the breaker escalated the bank into the non-expiring `CircuitBroken` end state,
        // restore the operational state it held beforehand (a no-op otherwise).
        self.config.operational_state = self.cb_effective_operational_state();
        self.cb_tier = 0;
        self.cb_halt_started_at = 0;
        self.cb_halt_ended_at = 0;
        self.cb_tier3_consecutive_trips = 0;
        self.cb_last_observed_slot = 0;
        self.cb_last_oracle_source_time = 0;
    }

    fn trip_cb_halt(&mut self, now: i64, deviation_bps: u64, breached_tier: u8) {
        let new_tier = if self.cb_tier > 0 {
            (self.cb_tier + 1).min(3)
        } else {
            breached_tier
        };
        let dur_sec = self.config.cb_tier_durations_seconds[(new_tier - 1) as usize] as i64;
        self.cb_tier = new_tier;
        self.cb_halt_started_at = now;
        self.cb_halt_ended_at = now.saturating_add(dur_sec);
        msg!("CB halt tier {} duration {}s", new_tier, dur_sec);
        #[cfg(not(test))]
        emit!(crate::events::CircuitBreakerTrippedEvent {
            tier: new_tier,
            deviation_bps,
            halt_started_at: now,
            halt_ended_at: self.cb_halt_ended_at,
        });
        #[cfg(test)]
        let _ = deviation_bps;

        if new_tier == 3 {
            self.cb_tier3_consecutive_trips = self.cb_tier3_consecutive_trips.saturating_add(1);
            if self.cb_tier3_consecutive_trips >= CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK {
                if self.config.operational_state == BankOperationalState::CircuitBroken {
                    return;
                }
                self.cb_pre_break_state = self.config.operational_state as u8;
                self.config.operational_state = BankOperationalState::CircuitBroken;
                msg!(
                    "CB storm: {} consecutive tier-3 trips → bank forced CircuitBroken",
                    self.cb_tier3_consecutive_trips
                );
                #[cfg(not(test))]
                emit!(crate::events::CircuitBreakerAutoBrokenEvent {
                    consecutive_tier3_trips: self.cb_tier3_consecutive_trips,
                    current_timestamp: now,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum BankVaultType {
    Liquidity,
    Insurance,
    Fee,
}

impl BankVaultType {
    pub fn get_seed(self) -> &'static [u8] {
        match self {
            BankVaultType::Liquidity => LIQUIDITY_VAULT_SEED.as_bytes(),
            BankVaultType::Insurance => INSURANCE_VAULT_SEED.as_bytes(),
            BankVaultType::Fee => FEE_VAULT_SEED.as_bytes(),
        }
    }

    pub fn get_authority_seed(self) -> &'static [u8] {
        match self {
            BankVaultType::Liquidity => LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            BankVaultType::Insurance => INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            BankVaultType::Fee => FEE_VAULT_AUTHORITY_SEED.as_bytes(),
        }
    }
}

#[cfg(test)]
mod cb_tests {
    use super::*;
    use bytemuck::Zeroable;
    use marginfi_type_crate::types::OraclePriceWithConfidence;

    fn make_cb_bank() -> Bank {
        let mut b = Bank::zeroed();
        b.flags |= CIRCUIT_BREAKER_ENABLED;
        b.config.cb_deviation_bps_tiers = [500, 1000, 2500]; // 5% / 10% / 25%
        b.config.cb_tier_durations_seconds = [600, 3600, 14400]; // 10m / 1h / 4h
        b.config.cb_escalation_window_mult = 2;
        b.config.cb_ema_alpha_bps = 1000; // α = 0.1
        b
    }

    fn price(n: u32) -> I80F48 {
        I80F48::from_num(n)
    }

    fn price_f(n: f64) -> I80F48 {
        I80F48::from_num(n)
    }

    /// Observation with zero confidence and `source_time = 0` (skips oracle-source dedup).
    fn obs(p: I80F48) -> OraclePriceWithConfidence {
        OraclePriceWithConfidence {
            price: p,
            confidence: I80F48::ZERO,
            source_time: 0,
        }
    }

    /// Feed observations on distinct slots starting at `start_slot`, stepping by
    /// `CB_MIN_PULSE_SLOT_GAP` so each call passes the slot-gap rate limit.
    fn feed(b: &mut Bank, now: i64, start_slot: u64, prices: &[I80F48]) {
        for (i, p) in prices.iter().enumerate() {
            let step = (i as u64) * CB_MIN_PULSE_SLOT_GAP;
            b.update_circuit_breaker(now + i as i64, start_slot + step, obs(*p))
                .unwrap();
        }
    }

    fn trip_tier3_storm(b: &mut Bank) {
        let big = price(200);
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(big)).unwrap();
        let halt_ended_1 = b.cb_halt_ended_at;
        b.update_circuit_breaker(halt_ended_1 + 1, 1_004, obs(big))
            .unwrap();
        let halt_ended_2 = b.cb_halt_ended_at;
        b.update_circuit_breaker(halt_ended_2 + 1, 1_006, obs(big))
            .unwrap();
    }

    #[test]
    fn disabled_is_noop() {
        let mut b = Bank::zeroed(); // flag off
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        assert_eq!(b.cb_reference_price, I80F48::ZERO.into());
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn first_observation_seeds_reference() {
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        assert_eq!(I80F48::from(b.cb_reference_price), price(100));
        assert_eq!(I80F48::from(b.cb_window_reference_price), price(100));
        assert_eq!(b.cb_window_started_at, 1_000);
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_last_observed_slot, 1_000);
    }

    #[test]
    fn clean_reads_update_ema() {
        let mut b = make_cb_bank();
        feed(&mut b, 1_000, 1_000, &[price(100), price(101)]);
        // α=0.1: ref = 0.1 * 101 + 0.9 * 100 = 100.1
        let r: I80F48 = b.cb_reference_price.into();
        assert!((r - I80F48::from_num(100.1)).abs() < I80F48::from_num(0.001));
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn first_breach_trips_halt_immediately() {
        // The new model trips on the *first* breaching observation — there is no sustained-
        // observation wait. A single 10% spike vs ref=100 is 1000 bps → tier 2, so the bank
        // halts at tier 2 on that one pulse.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);

        b.update_circuit_breaker(1_001, 1_002, obs(price(110)))
            .unwrap();
        assert_eq!(b.cb_tier, 2);
        // Tier-2 duration = 60m → halt_ended_at = now + 3600.
        assert_eq!(b.cb_halt_started_at, 1_001);
        assert_eq!(b.cb_halt_ended_at, 1_001 + 60 * 60);
        assert!(b.is_cb_halted(1_001));
        // Tier-2 trip does not count toward the tier-3 storm counter.
        assert_eq!(b.cb_tier3_consecutive_trips, 0);
    }

    #[test]
    fn trip_tier_matches_breach_magnitude() {
        // The trip tier when starting from tier 0 is exactly the tier of the breaching
        // observation. A first observation big enough to clear the tier-3 threshold (>=2500
        // bps) trips straight to tier 3.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();

        // 200 vs ref=100 → 10000 bps → tier 3.
        b.update_circuit_breaker(1_001, 1_002, obs(price(200)))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
        // First tier-3 trip → storm counter starts at 1.
        assert_eq!(b.cb_tier3_consecutive_trips, 1);
        // Tier-3 duration = 4h.
        assert_eq!(b.cb_halt_ended_at, 1_001 + 240 * 60);
    }

    #[test]
    fn slow_staircase_trips_long_window_cap() {
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();

        for step in 1..200 {
            b.update_circuit_breaker(
                1_000 + step as i64,
                1_000 + step * CB_MIN_PULSE_SLOT_GAP,
                obs(price_f(100.0 + step as f64 / 10.0)),
            )
            .unwrap();
            assert_eq!(b.cb_tier, 0);
        }

        b.update_circuit_breaker(
            1_200,
            1_000 + 200 * CB_MIN_PULSE_SLOT_GAP,
            obs(price_f(120.0)),
        )
        .unwrap();
        assert_eq!(b.cb_tier, 2);
        assert_eq!(b.cb_halt_started_at, 1_200);
    }

    #[test]
    fn clean_expired_window_rolls_anchor_forward() {
        let mut b = make_cb_bank();
        b.config.cb_deviation_bps_tiers = [5_000, 6_000, 7_000];
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();

        b.update_circuit_breaker(
            1_000 + CB_WINDOW_SECONDS + 1,
            1_000 + CB_MIN_PULSE_SLOT_GAP,
            obs(price(110)),
        )
        .unwrap();

        assert_eq!(b.cb_tier, 0);
        assert_eq!(I80F48::from(b.cb_window_reference_price), price(110));
        assert_eq!(b.cb_window_started_at, 1_000 + CB_WINDOW_SECONDS + 1);
    }

    #[test]
    fn same_slot_replay_is_deduped() {
        // Spamming the pulse in a single slot must not trip more than once: only the first
        // observation in a slot is processed, and a halt freezes everything after.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        // All three of these are in slot 1_002 — the first trips a tier-2 halt, the rest are
        // deduped and also frozen by the halt.
        b.update_circuit_breaker(1_001, 1_002, obs(price(110)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(110)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(110)))
            .unwrap();
        assert_eq!(b.cb_tier, 2);
        assert_eq!(b.cb_halt_started_at, 1_001);
    }

    #[test]
    fn same_slot_replay_before_breach_is_deduped() {
        // A clean observation replayed in one slot updates the EMA exactly once.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(101)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(101)))
            .unwrap();
        // EMA advanced once: 0.1*101 + 0.9*100 = 100.1.
        let r: I80F48 = b.cb_reference_price.into();
        assert!((r - I80F48::from_num(100.1)).abs() < I80F48::from_num(0.001));
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn sub_min_gap_pulses_are_rate_limited() {
        // Pulses inside the min-slot-gap window must be dropped, even if the price would breach.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        // Slot 1_001 is within the min-gap (≥ 1_000 + CB_MIN_PULSE_SLOT_GAP = 1_002), so a breach
        // observation here is silently dropped — no trip.
        b.update_circuit_breaker(1_001, 1_001, obs(price(130)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn stale_publish_does_not_trip_across_slots() {
        // A single stale Pyth publication replayed across many Solana slots must not trip the
        // halt — otherwise an attacker could spam `pulse` to act on one stale reading. The
        // strictly-advancing source-time dedup drops every replay (same `source_time` as the
        // seed), so detection never runs on the stale price.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(
            1_000,
            1_000,
            OraclePriceWithConfidence {
                price: price(100),
                confidence: I80F48::ZERO,
                source_time: 500, // seed with publish-time 500
            },
        )
        .unwrap();
        // 4 pulses, distinct Solana slots past the min gap, but same oracle source_time as the
        // seed → all dropped by the source-time dedup → no trip.
        for i in 1..=4 {
            b.update_circuit_breaker(
                1_000 + i,
                1_000 + (i as u64) * CB_MIN_PULSE_SLOT_GAP,
                OraclePriceWithConfidence {
                    price: price(130),
                    confidence: I80F48::ZERO,
                    source_time: 500, // identical stale publish
                },
            )
            .unwrap();
        }
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_halt_started_at, 0);
    }

    #[test]
    fn clean_read_updates_ema_without_tripping() {
        // A below-tier-0 observation never trips; it advances the EMA and leaves tier at 0.
        let mut b = make_cb_bank();
        feed(&mut b, 1_000, 1_000, &[price(100), price(101)]);
        assert_eq!(b.cb_tier, 0);
        b.update_circuit_breaker(1_002, 1_000 + 2 * CB_MIN_PULSE_SLOT_GAP, obs(price(102)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);
        // ref: 100 → 100.1 → 100.29.
        let r: I80F48 = b.cb_reference_price.into();
        assert!((r - I80F48::from_num(100.29)).abs() < I80F48::from_num(0.001));
    }

    #[test]
    fn halt_freezes_detection_and_ema() {
        let mut b = make_cb_bank();
        b.cb_tier = 1;
        b.cb_halt_started_at = 1_000;
        b.cb_halt_ended_at = 1_600; // 10 min later
        b.cb_reference_price = price(100).into();
        // Any read during halt is a no-op
        b.update_circuit_breaker(1_100, 1_100, obs(price(50)))
            .unwrap();
        assert_eq!(b.cb_tier, 1);
        assert_eq!(I80F48::from(b.cb_reference_price), price(100));
    }

    #[test]
    fn escalation_window_bumps_tier() {
        let mut b = make_cb_bank();
        b.cb_tier = 1;
        b.cb_halt_started_at = 1_000;
        b.cb_halt_ended_at = 1_600;
        b.cb_reference_price = price(100).into();
        // Escalation window = 600 * 2 = 1200 → deadline = 2800. Now = 1_700 (in window).
        // A single re-breach inside the window escalates tier 1 → 2.
        b.update_circuit_breaker(1_700, 1_700, obs(price(110)))
            .unwrap();
        assert_eq!(b.cb_tier, 2);
    }

    #[test]
    fn escalation_window_expiry_resets_tier_and_halt_timestamps() {
        let mut b = make_cb_bank();
        b.cb_tier = 1;
        b.cb_halt_started_at = 1_000;
        b.cb_halt_ended_at = 1_600;
        b.cb_reference_price = price(100).into();
        // Deadline = 2800. A clean read past that resets tier and zeros halt timestamps.
        b.update_circuit_breaker(3_000, 3_000, obs(price(100)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_halt_started_at, 0);
        assert_eq!(b.cb_halt_ended_at, 0);
    }

    #[test]
    fn tier_capped_at_three() {
        let mut b = make_cb_bank();
        b.cb_tier = 3;
        b.cb_halt_started_at = 1_000;
        b.cb_halt_ended_at = 1_600;
        b.cb_reference_price = price(100).into();
        // Still inside escalation window for tier 3 (240m * 2 = 480m → way past 1_700).
        // A re-breach escalates `(3 + 1).min(3) = 3`.
        b.update_circuit_breaker(1_700, 1_700, obs(price(200)))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
    }

    #[test]
    fn confidence_absorbs_noise() {
        // 5% move with a 5% confidence band → effective delta 0, no breach.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(
            1_001,
            1_000 + CB_MIN_PULSE_SLOT_GAP,
            OraclePriceWithConfidence {
                price: price(105),
                confidence: I80F48::from_num(5), // 5% of 100
                source_time: 0,
            },
        )
        .unwrap();
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn confidence_does_not_hide_real_breach() {
        // 20% move with a 5% confidence band → effective delta 15% → still trips tier 2 threshold.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(
            1_001,
            1_000 + CB_MIN_PULSE_SLOT_GAP,
            OraclePriceWithConfidence {
                price: price(120),
                confidence: I80F48::from_num(5),
                source_time: 0,
            },
        )
        .unwrap();
        // Effective deviation = 15% = 1500 bps → tier 2.
        assert_eq!(b.cb_tier, 2);
        assert!(b.is_cb_halted(1_001));
    }

    #[test]
    fn is_cb_halted_respects_flag_tier_and_time() {
        let mut b = make_cb_bank();
        assert!(!b.is_cb_halted(1_000)); // tier 0
        b.cb_tier = 1;
        b.cb_halt_ended_at = 2_000;
        assert!(b.is_cb_halted(1_500));
        assert!(!b.is_cb_halted(2_500)); // past ended_at
        b.flags &= !CIRCUIT_BREAKER_ENABLED;
        assert!(!b.is_cb_halted(1_500)); // flag off
    }

    #[test]
    fn advancing_source_time_trips_breach() {
        // A strictly-advancing oracle source_time on a fresh slot must be accepted by the dedup
        // gate, so the breaching observation trips a halt.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(
            1_000,
            1_000,
            OraclePriceWithConfidence {
                price: price(100),
                confidence: I80F48::ZERO,
                source_time: 500,
            },
        )
        .unwrap();
        b.update_circuit_breaker(
            1_001,
            1_000 + CB_MIN_PULSE_SLOT_GAP,
            OraclePriceWithConfidence {
                price: price(110), // 10% spike → tier 2 (1000 bps)
                confidence: I80F48::ZERO,
                source_time: 501, // strictly advances
            },
        )
        .unwrap();
        assert_eq!(b.cb_tier, 2);
        assert_eq!(b.cb_last_oracle_source_time, 501);
    }

    #[test]
    fn negative_confidence_clamped_to_zero() {
        // Below-tier-1 delta (4%) with a *negative* confidence: an unclamped subtraction would
        // inflate effective delta to 5% and cross tier 1 (500 bps). The clamp at
        // `oracle.confidence.max(I80F48::ZERO)` keeps effective delta at the raw 4%, no breach.
        let mut b = make_cb_bank();
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(
            1_001,
            1_000 + CB_MIN_PULSE_SLOT_GAP,
            OraclePriceWithConfidence {
                price: price(104),
                confidence: I80F48::from_num(-1),
                source_time: 0,
            },
        )
        .unwrap();
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn near_zero_ref_price_reseeds_without_tripping() {
        // If the cached reference somehow decayed below `CB_MIN_REF_PRICE`, the next pulse must
        // reseed from the live observation rather than divide and produce a megabps deviation that
        // would falsely trip the halt. The reseed branch returns before detection.
        let mut b = make_cb_bank();
        // Below CB_MIN_REF_PRICE (1<<20 bits) but above zero, to skip the first-observation seed
        // branch and hit the near-zero guard instead.
        b.cb_reference_price = I80F48::from_bits(1).into();
        b.cb_last_observed_slot = 1_000; // so the dedup gate passes for slot >= 1_002
        b.update_circuit_breaker(2_000, 1_002, obs(price(500)))
            .unwrap();
        assert_eq!(I80F48::from(b.cb_reference_price), price(500));
        assert_eq!(b.cb_tier, 0);
    }

    #[test]
    fn tier3_storm_promotes_to_circuit_broken() {
        // After `CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK` (= 3) consecutive tier-3 trips with no clean
        // escalation window in between, the bank must enter the non-expiring
        // `CircuitBroken` end state so a sustained attacker can't keep it halted indefinitely.
        // Each trip now happens on a single pulse.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::Operational;

        let big = price(200); // 100% spike vs ref=100 → tier 3 (>=2500 bps).

        // ---- Trip 1: seed at 100, then one tier-3 breach.
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(big)).unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(b.cb_tier3_consecutive_trips, 1);
        let halt_ended_1 = b.cb_halt_ended_at;
        assert_eq!(halt_ended_1, 1_001 + 240 * 60); // tier-3 duration = 4h
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::Operational
        );

        // ---- Trip 2: resume after the halt expires but inside the escalation window
        // (mult=2 → 8h). One breaching pulse escalates `(3 + 1).min(3) = 3`.
        b.update_circuit_breaker(halt_ended_1 + 1, 1_004, obs(big))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(b.cb_tier3_consecutive_trips, 2);
        let halt_ended_2 = b.cb_halt_ended_at;
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::Operational
        );

        // ---- Trip 3: crosses `CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK` and triggers the storm brake.
        b.update_circuit_breaker(halt_ended_2 + 1, 1_006, obs(big))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(
            b.cb_tier3_consecutive_trips,
            CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK
        );
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );
        // The prior operational state is stashed for `clear_circuit_breaker` to restore.
        assert_eq!(
            b.cb_pre_break_state,
            BankOperationalState::Operational as u8
        );
    }

    #[test]
    fn two_consecutive_tier3_trips_do_not_force_break() {
        // Boundary case: the storm brake must fire only at `CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK` (= 3),
        // not earlier. After two trips the bank stays operational.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::Operational;
        let big = price(200);

        // Trip 1
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(big)).unwrap();
        let halt_ended_1 = b.cb_halt_ended_at;
        assert_eq!(b.cb_tier3_consecutive_trips, 1);

        // Trip 2 inside the escalation window — counter reaches 2, brake must not fire yet.
        b.update_circuit_breaker(halt_ended_1 + 1, 1_004, obs(big))
            .unwrap();
        assert_eq!(b.cb_tier3_consecutive_trips, 2);
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::Operational
        );
    }

    #[test]
    fn clean_escalation_expiry_resets_storm_counter() {
        // A clean escalation-window expiry must zero the tier-3 storm counter. Without this,
        // widely-spaced attacks could accrue toward the brake threshold over time without ever
        // sustaining pressure on the bank.
        let mut b = make_cb_bank();
        let big = price(200);

        // One tier-3 trip → counter = 1.
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(big)).unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(b.cb_tier3_consecutive_trips, 1);

        // Tier-3 dur = 240m, esc_mult = 2 → escalation_deadline = halt_ended_at + 28800.
        let escalation_deadline = b.cb_halt_ended_at + (240 * 60) * 2;

        // Clean read past the escalation deadline triggers the expiry branch: tier→0, halt
        // timestamps→0, AND tier3_consecutive_trips→0.
        b.update_circuit_breaker(escalation_deadline + 1, 1_010, obs(price(100)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_halt_started_at, 0);
        assert_eq!(b.cb_halt_ended_at, 0);
        assert_eq!(b.cb_tier3_consecutive_trips, 0);
    }

    #[test]
    fn escalates_through_full_tier_ladder() {
        // Drives a pulse sequence across three phases (operational → tier 1 → tier 2 → tier 3)
        // to verify the escalation rule `(cb_tier + 1).min(3)` ratchets the tier on each
        // re-breach inside the escalation window. Each trip is a single breaching pulse.
        let mut b = make_cb_bank();

        // ---- Phase 1: trip to tier 1 from operational. price=107 keeps deviation in
        // [500, 1000) bps so `breached` is exactly 1, and tier-from-trip-when-tier-was-zero is
        // `breached`.
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(107)))
            .unwrap();
        assert_eq!(b.cb_tier, 1);
        // Tier-1 duration = 10m → halt_ended_at = 1001 + 600.
        let halt_ended_1 = b.cb_halt_ended_at;
        assert_eq!(halt_ended_1, 1_001 + 10 * 60);

        // ---- Phase 2: re-breach in escalation window → tier 2. Tier-1 escalation deadline =
        // halt_ended_at + tier_dur(600) * mult(2) = halt_ended_1 + 1200; the pulse below stays
        // strictly before that.
        b.update_circuit_breaker(halt_ended_1 + 1, 1_004, obs(price(110)))
            .unwrap();
        assert_eq!(b.cb_tier, 2);
        // Tier-2 duration = 60m → halt_ended_at extends by 3600s from the trip pulse time.
        let halt_ended_2 = b.cb_halt_ended_at;
        assert_eq!(halt_ended_2, halt_ended_1 + 1 + 60 * 60);

        // ---- Phase 3: re-breach in tier-2 escalation window → tier 3. Tier-2 escalation
        // deadline = halt_ended_2 + tier_dur(3600) * mult(2) = halt_ended_2 + 7200.
        b.update_circuit_breaker(halt_ended_2 + 1, 1_006, obs(price(120)))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(b.cb_halt_ended_at, halt_ended_2 + 1 + 240 * 60);
        // First tier-3 trip → storm counter starts at 1.
        assert_eq!(b.cb_tier3_consecutive_trips, 1);
    }

    #[test]
    fn clean_window_recovery_then_fresh_breach_starts_at_tier_one() {
        // After a tier-1 trip and a clean escalation-window expiry, the next breach must trip to
        // tier 1 again — not tier 2. The clean expiry resets `cb_tier` to 0, which routes the
        // next trip through the from-operational branch (`new_tier = breached`) instead of the
        // from-escalation branch (`new_tier = (cb_tier + 1).min(3)`).
        let mut b = make_cb_bank();

        // Tier-1 trip.
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(107)))
            .unwrap();
        assert_eq!(b.cb_tier, 1);

        // Clean expiry: now > halt_ended_at + tier_dur(600) * mult(2).
        let clean_window_end = b.cb_halt_ended_at + (10 * 60) * 2;
        b.update_circuit_breaker(clean_window_end + 1, 1_004, obs(price(100)))
            .unwrap();
        assert_eq!(b.cb_tier, 0);

        // Fresh breach → tier 1 (not tier 2). price=107 keeps deviation in [500, 1000) bps.
        b.update_circuit_breaker(clean_window_end + 2, 1_006, obs(price(107)))
            .unwrap();
        assert_eq!(b.cb_tier, 1);
    }

    #[test]
    fn storm_brake_escalates_reduce_only_to_circuit_broken() {
        // A bank already in `ReduceOnly` (e.g. admin-set) must escalate to `CircuitBroken` once
        // the storm threshold is crossed, and stash `ReduceOnly` as the pre-break state.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::ReduceOnly;
        trip_tier3_storm(&mut b);

        assert_eq!(
            b.cb_tier3_consecutive_trips,
            CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK
        );
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );
        assert_eq!(b.cb_pre_break_state, BankOperationalState::ReduceOnly as u8);
    }

    #[test]
    fn storm_brake_escalates_paused_to_circuit_broken() {
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::Paused;
        trip_tier3_storm(&mut b);

        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );
        assert_eq!(b.cb_pre_break_state, BankOperationalState::Paused as u8);

        b.reset_cb_runtime_state();
        assert_eq!(b.config.operational_state, BankOperationalState::Paused);
    }

    #[test]
    fn storm_brake_records_killed_by_bankruptcy_before_circuit_breaking() {
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::KilledByBankruptcy;
        trip_tier3_storm(&mut b);

        assert_eq!(
            b.cb_tier3_consecutive_trips,
            CB_MAX_TIER3_BEFORE_CIRCUIT_BREAK
        );
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );
        assert_eq!(
            b.cb_pre_break_state,
            BankOperationalState::KilledByBankruptcy as u8
        );

        b.reset_cb_runtime_state();
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::KilledByBankruptcy
        );
    }

    #[test]
    fn circuit_broken_bank_is_full_noop() {
        // A `CircuitBroken` bank freezes the breaker entirely: no detection, no EMA, no
        // escalation expiry — until an admin clears it.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::CircuitBroken;
        b.cb_tier = 3;
        b.cb_halt_started_at = 1_000;
        b.cb_halt_ended_at = 2_000;
        b.cb_reference_price = price(100).into();
        // A wild observation, well past any escalation deadline, changes nothing.
        b.update_circuit_breaker(9_999_999, 9_999_999, obs(price(1)))
            .unwrap();
        assert_eq!(b.cb_tier, 3);
        assert_eq!(b.cb_halt_ended_at, 2_000);
        assert_eq!(I80F48::from(b.cb_reference_price), price(100));
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );
    }

    #[test]
    fn ema_shift_clip_caps_per_pulse_movement() {
        // A clean 10× spike with α=0.1 would shift the EMA by 0.1 × 900 = 90 absolute units in
        // one pulse without the cap. But 900% deviation breaches tier 3 long before the EMA
        // path is reached, so use a *high-confidence* observation: a 1000-unit price with a
        // 999-unit confidence band has effective deviation 1% — below tier 0 — so it takes the
        // clean EMA path. With CB_MAX_EMA_SHIFT_BPS_PER_PULSE = 500 (5%) the shift is clipped to
        // ref_price * 0.05 = 5.
        let mut b = make_cb_bank();
        b.config.cb_ema_alpha_bps = 1000; // α = 0.1
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(
            1_001,
            1_002,
            OraclePriceWithConfidence {
                price: price(1_000),
                confidence: I80F48::from_num(899), // effective delta = 900 - 899 = 1 unit
                source_time: 0,
            },
        )
        .unwrap();
        assert_eq!(b.cb_tier, 0);
        let r: I80F48 = b.cb_reference_price.into();
        // ref starts at 100, raw EMA target 0.1*1000 + 0.9*100 = 190, shift clipped to
        // 100 * 0.05 = 5 → ref = 105 after one pulse.
        assert!(
            r <= I80F48::from_num(105) + I80F48::from_num(0.001),
            "EMA shifted further than the per-pulse cap allows: {}",
            r
        );
        assert!(
            r >= I80F48::from_num(105) - I80F48::from_num(0.001),
            "EMA shift was clipped below the cap: {}",
            r
        );
    }

    // ---- Inline price gate (`cb_price_gate`) -------------------------------------------------

    #[test]
    fn price_gate_passes_below_tier_zero_threshold() {
        // A deviation strictly below `cb_deviation_bps_tiers[0]` (500 bps) does not trip the gate.
        let mut b = make_cb_bank();
        b.cb_reference_price = price(100).into();
        // 4% deviation → 400 bps < 500.
        assert!(b.cb_price_gate(obs(price(104))).is_ok());
    }

    #[test]
    fn price_gate_trips_at_tier_zero_threshold() {
        // A deviation at/above `cb_deviation_bps_tiers[0]` (500 bps) fails the current action
        // with `CircuitBreakerPriceJump`, even before any halt is tripped.
        let mut b = make_cb_bank();
        b.cb_reference_price = price(100).into();
        // 5% deviation → exactly 500 bps.
        let err = b.cb_price_gate(obs(price(105))).unwrap_err();
        assert_eq!(err, error!(MarginfiError::CircuitBreakerPriceJump));
        // A larger jump also trips.
        assert!(b.cb_price_gate(obs(price(200))).is_err());
    }

    #[test]
    fn price_gate_noop_when_cb_disabled() {
        // With the flag off the gate is a pure no-op regardless of how far the price moved.
        let mut b = make_cb_bank();
        b.flags &= !CIRCUIT_BREAKER_ENABLED;
        b.cb_reference_price = price(100).into();
        assert!(b.cb_price_gate(obs(price(1_000))).is_ok());
    }

    #[test]
    fn price_gate_noop_when_reference_unseeded() {
        // Without a usable reference there is nothing to compare against — a cold bank passes.
        let mut b = make_cb_bank();
        // Reference left at zero (cold). Also exercise the near-zero guard.
        assert!(b.cb_price_gate(obs(price(1_000))).is_ok());
        b.cb_reference_price = I80F48::from_bits(1).into(); // <= CB_MIN_REF_PRICE
        assert!(b.cb_price_gate(obs(price(1_000))).is_ok());
    }

    #[test]
    fn price_gate_subtracts_confidence() {
        // Confidence is subtracted from the raw delta so a wide-band feed doesn't trip on noise.
        let mut b = make_cb_bank();
        b.cb_reference_price = price(100).into();
        // Raw delta 6% but a 2% confidence band → effective 4% < 500 bps → passes.
        assert!(b
            .cb_price_gate(OraclePriceWithConfidence {
                price: price(106),
                confidence: I80F48::from_num(2),
                source_time: 0,
            })
            .is_ok());
        // Raw delta 8% with a 2% band → effective 6% >= 500 bps → trips.
        assert!(b
            .cb_price_gate(OraclePriceWithConfidence {
                price: price(108),
                confidence: I80F48::from_num(2),
                source_time: 0,
            })
            .is_err());
    }

    // ---- Pre-break state restore -------------------------------------------------------------

    #[test]
    fn cb_effective_operational_state_resolves_pre_break() {
        // For a non-`CircuitBroken` bank the effective state is just `operational_state`.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::ReduceOnly;
        assert_eq!(
            b.cb_effective_operational_state(),
            BankOperationalState::ReduceOnly
        );

        // For a `CircuitBroken` bank it resolves to the stashed pre-break state.
        b.config.operational_state = BankOperationalState::CircuitBroken;
        b.cb_pre_break_state = BankOperationalState::ReduceOnly as u8;
        assert_eq!(
            b.cb_effective_operational_state(),
            BankOperationalState::ReduceOnly
        );
        b.cb_pre_break_state = BankOperationalState::Operational as u8;
        assert_eq!(
            b.cb_effective_operational_state(),
            BankOperationalState::Operational
        );
        b.cb_pre_break_state = BankOperationalState::Paused as u8;
        assert_eq!(
            b.cb_effective_operational_state(),
            BankOperationalState::Paused
        );
        b.cb_pre_break_state = BankOperationalState::KilledByBankruptcy as u8;
        assert_eq!(
            b.cb_effective_operational_state(),
            BankOperationalState::KilledByBankruptcy
        );
    }

    #[test]
    fn reset_cb_runtime_state_restores_pre_break_state() {
        // After a storm forces a bank to `CircuitBroken`, `reset_cb_runtime_state` (used by
        // `clear_circuit_breaker`) must restore the operational state it held beforehand and
        // zero every runtime field.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::ReduceOnly;
        trip_tier3_storm(&mut b);
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::CircuitBroken
        );

        b.reset_cb_runtime_state();
        // Operational state restored to the pre-break value.
        assert_eq!(b.config.operational_state, BankOperationalState::ReduceOnly);
        // All runtime fields zeroed.
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_halt_started_at, 0);
        assert_eq!(b.cb_halt_ended_at, 0);
        assert_eq!(b.cb_tier3_consecutive_trips, 0);
        assert_eq!(b.cb_last_observed_slot, 0);
        assert_eq!(b.cb_last_oracle_source_time, 0);
    }

    #[test]
    fn reset_cb_runtime_state_leaves_non_broken_state_unchanged() {
        // For a bank that never entered `CircuitBroken`, `reset_cb_runtime_state` only clears
        // runtime fields — the operational state is left as-is.
        let mut b = make_cb_bank();
        b.config.operational_state = BankOperationalState::Operational;
        b.update_circuit_breaker(1_000, 1_000, obs(price(100)))
            .unwrap();
        b.update_circuit_breaker(1_001, 1_002, obs(price(110)))
            .unwrap();
        assert_eq!(b.cb_tier, 2);

        b.reset_cb_runtime_state();
        assert_eq!(
            b.config.operational_state,
            BankOperationalState::Operational
        );
        assert_eq!(b.cb_tier, 0);
        assert_eq!(b.cb_halt_ended_at, 0);
    }
}
