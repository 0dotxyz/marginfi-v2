#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    zero_copy, *,
};

use super::{Balance, LendingAccount, WrappedI80F48, MAX_LENDING_ACCOUNT_BALANCES};
use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

// Records key information about the account during receivership liquidation. Records the result of
// the last few liquidation events on this account. Typically, the liquidator pays to create this
// account, and if a user doesn't have one, it means it has never been subject to receivership
// liquidation.
assert_struct_size!(LiquidationRecord, 512);
assert_struct_align!(LiquidationRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone)
)]
pub struct LiquidationRecord {
    /// This account's own key. A PDA derived from `marginfi_account`
    pub key: Pubkey,
    /// Account this record tracks
    pub marginfi_account: Pubkey,
    /// The key that paid to create this account. At some point, we may allow this wallet to reclaim
    /// the rent paid to open a record.
    pub record_payer: Pubkey,
    /// The liquidator taking receivership of the `marginfi_account` to complete a liquidation. Pays
    /// the liquidation fee.
    /// * Always pubkey default unless actively within a liquidation event.
    pub liquidation_receiver: Pubkey,
    /// Basic historical data for the last few liquidation events on this account
    pub entries: [LiquidationEntry; 4],
    pub cache: LiquidationCache,

    _reserved0: [u8; 64],
    _reserved2: [u8; 16],
    _reserved3: [u8; 8],
}

/// Used to record key details of the last few liquidation events on the account
#[repr(C)]
#[cfg_attr(
    feature = "anchor",
    zero_copy,
    derive(BorshDeserialize, BorshSerialize)
)]
#[cfg_attr(not(feature = "anchor"), derive(Clone, Copy, Pod, Zeroable))]
#[derive(Debug, Default, PartialEq)]
pub struct LiquidationEntry {
    /// Dollar amount seized
    /// * An f64 stored as bytes
    pub asset_amount_seized: [u8; 8],
    /// Dollar amount repaid
    /// * An f64 stored as bytes
    pub liab_amount_repaid: [u8; 8],
    pub placeholder0: u64,
    pub timestamp: i64,
    _reserved0: [u8; 16],
}

// Stores data used by the liquidator during the liquidation process
#[repr(C)]
#[cfg_attr(
    feature = "anchor",
    zero_copy,
    derive(BorshDeserialize, BorshSerialize)
)]
#[cfg_attr(not(feature = "anchor"), derive(Clone, Copy, Pod, Zeroable))]
#[derive(Debug, PartialEq, Default)]
pub struct LiquidationCache {
    /// Internal risk engine asset value snapshot taken when liquidation begins, using maintenance
    /// weight with all confidence adjustments.
    /// * Uses SPOT price
    /// * In dollars
    pub asset_value_maint: WrappedI80F48,
    /// Internal risk engine liability value snapshot taken when liquidation begins, using
    /// maintenance weight with all confidence adjustments.
    /// * Uses SPOT price
    /// * In dollars
    pub liability_value_maint: WrappedI80F48,
    /// Actual cash value of assets pre-liquidation (inclusive of price adjustment for oracle
    /// confidence, but without any weights)
    /// * Liquidator is allowed to seize up to `liability_value_equity` - this amount
    /// * Uses EMA price
    /// * In dollars
    pub asset_value_equity: WrappedI80F48,
    /// Actual cash value of liabilities pre-liquidation (inclusive of price adjustment for oracle
    /// confidence, but without any weights)
    /// * Liquidator is allowed to seize up to this amount - `asset_value_equity`
    /// * Uses EMA price
    /// * In dollars
    pub liability_value_equity: WrappedI80F48,
    pub _placeholder: u64,
    _reserved0: [u8; 32],
}

impl LiquidationRecord {
    pub const LEN: usize = std::mem::size_of::<LiquidationRecord>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::LIQUIDATION_RECORD;
}

impl Default for LiquidationRecord {
    fn default() -> Self {
        LiquidationRecord {
            key: Pubkey::default(),
            marginfi_account: Pubkey::default(),
            record_payer: Pubkey::default(),
            liquidation_receiver: Pubkey::default(),
            entries: [LiquidationEntry::default(); 4],
            cache: LiquidationCache::default(),
            _reserved0: [0u8; 64],
            _reserved2: [0u8; 16],
            _reserved3: [0u8; 8],
        }
    }
}

// **** Unused compact lending account/balance concepts *********

// Note: These might be useful to store in the Liquidation record's cache for some use-cases, but
// without a solid justification it just doesn't make sense to use the ~1200 bytes they consume when
// the same information is already available on the user account itself.

// A compact LendingAccount
#[repr(C)]
#[cfg_attr(
    feature = "anchor",
    zero_copy,
    derive(Default, BorshDeserialize, BorshSerialize)
)]
#[cfg_attr(not(feature = "anchor"), derive(Default, Clone, Copy, Pod, Zeroable))]
pub struct MiniLendingAccount {
    pub balances: [MiniLendingAccountBalance; MAX_LENDING_ACCOUNT_BALANCES],
}

impl MiniLendingAccount {
    /// Convert a `LendingAccount` into its “mini” form, copying each `Balance` slot in index order.
    pub fn from_lending_account(src: &LendingAccount) -> Self {
        let mut mini = MiniLendingAccount::default();
        for (i, bal) in src.balances.iter().enumerate() {
            mini.balances[i] = MiniLendingAccountBalance::from(bal);
        }
        mini
    }
}

// A compact account Balance
#[repr(C)]
#[cfg_attr(
    feature = "anchor",
    zero_copy,
    derive(Default, BorshDeserialize, BorshSerialize)
)]
#[cfg_attr(not(feature = "anchor"), derive(Default, Clone, Copy, Pod, Zeroable))]
pub struct MiniLendingAccountBalance {
    pub bank_pk: Pubkey,
    pub asset_shares: WrappedI80F48,
    pub liability_shares: WrappedI80F48,
    pub active: u8,
    pub bank_asset_tag: u8,
    _padding: [u8; 6],
}

impl From<&Balance> for MiniLendingAccountBalance {
    fn from(b: &Balance) -> Self {
        MiniLendingAccountBalance {
            asset_shares: b.asset_shares,
            liability_shares: b.liability_shares,
            bank_pk: b.bank_pk,
            active: b.active,
            bank_asset_tag: b.bank_asset_tag,
            _padding: [0; 6],
        }
    }
}
