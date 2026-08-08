use anchor_lang::err;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::constants::BANK_SAME_ASSET_EMODE_ELIGIBLE;
use marginfi_type_crate::types::{
    Bank, BankConfig, EmodeSettings, MarginfiGroup, RequirementType, EMODE_ON, EMODE_TAG_EMPTY,
};

use crate::state::bank::BankImpl;
use crate::{check, errors::MarginfiError, math_error, prelude::MarginfiResult};
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
/// Same-asset e-mode is disabled by default for new groups.
pub const DEFAULT_INIT_MAX_SAME_ASSET_EMODE_LEVERAGE: I80F48 = I80F48!(1);

/// Default maximum allowed same-asset leverage for group initialization (maintenance).
/// Same-asset e-mode is disabled by default for new groups.
pub const DEFAULT_MAINT_MAX_SAME_ASSET_EMODE_LEVERAGE: I80F48 = I80F48!(1);

pub trait EmodeSettingsImpl {
    fn validate_entries_with_liability_weights(
        &self,
        bank_config: &BankConfig,
        liquidator_fee: I80F48,
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

/// A liquidation clears only while the liquidatee's credit outweighs the seized collateral, leaving
/// `1 / leverage` for fees. The insurance cut gives way first, so this share is what must fit.
fn fee_fits_leverage(liquidator_fee: I80F48, leverage: I80F48) -> MarginfiResult<bool> {
    Ok(liquidator_fee
        .checked_mul(leverage)
        .ok_or_else(math_error!())?
        < I80F48::ONE)
}

/// Same-asset weights are synthesized from the group leverage rather than stored as entries, so an
/// eligible bank's fee is checked against that leverage here.
pub fn check_same_asset_fee(bank: &Bank, group: &MarginfiGroup) -> MarginfiResult {
    if !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE) {
        return Ok(());
    }
    let leverage: I80F48 = u32_to_basis(group.same_asset_emode_maint_leverage);
    if leverage <= I80F48::ONE {
        return Ok(());
    }
    let fee = bank.liquidator_fee();
    check!(
        fee_fits_leverage(fee, leverage)?,
        MarginfiError::MaxMaintLeverageExceeded,
        "same-asset: liquidator fee ({}) leaves no room at {} leverage",
        fee,
        leverage
    );
    Ok(())
}

/// Exclusive upper bound on a leverage that still encodes at or below `cap`, so a leverage sitting
/// exactly on the cap is admitted even though `basis_to_u32` decodes a hair below what it stores.
/// A cap of 0 is unset and bounds nothing.
fn leverage_ceiling(cap: u32) -> I80F48 {
    if cap == 0 {
        return I80F48::MAX;
    }
    u32_to_basis(cap) + u32_to_basis(1)
}

impl EmodeSettingsImpl for EmodeSettings {
    fn validate_entries_with_liability_weights(
        &self,
        bank_config: &BankConfig,
        liquidator_fee: I80F48,
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

        let init_ceiling: I80F48 = leverage_ceiling(emode_max_init_leverage);
        let maint_ceiling: I80F48 = leverage_ceiling(emode_max_maint_leverage);

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
                max_leverage_init < init_ceiling,
                MarginfiError::MaxInitLeverageExceeded,
                "emode entry tag {}: init leverage ({}) exceeds max allowed ({})",
                entry.collateral_bank_emode_tag,
                max_leverage_init,
                u32_to_basis(emode_max_init_leverage)
            );

            let max_leverage_maint = calculate_max_leverage(asset_maint_w, liab_maint_w)?;
            check!(
                fee_fits_leverage(liquidator_fee, max_leverage_maint)?,
                MarginfiError::MaxMaintLeverageExceeded,
                "emode entry tag {}: liquidator fee ({}) leaves no room at {} leverage",
                entry.collateral_bank_emode_tag,
                liquidator_fee,
                max_leverage_maint
            );
            check!(
                max_leverage_maint < maint_ceiling,
                MarginfiError::MaxMaintLeverageExceeded,
                "emode entry tag {}: maint leverage ({}) exceeds max allowed ({})",
                entry.collateral_bank_emode_tag,
                max_leverage_maint,
                u32_to_basis(emode_max_maint_leverage)
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
    use marginfi_type_crate::constants::DEFAULT_LIQUIDATION_FEE;
    use marginfi_type_crate::types::{basis_to_u32, BankConfig};
    use marginfi_type_crate::types::{
        reconcile_emode_configs, EmodeConfig, EmodeEntry, RequirementType, MAX_EMODE_ENTRIES,
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
                DEFAULT_LIQUIDATION_FEE,
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
            DEFAULT_LIQUIDATION_FEE,
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
            DEFAULT_LIQUIDATION_FEE,
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
            DEFAULT_LIQUIDATION_FEE,
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

        let reconciled = reconcile_emode_configs(vec![config1, config2], RequirementType::Initial);

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

        let reconciled = reconcile_emode_configs(vec![config1, config2], RequirementType::Initial);

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

        let reconciled =
            reconcile_emode_configs(vec![config1, config2, config3], RequirementType::Initial);

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq_with_tolerance!(
            reconciled.entries[0].asset_weight,
            I80F48::from_num(0.6),
            I80F48::from_num(1e-7)
        );

        let reconciled = reconcile_emode_configs(
            vec![config1, config2, config3],
            RequirementType::Maintenance,
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
            DEFAULT_LIQUIDATION_FEE,
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
            DEFAULT_LIQUIDATION_FEE,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(
            result.is_err(),
            "Should fail when asset_init_w >= liab_init_w"
        );
    }

    /// `basis_to_u32(100)` is exactly `u32::MAX`, so the top of the range must still bound.
    #[test]
    fn test_validate_emode_entry_against_the_maximum_cap() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();
        bank_config.liability_weight_init = I80F48::from_num(1.0).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();
        let cap = basis_to_u32(I80F48::from_num(100));

        // 0.999 against liab 1.0 is 1000x, past even a 100x cap
        settings.emode_config.entries[0] = create_entry(1, 0, 0.9, 0.999);
        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            I80F48!(0.0001),
            cap,
            cap,
        );
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::MaxMaintLeverageExceeded.into()
        );
    }

    /// An unset group cap bounds nothing, but the fee still has to fit the entry's own leverage.
    #[test]
    fn test_validate_emode_entries_with_unset_group_caps() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();
        bank_config.liability_weight_init = I80F48::from_num(1.0).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();

        // 10x maint against a 2.5% fee, no cap to clear
        settings.emode_config.entries[0] = create_entry(1, 0, 0.8, 0.9);
        assert!(settings
            .validate_entries_with_liability_weights(&bank_config, DEFAULT_LIQUIDATION_FEE, 0, 0)
            .is_ok());

        // 100x maint leaves 1% of room, under the 2.5% fee
        settings.emode_config.entries[0] = create_entry(1, 0, 0.98, 0.99);
        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            DEFAULT_LIQUIDATION_FEE,
            0,
            0,
        );
        assert_eq!(
            result.err().unwrap(),
            MarginfiError::MaxMaintLeverageExceeded.into()
        );
    }

    /// The encoded group cap must decode to at least the value stored, or an entry sitting exactly
    /// on the ceiling is rejected.
    #[test]
    fn test_validate_emode_entry_at_exactly_the_maint_cap() {
        use bytemuck::Zeroable;

        let mut settings = EmodeSettings::zeroed();
        let mut bank_config = BankConfig::zeroed();

        let emode_max_init_leverage = basis_to_u32(DEFAULT_INIT_MAX_EMODE_LEVERAGE);
        let emode_max_maint_leverage = basis_to_u32(DEFAULT_MAINT_MAX_EMODE_LEVERAGE);

        bank_config.liability_weight_init = I80F48::from_num(1.0).into();
        bank_config.liability_weight_maint = I80F48::from_num(1.0).into();

        // CW_init 0.9 => 10x, under the 15x cap. CW_maint 0.95 => exactly the 20x cap.
        settings.emode_config.entries[0] = create_entry(1, 0, 0.9, 0.95);

        let result = settings.validate_entries_with_liability_weights(
            &bank_config,
            DEFAULT_LIQUIDATION_FEE,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(result.is_ok(), "entry at exactly the cap should validate");
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
            DEFAULT_LIQUIDATION_FEE,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        );
        assert!(
            result.is_err(),
            "Should fail when leverage exceeds DEFAULT_MAINT_MAX_EMODE_LEVERAGE (20x)"
        );
    }
}
