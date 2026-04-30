use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use crate::{assert_struct_align, assert_struct_size};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::WrappedI80F48;

pub const EMODE_ON: u64 = 1;

pub const MAX_EMODE_ENTRIES: usize = 10;
pub const EMODE_TAG_EMPTY: u16 = 0;

assert_struct_size!(EmodeSettings, 424);
assert_struct_align!(EmodeSettings, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// Controls the bank's e-mode configuration, allowing certain collateral sources to be treated more
/// favorably as collateral when used to borrow from this bank.
pub struct EmodeSettings {
    /// This bank's NON-unique id that other banks will use to determine what emode rate to use when
    /// this bank is offered as collateral.
    ///
    /// For example, all stablecoin banks might share the same emode_tag, and in their entries, each
    /// such stablecoin bank will recognize that collateral sources with this "stable" tag get
    /// preferential weights. When a new stablecoin is added that is considered riskier, it may get
    /// a new, less favorable emode tag, and eventually get upgraded to the same one as the other
    /// stables
    ///
    /// * 0 is in an invalid tag and will do nothing.
    pub emode_tag: u16,
    // To next 8-byte multiple
    pub pad0: [u8; 6],

    /// Unix timestamp from the system clock when emode state was last updated
    pub timestamp: i64,
    /// EMODE_ON (1) - If set, at least one entry is configured
    /// 2, 4, 8, etc, Reserved for future use
    pub flags: u64,

    pub emode_config: EmodeConfig,
}

assert_struct_size!(EmodeConfig, 400);
assert_struct_align!(EmodeConfig, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// An emode configuration. Each bank has one such configuration, but this may also be the
/// intersection of many configurations (see `reconcile_emode_configs`). For example, the risk
/// engine creates such an intersection from all the emode config of all banks the user is borrowing
/// from.
pub struct EmodeConfig {
    pub entries: [EmodeEntry; MAX_EMODE_ENTRIES],
}

impl EmodeConfig {
    /// Creates an EmodeConfig from a slice of EmodeEntry items.
    /// Entries will be sorted by tag.
    /// Panics if more than MAX_EMODE_ENTRIES are provided.
    /// * No heap allocation
    pub fn from_entries(entries: &[EmodeEntry]) -> Self {
        let count = entries.len();
        if count > MAX_EMODE_ENTRIES {
            panic!(
                "Too many EmodeEntry items {:?}, maximum allowed {:?}",
                count, MAX_EMODE_ENTRIES
            );
        }

        let mut config = Self::zeroed();
        for (i, entry) in entries.iter().enumerate() {
            config.entries[i] = *entry;
        }
        config.entries[..count].sort_by_key(|e| e.collateral_bank_emode_tag);

        config
    }

    pub fn find_with_tag(&self, tag: u16) -> Option<&EmodeEntry> {
        if tag == EMODE_TAG_EMPTY {
            return None;
        }
        self.entries.iter().find(|e| e.tag_equals(tag))
    }
    /// True if any entries are present in the mode configuration. Typically, this is the definition
    /// of flag `EMODE_ON`
    pub fn has_entries(&self) -> bool {
        self.entries.iter().any(|e| !e.is_empty())
    }
}

impl Default for EmodeSettings {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl EmodeSettings {
    /// True if an emode configuration has been set (EMODE_ON)
    pub fn is_enabled(&self) -> bool {
        self.flags & EMODE_ON != 0
    }
}

pub const APPLIES_TO_ISOLATED: u16 = 1;

assert_struct_size!(EmodeEntry, 40);
assert_struct_align!(EmodeEntry, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
pub struct EmodeEntry {
    /// emode_tag of the bank(s) whose collateral you wish to treat preferentially.
    pub collateral_bank_emode_tag: u16,
    /// * APPLIES_TO_ISOLATED (1) - (NOT YET IMPLEMENTED) if set, isolated banks with this tag
    ///   also benefit. If not set, isolated banks continue to offer zero collateral, even if they
    ///   use this tag.
    /// * 2, 4, 8, 16, 32, etc - reserved for future use
    pub flags: u8,
    // To next 8-byte multiple
    pub pad0: [u8; 5],
    /// Note: If set below the collateral bank's weight, does nothing.
    pub asset_weight_init: WrappedI80F48,
    /// Note: If set below the collateral bank's weight, does nothing.
    pub asset_weight_maint: WrappedI80F48,
}

impl EmodeEntry {
    pub fn is_empty(&self) -> bool {
        self.collateral_bank_emode_tag == EMODE_TAG_EMPTY
    }
    pub fn tag_equals(&self, tag: u16) -> bool {
        self.collateral_bank_emode_tag == tag
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReconciledEmodeRequirementType {
    Initial,
    Maintenance,
    Equity,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ReconciledEmodeEntry {
    pub collateral_bank_emode_tag: u16,
    pub flags: u8,
    pub asset_weight: I80F48,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ReconciledSameAssetConfig {
    pub mint: Pubkey,
    pub asset_weight: I80F48,
}

impl ReconciledSameAssetConfig {
    pub fn is_enabled(&self) -> bool {
        self.mint != Pubkey::default()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ReconciledEmodeConfig {
    pub entries: [ReconciledEmodeEntry; MAX_EMODE_ENTRIES],
    pub count: u8,
    pub same_asset: ReconciledSameAssetConfig,
}

impl Default for ReconciledEmodeConfig {
    fn default() -> Self {
        Self {
            entries: [ReconciledEmodeEntry::default(); MAX_EMODE_ENTRIES],
            count: 0,
            same_asset: ReconciledSameAssetConfig::default(),
        }
    }
}

impl ReconciledEmodeConfig {
    pub fn find_with_tag(&self, tag: u16) -> Option<&ReconciledEmodeEntry> {
        if tag == EMODE_TAG_EMPTY {
            return None;
        }

        self.entries[..self.count as usize]
            .iter()
            .find(|entry| entry.collateral_bank_emode_tag == tag)
    }
}

fn projected_asset_weight(
    entry: &EmodeEntry,
    requirement_type: ReconciledEmodeRequirementType,
) -> I80F48 {
    match requirement_type {
        ReconciledEmodeRequirementType::Initial => entry.asset_weight_init.into(),
        ReconciledEmodeRequirementType::Maintenance => entry.asset_weight_maint.into(),
        ReconciledEmodeRequirementType::Equity => I80F48::ONE,
    }
}

/// Users who borrow multiple e-mode assets at the same time get the LEAST FAVORABLE treatment
/// between the borrowed assets, regardless of the amount of each asset borrowed. For example, if
/// borrowing an LST and USDC against SOL, the user would normally get an emode benefit for LST/SOL,
/// but since they are also borrowing USDC, they get only standard rates.
///
/// Returns the INTERSECTION of liability-side classic emode configs, projected down to the single
/// asset weight relevant for the active requirement type.
///
/// If one config has a collateral_bank_emode_tag and the others do not, ***we don't make an
/// ReconciledEmodeEntry for it at all***, i.e. there is no benefit for that collateral
///
/// * Note: Takes a generic iterator as input to avoid heap allocating a Vec.
///
/// ***Example 1***
/// * bank | tag | flags | init | maint
/// * 0       101    1       70     75
/// * 1       101    0       60     80
/// - Result when reconciling `Initial`
/// * tag | projected asset weight
/// * 101    60
///
///
/// ***Example 2***
/// * bank  | tag | flags | init | maint
/// * 0       99     1       70     75
/// * 1       101    0       60     80
/// - Result
/// * tag | projected asset weight
/// * empty
///
///
/// ***Example 3***
/// * bank  | tag | flags | init | maint
/// * 0       101    1       70     75
/// * 1       101    0       60     80
/// * 2       101    0       60     80 (note this bank has multiple entries)
/// * 2       99     0       60     80
/// - Result when reconciling `Maintenance`
/// * tag | projected asset weight
/// * 101    75
pub fn reconcile_emode_configs<I>(
    configs: I,
    requirement_type: ReconciledEmodeRequirementType,
) -> ReconciledEmodeConfig
where
    I: IntoIterator<Item = EmodeConfig>,
{
    let mut iter = configs.into_iter();
    let first = match iter.next() {
        None => return ReconciledEmodeConfig::default(),
        Some(cfg) => cfg,
    };

    let mut merged_entries: BTreeMap<u16, (I80F48, u8, usize)> = BTreeMap::new();
    let mut num_configs = 1;

    for entry in first.entries.iter().filter(|entry| !entry.is_empty()) {
        merged_entries.insert(
            entry.collateral_bank_emode_tag,
            (
                projected_asset_weight(entry, requirement_type),
                entry.flags,
                1,
            ),
        );
    }

    for cfg in iter {
        num_configs += 1;
        for entry in cfg.entries.iter().filter(|entry| !entry.is_empty()) {
            let tag = entry.collateral_bank_emode_tag;
            // Ignore entries that don't exist on the first config, we won't be using them anyways.
            if let Some((merged_weight, merged_flags, cnt)) = merged_entries.get_mut(&tag) {
                // Once a tag misses any prior config, it can never make it back into the
                // intersection, so we can skip further weight work for it.
                if *cnt == num_configs - 1 {
                    let new_weight = projected_asset_weight(entry, requirement_type);
                    if new_weight < *merged_weight {
                        *merged_weight = new_weight;
                    }
                    if entry.flags < *merged_flags {
                        *merged_flags = entry.flags;
                    }
                    *cnt += 1;
                }
            }
        }
    }

    let mut reconciled = ReconciledEmodeConfig::default();
    let mut buf_len = 0usize;

    // `merged_entries` is a BTreeMap, so iteration already yields tags in sorted order.
    for (tag, (asset_weight, flags, cnt)) in merged_entries {
        if cnt == num_configs {
            reconciled.entries[buf_len] = ReconciledEmodeEntry {
                collateral_bank_emode_tag: tag,
                flags,
                asset_weight,
            };
            buf_len += 1;
        }
    }

    reconciled.count = buf_len as u8;
    reconciled
}

/// The same functionality as `reconcile_emode_configs`, but uses more heap space (which renders it
/// unusable on-chain). Perfectly fine for off-chain applications where heap space is not a concern.
pub fn reconcile_emode_configs_classic(configs: Vec<EmodeConfig>) -> EmodeConfig {
    // TODO benchmark this in the mock program
    // If no configs, return a zeroed config.
    if configs.is_empty() {
        return EmodeConfig::zeroed();
    }
    // If only one config, return it.
    if configs.len() == 1 {
        return configs.into_iter().next().unwrap();
    }

    let num_configs = configs.len();
    // Stores (tag, (entry, tag_count)), where tag_count is how many times we've seen this tag. This
    // BTreeMap is logically easier on the eyes, but is probably fairly CU expensive, and should be
    // benchmarked at some point, a simple Vec might actually be more performant here
    let mut merged_entries: BTreeMap<u16, (EmodeEntry, usize)> = BTreeMap::new();

    for config in &configs {
        for entry in config.entries.iter() {
            if entry.is_empty() {
                continue;
            }
            // Note: We assume that entries is de-duped and each tag appears at most one time!
            let tag = entry.collateral_bank_emode_tag;
            // Insert or merge the entry: if an entry with the same tag already exists, take the
            // lesser of each field, increment how many times we've seen this tag
            merged_entries
                .entry(tag)
                .and_modify(|(merged, tag_count)| {
                    // Note: More complex flag merging logic may be needed in the future
                    merged.flags = merged.flags.min(entry.flags);
                    let current_init: I80F48 = merged.asset_weight_init.into();
                    let new_init: I80F48 = entry.asset_weight_init.into();
                    if new_init < current_init {
                        merged.asset_weight_init = entry.asset_weight_init;
                    }
                    let current_maint: I80F48 = merged.asset_weight_maint.into();
                    let new_maint: I80F48 = entry.asset_weight_maint.into();
                    if new_maint < current_maint {
                        merged.asset_weight_maint = entry.asset_weight_maint;
                    }
                    *tag_count += 1;
                })
                .or_insert((*entry, 1));
        }
    }

    // Collect only the tags that appear in EVERY config.
    let final_entries: Vec<EmodeEntry> = merged_entries
        .into_iter()
        .filter(|(_, (_, tag_count))| *tag_count == num_configs)
        .map(|(_, (merged_entry, _))| merged_entry)
        .collect();

    // Sort the entries by tag and build a config from them
    EmodeConfig::from_entries(&final_entries)
}

/// Compute the same-asset emode asset weight given a decoded leverage value and liability weight.
///
/// Formula: w_asset = w_liab * (1 - 1/L)
///
/// For example, with leverage=100 and liability_weight=1.0:
///   w_asset = 1.0 * (1 - 1/100) = 0.99
///
/// Returns `I80F48::ZERO` if leverage is less than or equal to 1.
pub fn compute_same_asset_emode_weight(leverage: I80F48, liability_weight: I80F48) -> I80F48 {
    if leverage <= I80F48::ONE {
        return I80F48::ZERO;
    }
    let inverse = I80F48::ONE.checked_div(leverage).unwrap_or(I80F48::ZERO);
    let factor = I80F48::ONE - inverse;
    liability_weight.checked_mul(factor).unwrap_or(I80F48::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixed::types::I80F48;

    // -----------------------------------------------------------------------
    // compute_same_asset_emode_weight
    // -----------------------------------------------------------------------

    #[test]
    fn same_asset_weight_leverage_100_liab_1() {
        // w = 1.0 * (1 - 1/100) = 0.99
        let w = compute_same_asset_emode_weight(I80F48::from_num(100), I80F48::ONE);
        let expected = I80F48::from_num(0.99);
        let tolerance = I80F48::from_num(1e-9);
        let diff = (w - expected).abs();
        assert!(diff <= tolerance, "diff {} > tolerance {}", diff, tolerance);
    }

    #[test]
    fn same_asset_weight_leverage_2_liab_1() {
        // w = 1.0 * (1 - 1/2) = 0.5
        let w = compute_same_asset_emode_weight(I80F48::from_num(2), I80F48::ONE);
        let expected = I80F48::from_num(0.5);
        let tolerance = I80F48::from_num(1e-9);
        let diff = (w - expected).abs();
        assert!(diff <= tolerance, "diff {} > tolerance {}", diff, tolerance);
    }

    #[test]
    fn same_asset_weight_leverage_10_liab_1() {
        // w = 1.0 * (1 - 1/10) = 0.9
        let w = compute_same_asset_emode_weight(I80F48::from_num(10), I80F48::ONE);
        let expected = I80F48::from_num(0.9);
        let tolerance = I80F48::from_num(1e-9);
        let diff = (w - expected).abs();
        assert!(diff <= tolerance, "diff {} > tolerance {}", diff, tolerance);
    }

    #[test]
    fn same_asset_weight_with_liability_weight_greater_than_1() {
        // w = 1.05 * (1 - 1/100) = 1.05 * 0.99 = 1.0395
        let liab_w = I80F48::from_num(1.05);
        let w = compute_same_asset_emode_weight(I80F48::from_num(100), liab_w);
        let expected = I80F48::from_num(1.0395);
        let tolerance = I80F48::from_num(1e-9);
        let diff = (w - expected).abs();
        assert!(diff <= tolerance, "diff {} > tolerance {}", diff, tolerance);
    }

    #[test]
    fn same_asset_weight_fractional_leverage_changes_weight() {
        let w = compute_same_asset_emode_weight(I80F48::from_num(1.5), I80F48::ONE);
        let expected = I80F48::from_num(1.0 / 3.0);
        let tolerance = I80F48::from_num(1e-9);
        let diff = (w - expected).abs();
        assert!(diff <= tolerance, "diff {} > tolerance {}", diff, tolerance);
    }

    #[test]
    fn same_asset_weight_leverage_0_returns_zero() {
        // Runtime may still encounter old/default zero values even though new config writes exclude it.
        let w = compute_same_asset_emode_weight(I80F48::ZERO, I80F48::ONE);
        assert_eq!(w, I80F48::ZERO);
    }

    #[test]
    fn same_asset_weight_leverage_1_returns_zero() {
        // Leverage 1 is the effective no-op floor (1 - 1/1 = 0).
        let w = compute_same_asset_emode_weight(I80F48::ONE, I80F48::ONE);
        assert_eq!(w, I80F48::ZERO);
    }

    #[test]
    fn same_asset_weight_never_reaches_1_for_finite_leverage() {
        // Even at the configured max leverage, weight should remain strictly < liab_weight.
        let w = compute_same_asset_emode_weight(I80F48::from_num(100), I80F48::ONE);
        assert!(
            w < I80F48::ONE,
            "same-asset weight should never equal or exceed the liability weight"
        );
    }

    #[test]
    fn same_asset_weight_monotonically_increases_with_leverage() {
        let w2 = compute_same_asset_emode_weight(I80F48::from_num(2), I80F48::ONE);
        let w10 = compute_same_asset_emode_weight(I80F48::from_num(10), I80F48::ONE);
        let w100 = compute_same_asset_emode_weight(I80F48::from_num(100), I80F48::ONE);
        assert!(w2 < w10, "2x should be less than 10x");
        assert!(w10 < w100, "10x should be less than 100x");
    }

    #[test]
    fn reconciled_emode_configs_intersects_and_projects_initial_weights() {
        let config1 = EmodeConfig::from_entries(&[
            EmodeEntry {
                collateral_bank_emode_tag: 101,
                flags: 1,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.7).into(),
                asset_weight_maint: I80F48::from_num(0.8).into(),
            },
            EmodeEntry {
                collateral_bank_emode_tag: 303,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.6).into(),
                asset_weight_maint: I80F48::from_num(0.7).into(),
            },
        ]);
        let config2 = EmodeConfig::from_entries(&[
            EmodeEntry {
                collateral_bank_emode_tag: 101,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.65).into(),
                asset_weight_maint: I80F48::from_num(0.75).into(),
            },
            EmodeEntry {
                collateral_bank_emode_tag: 202,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.55).into(),
                asset_weight_maint: I80F48::from_num(0.65).into(),
            },
        ]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2],
            ReconciledEmodeRequirementType::Initial,
        );

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq!(reconciled.entries[0].asset_weight, I80F48::from_num(0.65));
        assert!(!reconciled.same_asset.is_enabled());
    }

    #[test]
    fn reconciled_emode_configs_merges_flags_with_least_favorable_rule() {
        let config1 = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 101,
            flags: 1,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.7).into(),
            asset_weight_maint: I80F48::from_num(0.8).into(),
        }]);
        let config2 = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 101,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.65).into(),
            asset_weight_maint: I80F48::from_num(0.75).into(),
        }]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2],
            ReconciledEmodeRequirementType::Initial,
        );

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].flags, 0);
    }

    #[test]
    fn reconciled_emode_configs_keep_sorted_order_without_post_sort() {
        let config1 = EmodeConfig::from_entries(&[
            EmodeEntry {
                collateral_bank_emode_tag: 202,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.7).into(),
                asset_weight_maint: I80F48::from_num(0.8).into(),
            },
            EmodeEntry {
                collateral_bank_emode_tag: 101,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.6).into(),
                asset_weight_maint: I80F48::from_num(0.7).into(),
            },
        ]);
        let config2 = EmodeConfig::from_entries(&[
            EmodeEntry {
                collateral_bank_emode_tag: 101,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.5).into(),
                asset_weight_maint: I80F48::from_num(0.65).into(),
            },
            EmodeEntry {
                collateral_bank_emode_tag: 202,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48::from_num(0.55).into(),
                asset_weight_maint: I80F48::from_num(0.75).into(),
            },
        ]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2],
            ReconciledEmodeRequirementType::Maintenance,
        );

        assert_eq!(reconciled.count, 2);
        assert_eq!(reconciled.entries[0].collateral_bank_emode_tag, 101);
        assert_eq!(reconciled.entries[1].collateral_bank_emode_tag, 202);
    }

    #[test]
    fn reconciled_emode_configs_project_equity_to_one() {
        let config = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 404,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.3).into(),
            asset_weight_maint: I80F48::from_num(0.4).into(),
        }]);

        let reconciled =
            reconcile_emode_configs(vec![config], ReconciledEmodeRequirementType::Equity);

        assert_eq!(reconciled.count, 1);
        assert_eq!(reconciled.entries[0].asset_weight, I80F48::ONE);
    }

    #[test]
    fn reconciled_emode_configs_do_not_reintroduce_tags_after_they_drop_out() {
        let config1 = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 101,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.7).into(),
            asset_weight_maint: I80F48::from_num(0.8).into(),
        }]);
        let config2 = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 202,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.6).into(),
            asset_weight_maint: I80F48::from_num(0.7).into(),
        }]);
        let config3 = EmodeConfig::from_entries(&[EmodeEntry {
            collateral_bank_emode_tag: 101,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::from_num(0.5).into(),
            asset_weight_maint: I80F48::from_num(0.6).into(),
        }]);

        let reconciled = reconcile_emode_configs(
            vec![config1, config2, config3],
            ReconciledEmodeRequirementType::Initial,
        );

        assert_eq!(reconciled.count, 0);
    }
}
