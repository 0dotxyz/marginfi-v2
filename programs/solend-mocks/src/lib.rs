pub mod cpi;
pub mod macros;
pub mod state;

use anchor_lang::prelude::*;

declare_id!("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo");

#[program]
pub mod solend_mocks {}

#[error_code]
pub enum SolendMocksError {
    #[msg("Math error")]
    MathError,
    #[msg("Invalid account data")]
    InvalidAccountData,
    #[msg("Invalid obligation collateral")]
    InvalidObligationCollateral,
    #[msg("Invalid obligation liquidity")]
    InvalidObligationLiquidity,

    #[msg("Reserve lending market mismatch")]
    InvalidReserveLendingMarket,
    #[msg("Solend reserve is stale and must be refreshed")]
    ReserveStale,
    #[msg("Reserve configuration is invalid")]
    InvalidReserveConfig,
    #[msg("Reserve state is inconsistent")]
    InvalidReserveState,
}
