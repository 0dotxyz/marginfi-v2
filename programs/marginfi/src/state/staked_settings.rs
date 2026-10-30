use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::CIRCUIT_BREAKER_ENABLED,
    types::{BankConfig, RiskTier, StakedSettings},
};

use crate::{check, state::bank_config::BankConfigImpl, MarginfiError, MarginfiResult};

pub trait StakedSettingsImpl {
    fn validate(&self) -> MarginfiResult;
    fn apply_circuit_breaker_config(&self, config: &mut BankConfig) -> MarginfiResult;
}

impl StakedSettingsImpl for StakedSettings {
    /// Same checks as `bank.validate()` minus the liability and interest ones (staked banks use
    /// placeholders for those), plus `validate_circuit_breaker()` while the breaker is enabled.
    fn validate(&self) -> MarginfiResult {
        let asset_init_w = I80F48::from(self.asset_weight_init);
        let asset_maint_w = I80F48::from(self.asset_weight_maint);

        check!(
            asset_init_w >= I80F48::ZERO && asset_init_w <= I80F48::ONE,
            MarginfiError::InvalidConfig
        );
        check!(asset_maint_w >= asset_init_w, MarginfiError::InvalidConfig);
        check!(asset_maint_w <= I80F48::ONE, MarginfiError::InvalidConfig);
        if self.risk_tier == RiskTier::Isolated {
            check!(asset_init_w == I80F48::ZERO, MarginfiError::InvalidConfig);
            check!(asset_maint_w == I80F48::ZERO, MarginfiError::InvalidConfig);
        }

        if self.flags & CIRCUIT_BREAKER_ENABLED != 0 {
            self.apply_circuit_breaker_config(&mut BankConfig::default())?;
        }

        Ok(())
    }

    /// Copies the `cb_*` values onto a staked bank's config and validates them.
    fn apply_circuit_breaker_config(&self, config: &mut BankConfig) -> MarginfiResult {
        config.cb_deviation_bps_tiers = self.cb_deviation_bps_tiers;
        config.cb_tier_durations_seconds = self.cb_tier_durations_seconds;
        config.cb_escalation_window_mult = self.cb_escalation_window_mult;
        config.cb_ema_alpha_bps = self.cb_ema_alpha_bps;
        config.cb_window_seconds = self.cb_window_seconds;
        config.cb_window_max_up_bps = self.cb_window_max_up_bps;
        config.cb_window_max_down_bps = self.cb_window_max_down_bps;
        config.validate_circuit_breaker()
    }
}
