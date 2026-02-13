use anchor_lang::prelude::*;

use crate::constants::*;
use library::math::{casting::*, safe_math::*};

#[account]
#[derive(InitSpace)]
pub struct LendingRewardsAdmin {
    pub authority: Pubkey,

    pub lending_program: Pubkey,

    #[max_len(MAX_AUTH_COUNT)]
    pub auths: Vec<Pubkey>, // Configurators
    pub bump: u8,
}

#[account]
#[derive(InitSpace, Default)]
pub struct LendingRewardsRateModel {
    /// @dev mint address
    pub mint: Pubkey,

    /// @dev tvl below which rewards rate is 0. If current TVL is below this value, triggering `update_rate()` on the fToken
    /// might bring the total TVL above this cut-off.
    pub start_tvl: u64,

    /// @dev for how long current rewards should run
    pub duration: u64,

    /// @dev when current rewards got started
    pub start_time: u64,

    /// @dev current annualized reward based on input params (duration, rewardAmount)
    pub yearly_reward: u64,

    /// @dev Duration for the next rewards phase
    pub next_duration: u64,

    /// @dev Amount of rewards for the next phase
    pub next_reward_amount: u64,

    pub bump: u8,
}

impl LendingRewardsRateModel {
    /// Calculates the current rewards rate (APR) and provides timing information
    /// @param total_assets amount of assets in the lending
    /// @return current_rate current rewards rate percentage per year with 1e12 RATE_PRECISION
    /// @return current_start_time start time of current rewards
    /// @return current_end_time end time of current rewards
    /// @return next_start_time start time of next rewards (0 if none)
    /// @return next_end_time end time of next rewards (0 if none)
    /// @return next_rate next rewards rate percentage per year with 1e12 RATE_PRECISION (0 if none)
    pub fn get_rate(&self, total_assets: u64) -> Result<(u64, u64, u64, u64, u64, u64)> {
        let current_time: u64 = Clock::get()?.unix_timestamp.cast()?;
        let start_time = self.start_time;
        let end_time = start_time.safe_add(self.duration)?;

        // If not started yet or start_time is 0
        if start_time == 0 || current_time < start_time {
            return Ok((0, start_time, end_time, 0, 0, 0));
        }

        // Current rewards period
        // @dev no need to check if current_time > end_time as it's checked in the caller
        let current_rate = if total_assets < self.start_tvl {
            0
        } else {
            let rate = self
                .yearly_reward
                .cast::<u128>()?
                .safe_mul(RETURN_PERCENT_PRECISION)?
                .safe_div(total_assets.cast()?)?;

            if rate > MAX_RATE {
                MAX_RATE.cast()?
            } else {
                rate.cast()?
            }
        };

        // Next rewards period
        let next_reward_amount = self.next_reward_amount;
        let (next_start_time, next_end_time, next_rate) = if next_reward_amount > 0 {
            let next_duration = self.next_duration;
            let next_start = end_time; // Next rewards start when current ones end
            let next_end = next_start.safe_add(next_duration)?;

            // Calculate next rewards rate
            let next_rate = if total_assets < self.start_tvl {
                0
            } else {
                let next_yearly_reward = next_reward_amount
                    .cast::<u128>()?
                    .safe_mul(SECONDS_PER_YEAR)?
                    .safe_div(next_duration.cast()?)?;

                let rate = next_yearly_reward
                    .safe_mul(RETURN_PERCENT_PRECISION)? // 1e14
                    .safe_div(total_assets.cast()?)?;

                if rate > MAX_RATE {
                    MAX_RATE.cast()?
                } else {
                    rate.cast()?
                }
            };

            (next_start, next_end, next_rate)
        } else {
            (0, 0, 0)
        };

        Ok((
            current_rate,
            start_time,
            end_time,
            next_start_time,
            next_end_time,
            next_rate,
        ))
    }
}
