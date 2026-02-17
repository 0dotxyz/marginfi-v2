pub mod state;

use anchor_lang::prelude::*;

// Jupiter Lend (Fluid) Earn / fToken lending program ID (mainnet)
declare_id!("jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9");

// Declare the JupLend lending program for CPI.
//
// NOTE: This relies on the Anchor IDL located at `idls/juplend_earn.json` in the repo root.
declare_program!(juplend_earn);

#[program]
pub mod juplend_mocks {}

#[error_code]
pub enum JuplendMocksError {
    #[msg("Math error")]
    MathError,
}
