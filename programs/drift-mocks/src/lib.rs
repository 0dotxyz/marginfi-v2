pub mod constants;
pub mod macros;
pub mod state;

use anchor_lang::prelude::*;

declare_id!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

// Declare the drift program for CPI
declare_program!(drift);

// Note: Drift also uses these external programs which we might need later:
// declare_program!(pyth_solana_receiver_sdk);
// declare_program!(switchboard_on_demand);

#[program]
pub mod drift_mocks {}

#[error_code]
pub enum DriftMocksError {
    #[msg("Math overflow in scaling calculation")]
    ScalingOverflow,
    #[msg("Invalid balance type")]
    InvalidBalanceType,
    #[msg("Market index out of bounds")]
    InvalidMarketIndex,
    #[msg("Position not found")]
    PositionNotFound,
    #[msg("Drift Maths Gone Wrong")]
    MathError,
    #[msg("Invalid position index")]
    InvalidPositionIndex,
    #[msg("Invalid position state")]
    InvalidPositionState,
    #[msg("No admin deposits found in positions 2-7")]
    NoAdminDeposit,
    #[msg("Too many active deposits - account bricked by admin operations")]
    TooManyActiveDeposits,
    #[msg("Missing required reward account parameters")]
    MissingRewardAccounts,
}
