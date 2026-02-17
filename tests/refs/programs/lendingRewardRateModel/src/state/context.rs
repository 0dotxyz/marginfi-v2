use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

use crate::constants::*;
use crate::errors::ErrorCodes;
use crate::events::*;
use crate::invokes::lending::*;
use crate::state::seeds::*;
use crate::state::state::*;

use library::math::{casting::*, safe_math::*};

#[derive(Accounts)]
pub struct InitLendingRewardsAdmin<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = 8 + LendingRewardsAdmin::INIT_SPACE,
        seeds = [LENDING_REWARDS_ADMIN_SEED],
        bump,
    )]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAuthority<'info> {
    #[account(address = lending_rewards_admin.authority @ ErrorCodes::OnlyAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,
}

#[derive(Accounts)]
pub struct UpdateAuths<'info> {
    #[account(address = lending_rewards_admin.authority @ ErrorCodes::OnlyAuthority)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,
}

#[derive(Accounts)]
pub struct InitLendingRewardsRateModel<'info> {
    #[account(mut, constraint = lending_rewards_admin.auths.contains(&authority.key()) || authority.key() == PROTOCOL_INIT_AUTH @ ErrorCodes::OnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,

    /// CHECK: safe
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + LendingRewardsRateModel::INIT_SPACE,
        seeds = [LENDING_REWARDS_RATE_MODEL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub lending_rewards_rate_model: Account<'info, LendingRewardsRateModel>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LendingRewards<'info> {
    #[account(constraint = lending_rewards_admin.auths.contains(&authority.key()) @ ErrorCodes::OnlyAuths)]
    pub authority: Signer<'info>,

    #[account()]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,

    #[account(mut)]
    /// CHECK: This will be checked during lending program invoke
    pub lending_account: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub mint: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub f_token_mint: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub supply_token_reserves_liquidity: UncheckedAccount<'info>,

    #[account(mut, has_one = mint @ ErrorCodes::InvalidMint)]
    pub lending_rewards_rate_model: Account<'info, LendingRewardsRateModel>,

    /// CHECK: This will be checked during lending program invoke
    #[account(address = lending_rewards_admin.lending_program @ ErrorCodes::InvalidLendingProgram)]
    pub lending_program: UncheckedAccount<'info>,
}

impl<'info> LendingRewards<'info> {
    pub fn start(
        &mut self,
        reward_amount: u64,
        duration: u64,
        start_time: u64,
        start_tvl: u64,
    ) -> Result<()> {
        let lending_rewards_rate_model = &mut self.lending_rewards_rate_model;

        if duration == 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        if reward_amount == 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        let current_time = Clock::get()?.unix_timestamp.cast()?;
        let end_time = lending_rewards_rate_model
            .start_time
            .safe_add(lending_rewards_rate_model.duration)?;

        if current_time <= end_time {
            return Err(ErrorCodes::NotEnded.into());
        } else {
            if lending_rewards_rate_model.next_reward_amount > 0 {
                return Err(ErrorCodes::MustTransitionToNext.into());
            }

            if lending_rewards_rate_model.yearly_reward > 0 {
                // if any rewards have existed already,
                // make sure previous rewards are fully synced up until end_time before overwriting the configs
                let accounts = UpdateRateAccounts {
                    lending_account: self.lending_account.to_account_info(),
                    mint: self.mint.to_account_info(),
                    f_token_mint: self.f_token_mint.to_account_info(),
                    supply_token_reserves_liquidity: self
                        .supply_token_reserves_liquidity
                        .to_account_info(),
                    rewards_rate_model: lending_rewards_rate_model.to_account_info(),
                    lending_program: self.lending_program.clone(),
                };

                accounts.update_rate()?;
            }
        }

        let actual_start_time = if start_time == 0 {
            current_time
        } else {
            require!(start_time >= current_time, ErrorCodes::InvalidParams);
            start_time
        };

        lending_rewards_rate_model.start_time = actual_start_time;
        lending_rewards_rate_model.duration = duration;
        lending_rewards_rate_model.yearly_reward = reward_amount
            .cast::<u128>()?
            .safe_mul(SECONDS_PER_YEAR)?
            .safe_div(duration.cast()?)?
            .cast()?;
        lending_rewards_rate_model.start_tvl = start_tvl;

        emit!(LogStartRewards {
            reward_amount,
            duration,
            start_time: actual_start_time,
            mint: lending_rewards_rate_model.mint,
        });

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        let lending_rewards_rate_model = &self.lending_rewards_rate_model;
        let current_time: u64 = Clock::get()?.unix_timestamp.cast()?;
        let end_time = lending_rewards_rate_model
            .start_time
            .safe_add(lending_rewards_rate_model.duration)?;

        if lending_rewards_rate_model.start_time == 0 || current_time > end_time {
            return Err(ErrorCodes::AlreadyStopped.into());
        }

        if lending_rewards_rate_model.next_reward_amount > 0 {
            return Err(ErrorCodes::NextRewardsQueued.into());
        }

        let accounts = UpdateRateAccounts {
            lending_account: self.lending_account.to_account_info(),
            mint: self.mint.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            rewards_rate_model: self.lending_rewards_rate_model.to_account_info(),
            lending_program: self.lending_program.clone(),
        };

        accounts.update_rate()?;

        let lending_rewards_rate_model = &mut self.lending_rewards_rate_model;

        lending_rewards_rate_model.duration = current_time
            .saturating_sub(lending_rewards_rate_model.start_time)
            .saturating_sub(1);

        emit!(LogStopRewards {
            mint: lending_rewards_rate_model.mint,
        });

        Ok(())
    }

    pub fn cancel(&mut self) -> Result<()> {
        let lending_rewards_rate_model = &mut self.lending_rewards_rate_model;

        let current_time: u64 = Clock::get()?.unix_timestamp.cast()?;
        let end_time = lending_rewards_rate_model
            .start_time
            .safe_add(lending_rewards_rate_model.duration)?;

        if lending_rewards_rate_model.next_reward_amount == 0 {
            return Err(ErrorCodes::NoQueuedRewards.into());
        }

        if current_time > end_time {
            return Err(ErrorCodes::MustTransitionToNext.into());
        }

        lending_rewards_rate_model.next_duration = 0;
        lending_rewards_rate_model.next_reward_amount = 0;

        emit!(LogCancelQueuedRewards {
            mint: lending_rewards_rate_model.mint,
        });

        Ok(())
    }

    pub fn queue(&mut self, reward_amount: u64, duration: u64) -> Result<()> {
        let lending_rewards_rate_model = &mut self.lending_rewards_rate_model;
        let current_time = Clock::get()?.unix_timestamp.cast()?;
        let end_time = lending_rewards_rate_model
            .start_time
            .safe_add(lending_rewards_rate_model.duration)?;

        if duration == 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        if reward_amount == 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        if lending_rewards_rate_model.start_time == 0 {
            return Err(ErrorCodes::NoRewardsStarted.into());
        }

        if lending_rewards_rate_model.next_reward_amount > 0 {
            return Err(ErrorCodes::NextRewardsQueued.into());
        }

        let start_tvl = lending_rewards_rate_model.start_tvl;

        if current_time > end_time {
            return self.start(reward_amount, duration, current_time, start_tvl);
        }

        lending_rewards_rate_model.next_reward_amount = reward_amount;
        lending_rewards_rate_model.next_duration = duration;

        emit!(LogQueueNextRewards {
            reward_amount,
            duration,
            mint: lending_rewards_rate_model.mint,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransitionToNextRewards<'info> {
    #[account()]
    pub lending_rewards_admin: Account<'info, LendingRewardsAdmin>,

    /// CHECK: This will be checked during lending program invoke
    #[account(mut)]
    pub lending_account: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub mint: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub f_token_mint: UncheckedAccount<'info>,

    /// CHECK: This will be checked during lending program invoke
    pub supply_token_reserves_liquidity: UncheckedAccount<'info>,

    #[account(mut, has_one = mint @ ErrorCodes::InvalidMint)]
    pub lending_rewards_rate_model: Account<'info, LendingRewardsRateModel>,

    /// CHECK: This will be checked during lending program invoke
    #[account(address = lending_rewards_admin.lending_program @ ErrorCodes::InvalidLendingProgram)]
    pub lending_program: UncheckedAccount<'info>,
}

impl<'info> TransitionToNextRewards<'info> {
    pub fn next(&mut self) -> Result<()> {
        let lending_rewards_rate_model = &self.lending_rewards_rate_model;
        let current_time: u64 = Clock::get()?.unix_timestamp.cast()?;
        let start_time = lending_rewards_rate_model.start_time;
        let end_time = start_time.safe_add(lending_rewards_rate_model.duration)?;

        require!(current_time > end_time, ErrorCodes::NotEnded);

        let next_reward_amount = lending_rewards_rate_model.next_reward_amount;
        require!(next_reward_amount > 0, ErrorCodes::NoQueuedRewards);

        let next_duration = lending_rewards_rate_model.next_duration;

        let accounts = UpdateRateAccounts {
            lending_account: self.lending_account.to_account_info(),
            mint: self.mint.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            rewards_rate_model: self.lending_rewards_rate_model.to_account_info(),
            lending_program: self.lending_program.clone(),
        };

        accounts.update_rate()?;

        let lending_rewards_rate_model = &mut self.lending_rewards_rate_model;

        lending_rewards_rate_model.start_time = end_time;
        lending_rewards_rate_model.duration = next_duration;
        lending_rewards_rate_model.yearly_reward = next_reward_amount
            .cast::<u128>()?
            .safe_mul(SECONDS_PER_YEAR)?
            .safe_div(next_duration.cast()?)?
            .cast()?;

        let new_end_time = end_time.safe_add(next_duration)?;

        lending_rewards_rate_model.next_duration = 0;
        lending_rewards_rate_model.next_reward_amount = 0;

        emit!(LogTransitionedToNextRewards {
            start_time: end_time,
            end_time: new_end_time,
            mint: lending_rewards_rate_model.mint,
        });

        Ok(())
    }
}
