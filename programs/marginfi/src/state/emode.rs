use anchor_lang::err;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::types::{BankConfig, EmodeSettings, EMODE_ON, EMODE_TAG_EMPTY, RequirementType};

use crate::{
    check, errors::MarginfiError, math_error, prelude::MarginfiResult,
    state::bank_config::BankConfigImpl,
};
use marginfi_type_crate::types::u32_to_basis;

/// Default Maximum allowed theoretical leverage for emode configurations (initial).
/// L = 1 / (1 - CW/LW) where CW is collateral weight and LW is liability weight.
/// A value of 15 means positions can theoretically leverage up to 15x through recursive borrowing.
pub const DEFAULT_INIT_MAX_EMODE_LEVERAGE: I80F48 = I80F48!(15);

/// Default Maximum allowed theoretical leverage for emode configurations (maintenance).
/// L = 1 / (1 - CW/LW) where CW is collateral weight and LW is liability weight.
/// A value of 20 means positions can theoretically leverage up to 20x through recursive borrowing.
pub const DEFAULT_MAINT_MAX_EMODE_LEVERAGE: I80F48 = I80F48!(20);

/// Default maximum allowed same-asset leverage for group initialization (initial).
/// This is set to 2x the regular initial emode default leverage.
pub const DEFAULT_INIT_MAX_SAME_ASSET_EMODE_LEVERAGE: I80F48 = I80F48!(30);

/// Default maximum allowed same-asset leverage for group initialization (maintenance).
/// This is set to 2x the regular maintenance emode default leverage.
pub const DEFAULT_MAINT_MAX_SAME_ASSET_EMODE_LEVERAGE: I80F48 = I80F48!(40);

pub trait EmodeSettingsImpl {
    fn validate_entries_with_liability_weights(
        &self,
        bank_config: &BankConfig,
        emode_max_init_leverage: u32,
        emode_max_maint_leverage: u32,
    ) -> MarginfiResult;
    fn check_dupes(&self) -> MarginfiResult;
    fn is_enabled(&self) -> bool;
    fn update_emode_enabled(&mut self);
}

/// Calculate theoretical maximum leverage given collateral and liability weights.
/// Formula: L = 1 / (1 - CW/LW)
///
/// # Arguments
/// * `collateral_weight` - The collateral weight (CW)
/// * `liability_weight` - The liability weight (LW)
///
/// # Returns
/// * Ok(leverage) if calculation is valid (CW < LW)
/// * Err if leverage would be infinite or negative (CW >= LW)
#[inline]
pub fn calculate_max_leverage(
    collateral_weight: I80F48,
    liability_weight: I80F48,
) -> MarginfiResult<I80F48> {
    // Ensure liability weight is positive
    check!(
        liability_weight > I80F48::ZERO,
        MarginfiError::BadEmodeConfig
    );

    // Ensure collateral weight < liability weight (strictly less than)
    check!(
        collateral_weight < liability_weight,
        MarginfiError::BadEmodeConfig
    );

    //  ratio =  CW/LW
    let ratio: I80F48 = collateral_weight
        .checked_div(liability_weight)
        .ok_or_else(math_error!())?;

    // denominator = 1 - CW/LW
    let denominator: I80F48 = I80F48::ONE - ratio;

    check!(denominator > I80F48::ZERO, MarginfiError::BadEmodeConfig);

    //  leverage: 1 / (1 - CW/LW)
    let leverage: I80F48 = I80F48::ONE
        .checked_div(denominator)
        .ok_or_else(math_error!())?;

    Ok(leverage)
}

impl EmodeSettingsImpl for EmodeSettings {
    fn validate_entries_with_liability_weights(
        &self,
        bank_config: &BankConfig,
        emode_max_init_leverage: u32,
        emode_max_maint_leverage: u32,
    ) -> MarginfiResult {
        let liab_init_w: I80F48 = bank_config.get_weight(
            RequirementType::Initial,
            marginfi_type_crate::types::BalanceSide::Liabilities,
        );
        let liab_maint_w: I80F48 = bank_config.get_weight(
            RequirementType::Maintenance,
            marginfi_type_crate::types::BalanceSide::Liabilities,
        );

        let max_allowed_init_leverage: I80F48 = u32_to_basis(emode_max_init_leverage);
        let max_allowed_maint_leverage: I80F48 = u32_to_basis(emode_max_maint_leverage);

        for entry in self.emode_config.entries {
            if entry.is_empty() {
                continue;
            }
            let asset_init_w: I80F48 = I80F48::from(entry.asset_weight_init);
            let asset_maint_w: I80F48 = I80F48::from(entry.asset_weight_maint);

            // Basic sanity checks
            check!(
                asset_init_w >= I80F48::ZERO,
                MarginfiError::BadEmodeConfig,
                "emode entry tag {}: asset_init_w ({}) must be >= 0",
                entry.collateral_bank_emode_tag,
                asset_init_w
            );
            check!(
                asset_maint_w >= asset_init_w,
                MarginfiError::BadEmodeConfig,
                "emode entry tag {}: asset_maint_w ({}) must be >= asset_init_w ({})",
                entry.collateral_bank_emode_tag,
                asset_maint_w,
                asset_init_w
            );

            let max_leverage_init = calculate_max_leverage(asset_init_w, liab_init_w)?;
            check!(
                max_leverage_init <= max_allowed_init_leverage,
                MarginfiError::MaxInitLeverageExceeded,
                "emode entry tag {}: init leverage ({}) exceeds max allowed ({})",
                entry.collateral_bank_emode_tag,
                max_leverage_init,
                max_allowed_init_leverage
            );

            let max_leverage_maint = calculate_max_leverage(asset_maint_w, liab_maint_w)?;
            check!(
                max_leverage_maint <= max_allowed_maint_leverage,
                MarginfiError::MaxMaintLeverageExceeded,
                "emode entry tag {}: maint leverage ({}) exceeds max allowed ({})",
                entry.collateral_bank_emode_tag,
                max_leverage_maint,
                max_allowed_maint_leverage
            );
        }

        // Validate that no duplicates exist (other than EMODE_TAG_EMPTY - 0)
        self.check_dupes()?;

        Ok(())
    }

    /// Note: expects entries to be sorted. Empty-tag slots are skipped, and duplicate
    /// non-empty tags are detected with a single pass over the in-place array.
    fn check_dupes(&self) -> MarginfiResult {
        let mut prev_tag = EMODE_TAG_EMPTY;

        for entry in self
            .emode_config
            .entries
            .iter()
            .filter(|entry| !entry.is_empty())
        {
            if entry.collateral_bank_emode_tag == prev_tag {
                return err!(MarginfiError::BadEmodeConfig);
            }

            prev_tag = entry.collateral_bank_emode_tag;
        }

        Ok(())
    }

    /// True if an emode configuration has been set (EMODE_ON)
    fn is_enabled(&self) -> bool {
        self.flags & EMODE_ON != 0
    }

    /// Sets EMODE on flag if configuration has any entries, removes the flag if it has no entries.
    fn update_emode_enabled(&mut self) {
        if self.emode_config.has_entries() {
            self.flags |= EMODE_ON;
        } else {
            self.flags &= !EMODE_ON;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_eq_with_tolerance;
    use bytemuck::Zeroable;
    use fixed_macro::types::I80F48;
    use marginfi_type_crate::types::{basis_to_u32, BankConfig};
    use marginfi_type_crate::types::{
        reconcile_emode_configs, EmodeConfig, EmodeEntry, ReconciledEmodeRequirementType,
        MAX_EMODE_ENTRIES,
    };
    fn create_entry(tag: u16, flags: u8, init: f32, maint: f32) -> EmodeEntry {
        EmodeEntry {
            collateral_bank_emode_tag: tag,
            flags,
            pad0: [0u8; 5],
            asset_weight_init: I80F48::from_num(init).into(),
            asset_weight_maint: I80F48::from_num(maint).into(),
        }
    }

    /// "Standard" entry with flags=0, init=0.7, maint=0.8.
    fn generic_entry(tag: u16) -> EmodeEntry {
        create_entry(tag, 0, 0.7, 0.8)
    }

    #[test]
    fn test_emode_valid_entries() {
        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        settings.emode_config.entries[0] = generic_entry(1);
        settings.emode_config.entries[1] = generic_entry(2);
        settings.emode_config.entries[2] = generic_entry(3);
        // Note: The remaining entries stay zeroed (and are skipped during validation).
        assert!(settings
            .validate_entries_with_liability_weights(
                &bank_config,
                emode_max_init_leverage,
                emode_max_maint_leverage
            )
            .is_ok());
    }

    #[test]
    fn test_emode_invalid_duplicate_tags() {
        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        settings.emode_config.entries[0] = generic_entry(1);
        settings.emode_config.entries[1] = generic_entry(1); // Duplicate tag: 1.
        settings.emode_config.entries[2] = generic_entry(2);
        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), MarginfiError::BadEmodeConfig.into());
    }

    #[test]
    fn test_check_dupes_accepts_zero_entries_between_unique_tags() {
        let mut settings = EmodeSettings::zeroed();
        settings.emode_config.entries[0] = generic_entry(1);
        settings.emode_config.entries[2] = generic_entry(2);
        settings.emode_config.entries[5] = generic_entry(3);

        assert!(settings.check_dupes().is_ok());
    }

    #[test]
    fn test_check_dupes_rejects_duplicate_tags_separated_by_zero_entries() {
        let mut settings = EmodeSettings::zeroed();
        settings.emode_config.entries[0] = generic_entry(7);
        settings.emode_config.entries[3] = generic_entry(7);

        let result = settings.check_dupes();
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), MarginfiError::BadEmodeConfig.into());
    }

    #[test]
    fn test_emode_invalid_weight_too_high() {
        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        // Using asset weight greater than liability weight is invalid (CW >= LW).
        let entry = EmodeEntry {
            collateral_bank_emode_tag: 1,
            flags: 0,
            pad0: [0u8; 5],
            asset_weight_init: I80F48!(1.2).into(), // Equals liab_init_w (invalid!)
            asset_weight_maint: I80F48!(1.3).into(), // Exceeds liab_maint_w (invalid!)
        };
        settings.emode_config.entries[0] = entry;
        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), MarginfiError::BadEmodeConfig.into());
    }

    #[test]
    fn test_emode_invalid_weight_main_le_init() {
        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        let entry = EmodeEntry {
            collateral_bank_emode_tag: 1,
            flags: 0,
            pad0: [0u8; 5],
            asset_weight_init: I80F48!(0.8).into(),
            asset_weight_maint: I80F48!(0.7).into(),
        };
        settings.emode_config.entries[0] = entry;
        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), MarginfiError::BadEmodeConfig.into());
    }

    #[test]
    fn test_reconcile_emode_single_common_tag() {
        // Example 1:
        // * Config1 has an entry with tag 101, flags 1, init 0.7, maint 0.75.
        // * Config2 has an entry with tag 101, flags 0, init 0.6, maint 0.8.
        let entry1 = create_entry(101, 1, 0.7, 0.75);
        let entry2 = create_entry(101, 0, 0.6, 0.8);
        let config1 = EmodeConfig::from_entries(&[entry1]);
        let config2 = EmodeConfig::from_entries(&[entry2]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2],
            ReconciledEmodeRequirementType::Initial,
        );

        // Expected: For tag 101 - init, init = min(0.7,0.6)=0.6
        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq_with_tolerance!(
            reconciled.entries[0].asset_weight,
            I80F48::from_num(0.6),
            I80F48::from_num(1e-7)
        );
    }

    #[test]
    fn test_reconcile_emode_no_common_tags() {
        // Example 2:
        // * Config1 has an entry with tag 99.
        // * Config2 has an entry with tag 101.
        // * Since there is no common tag across both, the result should be an empty (zeroed) config.
        let config1 = EmodeConfig::from_entries(&[generic_entry(99)]);
        let config2 = EmodeConfig::from_entries(&[generic_entry(101)]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2],
            ReconciledEmodeRequirementType::Initial,
        );

        assert_eq!(reconciled.count, 0);
    }

    #[test]
    fn test_reconcile_emode_multiple_configs() {
        // Example 3:
        // * Config1 has entries with tags 101 and 99.
        // * Config2 has an entry with tag 101.
        // * Config3 has an entry with tag 101.
        // * Only tag 101 is common to all configs.
        // * For tag 101:
        //   - Config1: flags 1, init 0.7, maint 0.75.
        //   - Config2: flags 0, init 0.6, maint 0.8.
        //   - Config3: flags 0, init 0.65, maint 0.8.
        // * The reconciled entry should have:
        //   - flags = min(1, 0, 0) = 0,
        //   - init   = min(0.7, 0.6, 0.65) = 0.6,
        //   - maint  = min(0.75, 0.8, 0.8) = 0.75.
        let entry1 = create_entry(101, 1, 0.7, 0.75);
        let entry2 = create_entry(101, 0, 0.6, 0.8);
        let entry3 = create_entry(101, 0, 0.65, 0.8);

        let config1 = EmodeConfig::from_entries(&[entry1, generic_entry(99)]);
        let config2 = EmodeConfig::from_entries(&[entry2]);
        let config3 = EmodeConfig::from_entries(&[entry3]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2, config3],
            ReconciledEmodeRequirementType::Initial,
        );

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq_with_tolerance!(
            reconciled.entries[0].asset_weight,
            I80F48::from_num(0.6),
            I80F48::from_num(1e-7)
        );

        let reconciled = reconcile_emode_configs(
            vec![config1, config2, config3],
            ReconciledEmodeRequirementType::Maintenance,
        );

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq_with_tolerance!(
            reconciled.entries[0].asset_weight,
            I80F48::from_num(0.75),
            I80F48::from_num(1e-7)
        );
    }

    #[test]
    #[should_panic(expected = "Too many EmodeEntry items")]
    fn test_emode_from_entries_panics_on_too_many_entries() {
        // Generate more entries than allowed.
        let mut entries = Vec::new();
        for i in 0..(MAX_EMODE_ENTRIES as u16 + 1) {
            entries.push(generic_entry(i));
        }
        // This call should panic.
        let _ = EmodeConfig::from_entries(&entries);
    }

    #[test]
    fn test_calculate_max_leverage_valid() {
        // Test case: CW = 0.9, LW = 1.0
        // Expected leverage: 1 / (1 - 0.9/1.0) = 1 / 0.1 = 10x
        let cw = I80F48::from_num(0.9);
        let lw = I80F48::from_num(1.0);
        let leverage = calculate_max_leverage(cw, lw).unwrap();
        let expected = I80F48::from_num(10.0);
        assert!(
            (leverage - expected).abs() < I80F48::from_num(0.01),
            "Expected ~10x leverage, got {}",
            leverage
        );

        // Test case: CW = 0.95, LW = 1.0
        // Expected leverage: 1 / (1 - 0.95/1.0) = 1 / 0.05 = 20x
        let cw = I80F48::from_num(0.95);
        let lw = I80F48::from_num(1.0);
        let leverage = calculate_max_leverage(cw, lw).unwrap();
        let expected = I80F48::from_num(20.0);
        assert!(
            (leverage - expected).abs() < I80F48::from_num(0.01),
            "Expected ~20x leverage, got {}",
            leverage
        );

        // Test case: CW = 1.0, LW = 1.1
        // Expected leverage: 1 / (1 - 1.0/1.1) = 1 / 0.0909 = ~11x
        let cw = I80F48::from_num(1.0);
        let lw = I80F48::from_num(1.1);
        let leverage = calculate_max_leverage(cw, lw).unwrap();
        let expected = I80F48::from_num(11.0);
        assert!(
            (leverage - expected).abs() < I80F48::from_num(0.1),
            "Expected ~11x leverage, got {}",
            leverage
        );
    }

    #[test]
    fn test_calculate_max_leverage_invalid_cw_equals_lw() {
        // Test case: CW = LW = 1.0 (would result in infinite leverage)
        let cw = I80F48::from_num(1.0);
        let lw = I80F48::from_num(1.0);
        let result = calculate_max_leverage(cw, lw);
        assert!(result.is_err(), "Should fail when CW = LW");
    }

    #[test]
    fn test_calculate_max_leverage_invalid_cw_greater_than_lw() {
        // Test case: CW > LW (would result in negative leverage)
        let cw = I80F48::from_num(1.1);
        let lw = I80F48::from_num(1.0);
        let result = calculate_max_leverage(cw, lw);
        assert!(result.is_err(), "Should fail when CW > LW");
    }

    #[test]
    fn test_validate_emode_with_liability_weights_valid() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        // Set max emode leverage to default
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        // Set liability weights: init = 1.2, maint = 1.0
        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();

        // Set asset weights that result in safe leverage
        // CW_init = 0.84, LW_init = 1.2 => L = 1/(1-0.84/1.2) = 1/(1-0.7) = 3.33x
        // CW_maint = 0.9, LW_maint = 1.0 => L = 1/(1-0.9/1.0) = 1/0.1 = 10x
        settings.emode_config.entries[0] = create_entry(1, 0, 0.84, 0.9);

        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(result.is_ok(), "Valid emode config should pass validation");
    }

    #[test]
    fn test_validate_emode_with_liability_weights_invalid_cw_exceeds_lw() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        // Set max emode leverage to default
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        // Set liability weights
        bank_config.liability_weight_init = I80F48::from_num(1.2).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();

        // Set asset weights where init exceeds liability weight
        // CW_init = 1.3 > LW_init = 1.2 (invalid!)
        settings.emode_config.entries[0] = create_entry(1, 0, 1.3, 0.9);

        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(
            result.is_err(),
            "Should fail when asset_init_w >= liab_init_w"
        );
    }

    #[test]
    fn test_validate_emode_with_liability_weights_invalid_leverage_too_high() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        // Set max emode leverage to default
        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        // Set liability weights
        bank_config.liability_weight_init = I80F48::from_num(1.0).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();

        // Set asset weights that result in >20x leverage
        // CW = 0.96, LW = 1.0 => L = 1/(1-0.96/1.0) = 1/0.04 = 25x (exceeds MAX_EMODE_LEVERAGE)
        settings.emode_config.entries[0] = create_entry(1, 0, 0.96, 0.96);

        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(
            result.is_err(),
            "Should fail when leverage exceeds DEFAULT_MAINT_MAX_EMODE_LEVERAGE (20x)"
        );
    }
}
