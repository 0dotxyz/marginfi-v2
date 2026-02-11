use crate::state::emode::{DEFAULT_INIT_MAX_EMODE_LEVERAGE, DEFAULT_MAINT_MAX_EMODE_LEVERAGE};
use crate::{prelude::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::basis_to_u32;
use marginfi_type_crate::{constants::DAILY_RESET_INTERVAL, types::MarginfiGroup};
use std::fmt::Debug;

pub const PROGRAM_FEES_ENABLED: u64 = 1;

pub trait MarginfiGroupImpl {
    fn update_admin(&mut self, new_admin: Pubkey);
    fn update_emode_admin(&mut self, new_emode_admin: Pubkey);
    fn update_curve_admin(&mut self, new_curve_admin: Pubkey);
    fn update_limit_admin(&mut self, new_limit_admin: Pubkey);
    fn update_emissions_admin(&mut self, new_emissions_admin: Pubkey);
    fn update_metadata_admin(&mut self, new_metadata_admin: Pubkey);
    fn update_risk_admin(&mut self, new_risk_admin: Pubkey);
    fn set_initial_configuration(&mut self, admin_pk: Pubkey);
    fn get_group_bank_config(&self) -> GroupBankConfig;
    fn set_program_fee_enabled(&mut self, fee_enabled: bool);
    fn program_fees_enabled(&self) -> bool;
    fn add_bank(&mut self) -> MarginfiResult;
    fn is_protocol_paused(&self) -> bool;
    fn update_withdrawn_equity(
        &mut self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> MarginfiResult;
}

impl MarginfiGroupImpl for MarginfiGroup {
    fn update_admin(&mut self, new_admin: Pubkey) {
        if self.admin == new_admin {
            msg!("No change to admin: {:?}", new_admin);
            // do nothing
        } else {
            msg!("Set admin from {:?} to {:?}", self.admin, new_admin);
            self.admin = new_admin;
        }
    }

    fn update_emode_admin(&mut self, new_emode_admin: Pubkey) {
        if self.emode_admin == new_emode_admin {
            msg!("No change to emode admin: {:?}", new_emode_admin);
            // do nothing
        } else {
            msg!(
                "Set emode admin from {:?} to {:?}",
                self.emode_admin,
                new_emode_admin
            );
            self.emode_admin = new_emode_admin;
        }
    }

    fn update_curve_admin(&mut self, new_curve_admin: Pubkey) {
        if self.delegate_curve_admin == new_curve_admin {
            msg!("No change to curve admin: {:?}", new_curve_admin);
            // do nothing
        } else {
            msg!(
                "Set curve admin from {:?} to {:?}",
                self.delegate_curve_admin,
                new_curve_admin
            );
            self.delegate_curve_admin = new_curve_admin;
        }
    }

    fn update_limit_admin(&mut self, new_limit_admin: Pubkey) {
        if self.delegate_limit_admin == new_limit_admin {
            msg!("No change to limit admin: {:?}", new_limit_admin);
            // do nothing
        } else {
            msg!(
                "Set limit admin from {:?} to {:?}",
                self.delegate_limit_admin,
                new_limit_admin
            );
            self.delegate_limit_admin = new_limit_admin;
        }
    }

    fn update_emissions_admin(&mut self, new_emissions_admin: Pubkey) {
        if self.delegate_emissions_admin == new_emissions_admin {
            msg!("No change to emissions admin: {:?}", new_emissions_admin);
            // do nothing
        } else {
            msg!(
                "Set emissions admin from {:?} to {:?}",
                self.delegate_emissions_admin,
                new_emissions_admin
            );
            self.delegate_emissions_admin = new_emissions_admin;
        }
    }

    fn update_metadata_admin(&mut self, new_meta_admin: Pubkey) {
        if self.metadata_admin == new_meta_admin {
            msg!("No change to meta admin: {:?}", new_meta_admin);
            // do nothing
        } else {
            msg!(
                "Set meta admin from {:?} to {:?}",
                self.metadata_admin,
                new_meta_admin
            );
            self.metadata_admin = new_meta_admin;
        }
    }
    fn update_risk_admin(&mut self, new_risk_admin: Pubkey) {
        if self.risk_admin == new_risk_admin {
            msg!("No change to risk admin: {:?}", new_risk_admin);
            // do nothing
        } else {
            msg!(
                "Set risk admin from {:?} to {:?}",
                self.risk_admin,
                new_risk_admin
            );
            self.risk_admin = new_risk_admin;
        }
    }

    /// Set the group parameters when initializing a group.
    /// This should be called only when the group is first initialized.
    #[allow(clippy::too_many_arguments)]
    fn set_initial_configuration(&mut self, admin_pk: Pubkey) {
        self.admin = admin_pk;
        self.set_program_fee_enabled(true);
        self.emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        self.emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);
    }

    fn get_group_bank_config(&self) -> GroupBankConfig {
        GroupBankConfig {
            program_fees: self.program_fees_enabled(),
        }
    }

    fn set_program_fee_enabled(&mut self, fee_enabled: bool) {
        if fee_enabled {
            self.group_flags |= PROGRAM_FEES_ENABLED;
        } else {
            self.group_flags &= !PROGRAM_FEES_ENABLED;
        }
    }

    /// True if program fees are enabled
    fn program_fees_enabled(&self) -> bool {
        (self.group_flags & PROGRAM_FEES_ENABLED) != 0
    }

    // Increment the bank count by 1. If you managed to create 16,000 banks, congrats, does
    // nothing.
    fn add_bank(&mut self) -> MarginfiResult {
        self.banks = self.banks.saturating_add(1);

        let clock = Clock::get()?;
        self.fee_state_cache.last_update = clock.unix_timestamp;

        Ok(())
    }

    /// Returns true if the protocol is in a paused state and the time has not yet expired, false it
    /// not paused or timer has expired.
    fn is_protocol_paused(&self) -> bool {
        // Note: In rare event clock fails to unwrap, time = 0 always fails the is_expired check.
        let current_timestamp = Clock::get().map(|c| c.unix_timestamp).unwrap_or(0);

        self.panic_state_cache.is_paused_flag()
            && !self.panic_state_cache.is_expired(current_timestamp)
    }

    fn update_withdrawn_equity(
        &mut self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> MarginfiResult {
        if current_timestamp.saturating_sub(
            self.deleverage_withdraw_window_cache
                .last_daily_reset_timestamp,
        ) >= DAILY_RESET_INTERVAL
        {
            self.deleverage_withdraw_window_cache.withdrawn_today = 0;
            self.deleverage_withdraw_window_cache
                .last_daily_reset_timestamp = current_timestamp;
        }
        self.deleverage_withdraw_window_cache.withdrawn_today = self
            .deleverage_withdraw_window_cache
            .withdrawn_today
            .saturating_add(withdrawn_equity.to_num());

        // Note: treat zero limit as "no limit" here for backwards compatibility.
        if self.deleverage_withdraw_window_cache.daily_limit != 0
            && self.deleverage_withdraw_window_cache.withdrawn_today
                > self.deleverage_withdraw_window_cache.daily_limit
        {
            msg!(
                "trying to withdraw more than daily limit: {} > {}",
                self.deleverage_withdraw_window_cache.withdrawn_today,
                self.deleverage_withdraw_window_cache.daily_limit
            );
            return err!(MarginfiError::DailyWithdrawalLimitExceeded);
        }
        Ok(())
    }
}

/// Group level configuration to be used in bank accounts.
#[derive(Clone, Debug)]
pub struct GroupBankConfig {
    pub program_fees: bool,
}
