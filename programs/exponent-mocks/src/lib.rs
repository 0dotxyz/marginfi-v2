#![allow(
    clippy::diverging_sub_expression,
    clippy::too_many_arguments,
    unexpected_cfgs
)]

pub mod state;

use anchor_lang::prelude::*;

// Exponent Finance program ID (mainnet). Used as the expected owner when loading the PT vault
// account via `AccountLoader` (owner + discriminator verified).
declare_id!("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7");

#[program]
pub mod exponent_mocks {}

#[error_code]
pub enum ExponentMocksError {
    #[msg("Math error")]
    MathError,
}
