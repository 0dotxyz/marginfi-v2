// Shared by the anchor and anchor-free encoding tests. Instruction-argument types carry two
// derives: `AnchorSerialize` (anchor's `anchor_derive_serde`) under `anchor`, and
// `borsh::BorshSerialize` under `ix_builders` alone. Those are different proc macros, so the
// encodings are compared here against one committed table rather than against each other.

use marginfi_type_crate::types::{
    BankOperationalState, EmodeEntry, KaminoConfigCompact, OracleSetup, OrderTrigger, Pubkey,
    RiskTier, StakedSettingsConfig, WrappedI80F48,
};

fn wi(seed: u8) -> WrappedI80F48 {
    let mut value = [0u8; 16];
    for (i, b) in value.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    WrappedI80F48 { value }
}

fn pk(seed: u8) -> Pubkey {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = seed.wrapping_mul(i as u8 + 1);
    }
    Pubkey::from(bytes)
}

pub fn sample_kamino_config() -> KaminoConfigCompact {
    KaminoConfigCompact {
        oracle: pk(3),
        asset_weight_init: wi(11),
        asset_weight_maint: wi(29),
        deposit_limit: 0x0102_0304_0506_0708,
        oracle_setup: OracleSetup::KaminoPythPush,
        operational_state: BankOperationalState::ReduceOnly,
        risk_tier: RiskTier::Isolated,
        config_flags: 0xA5,
        total_asset_value_init_limit: 0x1122_3344_5566_7788,
        oracle_max_age: 0xBEEF,
        oracle_max_confidence: 0xDEAD_BEEF,
    }
}

pub fn sample_staked_settings() -> StakedSettingsConfig {
    StakedSettingsConfig {
        oracle: pk(7),
        asset_weight_init: wi(41),
        asset_weight_maint: wi(59),
        deposit_limit: 0x1111_2222_3333_4444,
        total_asset_value_init_limit: 0x5555_6666_7777_8888,
        oracle_max_age: 0x0BAD,
        risk_tier: RiskTier::Collateral,
    }
}

pub fn sample_order_trigger() -> OrderTrigger {
    OrderTrigger::Both {
        stop_loss: wi(71),
        take_profit: wi(83),
        max_slippage: 0x0123_4567,
    }
}

pub fn sample_emode_entry() -> EmodeEntry {
    EmodeEntry {
        collateral_bank_emode_tag: 0xCAFE,
        flags: 0x5A,
        pad0: [1, 2, 3, 4, 5],
        asset_weight_init: wi(97),
        asset_weight_maint: wi(113),
    }
}

/// Borsh spec applied by hand: fields in declaration order, integers little-endian, a fieldless
/// enum as its `u8` index, a data enum as its `u8` index followed by the variant's fields.
struct Expected(Vec<u8>);

impl Expected {
    fn new() -> Self {
        Expected(Vec::new())
    }
    fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u64(mut self, v: u64) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn bytes(mut self, v: &[u8]) -> Self {
        self.0.extend_from_slice(v);
        self
    }
    fn i80f48(self, v: &WrappedI80F48) -> Self {
        self.bytes(&v.value)
    }
    fn pubkey(self, v: &Pubkey) -> Self {
        self.bytes(v.as_ref())
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

fn expected_kamino_config(c: &KaminoConfigCompact) -> Vec<u8> {
    Expected::new()
        .pubkey(&c.oracle)
        .i80f48(&c.asset_weight_init)
        .i80f48(&c.asset_weight_maint)
        .u64(c.deposit_limit)
        .u8(c.oracle_setup as u8)
        .u8(c.operational_state as u8)
        .u8(c.risk_tier as u8)
        .u8(c.config_flags)
        .u64(c.total_asset_value_init_limit)
        .u16(c.oracle_max_age)
        .u32(c.oracle_max_confidence)
        .done()
}

fn expected_staked_settings(c: &StakedSettingsConfig) -> Vec<u8> {
    Expected::new()
        .pubkey(&c.oracle)
        .i80f48(&c.asset_weight_init)
        .i80f48(&c.asset_weight_maint)
        .u64(c.deposit_limit)
        .u64(c.total_asset_value_init_limit)
        .u16(c.oracle_max_age)
        .u8(c.risk_tier as u8)
        .done()
}

fn expected_order_trigger(t: &OrderTrigger) -> Vec<u8> {
    match t {
        OrderTrigger::StopLoss {
            threshold,
            max_slippage,
        } => Expected::new()
            .u8(0)
            .i80f48(threshold)
            .u32(*max_slippage)
            .done(),
        OrderTrigger::TakeProfit {
            threshold,
            max_slippage,
        } => Expected::new()
            .u8(1)
            .i80f48(threshold)
            .u32(*max_slippage)
            .done(),
        OrderTrigger::Both {
            stop_loss,
            take_profit,
            max_slippage,
        } => Expected::new()
            .u8(2)
            .i80f48(stop_loss)
            .i80f48(take_profit)
            .u32(*max_slippage)
            .done(),
    }
}

fn expected_emode_entry(e: &EmodeEntry) -> Vec<u8> {
    Expected::new()
        .u16(e.collateral_bank_emode_tag)
        .u8(e.flags)
        .bytes(&e.pad0)
        .i80f48(&e.asset_weight_init)
        .i80f48(&e.asset_weight_maint)
        .done()
}

/// `(label, encoded by the derive, encoded by hand)` for each sample.
pub fn encodings() -> Vec<(&'static str, Vec<u8>, Vec<u8>)> {
    fn ser<T: borsh::BorshSerialize>(v: &T) -> Vec<u8> {
        borsh::to_vec(v).expect("borsh encode")
    }
    let kamino = sample_kamino_config();
    let staked = sample_staked_settings();
    let trigger = sample_order_trigger();
    let emode = sample_emode_entry();
    let wrapped = wi(200);
    vec![
        (
            "KaminoConfigCompact",
            ser(&kamino),
            expected_kamino_config(&kamino),
        ),
        (
            "StakedSettingsConfig",
            ser(&staked),
            expected_staked_settings(&staked),
        ),
        (
            "OrderTrigger",
            ser(&trigger),
            expected_order_trigger(&trigger),
        ),
        ("EmodeEntry", ser(&emode), expected_emode_entry(&emode)),
        (
            "WrappedI80F48",
            ser(&wrapped),
            Expected::new().i80f48(&wrapped).done(),
        ),
    ]
}
