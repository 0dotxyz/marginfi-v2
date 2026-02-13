use anchor_lang::prelude::*;

use library::structs::AddressBool;

#[event]
pub struct LogUserPosition {
    pub user: Pubkey,
    pub nft_id: u32,
    pub vault_id: u16,
    pub position_mint: Pubkey,
    pub tick: i32,
    pub col: u64,
    pub borrow: u64,
}

#[event]
pub struct LogOperate {
    pub signer: Pubkey,
    pub nft_id: u32,
    pub new_col: i128,
    pub new_debt: i128,
    pub to: Pubkey,
}

#[event]
pub struct LogClosePosition {
    pub signer: Pubkey,
    pub position_id: u32,
    pub vault_id: u16,
    pub position_mint: Pubkey,
}

#[event]
pub struct LogLiquidateInfo {
    pub vault_id: u16,
    pub start_tick: i32,
    pub end_tick: i32,
}

#[event]
pub struct LogLiquidate {
    pub signer: Pubkey,
    pub col_amount: u64,
    pub debt_amount: u64,
    pub to: Pubkey,
}

#[event]
pub struct LogAbsorb {
    pub col_amount: u64,
    pub debt_amount: u64,
}

// Add this to your events.rs file
#[event]
pub struct LogRebalance {
    pub supply_amt: i128,
    pub borrow_amt: i128,
}

// Event definitions
#[event]
pub struct LogUpdateAuths {
    pub auth_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateSupplyRateMagnifier {
    pub supply_rate_magnifier: i16,
}

#[event]
pub struct LogUpdateBorrowRateMagnifier {
    pub borrow_rate_magnifier: i16,
}

#[event]
pub struct LogUpdateCollateralFactor {
    pub collateral_factor: u16,
}

#[event]
pub struct LogUpdateLiquidationThreshold {
    pub liquidation_threshold: u16,
}

#[event]
pub struct LogUpdateLiquidationMaxLimit {
    pub liquidation_max_limit: u16,
}

#[event]
pub struct LogUpdateWithdrawGap {
    pub withdraw_gap: u16,
}

#[event]
pub struct LogUpdateLiquidationPenalty {
    pub liquidation_penalty: u16,
}

#[event]
pub struct LogUpdateBorrowFee {
    pub borrow_fee: u16,
}

#[event]
pub struct LogUpdateExchangePrices {
    pub vault_supply_exchange_price: u64,
    pub vault_borrow_exchange_price: u64,
    pub liquidity_supply_exchange_price: u64,
    pub liquidity_borrow_exchange_price: u64,
}

#[event]
pub struct LogUpdateCoreSettings {
    pub supply_rate_magnifier: i16,
    pub borrow_rate_magnifier: i16,
    pub collateral_factor: u16,
    pub liquidation_threshold: u16,
    pub liquidation_max_limit: u16,
    pub withdraw_gap: u16,
    pub liquidation_penalty: u16,
    pub borrow_fee: u16,
}

#[event]
pub struct LogUpdateOracle {
    pub new_oracle: Pubkey,
}

#[event]
pub struct LogUpdateRebalancer {
    pub new_rebalancer: Pubkey,
}

#[event]
pub struct LogInitVaultConfig {
    pub vault_config: Pubkey,
}

#[event]
pub struct LogInitVaultState {
    pub vault_state: Pubkey,
}

#[event]
pub struct LogInitBranch {
    pub branch: Pubkey,
    pub branch_id: u32,
}

#[event]
pub struct LogInitTickHasDebtArray {
    pub tick_has_debt_array: Pubkey,
}

#[event]
pub struct LogInitTick {
    pub tick: Pubkey,
}

#[event]
pub struct LogInitTickIdLiquidation {
    pub tick_id_liquidation: Pubkey,
    pub tick: i32,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}

#[event]
pub struct LogUpdateLookupTable {
    pub lookup_table: Pubkey,
}

#[event]
pub struct LogLiquidationRoundingDiff {
    pub vault_id: u16,
    pub actual_debt_amt: u64,
    pub debt_amount: u64,
    pub diff: u64,
}
