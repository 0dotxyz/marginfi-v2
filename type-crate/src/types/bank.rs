use crate::{
    assert_struct_align, assert_struct_size,
    constants::discriminators,
    types::{BankCache, BankConfig},
};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::{BankRateLimiter, EmodeSettings, WrappedI80F48};

assert_struct_size!(Bank, 1856);
assert_struct_align!(Bank, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy), derive(Default, PartialEq, Eq))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct Bank {
    /// The SPL token mint this bank manages
    pub mint: Pubkey,
    /// Number of decimals of the `mint`. Must be < 24.
    pub mint_decimals: u8,

    /// The `MarginfiGroup` this bank belongs to
    pub group: Pubkey,

    // Note: The padding is here, not after mint_decimals. Pubkey has alignment 1, so those 32
    // bytes can cross the alignment 8 threshold, but WrappedI80F48 has alignment 8 and cannot
    pub _pad0: [u8; 7], // 1x u8 + 7 = 8

    /// Monotonically increases as interest rate accumulates. For typical banks, a user's asset
    /// value in token = (number of shares the user has * asset_share_value).
    /// * A float (arbitrary decimals)
    /// * Initially 1
    pub asset_share_value: WrappedI80F48,
    /// Monotonically increases as interest rate accumulates. For typical banks, a user's liabilty
    /// value in token = (number of shares the user has * liability_share_value)
    /// * A float (arbitrary decimals)
    /// * Initially 1
    pub liability_share_value: WrappedI80F48,

    /// The SPL token account holding deposited liquidity
    pub liquidity_vault: Pubkey,
    /// PDA bump for the liquidity vault
    pub liquidity_vault_bump: u8,
    /// PDA bump for the liquidity vault authority
    pub liquidity_vault_authority_bump: u8,

    /// The SPL token account holding insurance fund tokens
    pub insurance_vault: Pubkey,
    /// PDA bump for the insurance vault
    pub insurance_vault_bump: u8,
    /// PDA bump for the insurance vault authority
    pub insurance_vault_authority_bump: u8,

    pub _pad1: [u8; 4], // 4x u8 + 4 = 8

    /// Fees collected and pending withdraw for the `insurance_vault`
    pub collected_insurance_fees_outstanding: WrappedI80F48,

    /// The SPL token account holding collected group fees
    pub fee_vault: Pubkey,
    /// PDA bump for the fee vault
    pub fee_vault_bump: u8,
    /// PDA bump for the fee vault authority
    pub fee_vault_authority_bump: u8,

    pub _pad2: [u8; 6], // 2x u8 + 6 = 8

    /// Fees collected and pending withdraw for the `fee_vault`
    pub collected_group_fees_outstanding: WrappedI80F48,

    /// Sum of all liability shares held by all borrowers in this bank.
    /// Multiply by `liability_share_value` to get the total liability amount in native token units.
    pub total_liability_shares: WrappedI80F48,
    /// Sum of all asset shares held by all depositors in this bank.
    /// Multiply by `asset_share_value` to get the total asset amount in native token units.
    /// * For Kamino banks, this is the quantity of collateral tokens (NOT liquidity tokens) in the
    ///   bank, and also uses `mint_decimals`, though the mint itself will always show (6) decimals
    ///   exactly (i.e Kamino ignores this and treats it as if it was using `mint_decimals`)
    pub total_asset_shares: WrappedI80F48,

    /// Unix timestamp (i64) of the last interest accrual
    pub last_update: i64,

    /// The bank's configuration parameters (weights, limits, oracle setup, interest rate config)
    pub config: BankConfig,

    /// Bank flags bitfield (u64).
    ///
    /// - Bit 0 (1): `EMISSIONS_FLAG_BORROW_ACTIVE` — borrow-side emissions are active
    /// - Bit 1 (2): `EMISSIONS_FLAG_LENDING_ACTIVE` — lending-side emissions are active
    /// - Bit 2 (4): `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` — anyone can settle bad debt
    /// - Bit 3 (8): `FREEZE_SETTINGS` — bank configuration is frozen (only limits can change)
    /// - Bit 4 (16): `CLOSE_ENABLED_FLAG` — bank can be closed (set at creation for banks >= 0.1.4)
    /// - Bit 5 (32): `TOKENLESS_REPAYMENTS_ALLOWED` — risk admin can repay debt without tokens
    /// - Bit 6 (64): `TOKENLESS_REPAYMENTS_COMPLETE` — all debt cleared, lender purge enabled
    pub flags: u64,
    /// Emissions APR. Number of emitted tokens (emissions_mint) per 1e(bank.mint_decimal) tokens
    /// (bank mint) (native amount) per 1 YEAR.
    pub emissions_rate: u64,
    /// Remaining emissions tokens available for distribution
    pub emissions_remaining: WrappedI80F48,
    /// The SPL token mint used for emissions rewards
    pub emissions_mint: Pubkey,

    /// Fees collected and pending withdraw for the `FeeState.global_fee_wallet`'s canonical ATA for `mint`
    pub collected_program_fees_outstanding: WrappedI80F48,

    /// Controls this bank's emode configuration, which enables some banks to treat the assets of
    /// certain other banks more preferentially as collateral.
    pub emode: EmodeSettings,

    /// Set with `update_fees_destination_account`. Fees can be withdrawn to the canonical ATA of
    /// this wallet without the admin's input (withdraw_fees_permissionless). If pubkey default, the
    /// bank doesn't support this feature, and the fees must be collected manually (withdraw_fees).
    pub fees_destination_account: Pubkey,

    /// Cached bank metrics (interest rates, oracle price, etc.)
    pub cache: BankCache,
    /// Number of user lending positions currently open in this bank
    /// * For banks created prior to 0.1.4, this is the number of positions opened/closed after
    ///   0.1.4 goes live, and may be negative.
    /// * For banks created in 0.1.4 or later, this is the number of positions open in total, and
    ///   the bank may safely be closed if this is zero. Will never go negative.
    pub lending_position_count: i32,
    /// Number of user borrowing positions currently open in this bank
    /// * For banks created prior to 0.1.4, this is the number of positions opened/closed after
    ///   0.1.4 goes live, and may be negative.
    /// * For banks created in 0.1.4 or later, this is the number of positions open in total, and
    ///   the bank may safely be closed if this is zero. Will never go negative.
    pub borrowing_position_count: i32,

    /// Reserved for future use
    pub _padding_0: [u8; 16],

    /// Integration account slot 1 (default Pubkey for non-integrations).
    /// - Kamino: reserve
    /// - Drift: spot market
    /// - Solend: reserve
    /// - JupLend: lending state
    pub integration_acc_1: Pubkey,
    /// Integration account slot 2 (default Pubkey for non-integrations).
    /// - Kamino: obligation
    /// - Drift: user
    /// - Solend: obligation
    /// - JupLend: fToken vault
    pub integration_acc_2: Pubkey,
    /// Integration account slot 3 (default Pubkey for non-integrations).
    /// - Drift: user stats
    /// - JupLend: withdraw intermediary ATA (ATA of liquidity_vault_authority for bank mint)
    pub integration_acc_3: Pubkey,

    /// Rate limiter for controlling withdraw/borrow outflow.
    /// Tracks net outflow (outflows - inflows) in native tokens.
    pub rate_limiter: BankRateLimiter,

    pub _pad_0: [u8; 16], // 16B

    /// The seed used to derive this bank's PDA, captured at creation time.
    ///
    /// * `0` for legacy banks created via `lending_pool_add_bank` (the bank account is a fresh
    ///   keypair, not a PDA, so there is no seed). Banks created before this field existed will
    ///   also read `0`, since these bytes were previously reserve padding.
    /// * Otherwise the `bank_seed: u64` argument passed to `lending_pool_add_bank_with_seed`,
    ///   `lending_pool_add_bank_permissionless`, `lending_pool_clone_bank`,
    ///   `lending_pool_add_bank_kamino`, `lending_pool_add_bank_drift`,
    ///   `lending_pool_add_bank_solend`, or `lending_pool_add_bank_juplend`. Lets clients
    ///   re-derive the bank PDA and any seed-derived child PDAs from on-chain data alone.
    pub bank_seed: u64, // 8B; carved from the first 8B of the old _padding_1: [[u64; 2]; 7] (112B).
    pub _padding_1: [u64; 13], // 8 * 13 = 104B; total reserve still 16 + 8 + 104 = 128B.
}

impl Bank {
    pub const LEN: usize = std::mem::size_of::<Bank>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::BANK;
}

#[repr(u8)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum RiskTier {
    #[default]
    Collateral = 0,
    /// ## Isolated Risk
    /// Assets in this tier can be borrowed only in isolation.
    /// They can't be borrowed together with other assets.
    ///
    /// For example, if users has USDC, and wants to borrow XYZ which is isolated,
    /// they can't borrow XYZ together with SOL, only XYZ alone.
    Isolated = 1,
}
unsafe impl Zeroable for RiskTier {}
unsafe impl Pod for RiskTier {}

#[repr(u8)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum BankOperationalState {
    /// All operations are halted
    Paused,
    /// Normal operations
    Operational,
    /// Only withdrawals and repayments are allowed (no new deposits or borrows)
    ReduceOnly,
    /// Bank was killed by a bankruptcy event (irrecoverable)
    KilledByBankruptcy,
}
unsafe impl Zeroable for BankOperationalState {}
unsafe impl Pod for BankOperationalState {}

#[repr(u8)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum OracleSetup {
    None = 0,
    PythLegacy = 1,
    SwitchboardV2 = 2,
    PythPushOracle = 3,
    SwitchboardPull = 4,
    StakedWithPythPush = 5,
    KaminoPythPush = 6,
    KaminoSwitchboardPull = 7,
    Fixed = 8,
    DriftPythPull = 9,
    DriftSwitchboardPull = 10,
    SolendPythPull = 11,
    SolendSwitchboardPull = 12,
    FixedKamino = 13,
    FixedDrift = 14,
    JuplendPythPull = 15,
    JuplendSwitchboardPull = 16,
    FixedJuplend = 17,
}
unsafe impl Zeroable for OracleSetup {}
unsafe impl Pod for OracleSetup {}

impl OracleSetup {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::PythLegacy),    // Deprecated
            2 => Some(Self::SwitchboardV2), // Deprecated
            3 => Some(Self::PythPushOracle),
            4 => Some(Self::SwitchboardPull),
            5 => Some(Self::StakedWithPythPush),
            6 => Some(Self::KaminoPythPush),
            7 => Some(Self::KaminoSwitchboardPull),
            8 => Some(Self::Fixed),
            9 => Some(Self::DriftPythPull),
            10 => Some(Self::DriftSwitchboardPull),
            11 => Some(Self::SolendPythPull),
            12 => Some(Self::SolendSwitchboardPull),
            13 => Some(Self::FixedKamino),
            14 => Some(Self::FixedDrift),
            15 => Some(Self::JuplendPythPull),
            16 => Some(Self::JuplendSwitchboardPull),
            17 => Some(Self::FixedJuplend),
            _ => None,
        }
    }
}
