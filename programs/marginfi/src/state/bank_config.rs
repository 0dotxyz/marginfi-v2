use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{MAX_PYTH_ORACLE_AGE, ORACLE_MIN_AGE, TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE},
    types::{BalanceSide, BankConfig, OracleSetup, RiskTier},
};

use crate::{
    check,
    errors::MarginfiError,
    prelude::MarginfiResult,
    state::{
        interest_rate::InterestRateConfigImpl, marginfi_account::RequirementType,
        price::OraclePriceFeedAdapter,
    },
};

pub trait BankConfigImpl {
    fn get_weights(&self, req_type: RequirementType) -> (I80F48, I80F48);
    fn get_weight(&self, requirement_type: RequirementType, balance_side: BalanceSide) -> I80F48;
    fn validate(&self) -> MarginfiResult;
    fn is_deposit_limit_active(&self) -> bool;
    fn is_borrow_limit_active(&self) -> bool;
    fn update_config_flag(&mut self, value: bool, flag: u8);
    fn validate_oracle_setup(
        &self,
        ais: &[AccountInfo],
        lst_mint: Option<Pubkey>,
        stake_pool: Option<Pubkey>,
        sol_pool: Option<Pubkey>,
    ) -> MarginfiResult;
    fn usd_init_limit_active(&self) -> bool;
    fn get_oracle_max_age(&self) -> u64;
}

impl BankConfigImpl for BankConfig {
    #[inline]
    fn get_weights(&self, req_type: RequirementType) -> (I80F48, I80F48) {
        match req_type {
            RequirementType::Initial => (
                self.asset_weight_init.into(),
                self.liability_weight_init.into(),
            ),
            RequirementType::Maintenance => (
                self.asset_weight_maint.into(),
                self.liability_weight_maint.into(),
            ),
            RequirementType::Equity => (I80F48::ONE, I80F48::ONE),
        }
    }

    #[inline]
    fn get_weight(&self, requirement_type: RequirementType, balance_side: BalanceSide) -> I80F48 {
        match (requirement_type, balance_side) {
            (RequirementType::Initial, BalanceSide::Assets) => self.asset_weight_init.into(),
            (RequirementType::Initial, BalanceSide::Liabilities) => {
                self.liability_weight_init.into()
            }
            (RequirementType::Maintenance, BalanceSide::Assets) => self.asset_weight_maint.into(),
            (RequirementType::Maintenance, BalanceSide::Liabilities) => {
                self.liability_weight_maint.into()
            }
            (RequirementType::Equity, _) => I80F48::ONE,
        }
    }

    fn validate(&self) -> MarginfiResult {
        let asset_init_w = I80F48::from(self.asset_weight_init);
        let asset_maint_w = I80F48::from(self.asset_weight_maint);

        check!(
            asset_init_w >= I80F48::ZERO && asset_init_w <= I80F48::ONE,
            MarginfiError::InvalidConfig
        );
        check!(
            asset_maint_w <= (I80F48::ONE + I80F48::ONE),
            MarginfiError::InvalidConfig
        );
        check!(asset_maint_w >= asset_init_w, MarginfiError::InvalidConfig);

        let liab_init_w = I80F48::from(self.liability_weight_init);
        let liab_maint_w = I80F48::from(self.liability_weight_maint);

        check!(liab_init_w >= I80F48::ONE, MarginfiError::InvalidConfig);
        check!(
            liab_maint_w <= liab_init_w && liab_maint_w >= I80F48::ONE,
            MarginfiError::InvalidConfig
        );

        self.interest_rate_config.validate()?;

        if self.risk_tier == RiskTier::Isolated {
            check!(asset_init_w == I80F48::ZERO, MarginfiError::InvalidConfig);
            check!(asset_maint_w == I80F48::ZERO, MarginfiError::InvalidConfig);
        }

        check!(
            self.oracle_max_age >= ORACLE_MIN_AGE,
            MarginfiError::InvalidOracleSetup
        );

        Ok(())
    }

    #[inline]
    fn is_deposit_limit_active(&self) -> bool {
        self.deposit_limit != u64::MAX
    }

    #[inline]
    fn is_borrow_limit_active(&self) -> bool {
        self.borrow_limit != u64::MAX
    }

    fn update_config_flag(&mut self, value: bool, flag: u8) {
        if value {
            self.config_flags |= flag;
        } else {
            self.config_flags &= !flag;
        }
    }

    /// * lst_mint, stake_pool, sol_pool - required only if configuring
    ///   `OracleSetup::StakedWithPythPush` on initial setup. If configuring a staked bank after
    ///   initial setup, can be omitted
    fn validate_oracle_setup(
        &self,
        ais: &[AccountInfo],
        lst_mint: Option<Pubkey>,
        stake_pool: Option<Pubkey>,
        sol_pool: Option<Pubkey>,
    ) -> MarginfiResult {
        OraclePriceFeedAdapter::validate_bank_config(self, ais, lst_mint, stake_pool, sol_pool)?;
        Ok(())
    }

    fn usd_init_limit_active(&self) -> bool {
        self.total_asset_value_init_limit != TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE
    }

    #[inline]
    fn get_oracle_max_age(&self) -> u64 {
        match (self.oracle_max_age, self.oracle_setup) {
            (0, OracleSetup::PythPushOracle) => MAX_PYTH_ORACLE_AGE,
            (n, _) => n as u64,
        }
    }
}
