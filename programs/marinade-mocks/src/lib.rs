#![allow(
    clippy::diverging_sub_expression,
    clippy::too_many_arguments,
    unexpected_cfgs
)]

pub mod state;

use anchor_lang::prelude::*;

// Marinade Finance liquid-staking program ID (mainnet). Used as the expected owner when
// loading the Marinade `State` account via `AccountLoader` (owner + discriminator verified).
declare_id!("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD");

#[program]
pub mod marinade_mocks {}

#[error_code]
pub enum MarinadeMocksError {
    #[msg("Math error")]
    MathError,
}
