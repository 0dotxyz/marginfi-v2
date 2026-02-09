use bytemuck::{Pod, Zeroable};

use crate::{assert_struct_align, assert_struct_size};

use super::{WrappedI80F48, MAX_LENDING_ACCOUNT_BALANCES};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

pub const HEALTHY: u32 = 1;
pub const ENGINE_OK: u32 = 2;
pub const ORACLE_OK: u32 = 4;

assert_struct_size!(HealthCache, 304);
assert_struct_align!(HealthCache, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
/// A read-only cache of the internal risk engine's information. Only valid in borrow/withdraw if
/// the tx does not fail. To see the state in any context, e.g. to figure out if the risk engine is
/// failing due to some bad price information, use `pulse_health`.
pub struct HealthCache {
    /// Internal risk engine asset value, using initial weight (e.g. what is used for borrowing
    /// purposes), with all confidence adjustments, and other discounts on price.
    /// * Uses EMA price
    /// * In dollars
    pub asset_value: WrappedI80F48,
    /// Internal risk engine liability value, using initial weight (e.g. what is used for borrowing
    /// purposes), with all confidence adjustments, and other discounts on price.
    /// * Uses EMA price
    /// * In dollars
    pub liability_value: WrappedI80F48,
    /// Internal risk engine asset value, using maintenance weight (e.g. what is used for
    /// liquidation purposes), with all confidence adjustments.
    /// * Zero if the risk engine failed to load
    /// * Uses SPOT price
    /// * In dollars
    pub asset_value_maint: WrappedI80F48,
    /// Internal risk engine liability value, using maintenance weight (e.g. what is used for
    /// liquidation purposes), with all confidence adjustments.
    /// * Zero if the risk engine failed to load
    /// * Uses SPOT price
    /// * In dollars
    pub liability_value_maint: WrappedI80F48,
    /// The "true" value of assets without any confidence or weight adjustments. Internally, used
    /// only for bankruptcies.
    /// * Zero if the risk engine failed to load
    /// * Uses EMA price
    /// * In dollars
    pub asset_value_equity: WrappedI80F48,
    /// The "true" value of liabilities without any confidence or weight adjustments.
    /// Internally, used only for bankruptcies.
    /// * Zero if the risk engine failed to load
    /// * Uses EMA price
    /// * In dollars
    pub liability_value_equity: WrappedI80F48,
    /// Unix timestamp from the system clock when this cache was last updated
    pub timestamp: i64,
    /// The flags that indicate the state of the health cache. This is a u32 bitfield, where each
    /// bit represents a flag.
    ///
    /// * HEALTHY = 1 - If set, the account cannot be liquidated. If 0, the account is unhealthy and
    ///   can be liquidated.
    /// * ENGINE STATUS = 2 - If set, the engine did not error during the last health pulse. If 0,
    ///   the engine would have errored and this cache is likely invalid. `RiskEngineInitRejected`
    ///   is ignored and will allow the flag to be set anyways.
    /// * ORACLE OK = 4 - If set, the engine did not error due to an oracle issue. If 0, engine was
    ///   passed a bad bank or oracle account, or an oracle was stale. Check the order in which
    ///   accounts were passed and ensure each balance has the correct banks/oracles, and that
    ///   oracle cranks ran recently enough. Check `internal_err` and `err_index` for more details
    ///   in some circumstances. Invalid if generated after borrow/withdraw (these instructions will
    ///   ignore oracle issues if health is still satisfactory with some balance zeroed out).
    /// * 8, 16, 32, 64, 128, etc - reserved for future use
    pub flags: u32,
    /// If the engine errored, look here for the error code. If the engine returns ok, you may also
    /// check here to see if the risk engine rejected this tx (3009).
    pub mrgn_err: u32,
    /// Each price corresponds to that index of Balances in the LendingAccount. Useful for debugging
    /// or liquidator consumption, to determine how a user's position is priced internally.
    /// * An f64 stored as bytes
    pub prices: [[u8; 8]; MAX_LENDING_ACCOUNT_BALANCES],
    /// Errors in asset oracles are ignored (with prices treated as zero). If you see a zero price
    /// and the `ORACLE_OK` flag is not set, check here to see what error was ignored internally.
    pub internal_err: u32,
    /// Index in `balances` where `internal_err` appeared
    pub err_index: u8,
    /// Since 0.1.3, the version will be encoded here. See PROGRAM_VERSION.
    pub program_version: u8,
    pub pad0: [u8; 2],
    /// Error code from the liquidation health check during the last health pulse (0 if none)
    pub internal_liq_err: u32,
    /// Error code from the bankruptcy check during the last health pulse (0 if none)
    pub internal_bankruptcy_err: u32,
    // Note: the largest on-chain deployed cache was 304 bytes so all future caches must be at least
    // this big to avoid data corruption in the empty space.
    pub reserved0: [u8; 32],
    pub reserved1: [u8; 16],
}

impl HealthCache {
    /// True if account is healthy (cannot be liquidated)
    pub fn is_healthy(&self) -> bool {
        self.flags & HEALTHY != 0
    }

    pub fn set_healthy(&mut self, healthy: bool) {
        if healthy {
            self.flags |= HEALTHY;
        } else {
            self.flags &= !HEALTHY;
        }
    }

    /// True if the engine did not error during the last health pulse.
    pub fn is_engine_ok(&self) -> bool {
        self.flags & ENGINE_OK != 0
    }

    pub fn set_engine_ok(&mut self, ok: bool) {
        if ok {
            self.flags |= ENGINE_OK;
        } else {
            self.flags &= !ENGINE_OK;
        }
    }

    /// True if the engine did not detect an oracle input issue in the last health pulse
    pub fn is_oracle_ok(&self) -> bool {
        self.flags & ORACLE_OK != 0
    }

    pub fn set_oracle_ok(&mut self, ok: bool) {
        if ok {
            self.flags |= ORACLE_OK;
        } else {
            self.flags &= !ORACLE_OK;
        }
    }
}
