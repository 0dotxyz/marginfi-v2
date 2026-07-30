use crate::{assert_struct_align, assert_struct_size, constants::*, math_error, DriftMocksError};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

// Account discriminators from Drift IDL
pub const SPOT_MARKET_DISCRIMINATOR: [u8; 8] = [100, 177, 8, 107, 168, 65, 65, 39];
pub const USER_DISCRIMINATOR: [u8; 8] = [159, 117, 95, 227, 239, 151, 58, 236];
pub const USER_STATS_DISCRIMINATOR: [u8; 8] = [176, 223, 136, 27, 122, 79, 32, 227];

assert_struct_size!(SpotPosition, 40);
assert_struct_align!(SpotPosition, 8);
/// Minimal representation of a spot position within a User account
#[repr(C)]
#[zero_copy]
pub struct SpotPosition {
    /// The scaled balance of the position.
    /// * Precision: SPOT_BALANCE_PRECISION
    pub scaled_balance: u64,
    /// How many spot bids the user has open
    /// * Precision: token mint precision
    pub open_bids: i64,
    /// How many spot asks the user has open
    /// * Precision: token mint precision
    pub open_asks: i64,
    /// The cumulative deposits/borrows a user has made
    /// * Precision: token mint precision
    pub cumulative_deposits: i64,
    /// The market index of the corresponding spot market
    pub market_index: u16,
    /// Whether the position is deposit or borrow
    pub balance_type: SpotBalanceType,
    /// Number of open orders
    pub open_orders: u8,
    /// Padding
    pub padding: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
#[repr(u8)]
pub enum SpotBalanceType {
    Deposit, // 0
    Borrow,  // 1
}

unsafe impl Zeroable for SpotBalanceType {}
unsafe impl Pod for SpotBalanceType {}

assert_struct_size!(InsuranceFund, 112);
assert_struct_align!(InsuranceFund, 8);
/// Mirrors Drift's `SpotMarket.insurance_fund` field-for-field (`struct InsuranceFund`). u128 fields
/// are stored as raw bytes to preserve 8-byte alignment.
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/state/spot_market.rs#L689-L702
#[zero_copy]
#[repr(C)]
#[derive(Default)]
pub struct InsuranceFund {
    pub vault: Pubkey,
    pub total_shares: [u8; 16],
    pub user_shares: [u8; 16],
    pub shares_base: [u8; 16],
    pub unstaking_period: i64,
    pub last_revenue_settle_ts: i64,
    pub revenue_settle_period: i64,
    /// Percentage of interest taken by the insurance fund (PERCENTAGE_PRECISION = 1e6).
    pub total_factor: u32,
    pub user_factor: u32,
}

assert_struct_size!(MinimalSpotMarket, 768);
assert_struct_align!(MinimalSpotMarket, 8);
/// Minimal representation of Drift's SpotMarket account
/// Only includes the fields we actually need for marginfi integration
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/state/spot_market.rs#L33-L211
#[account(zero_copy, discriminator = &SPOT_MARKET_DISCRIMINATOR)]
#[repr(C)]
#[derive(Default)]
pub struct MinimalSpotMarket {
    /// The address of the spot market. It is a pda of the market index
    pub pubkey: Pubkey,
    /// The oracle used to price the markets deposits/borrows
    pub oracle: Pubkey,
    /// The token mint of the market
    pub mint: Pubkey,
    /// The vault used to store the market's deposits
    pub vault: Pubkey,

    /// SpotMarket fields between `vault` and `insurance_fund`; unused by marginfi, sized to match
    /// upstream.
    pub name: [u8; 32],
    pub historical_oracle_data: [u64; 6],
    pub historical_index_data: [u64; 5],
    pub revenue_pool: [u64; 3],
    pub spot_fee_pool: [u64; 3],
    pub insurance_fund: InsuranceFund,
    pub total_spot_fee: [u64; 2],

    /// All the fields we need for testing (stored as raw bytes for simplicity)
    pub deposit_balance: [u8; 16], // u128 in Drift
    pub borrow_balance: [u8; 16],              // u128 in Drift
    pub cumulative_deposit_interest: [u8; 16], // u128 in Drift
    pub cumulative_borrow_interest: [u8; 16],  // u128 in Drift

    pub _padding1: [u64; 5],
    /// Deposit ceiling in native mint precision; `0` is uncapped.
    pub max_token_deposits: u64,
    pub _padding2: [u64; 3],

    /// Last time the cumulative deposit and borrow interest was updated
    /// Offset: 568 bytes from start of struct (including discriminator)
    pub last_interest_ts: u64,

    pub _padding3: [u64; 11],
    pub _padding4: [u8; 4],
    /// Drift spot interest-rate curve params (`SpotMarket`), precision 1e6.
    pub optimal_utilization: u32,
    pub optimal_borrow_rate: u32,
    pub max_borrow_rate: u32,

    pub decimals: u32,
    pub market_index: u16,

    pub _padding5: [u16; 1],
    /// `MarketStatus`; `0` is `Initialized`, the warm-up state that rejects deposits, and `1` is
    /// `Active`.
    pub status: u8,
    pub _padding6: [u8; 1],
    /// `SpotOperation` pause bitmask; bit 0 (`UpdateCumulativeInterest`) stops interest accrual.
    pub paused_operations: u8,
    pub _padding7: [u8; 30],
    pub _padding8: [u8; 7],
    /// Borrow-rate floor, in units of `PERCENTAGE_PRECISION / 200`.
    pub min_borrow_rate: u8,
    pub _padding9: [u8; 6],

    pub pool_id: u8,

    /// Padding to reach 776 bytes total (including discriminator)
    pub _padding10: [u64; 5],
}

#[derive(Clone, Copy, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum UserStatus {
    Active = 0,
    BeingLiquidated = 0b00000001,
    Bankrupt = 0b00000010,
    ReduceOnly = 0b00000100,
    AdvancedLp = 0b00001000,
    ProtectedMakerOrders = 0b00010000,
}

unsafe impl Zeroable for UserStatus {}
unsafe impl Pod for UserStatus {}

assert_struct_size!(MinimalUser, 4368);
assert_struct_align!(MinimalUser, 8);
/// Minimal representation of Drift's User account
/// Only includes the fields we actually need
#[account(zero_copy, discriminator = &USER_DISCRIMINATOR)]
#[repr(C)]
pub struct MinimalUser {
    /// The owner/authority of the account
    pub authority: Pubkey,
    /// An addresses that can control the account on the authority's behalf
    pub delegate: Pubkey,
    /// Encoded display name for the account
    pub name: [u8; 32],

    /// The user's spot positions (8 positions)
    pub spot_positions: [SpotPosition; 8],

    /// Skip to the fields we need at the end
    pub _padding1: [u64; 256],
    pub _padding2: [u64; 128],
    pub _padding3: [u64; 64],
    pub _padding4: [u64; 32],
    pub _padding5: [u64; 8],
    pub _padding6: [u64; 2],
    pub _padding7: [u16; 1],

    /// Sub account id for this user account
    pub sub_account_id: u16,

    // Status and flags
    pub status: UserStatus,

    // Final padding to reach exactly 4376 bytes (including discriminator)
    pub _padding8: [u8; 27],
}

impl MinimalUser {
    pub fn is_being_liquidated(&self) -> bool {
        matches!(
            self.status,
            UserStatus::BeingLiquidated | UserStatus::Bankrupt
        )
    }
}

assert_struct_size!(MinimalUserStats, 240);
assert_struct_align!(MinimalUserStats, 8);
/// Minimal representation of Drift's UserStats account
/// Only includes the authority field we need
#[account(zero_copy, discriminator = &USER_STATS_DISCRIMINATOR)]
#[repr(C, align(8))]
pub struct MinimalUserStats {
    /// The authority for all of a user's sub accounts
    pub authority: Pubkey,

    /// Padding to reach 240 bytes total
    pub _padding1: [u64; 16],
    pub _padding2: [u64; 8],
    pub _padding3: [u64; 2],
}

// Implementation methods for MinimalSpotMarket
impl MinimalSpotMarket {
    /// Core scaled balance calculation used by both increment and decrement. Mirrors Drift's
    /// `get_spot_balance`:
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L16-L38
    ///
    /// # Parameters
    /// * `amount` - Token amount in native mint precision
    /// * `round_up` - If true, rounds up by 1 (used for withdrawals/decrements to prevent dust)
    fn get_scaled_balance(&self, amount: u64, round_up: bool) -> Result<u64> {
        let precision_increase = get_precision_increase(self.decimals)?;
        let cumulative_interest = u128::from_le_bytes(self.cumulative_deposit_interest);

        let mut balance: u64 = (amount as u128)
            .checked_mul(precision_increase)
            .ok_or_else(math_error!())?
            .checked_div(cumulative_interest)
            .ok_or_else(math_error!())?
            .try_into()?;

        // Drift rounds up withdrawals to prevent dust accumulation
        if round_up && balance != 0 {
            balance = balance
                .checked_add(1)
                .ok_or(error!(DriftMocksError::MathError))?;
        }

        Ok(balance)
    }

    /// Calculate scaled balance decrement for withdrawals (rounds up).
    pub fn get_scaled_balance_decrement(&self, amount: u64) -> Result<u64> {
        self.get_scaled_balance(amount, true)
    }

    /// Calculate scaled balance increment for deposits (floors).
    pub fn get_scaled_balance_increment(&self, amount: u64) -> Result<u64> {
        self.get_scaled_balance(amount, false)
    }

    /// Convert scaled balance back to token amount for withdrawals. Mirrors the `Deposit` branch of
    /// Drift's `get_token_amount`:
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L40-L62
    ///
    /// # Parameters
    /// * `scaled_balance` - Balance in Drift's internal scaled units (SPOT_BALANCE_PRECISION = 10^9)
    ///
    /// # Returns
    /// * Token amount in native mint precision (mint_decimals)
    pub fn get_withdraw_token_amount(&self, scaled_balance: u64) -> Result<u64> {
        let precision_increase = get_precision_increase(self.decimals)?;

        let cumulative_interest = u128::from_le_bytes(self.cumulative_deposit_interest);

        let floored_token_amount: u64 = (scaled_balance as u128)
            .checked_mul(cumulative_interest)
            .ok_or_else(math_error!())?
            .checked_div(precision_increase)
            .ok_or_else(math_error!())?
            .try_into()
            .map_err(|_| error!(DriftMocksError::MathError))?;

        Ok(floored_token_amount)
    }

    /// Check if the spot market's interest is stale and needs updating
    ///
    /// Returns true if the market hasn't been updated in the current timestamp.
    /// Unlike Kamino which checks slots, Drift uses timestamps for interest updates.
    ///
    /// Based on Drift documentation, interest should be updated before any operation
    /// that uses the oracle price for valuation (deposits, withdrawals, liquidations).
    ///
    /// Note that we allow last_interest_ts to be in the *future* compared to the current timestamp.
    /// This is useful for the external callers of the functions we expose, such as try_from_bank(),
    /// because they do not have to synchronize their clock before every call.
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        (self.last_interest_ts as i64) < current_timestamp
    }

    /// True when the market has no borrows. Utilization is then zero, so Drift accrues no interest
    /// and never stamps `last_interest_ts`, and the supply rate is exactly zero.
    pub fn has_no_borrows(&self) -> bool {
        u128::from_le_bytes(self.borrow_balance) == 0
    }
}

/// Drift's above-optimal borrow-curve `(utilization_bp, weight)` segments; `weights_divisor` == 1000.
/// Mirrors `INTEREST_RATE_SEGMENT_AND_WEIGHTS`:
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/constants.rs#L236-L243
const INTEREST_RATE_SEGMENT_AND_WEIGHTS: [(u128, u128); 6] = [
    (850_000, 50),
    (900_000, 100),
    (950_000, 150),
    (990_000, 200),
    (995_000, 250),
    (1_000_000, 250),
];

impl MinimalSpotMarket {
    /// `get_token_amount`: `balance * cumulative_interest / 10^(19 - decimals)`, in native mint
    /// precision.
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L16-L38
    fn token_amount(
        &self,
        balance: [u8; 16],
        cumulative_interest: [u8; 16],
        round_up: bool,
    ) -> Option<u128> {
        let scaled =
            u128::from_le_bytes(balance).checked_mul(u128::from_le_bytes(cumulative_interest))?;
        let divisor = get_precision_increase(self.decimals).ok()?;
        if round_up {
            scaled
                .checked_add(divisor.checked_sub(1)?)?
                .checked_div(divisor)
        } else {
            scaled.checked_div(divisor)
        }
    }

    /// Total deposits in native mint precision, the quantity Drift compares against
    /// `max_token_deposits`.
    pub fn get_deposits(&self) -> Option<u128> {
        self.token_amount(
            self.deposit_balance,
            self.cumulative_deposit_interest,
            false,
        )
    }

    /// Tokens the market still accepts, in native mint precision; `u64::MAX` when uncapped
    /// (`max_token_deposits == 0`). Drift's check is `deposits <= max_token_deposits` post-deposit.
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/state/spot_market.rs#L465-L486
    pub fn remaining_deposit_capacity(&self) -> Option<u64> {
        // A net-deposit position requires `Active`; every other status (Initialized, ReduceOnly,
        // Settlement, Delisted) rejects the deposit whatever the cap says.
        if self.status != 1 {
            return Some(0);
        }
        if self.max_token_deposits == 0 {
            return Some(u64::MAX);
        }
        let remaining = u128::from(self.max_token_deposits).saturating_sub(self.get_deposits()?);
        Some(remaining.min(u128::from(u64::MAX)) as u64)
    }

    /// Net Drift deposit (supply) APR (I80F48, 1.0 == 100%), net of the insurance-fund cut. Reads the
    /// market's stored balances and rate curve; the caller must ensure it was refreshed this slot
    /// (see [`MinimalSpotMarket::is_stale`]). Returns `None` on overflow. Mirrors `get_token_amount`
    /// then `calculate_utilization`/`calculate_borrow_rate`/`calculate_deposit_rate`:
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L40-L62
    pub fn deposit_rate(&self) -> Option<I80F48> {
        self.deposit_rate_at(0)
    }

    /// [`MinimalSpotMarket::deposit_rate`] as it would read after `extra` native tokens were
    /// deposited, which dilutes utilization.
    pub fn deposit_rate_at(&self, extra: u128) -> Option<I80F48> {
        if self.paused_operations & 1 != 0 {
            return Some(I80F48::ZERO);
        }
        drift_deposit_rate_from_parts(
            self.get_deposits()?.checked_add(extra)?,
            self.token_amount(self.borrow_balance, self.cumulative_borrow_interest, true)?,
            self.optimal_utilization as u128,
            self.optimal_borrow_rate as u128,
            self.max_borrow_rate as u128,
            self.min_borrow_rate as u128,
            self.insurance_fund.total_factor as u128,
        )
    }
}

/// Mirrors Drift's `calculate_utilization`: `borrow * SPOT_UTILIZATION_PRECISION / deposit`, with
/// both-zero -> 0 and borrows-without-deposits -> max utilization.
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L93-L110
pub fn calculate_utilization(deposit_token_amount: u128, borrow_token_amount: u128) -> u128 {
    borrow_token_amount
        .saturating_mul(PERCENTAGE_PRECISION)
        .checked_div(deposit_token_amount)
        .unwrap_or({
            if deposit_token_amount == 0 && borrow_token_amount == 0 {
                0
            } else {
                // borrows without deposits -> maximum utilization
                PERCENTAGE_PRECISION
            }
        })
}

/// Mirrors Drift's `calculate_borrow_rate`: linear `slope` up to `optimal_utilization`, then the
/// weighted above-optimal segments. `None` only when `utilization <= optimal_utilization == 0`.
///
/// Divergence: upstream floors the result at `get_min_borrow_rate()`; `MinimalSpotMarket` omits
/// `min_borrow_rate`, so this drops the `min_rate` floor.
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L182-L229
pub fn calculate_borrow_rate(
    utilization: u128,
    optimal_utilization: u128,
    optimal_borrow_rate: u128,
    max_borrow_rate: u128,
    min_borrow_rate: u128,
) -> Option<u128> {
    let weights_divisor = 1000;
    // `SpotMarket::get_min_borrow_rate`: the stored byte counts half-percent steps.
    let min_rate = min_borrow_rate.checked_mul(PERCENTAGE_PRECISION / 200)?;

    if utilization <= optimal_utilization {
        let slope = optimal_borrow_rate
            .saturating_mul(PERCENTAGE_PRECISION)
            .checked_div(optimal_utilization)?;
        let rate = utilization.saturating_mul(slope) / PERCENTAGE_PRECISION;
        return Some(rate.max(min_rate));
    }

    let total_extra_rate = max_borrow_rate.saturating_sub(optimal_borrow_rate);
    let mut rate = optimal_borrow_rate;
    let mut prev_util = optimal_utilization;

    for (bp, weight) in INTEREST_RATE_SEGMENT_AND_WEIGHTS {
        let segment_start = prev_util;
        let segment_end = bp;
        let segment_range = segment_end.saturating_sub(segment_start);
        let segment_rate_total = total_extra_rate.saturating_mul(weight) / weights_divisor;

        if utilization <= segment_end {
            let partial_util = utilization.saturating_sub(segment_start);
            let partial_rate = segment_rate_total
                .saturating_mul(partial_util)
                .checked_div(segment_range)?;
            rate = rate.saturating_add(partial_rate);
            break;
        } else {
            rate = rate.saturating_add(segment_rate_total);
            prev_util = segment_end;
        }
    }

    Some(rate.max(min_rate))
}

/// Mirrors Drift's `calculate_deposit_rate`:
/// `borrow_rate * (PERCENTAGE_PRECISION - total_factor) * utilization / SPOT_UTILIZATION_PRECISION / PERCENTAGE_PRECISION`.
/// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L231-L242
pub fn calculate_deposit_rate(
    borrow_rate: u128,
    utilization: u128,
    total_factor: u128,
) -> Option<u128> {
    Some(
        borrow_rate
            .checked_mul(PERCENTAGE_PRECISION.saturating_sub(total_factor))?
            .checked_mul(utilization)?
            / PERCENTAGE_PRECISION
            / PERCENTAGE_PRECISION,
    )
}

/// Net Drift deposit (supply) rate as I80F48 (1.0 == 100%) from market parts, decoupled from account
/// loading for unit testing and off-chain reuse. Composes `calculate_utilization` ->
/// `calculate_borrow_rate` -> `calculate_deposit_rate`. Rates and `total_factor` are 1e6 units.
pub fn drift_deposit_rate_from_parts(
    deposit_token_amount: u128,
    borrow_token_amount: u128,
    optimal_utilization: u128,
    optimal_borrow_rate: u128,
    max_borrow_rate: u128,
    min_borrow_rate: u128,
    total_factor: u128,
) -> Option<I80F48> {
    let utilization = calculate_utilization(deposit_token_amount, borrow_token_amount);
    let borrow_rate = calculate_borrow_rate(
        utilization,
        optimal_utilization,
        optimal_borrow_rate,
        max_borrow_rate,
        min_borrow_rate,
    )?;
    let deposit_rate = calculate_deposit_rate(borrow_rate, utilization, total_factor)?;
    Some(I80F48::checked_from_num(deposit_rate)? / I80F48::from_num(PERCENTAGE_PRECISION))
}

impl MinimalUser {
    pub fn count_active_deposits(&self) -> usize {
        self.spot_positions
            .iter()
            .filter(|pos| pos.scaled_balance > 0 && pos.balance_type == SpotBalanceType::Deposit)
            .count()
    }

    fn get_active_deposit_markets(&self) -> Vec<u16> {
        self.spot_positions
            .iter()
            .filter(|pos| pos.scaled_balance > 0 && pos.balance_type == SpotBalanceType::Deposit)
            .map(|pos| pos.market_index)
            .collect()
    }

    /// Check if Drift has bricked this account with excessive admin deposits
    /// We support 1 main asset + up to 2 reward assets (3 total active deposits)
    /// If Drift admin deposited more reward assets, the account cannot withdraw
    pub fn validate_not_bricked_by_admin_deposits(&self) -> Result<()> {
        let active_deposits = self.count_active_deposits();

        if active_deposits > 3 {
            msg!(
                "ERROR: Drift has {} active deposit positions",
                active_deposits
            );
            msg!(
                "Active market indexes: {:?}",
                self.get_active_deposit_markets()
            );
            msg!("This account has been bricked by Drift admin deposits!");
            msg!("Cannot withdraw when more than 3 assets have active balances");
            msg!("We support 1 main asset + up to 2 reward assets");
            msg!("SOLUTION: Fee admin wallet needs to harvest these rewards ASAP!");
            return Err(DriftMocksError::TooManyActiveDeposits.into());
        }

        Ok(())
    }

    /// Validate that reward accounts are provided when needed based on active deposits
    /// This helps give clearer error messages when users forget to include reward accounts
    pub fn validate_reward_accounts(
        &self,
        reward_spot_market_is_none: bool,
        reward_spot_market_2_is_none: bool,
    ) -> Result<()> {
        let active_deposits = self.count_active_deposits();

        if active_deposits >= 2 && reward_spot_market_is_none {
            // Account has multiple active deposit positions from Drift admin rewards
            // Must provide drift_reward_spot_market account to withdraw
            // SOLUTION: Include drift_reward_oracle and drift_reward_spot_market in the transaction
            msg!(
                "ERROR: Account has {} active deposit positions. Active market indexes: {:?}",
                active_deposits,
                self.get_active_deposit_markets()
            );
            return Err(DriftMocksError::MissingRewardAccounts.into());
        }

        if active_deposits >= 3 && reward_spot_market_2_is_none {
            // Account has 3+ active deposit positions - multiple admin deposits need harvesting
            // Must provide drift_reward_spot_market_2 account to withdraw
            // SOLUTION: Include both sets of reward accounts (drift_reward_oracle, drift_reward_spot_market, drift_reward_oracle_2, drift_reward_spot_market_2)
            msg!(
                "ERROR: Account has {} active deposit positions. Active market indexes: {:?}",
                active_deposits,
                self.get_active_deposit_markets()
            );
            return Err(DriftMocksError::MissingRewardAccounts.into());
        }

        Ok(())
    }

    /// Validate spot positions for marginfi integration
    /// - USDC (market_index 0) must use position[0]
    /// - All other assets must use position[1]
    /// - Indices 2+ can have admin deposits
    /// - Position must be deposit type
    pub fn validate_spot_position(&self, market_index: u16) -> Result<()> {
        let expected_position_index = if market_index == 0 { 0 } else { 1 };

        let position = &self.spot_positions[expected_position_index];

        if position.scaled_balance > 0 {
            // This position has balance - validate if it's our market
            if position.market_index != market_index {
                // 1. Must be at the expected index
                msg!(
                    "Position {} has balance for market {} but expected market {}",
                    expected_position_index,
                    position.market_index,
                    market_index
                );
                return Err(DriftMocksError::InvalidPositionIndex.into());
            }
            // 2. Must be deposit type
            if position.balance_type != SpotBalanceType::Deposit {
                msg!(
                    "Position {} has balance_type {:?} but expected Deposit",
                    expected_position_index,
                    position.balance_type
                );
                return Err(DriftMocksError::InvalidBalanceType.into());
            }
        } else {
            // This position is empty - just ensure it's truly empty
            if position.cumulative_deposits != 0 {
                msg!(
                    "Position {} should be empty but has cumulative_deposits {}",
                    expected_position_index,
                    position.cumulative_deposits
                );
                return Err(DriftMocksError::InvalidPositionState.into());
            }
        }

        Ok(())
    }

    /// Uses Drift's position indexing pattern:
    /// - USDC (market_index 0) uses position[0]
    /// - All other assets use position[1]
    pub fn get_scaled_balance(&self, market_index: u16) -> u64 {
        let position_index = if market_index == 0 { 0 } else { 1 };
        self.spot_positions[position_index].scaled_balance
    }

    /// Check if user has admin deposits (in indices 2-7) for a given market
    pub fn has_admin_deposit(&self, market_index: u16) -> Result<()> {
        // Check positions 2-7 for admin deposits
        for i in 2..8 {
            let position = &self.spot_positions[i];
            if position.market_index == market_index
                && position.scaled_balance > 0
                && position.balance_type == SpotBalanceType::Deposit
            {
                return Ok(());
            }
        }
        Err(DriftMocksError::NoAdminDeposit.into())
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    fn market(cap: u64, deposit_balance: u128) -> MinimalSpotMarket {
        let mut m = MinimalSpotMarket::zeroed();
        m.decimals = 6;
        m.status = 1; // Active
        m.max_token_deposits = cap;
        m.deposit_balance = deposit_balance.to_le_bytes();
        // 10^19 / 10^6 = 10^13 divisor, so this interest makes token_amount == deposit_balance.
        m.cumulative_deposit_interest = 10_000_000_000_000u128.to_le_bytes();
        m
    }

    #[test]
    fn capacity_is_the_exact_headroom_to_the_cap() {
        assert_eq!(market(1_000, 900).remaining_deposit_capacity(), Some(100));
        assert_eq!(market(1_000, 1_000).remaining_deposit_capacity(), Some(0));
        assert_eq!(market(1_000, 1_200).remaining_deposit_capacity(), Some(0));
    }

    /// A zero cap means unlimited, not full.
    #[test]
    fn zero_cap_reads_as_uncapped() {
        assert_eq!(market(0, 900).remaining_deposit_capacity(), Some(u64::MAX));
    }

    /// Only `Active` accepts a net-deposit position, so every other status reports full despite an
    /// unlimited cap: Initialized(0), and the wind-down states ReduceOnly(6)/Settlement(7)/Delisted(8).
    #[test]
    fn only_an_active_market_reports_capacity() {
        for status in [0u8, 2, 6, 7, 8] {
            let mut m = market(0, 0);
            m.status = status;
            assert_eq!(
                m.remaining_deposit_capacity(),
                Some(0),
                "status {status} must report full"
            );
        }
        let mut active = market(0, 0);
        active.status = 1;
        assert_eq!(active.remaining_deposit_capacity(), Some(u64::MAX));
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;
    use bytemuck::Zeroable;

    /// Upstream floors the borrow rate at `min_borrow_rate * PERCENTAGE_PRECISION / 200`, so a
    /// barely-utilized market still pays lenders that floor times utilization.
    #[test]
    fn borrow_rate_respects_the_floor() {
        // util 0.1, optimal 0.8 -> unfloored borrow = 0.1 * (0.1/0.8) = 0.0125; floor = 10 * 5000 = 5%.
        let floored = calculate_borrow_rate(100_000, 800_000, 100_000, 1_000_000, 10).unwrap();
        assert_eq!(floored, 50_000);
        let unfloored = calculate_borrow_rate(100_000, 800_000, 100_000, 1_000_000, 0).unwrap();
        assert_eq!(unfloored, 12_500);
    }

    /// A market with no borrows never advances `last_interest_ts` upstream.
    #[test]
    fn a_market_without_borrows_reports_no_borrows() {
        let mut m = MinimalSpotMarket::zeroed();
        assert!(m.has_no_borrows());
        m.borrow_balance = 1u128.to_le_bytes();
        assert!(!m.has_no_borrows());
    }

    /// Interest stops accruing while `UpdateCumulativeInterest` is paused, so lenders earn nothing.
    #[test]
    fn paused_interest_accrual_reports_zero_rate() {
        let mut m = MinimalSpotMarket::zeroed();
        m.decimals = 6;
        m.deposit_balance = 1_000u128.to_le_bytes();
        m.borrow_balance = 500u128.to_le_bytes();
        m.cumulative_deposit_interest = 10_000_000_000_000u128.to_le_bytes();
        m.cumulative_borrow_interest = 10_000_000_000_000u128.to_le_bytes();
        m.optimal_utilization = 800_000;
        m.optimal_borrow_rate = 100_000;
        m.max_borrow_rate = 1_000_000;
        assert_eq!(m.deposit_rate().unwrap(), I80F48::from_num(0.03125));
        m.paused_operations = 1;
        assert_eq!(m.deposit_rate().unwrap(), I80F48::ZERO);
    }

    #[test]
    fn deposit_rate_below_optimal() {
        // util 0.5 (< optimal 0.8): borrow = 0.5 * (0.125 / 0.8) = 0.078125;
        // supply = 0.078125 * 0.5 * (1 - 0.60) = 0.015625.
        let r = drift_deposit_rate_from_parts(1000, 500, 800_000, 125_000, 1_000_000, 0, 600_000);
        assert_eq!(r.unwrap(), I80F48::from_num(0.015625));
    }

    #[test]
    fn deposit_rate_above_optimal_uses_segments() {
        // util 0.95 (> optimal 0.8) walks the weighted segments to borrow = 0.625;
        // supply = 0.625 * 0.95 * (1 - 0) = 0.59375.
        let r = drift_deposit_rate_from_parts(1000, 950, 800_000, 250_000, 1_500_000, 0, 0);
        assert_eq!(r.unwrap(), I80F48::from_num(0.59375));
        assert_eq!(
            calculate_borrow_rate(950_000, 800_000, 250_000, 1_500_000, 0).unwrap(),
            625_000
        );
    }

    /// At and past full utilization the segment walk saturates at `max_borrow_rate`, and a market
    /// with borrows but no deposits takes the same branch.
    #[test]
    fn deposit_rate_at_and_past_full_utilization_is_capped() {
        // util 1.0: borrow caps at max 1.25; supply = 1.25 * 1.0 * (1 - 0.10) = 1.125.
        let full = drift_deposit_rate_from_parts(
            7_000_000_000_000,
            7_000_000_000_000,
            800_000,
            85_000,
            1_250_000,
            1,
            100_000,
        );
        assert_eq!(full.unwrap(), I80F48::from_num(1.125));
        let drained = drift_deposit_rate_from_parts(
            0,
            7_000_000_000_000,
            800_000,
            85_000,
            1_250_000,
            1,
            100_000,
        );
        assert_eq!(drained.unwrap(), I80F48::from_num(1.125));
        // util 1.25 past full: supply = 1.25 * 1.25 * (1 - 0.10) = 1.40625.
        let past = drift_deposit_rate_from_parts(
            4_000_000_000_000,
            5_000_000_000_000,
            800_000,
            85_000,
            1_250_000,
            1,
            100_000,
        );
        assert_eq!(past.unwrap(), I80F48::from_num(1.40625));
    }

    #[test]
    fn deposit_rate_zero_optimal_utilization_is_none() {
        // Matches upstream: the `slope` div only fails when utilization <= optimal == 0, i.e. no
        // borrows. (With borrows present, utilization > 0 takes the above-optimal segment path.)
        assert!(drift_deposit_rate_from_parts(1000, 0, 0, 100_000, 1_000_000, 0, 0).is_none());
    }

    /// `deposit_rate()` decodes the `[u8;16]` balances through `get_token_amount`, which scales by
    /// the cumulative interest index and the market's decimals.
    #[test]
    fn deposit_rate_method_decodes_balances() {
        // decimals=6 -> get_token_amount divides by 10^(19-6)=1e13.
        // deposit: 1e9 * 1e10 / 1e13 = 1e6 ; borrow: 5e8 * 1e10 / 1e13 = 5e5, so util 0.5.
        let mut m = MinimalSpotMarket::default();
        m.deposit_balance = 1_000_000_000u128.to_le_bytes();
        m.borrow_balance = 500_000_000u128.to_le_bytes();
        m.cumulative_deposit_interest = 10_000_000_000u128.to_le_bytes();
        m.cumulative_borrow_interest = 10_000_000_000u128.to_le_bytes();
        m.decimals = 6;
        m.optimal_utilization = 800_000;
        m.optimal_borrow_rate = 125_000;
        m.max_borrow_rate = 1_000_000;
        m.insurance_fund.total_factor = 600_000;
        assert_eq!(m.deposit_rate().unwrap(), I80F48::from_num(0.015625));
    }
}
