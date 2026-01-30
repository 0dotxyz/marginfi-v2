use crate::{assert_struct_align, assert_struct_size, constants::*, math_error, DriftMocksError};
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};

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
    Deposit = 0,
    Borrow = 1,
}

unsafe impl Zeroable for SpotBalanceType {}
unsafe impl Pod for SpotBalanceType {}

assert_struct_size!(MinimalSpotMarket, 768);
assert_struct_align!(MinimalSpotMarket, 8);
/// Minimal representation of Drift's SpotMarket account
/// Only includes the fields we actually need for marginfi integration
/// https://github.com/drift-labs/protocol-v2/tree/master/programs/drift/src/state/spot_market.rs#L35
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

    pub _padding1: [[u64; 4]; 9],
    pub _padding2: [u8; 8],

    /// All the fields we need for testing (stored as raw bytes for simplicity)
    pub deposit_balance: [u8; 16], // u128 in Drift
    pub borrow_balance: [u8; 16],              // u128 in Drift
    pub cumulative_deposit_interest: [u8; 16], // u128 in Drift
    pub cumulative_borrow_interest: [u8; 16],  // u128 in Drift

    pub _padding3: [u64; 9],

    /// Last time the cumulative deposit and borrow interest was updated
    /// Offset: 568 bytes from start of struct (including discriminator)
    pub last_interest_ts: u64,

    pub _padding4: [u64; 13],

    pub decimals: u32,
    pub market_index: u16,

    pub _padding5: [u16; 24],
    pub _padding6: [u8; 1],

    pub pool_id: u8,

    /// Padding to reach 776 bytes total (including discriminator)
    pub _padding7: [u64; 5],
}

#[derive(Clone, Copy, Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
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
    /// Core scaled balance calculation used by both increment and decrement.
    /// See `get_spot_balance` function on Drift program.
    /// https://github.com/drift-labs/protocol-v2/blob/master/programs/drift/src/math/spot_balance.rs#L16
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

    /// Convert scaled balance back to token amount for withdrawals
    ///
    /// # Parameters
    /// * `scaled_balance` - Balance in Drift's internal scaled units (SPOT_BALANCE_PRECISION = 10^9)
    ///
    /// # Returns
    /// * Token amount in native mint precision (mint_decimals)
    pub fn get_withdraw_token_amount(&self, scaled_balance: u64) -> Result<u64> {
        // See `get_token_amount` function on drift
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

    /// Adjust oracle price using Drift's internal exchange rate
    /// For Drift integration, we need to apply the exchange rate to the oracle price
    /// so that MarginFi's valuation calculations are correct.
    ///
    /// By definition: valuation = oracle_price * token_amount
    /// Where: token_amount = scaled_balance * cumulative_interest / SPOT_CUMULATIVE_INTEREST_PRECISION
    ///
    /// The goal here is to calculate such adjusted_oracle_price that:
    /// valuation = adjusted_oracle_price * scaled_balance
    ///
    /// Thus: adjusted_oracle_price * scaled_balance = oracle_price * scaled_balance * cumulative_interest / SPOT_CUMULATIVE_INTEREST_PRECISION
    ///
    /// Therefore: adjusted_oracle_price = oracle_price * cumulative_interest / SPOT_CUMULATIVE_INTEREST_PRECISION
    pub fn adjust_oracle_price(&self, oracle_price: i64) -> Result<i64> {
        // Apply essentially the same logic as `get_withdraw_token_amount`
        // but without the rounding:
        // valuation
        // = price * token_amount
        // = price * ( scaled_b * cum_i / 10^d )
        // = scaled_b * ( price * cum_i / 10^d )
        // = scaled_b * adjusted_oracle_price
        // Which then gives get the correct valuation

        // Delegate to the new type-specific wrapper
        self.adjust_i64(oracle_price)
    }

    /// Core adjustment method using u128 for intermediate calculations.
    /// Applies Drift's exchange rate: adjusted = raw * cumulative_interest / precision
    fn adjust_oracle_value_u128(&self, raw: u128) -> Result<u128> {
        let cumulative_interest = u128::from_le_bytes(self.cumulative_deposit_interest);

        let adjusted_value = raw
            .checked_mul(cumulative_interest)
            .ok_or_else(math_error!())?
            .checked_div(SPOT_CUMULATIVE_INTEREST_PRECISION)
            .ok_or_else(math_error!())?;

        Ok(adjusted_value)
    }

    /// Wrapper for i64 values (used by Pyth prices)
    #[inline]
    pub fn adjust_i64(&self, raw: i64) -> Result<i64> {
        // Safe conversion: i64 → u128 for positive prices
        let raw_u128 = u128::try_from(raw).map_err(|_| error!(DriftMocksError::MathError))?;

        let adjusted = self.adjust_oracle_value_u128(raw_u128)?;

        // Convert back to i64
        adjusted
            .try_into()
            .map_err(|_| error!(DriftMocksError::MathError))
    }

    /// Wrapper for u64 values (used by Pyth confidence intervals)
    #[inline]
    pub fn adjust_u64(&self, raw: u64) -> Result<u64> {
        let adjusted = self.adjust_oracle_value_u128(raw as u128)?;
        adjusted
            .try_into()
            .map_err(|_| error!(DriftMocksError::MathError))
    }

    /// Wrapper for i128 values (used by Switchboard Pull oracles)
    #[inline]
    pub fn adjust_i128(&self, raw: i128) -> Result<i128> {
        // Safe conversion: i128 → u128 for positive oracle values
        let raw_u128 = u128::try_from(raw).map_err(|_| error!(DriftMocksError::MathError))?;

        let adjusted = self.adjust_oracle_value_u128(raw_u128)?;

        // Convert back to i128
        adjusted
            .try_into()
            .map_err(|_| error!(DriftMocksError::MathError))
    }

    /// Check if the spot market's interest is stale and needs updating
    ///
    /// Returns true if the market hasn't been updated in the current timestamp.
    /// Unlike Kamino which checks slots, Drift uses timestamps for interest updates.
    ///
    /// Based on Drift documentation, interest should be updated before any operation
    /// that uses the oracle price for valuation (deposits, withdrawals, liquidations).
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        // Market is stale if last_interest_ts is before the current timestamp
        (self.last_interest_ts as i64) < current_timestamp
    }
}

// Implementation methods for MinimalUser
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
