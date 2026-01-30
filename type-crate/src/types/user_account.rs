use crate::{
    assert_struct_align, assert_struct_size,
    constants::{discriminators, ASSET_TAG_DEFAULT, EMPTY_BALANCE_THRESHOLD},
};
use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

use super::{HealthCache, WrappedI80F48};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

assert_struct_size!(MarginfiAccount, 2304);
assert_struct_align!(MarginfiAccount, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)
)]
pub struct MarginfiAccount {
    pub group: Pubkey,                   // 32
    pub authority: Pubkey,               // 32
    pub lending_account: LendingAccount, // 1728
    /// The flags that indicate the state of the account. This is u64 bitfield, where each bit
    /// represents a flag.
    ///
    /// Flags:MarginfiAccount
    /// - 1: `ACCOUNT_DISABLED` - Indicates that the account is disabled and no further actions can
    /// be taken on it.
    /// - 2: `ACCOUNT_IN_FLASHLOAN` - Only set when an account is within a flash loan, e.g. when
    ///   start_flashloan is called, then unset when the flashloan ends.
    /// - 4: `ACCOUNT_FLAG_DEPRECATED` - Deprecated, available for future use
    /// - 8: `ACCOUNT_TRANSFER_AUTHORITY_DEPRECATED` - the admin has flagged with account to be
    ///   moved, original owner can now call `set_account_transfer_authority`
    /// - 16: `ACCOUNT_IN_RECEIVERSHIP` - the account is eligible to be liquidated and has entered
    ///   receivership, a liquidator is able to control borrows and withdraws until the end of the
    ///   tx. This flag will only appear within a tx.
    /// - 32: `ACCOUNT_IN_DELEVERAGE - the account is being deleveraged by the risk admin
    /// - 64: `ACCOUNT_FROZEN` - the admin has frozen the account; only the group admin may perform
    ///   actions until unfrozen.
    pub account_flags: u64, // 8
    /// Set with `update_emissions_destination_account`. Emissions rewards can be withdrawn to the
    /// cannonical ATA of this wallet without the user's input (withdraw_emissions_permissionless).
    /// If pubkey default, the user has not opted into this feature, and must claim emissions
    /// manually (withdraw_emissions).
    pub emissions_destination_account: Pubkey, // 32
    pub health_cache: HealthCache,
    /// If this account was migrated from another one, store the original account key
    pub migrated_from: Pubkey, // 32
    /// If this account has been migrated to another one, store the destination account key
    pub migrated_to: Pubkey, // 32
    pub last_update: u64,
    /// If a PDA-based account, the account index, a seed used to derive the PDA that can be chosen
    /// arbitrarily (0.1.5 or later). Otherwise, does nothing.
    pub account_index: u16,
    /// If a PDA-based account (0.1.5 or later), a "vendor specific" id. Values < PDA_FREE_THRESHOLD
    /// can be used by anyone with no restrictions. Values >= PDA_FREE_THRESHOLD can only be used by
    /// a particular program via CPI. These values require being added to a list, contact us for
    /// more details. For legacy non-pda accounts, does nothing.
    ///
    /// Note: use a unique seed to tag accounts related to some particular program or campaign so
    /// you can easily fetch them all later.
    pub third_party_index: u16,
    /// This account's bump, if a PDA-based account (0.1.5 or later). Otherwise, does nothing.
    pub bump: u8,
    // For 8-byte alignment
    pub _pad0: [u8; 3],
    /// Stores information related to liquidations made against this account. A pda of this
    /// account's key, and "liq_record"
    /// * Typically pubkey default if this account has never been liquidated or close to liquidation
    /// * Opening this account is permissionless. Typically the liquidator pays, but e.g. we may
    ///   also charge the user if they are opening a risky position on the front end.
    pub liquidation_record: Pubkey,
    pub _padding0: [u64; 7],
}

impl MarginfiAccount {
    pub const LEN: usize = std::mem::size_of::<MarginfiAccount>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::ACCOUNT;

    /// Note: Only for accounts created by PDA
    #[cfg(feature = "anchor")]
    pub fn derive_pda(
        group: &Pubkey,
        authority: &Pubkey,
        account_index: u16,
        third_party_id: Option<u16>,
        program_id: &Pubkey,
    ) -> (Pubkey, u8) {
        use crate::constants::MARGINFI_ACCOUNT_SEED;
        Pubkey::find_program_address(
            &[
                MARGINFI_ACCOUNT_SEED.as_bytes(),
                group.as_ref(),
                authority.as_ref(),
                &account_index.to_le_bytes(),
                &third_party_id.unwrap_or(0).to_le_bytes(),
            ],
            program_id,
        )
    }
}

pub const ACCOUNT_DISABLED: u64 = 1 << 0;
pub const ACCOUNT_IN_FLASHLOAN: u64 = 1 << 1;
pub const ACCOUNT_FLAG_DEPRECATED: u64 = 1 << 2;
pub const ACCOUNT_TRANSFER_AUTHORITY_DEPRECATED: u64 = 1 << 3;
pub const ACCOUNT_IN_RECEIVERSHIP: u64 = 1 << 4;
pub const ACCOUNT_IN_DELEVERAGE: u64 = 1 << 5;
pub const ACCOUNT_FROZEN: u64 = 1 << 6;
pub const MAX_LENDING_ACCOUNT_BALANCES: usize = 16;

assert_struct_size!(LendingAccount, 1728);
assert_struct_align!(LendingAccount, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct LendingAccount {
    pub balances: [Balance; MAX_LENDING_ACCOUNT_BALANCES], // 104 * 16 = 1664
    pub _padding: [u64; 8],                                // 8 * 8 = 64
}

impl LendingAccount {
    pub fn get_balance(&self, bank_pk: &Pubkey) -> Option<&Balance> {
        self.balances
            .iter()
            .find(|balance| balance.is_active() && balance.bank_pk.eq(bank_pk))
    }

    pub fn get_active_balances_iter(&self) -> impl Iterator<Item = &Balance> {
        self.balances.iter().filter(|b| b.is_active())
    }
}

pub enum BalanceSide {
    Assets,
    Liabilities,
}

assert_struct_size!(Balance, 104);
assert_struct_align!(Balance, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct Balance {
    pub active: u8,
    pub bank_pk: Pubkey,
    /// Inherited from the bank when the position is first created and CANNOT BE CHANGED after that.
    /// Note that all balances created before the addition of this feature use `ASSET_TAG_DEFAULT`
    pub bank_asset_tag: u8,
    pub _pad0: [u8; 6],
    pub asset_shares: WrappedI80F48,
    pub liability_shares: WrappedI80F48,
    pub emissions_outstanding: WrappedI80F48,
    pub last_update: u64,
    pub _padding: [u64; 1],
}

impl Balance {
    pub fn is_active(&self) -> bool {
        self.active != 0
    }

    pub fn set_active(&mut self, value: bool) {
        self.active = value as u8;
    }

    /// Check whether a balance is empty while accounting for any rounding errors
    /// that might have occured during depositing/withdrawing.
    #[inline]
    pub fn is_empty(&self, side: BalanceSide) -> bool {
        let shares: I80F48 = match side {
            BalanceSide::Assets => self.asset_shares,
            BalanceSide::Liabilities => self.liability_shares,
        }
        .into();

        shares < EMPTY_BALANCE_THRESHOLD
    }

    pub fn get_side(&self) -> Option<BalanceSide> {
        let asset_shares = I80F48::from(self.asset_shares);
        let liability_shares = I80F48::from(self.liability_shares);

        assert!(
            asset_shares < EMPTY_BALANCE_THRESHOLD || liability_shares < EMPTY_BALANCE_THRESHOLD
        );

        if I80F48::from(self.liability_shares) >= EMPTY_BALANCE_THRESHOLD {
            Some(BalanceSide::Liabilities)
        } else if I80F48::from(self.asset_shares) >= EMPTY_BALANCE_THRESHOLD {
            Some(BalanceSide::Assets)
        } else {
            None
        }
    }

    pub fn empty_deactivated() -> Self {
        Balance {
            active: 0,
            bank_pk: Pubkey::default(),
            bank_asset_tag: ASSET_TAG_DEFAULT,
            _pad0: [0; 6],
            asset_shares: WrappedI80F48::from(I80F48::ZERO),
            liability_shares: WrappedI80F48::from(I80F48::ZERO),
            emissions_outstanding: WrappedI80F48::from(I80F48::ZERO),
            last_update: 0,
            _padding: [0; 1],
        }
    }
}
