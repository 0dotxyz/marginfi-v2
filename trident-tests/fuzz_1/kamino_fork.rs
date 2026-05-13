//! Kamino × marginfi fuzz: forked on-chain accounts and helpers.
//!
//! ## What this integration is
//!
//! Marginfi can add a **Kamino-type bank** (`lending_pool_add_bank_kamino`): the bank’s asset is a
//! position in a **Kamino reserve** (receipt = obligation + collateral shares), not raw tokens in
//! the marginfi liquidity vault. Users then:
//!
//! 1. **`kamino_init_obligation`** (permissionless) — creates the Kamino obligation + seeds it.
//! 2. **`kamino_deposit`** — user SPL → marginfi → CPI into Kamino `deposit_reserve_liquidity…`;
//!    marginfi credits **asset shares** on that bank.
//! 3. **`kamino_withdraw`** — CPI Kamino withdraw; marginfi debits shares.
//!
//! The **bank is initialized only** by `lending_pool_add_bank_kamino` (admin). There is no separate
//! “init bank” instruction after that; oracle keys are embedded in `KaminoConfigCompact` and checked
//! via `remaining_accounts` on that same instruction.
//!
//! ## Setup
//!
//! - Add `[[fuzz.programs]]` for Kamino Lending + Farms, and fork **`KAMINO_RESERVE`** (see `Trident.toml` notes).
//! - Set **`KAMINO_LENDING_MARKET`** to the reserve’s lending market (must match
//!   `MinimalReserve.lending_market` on **`KAMINO_RESERVE`**). Obligation + user-metadata + LMA PDAs
//!   are derived from that constant and the bank liquidity-vault authority (same seeds as marginfi).
//! - The harness reads **`MinimalReserve`** from **`KAMINO_RESERVE`** for mint / vault / token program.
//! - Fund classic SPL ATAs for the reserve’s **liquidity mint** for `kamino_init_obligation` / deposits.

use trident_fuzz::fuzzing::*;

use crate::constants::USDC_PYTH_PUSH;
use crate::types::marginfi::BankOperationalState;
use crate::types::marginfi::KaminoConfigCompact;
use crate::types::marginfi::MinimalReserve;
use crate::types::marginfi::OracleSetup;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;
use fixed_macro::types::I80F48;

pub const FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

/// Trident-generated `Kamino*` instruction structs flatten Anchor `Option` accounts into fixed
/// `Pubkey` slots. Use this for unused oracle / farm slots when your forked reserve has no farms
/// and only Pyth refresh (matches TS `null` for those accounts). If a CPI fails, set real farm

/// Seed passed into `lending_pool_add_bank_kamino` (must be unique per group × mint).
pub const KAMINO_BANK_SEED: u64 = 9_001;

// --- Replace with your forked mainnet (or local bankrun) account ---

/// Kamino reserve account (`integration_acc_1` / `MinimalReserve`).  
/// Mint and vault ATAs are read from this account at init / runtime.
pub const KAMINO_RESERVE: Pubkey = pubkey!("11111111111111111111111111111111");

/// Lending market for this fork (must match `MinimalReserve.lending_market` on [`KAMINO_RESERVE`]).
pub const KAMINO_LENDING_MARKET: Pubkey = pubkey!("11111111111111111111111111111111");

// Pyth push account wired to this reserve in your fork (often same as marginfi USDC push).
pub const KAMINO_PYTH_ORACLE: Pubkey = USDC_PYTH_PUSH;




/// Fields needed for Kamino CPIs, taken from on-chain [`MinimalReserve`] (authoritative for forks).
#[derive(Clone, Copy, Debug)]
pub struct KaminoReserveLayout {
    pub liquidity_mint: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_collateral_supply_vault: Pubkey,
}

pub fn read_reserve_layout(trident: &mut Trident, reserve: Pubkey) -> KaminoReserveLayout {
    let r = trident
        .get_account_with_type::<MinimalReserve>(&reserve, None)
        .expect("Kamino reserve: fork KAMINO_RESERVE or deploy klend + reserve first");
    debug_assert_eq!(
        r.lending_market, KAMINO_LENDING_MARKET,
        "KAMINO_LENDING_MARKET must match forked reserve lending_market"
    );
    KaminoReserveLayout {
        liquidity_mint: r.mint_pubkey,
        liquidity_token_program: r.token_program,
        reserve_liquidity_supply: r.supply_vault,
        reserve_collateral_mint: r.collateral_mint_pubkey,
        reserve_collateral_supply_vault: r.collateral_supply_vault,
    }
}

/// Mirrors `defaultKaminoBankConfig` in `tests/utils/kamino-utils.ts`.


