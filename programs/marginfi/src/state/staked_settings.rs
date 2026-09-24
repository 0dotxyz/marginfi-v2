use fixed::types::I80F48;
use marginfi_type_crate::types::{RiskTier, StakedSettings};

use crate::{check, MarginfiError, MarginfiResult};

pub trait StakedSettingsImpl {
    fn validate(&self) -> MarginfiResult;
}

impl StakedSettingsImpl for StakedSettings {
    /// Same as `bank.validate()`, except that liability rates and interest rates do not exist in
    /// this context (since Staked Collateral accounts cannot be borrowed against and such Banks
    /// will use placeholders for those values)
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

        Ok(())
    }
}
