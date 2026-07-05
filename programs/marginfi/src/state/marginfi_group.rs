use crate::state::emode::{DEFAULT_INIT_MAX_EMODE_LEVERAGE, DEFAULT_MAINT_MAX_EMODE_LEVERAGE};
use crate::{prelude::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{basis_to_u32, MAX_PREMIUM_ENTRIES, PREMIUM_TAG_EMPTY};
use marginfi_type_crate::{constants::DAILY_RESET_INTERVAL, types::MarginfiGroup};
use std::fmt::Debug;

pub const PROGRAM_FEES_ENABLED: u64 = 1;

pub trait MarginfiGroupImpl {
    fn update_admin(&mut self, new_admin: Pubkey);
    fn update_emode_admin(&mut self, new_emode_admin: Pubkey);
    fn update_curve_admin(&mut self, new_curve_admin: Pubkey);
    fn update_limit_admin(&mut self, new_limit_admin: Pubkey);
    fn update_flow_admin(&mut self, new_flow_admin: Pubkey);
    /// DEPRECATED: updates stored emissions-admin metadata only; currently grants no authority.
    fn update_emissions_admin(&mut self, new_emissions_admin: Pubkey);
    fn update_metadata_admin(&mut self, new_metadata_admin: Pubkey);
    fn update_risk_admin(&mut self, new_risk_admin: Pubkey);
    fn set_initial_configuration(&mut self, admin_pk: Pubkey);
    fn get_group_bank_config(&self) -> GroupBankConfig;
    fn set_program_fee_enabled(&mut self, fee_enabled: bool);
    fn program_fees_enabled(&self) -> bool;
    fn is_admin_or_limit_admin(&self, signer: Pubkey) -> bool;
    fn add_bank(&mut self) -> MarginfiResult;
    fn is_protocol_paused(&self) -> bool;
    fn update_withdrawn_equity(
        &mut self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> MarginfiResult;
    fn check_deleverage_withdraw_limit(
        &self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> MarginfiResult;
    fn find_premium_rate(&self, collateral_tag: u16, liability_tag: u16) -> u32;
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

    fn update_flow_admin(&mut self, new_flow_admin: Pubkey) {
        if self.delegate_flow_admin == new_flow_admin {
            msg!("No change to flow admin: {:?}", new_flow_admin);
            // do nothing
        } else {
            msg!(
                "Set flow admin from {:?} to {:?}",
                self.delegate_flow_admin,
                new_flow_admin
            );
            self.delegate_flow_admin = new_flow_admin;
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
        self.delegate_flow_admin = admin_pk;
        self.set_program_fee_enabled(true);
        self.emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        self.emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);
        self.premium_settings.entry_capacity = MAX_PREMIUM_ENTRIES as u16;
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

    fn is_admin_or_limit_admin(&self, signer: Pubkey) -> bool {
        signer == self.admin || signer == self.delegate_limit_admin
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
        let projected =
            self.projected_deleverage_withdrawn_today(withdrawn_equity, current_timestamp);
        if current_timestamp.saturating_sub(
            self.deleverage_withdraw_window_cache
                .last_daily_reset_timestamp,
        ) >= DAILY_RESET_INTERVAL
        {
            self.deleverage_withdraw_window_cache.withdrawn_today = 0;
            self.deleverage_withdraw_window_cache
                .last_daily_reset_timestamp = current_timestamp;
        }
        self.deleverage_withdraw_window_cache.withdrawn_today = projected;

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

    fn check_deleverage_withdraw_limit(
        &self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> MarginfiResult {
        let projected =
            self.projected_deleverage_withdrawn_today(withdrawn_equity, current_timestamp);

        if self.deleverage_withdraw_window_cache.daily_limit != 0
            && projected > self.deleverage_withdraw_window_cache.daily_limit
        {
            msg!(
                "trying to withdraw more than daily limit: {} > {}",
                projected,
                self.deleverage_withdraw_window_cache.daily_limit
            );
            return err!(MarginfiError::DailyWithdrawalLimitExceeded);
        }

        Ok(())
    }

    /// Look up the variable-borrow premium rate (milli-u32 encoding) for a (collateral tag,
    /// liability tag) pair. Missing pairs and untagged (0) banks pay no premium.
    /// * The SOLE accessor for `premium_entries`: a future group-account resize only needs to
    ///   extend this lookup past the current struct bounds.
    fn find_premium_rate(&self, collateral_tag: u16, liability_tag: u16) -> u32 {
        if collateral_tag == PREMIUM_TAG_EMPTY || liability_tag == PREMIUM_TAG_EMPTY {
            return 0;
        }
        let n = (self.premium_settings.entry_count as usize).min(MAX_PREMIUM_ENTRIES);
        self.premium_entries[..n]
            .iter()
            .find(|e| e.collateral_tag == collateral_tag && e.liability_tag == liability_tag)
            .map(|e| e.rate)
            .unwrap_or(0)
    }
}

trait MarginfiGroupDeleverageLimitExt {
    fn projected_deleverage_withdrawn_today(
        &self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> u32;
}

impl MarginfiGroupDeleverageLimitExt for MarginfiGroup {
    fn projected_deleverage_withdrawn_today(
        &self,
        withdrawn_equity: I80F48,
        current_timestamp: i64,
    ) -> u32 {
        let withdrawn_today = if current_timestamp.saturating_sub(
            self.deleverage_withdraw_window_cache
                .last_daily_reset_timestamp,
        ) >= DAILY_RESET_INTERVAL
        {
            0
        } else {
            self.deleverage_withdraw_window_cache.withdrawn_today
        };

        withdrawn_today.saturating_add(withdrawn_equity.to_num())
    }
}

/// Group level configuration to be used in bank accounts.
#[derive(Clone, Debug)]
pub struct GroupBankConfig {
    pub program_fees: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    use marginfi_type_crate::types::{Balance, Bank, PremiumEntry, PremiumSettings};
    use std::mem::{offset_of, size_of};

    /// The premium matrix must occupy exactly the bytes that were `_padding_0` (32B) and
    /// `_padding_1` (512B, last field) before 0.1.10, so pre-existing groups read as an empty
    /// matrix (count 0, flags 0).
    #[test]
    fn group_premium_field_layout() {
        use std::mem::align_of;

        assert_eq!(size_of::<MarginfiGroup>(), 1056);
        assert_eq!(offset_of!(MarginfiGroup, premium_settings), 512);
        assert_eq!(offset_of!(MarginfiGroup, premium_entries), 544);

        // PremiumSettings internals: 8 + 2 + 2 + 4 + 16 = 32, 8-aligned, no implicit padding
        // (Pod derive would reject implicit padding at compile time; these pin the EXPLICIT
        // pad placement so a future field reorder trips a test, not mainnet).
        assert_eq!(size_of::<PremiumSettings>(), 32);
        assert_eq!(align_of::<PremiumSettings>(), 8);
        assert_eq!(offset_of!(PremiumSettings, timestamp), 0);
        assert_eq!(offset_of!(PremiumSettings, entry_count), 8);
        assert_eq!(offset_of!(PremiumSettings, entry_capacity), 10);
        assert_eq!(offset_of!(PremiumSettings, _pad0), 12);
        assert_eq!(offset_of!(PremiumSettings, _reserved0), 16);

        // PremiumEntry: 2 + 2 + 4 = 8, 4-aligned; the array of 64 fills exactly the old
        // `_padding_1` region (544 + 512 = 1056, the struct end).
        assert_eq!(size_of::<PremiumEntry>(), 8);
        assert_eq!(align_of::<PremiumEntry>(), 4);
        assert_eq!(offset_of!(PremiumEntry, collateral_tag), 0);
        assert_eq!(offset_of!(PremiumEntry, liability_tag), 2);
        assert_eq!(offset_of!(PremiumEntry, rate), 4);

        // Zeroed (= any pre-0.1.10 mainnet group) reads as matrix off.
        let group = MarginfiGroup::zeroed();
        assert_eq!(group.premium_settings.entry_count, 0);
        assert_eq!(group.find_premium_rate(100, 200), 0);
    }

    /// The premium fields must occupy exactly the first 24 bytes of what was
    /// `Bank._padding_1: [u64; 13]` before 0.1.10, so pre-existing banks read as untagged,
    /// uncapped, and with no collected premium.
    #[test]
    fn bank_premium_field_layout() {
        assert_eq!(size_of::<Bank>(), 1856);
        assert_eq!(offset_of!(Bank, bank_seed), 1744);
        assert_eq!(offset_of!(Bank, premium_tag), 1752);
        assert_eq!(offset_of!(Bank, _pad3), 1754);
        assert_eq!(offset_of!(Bank, collected_premium_outstanding), 1760);
        assert_eq!(offset_of!(Bank, premium_activated_at), 1776);
        assert_eq!(offset_of!(Bank, _padding_1), 1784);
    }

    /// The premium fields must occupy exactly the bytes that were `_pad0: [u8; 4]` and
    /// `emissions_outstanding` before 0.1.10 — both zeroed on-chain by the emissions wind-down
    /// migration, so pre-existing balances read as rate 0 / nothing outstanding. (As
    /// defense-in-depth the engine additionally honors `premium_outstanding` only on
    /// `PREMIUM_ACTIVE` banks.)
    #[test]
    fn balance_premium_field_layout() {
        assert_eq!(size_of::<Balance>(), 104);
        assert_eq!(offset_of!(Balance, premium_rate_snapshot), 36);
        assert_eq!(offset_of!(Balance, premium_outstanding), 72);
        assert_eq!(offset_of!(Balance, last_update), 88);
    }

    fn group_with_entries(entries: &[(u16, u16, u32)]) -> MarginfiGroup {
        let mut group = MarginfiGroup::zeroed();
        for (i, (c, l, r)) in entries.iter().enumerate() {
            group.premium_entries[i] = PremiumEntry {
                collateral_tag: *c,
                liability_tag: *l,
                rate: *r,
            };
        }
        group.premium_settings.entry_count = entries.len() as u16;
        group
    }

    #[test]
    fn find_premium_rate_hit_and_miss() {
        let group = group_with_entries(&[(100, 200, 7), (100, 300, 9), (150, 200, 11)]);
        assert_eq!(group.find_premium_rate(100, 200), 7);
        assert_eq!(group.find_premium_rate(100, 300), 9);
        assert_eq!(group.find_premium_rate(150, 200), 11);
        // Missing pair defaults to 0
        assert_eq!(group.find_premium_rate(150, 300), 0);
        assert_eq!(group.find_premium_rate(999, 999), 0);
    }

    #[test]
    fn find_premium_rate_tag_zero_never_matches() {
        // A pathological entry with tag 0 (rejected by config validation, but belt-and-braces)
        let group = group_with_entries(&[(0, 200, 7), (100, 0, 9)]);
        assert_eq!(group.find_premium_rate(0, 200), 0);
        assert_eq!(group.find_premium_rate(100, 0), 0);
        assert_eq!(group.find_premium_rate(0, 0), 0);
    }

    #[test]
    fn find_premium_rate_respects_count() {
        let mut group = group_with_entries(&[(100, 200, 7), (100, 300, 9)]);
        // Entries past entry_count are ignored
        group.premium_settings.entry_count = 1;
        assert_eq!(group.find_premium_rate(100, 300), 0);
        assert_eq!(group.find_premium_rate(100, 200), 7);
        // count 0 => matrix off => everything is 0
        group.premium_settings.entry_count = 0;
        assert_eq!(group.find_premium_rate(100, 200), 0);
    }
}
