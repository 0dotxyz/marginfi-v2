use anchor_lang::prelude::*;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::constants::{ASSET_TAG_KAMINO, PYTH_PUSH_MIGRATED_DEPRECATED};
use marginfi_type_crate::types::{
    make_points, BankConfig, BankOperationalState, InterestRateConfig, OracleSetup, RatePoint,
    RiskTier, WrappedI80F48, INTEREST_CURVE_SEVEN_POINT,
};

use crate::errors::MarginfiError;
use crate::prelude::MarginfiResult;

// Byte offsets of the configured oracle pubkeys within Kamino's `Reserve` account data
// (including the 8-byte Anchor discriminator). Derived from `idls-complete/kamino_lending.json`:
// Reserve.config (offset 4848) → ReserveConfig.token_info (offset +176) → field within TokenInfo.
const KAMINO_RESERVE_SWITCHBOARD_PRICE_OFFSET: usize = 4848 + 176 + 128 + 8;
const KAMINO_RESERVE_PYTH_PRICE_OFFSET: usize = 4848 + 176 + 192 + 8;

/// Reads the oracle pubkey that Kamino's reserve was configured with, for the given setup type.
/// Returns `KaminoInvalidOracleSetup` for non-Kamino setups, `KaminoReserveValidationFailed` if
/// the account is shorter than expected.
pub fn read_kamino_reserve_oracle(
    reserve_account: &AccountInfo,
    oracle_setup: OracleSetup,
) -> MarginfiResult<Pubkey> {
    let data = reserve_account.try_borrow_data()?;
    read_kamino_reserve_oracle_from_bytes(&data, oracle_setup)
}

fn read_kamino_reserve_oracle_from_bytes(
    data: &[u8],
    oracle_setup: OracleSetup,
) -> MarginfiResult<Pubkey> {
    let offset = match oracle_setup {
        OracleSetup::KaminoPythPush => KAMINO_RESERVE_PYTH_PRICE_OFFSET,
        OracleSetup::KaminoSwitchboardPull => KAMINO_RESERVE_SWITCHBOARD_PRICE_OFFSET,
        _ => return err!(MarginfiError::KaminoInvalidOracleSetup),
    };
    let bytes: [u8; 32] = data
        .get(offset..offset + 32)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(MarginfiError::KaminoReserveValidationFailed)?;
    Ok(Pubkey::new_from_array(bytes))
}

/// Used to configure Kamino banks. A simplified version of `BankConfigCompact` which omits most
/// values related to interest since Kamino banks cannot earn interest or be borrowed against.
// TODO: Jon mentioned there are some extra options he wants to see in config, investigate later.
#[derive(AnchorDeserialize, AnchorSerialize, Debug, PartialEq, Eq)]
pub struct KaminoConfigCompact {
    pub oracle: Pubkey,
    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,
    pub deposit_limit: u64,
    /// Either `KaminoPythPush` or `KaminoSwitchboardPull`
    pub oracle_setup: OracleSetup,
    /// Bank operational state - allows starting banks in paused state
    pub operational_state: BankOperationalState,
    /// Risk tier - determines if assets can be borrowed in isolation
    pub risk_tier: RiskTier,
    /// Config flags for future-proofing
    pub config_flags: u8,
    pub total_asset_value_init_limit: u64,
    /// Currently unused: Kamino's oracle age applies to kamino banks.
    pub oracle_max_age: u16,
    /// Oracle confidence threshold (0 = use default 10%)
    pub oracle_max_confidence: u32,
}

impl KaminoConfigCompact {
    pub const LEN: usize = std::mem::size_of::<KaminoConfigCompact>();

    pub fn new(
        oracle: Pubkey,
        asset_weight_init: WrappedI80F48,
        asset_weight_maint: WrappedI80F48,
        deposit_limit: u64,
        oracle_setup: OracleSetup,
        operational_state: BankOperationalState,
        risk_tier: RiskTier,
        config_flags: u8,
        total_asset_value_init_limit: u64,
        oracle_max_age: u16,
        oracle_max_confidence: u32,
    ) -> Self {
        KaminoConfigCompact {
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            oracle_setup,
            operational_state,
            risk_tier,
            config_flags,
            total_asset_value_init_limit,
            oracle_max_age,
            oracle_max_confidence,
        }
    }

    /// Convert to BankConfig with the reserve key for Kamino banks
    pub fn to_bank_config(&self, reserve_key: Pubkey) -> BankConfig {
        // These are placeholder values: Kamino positions do not support borrowing and likely
        // never will, thus they will earn no interest.
        // Note: Some placeholder values are non-zero to handle downstream validation checks.
        let default_ir_config = InterestRateConfig {
            placeholder0: I80F48::ZERO.into(),
            placeholder1: I80F48::ZERO.into(),
            placeholder2: I80F48::ZERO.into(),
            protocol_fixed_fee_apr: I80F48::ZERO.into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: 0,
            hundred_util_rate: 1234567,
            points: make_points(&[RatePoint::new(12345, 123456)]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let keys = [
            self.oracle,
            reserve_key,
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        ];

        BankConfig {
            asset_weight_init: self.asset_weight_init,
            asset_weight_maint: self.asset_weight_maint,
            liability_weight_init: I80F48!(1.5).into(), // placeholder
            liability_weight_maint: I80F48!(1.25).into(), // placeholder
            deposit_limit: self.deposit_limit,
            interest_rate_config: default_ir_config,
            operational_state: self.operational_state,
            oracle_setup: self.oracle_setup,
            oracle_keys: keys,
            _pad0: [0; 6],
            borrow_limit: 0, // Can't ever borrow kamino assets
            risk_tier: self.risk_tier,
            asset_tag: ASSET_TAG_KAMINO,
            config_flags: self.config_flags,
            _pad1: [0; 5],
            total_asset_value_init_limit: self.total_asset_value_init_limit,
            oracle_max_age: self.oracle_max_age,
            _padding0: [0; 2],
            oracle_max_confidence: self.oracle_max_confidence,
            fixed_price: I80F48::ZERO.into(),
            _padding1: [0; 16],
        }
    }
}

impl Default for KaminoConfigCompact {
    fn default() -> Self {
        KaminoConfigCompact {
            oracle: Pubkey::default(),
            asset_weight_init: I80F48!(0.8).into(),
            asset_weight_maint: I80F48!(0.9).into(),
            deposit_limit: 1_000_000,
            oracle_setup: OracleSetup::KaminoPythPush,
            operational_state: BankOperationalState::Operational,
            risk_tier: RiskTier::Collateral,
            config_flags: PYTH_PUSH_MIGRATED_DEPRECATED,
            total_asset_value_init_limit: 1_000_000,
            oracle_max_age: 10,
            oracle_max_confidence: 0, // Use default 10%
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kamino's `Reserve` account is 8 (discriminator) + 8616 (data) = 8624 bytes.
    const KAMINO_RESERVE_DATA_LEN: usize = 8624;

    fn reserve_buffer_with_oracle(offset: usize, oracle: Pubkey) -> Vec<u8> {
        let mut buf = vec![0u8; KAMINO_RESERVE_DATA_LEN];
        buf[offset..offset + 32].copy_from_slice(oracle.as_ref());
        buf
    }

    #[test]
    fn reads_pyth_oracle_from_kamino_reserve() {
        let oracle = Pubkey::new_unique();
        let buf = reserve_buffer_with_oracle(KAMINO_RESERVE_PYTH_PRICE_OFFSET, oracle);
        let got =
            read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::KaminoPythPush).unwrap();
        assert_eq!(got, oracle);
    }

    #[test]
    fn reads_switchboard_oracle_from_kamino_reserve() {
        let oracle = Pubkey::new_unique();
        let buf = reserve_buffer_with_oracle(KAMINO_RESERVE_SWITCHBOARD_PRICE_OFFSET, oracle);
        let got = read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::KaminoSwitchboardPull)
            .unwrap();
        assert_eq!(got, oracle);
    }

    #[test]
    fn rejects_non_kamino_oracle_setup() {
        let buf = vec![0u8; KAMINO_RESERVE_DATA_LEN];
        let err =
            read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::PythPushOracle).unwrap_err();
        assert_eq!(err, MarginfiError::KaminoInvalidOracleSetup.into());
    }

    #[test]
    fn rejects_short_account_data() {
        let buf = vec![0u8; KAMINO_RESERVE_PYTH_PRICE_OFFSET];
        let err =
            read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::KaminoPythPush).unwrap_err();
        assert_eq!(err, MarginfiError::KaminoReserveValidationFailed.into());
    }

    #[test]
    fn pyth_and_switchboard_offsets_are_distinct() {
        let pyth_oracle = Pubkey::new_unique();
        let switchboard_oracle = Pubkey::new_unique();
        let mut buf = vec![0u8; KAMINO_RESERVE_DATA_LEN];
        buf[KAMINO_RESERVE_PYTH_PRICE_OFFSET..KAMINO_RESERVE_PYTH_PRICE_OFFSET + 32]
            .copy_from_slice(pyth_oracle.as_ref());
        buf[KAMINO_RESERVE_SWITCHBOARD_PRICE_OFFSET..KAMINO_RESERVE_SWITCHBOARD_PRICE_OFFSET + 32]
            .copy_from_slice(switchboard_oracle.as_ref());

        assert_eq!(
            read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::KaminoPythPush).unwrap(),
            pyth_oracle
        );
        assert_eq!(
            read_kamino_reserve_oracle_from_bytes(&buf, OracleSetup::KaminoSwitchboardPull)
                .unwrap(),
            switchboard_oracle
        );
    }
}
