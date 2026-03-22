use anchor_lang::prelude::*;

use crate::{
    constants::{
        BILLION, EXCHANGE_PRICES_PRECISION, FOUR_DECIMALS, INITIAL_BRANCH_DEBT_FACTOR,
        MAX_LIQUIDATION_ROUNDING_DIFF, X30,
    },
    errors::ErrorCodes,
    events::LogLiquidationRoundingDiff,
    invokes::*,
    state::*,
};

use library::math::{bn::*, casting::*, safe_math::*, tick::TickMath, u256::safe_multiply_divide};

#[derive(Default)]
pub struct OldState {
    // ## User's position before update ##
    pub old_col_raw: u128,
    pub old_net_debt_raw: u128, // total debt - dust debt
    pub old_tick: i32,
}

impl OldState {
    pub fn get_old_ratio(&self) -> Result<u128> {
        Ok(self
            .old_net_debt_raw
            .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
            .safe_div(self.old_col_raw)?)
    }
}

#[derive(Default)]
#[repr(C, packed)]
#[zero_copy]
pub struct OperateMemoryVars {
    // ## User's position after update ##
    pub col_raw: u128,
    pub debt_raw: u128,
    pub dust_debt_raw: u128,
    pub tick: i32,
    pub tick_id: u32,
}

impl OperateMemoryVars {
    pub fn fetch_latest_position(
        &mut self,
        tick_data: &Tick,
        tick_id_liquidation: &AccountLoader<TickIdLiquidation>,
        branch_accounts: &BranchAccounts,
    ) -> Result<u32> {
        // Check if tick's total ID = user's tick ID
        let (is_fully_liquidated, mut branch_id, connection_factor) =
            if tick_data.total_ids == self.tick_id {
                tick_data.get_tick_status()?
            } else {
                let tick_id_liquidation_load = tick_id_liquidation.load()?;
                tick_id_liquidation_load.validate(self.tick, self.tick_id)?;

                tick_id_liquidation_load.get_tick_status(self.tick_id)?
            };

        let initial_position_raw_debt: u128 = self.debt_raw;
        let mut position_raw_debt: u128 = initial_position_raw_debt;
        let mut position_raw_col: u128 = 0;

        if is_fully_liquidated {
            self.tick = COLD_TICK;
            position_raw_debt = 0;
        } else {
            let mut current_connection_factor: u128 = connection_factor.cast()?;

            // Below information about connection debt factor
            // If branch is merged, Connection debt factor is used to multiply in order to get perfect liquidation of user
            // For example: Considering user was at the top.
            // In first branch, the user liquidated to debt factor 0.5 and then branch got merged (branching starting from 1)
            // In second branch, it got liquidated to 0.4 but when the above branch merged the debt factor on this branch was 0.6
            // Meaning on 1st branch, user got liquidated by 50% & on 2nd by 33.33%. So a total of 66.6%.
            // What we will set a connection factor will be 0.6/0.5 = 1.2
            // So now to get user's position, this is what we'll do:
            // finalDebt = (0.4 / (1 * 1.2)) * debtBeforeLiquidation
            // 0.4 is current active branch's minima debt factor
            // 1 is debt factor from where user started
            // 1.2 is connection factor which we found out through 0.6 / 0.5
            let mut current_branch = branch_accounts.load(branch_id)?;
            while current_branch.is_merged() {
                // Merged branch
                current_connection_factor = mul_big_number(
                    current_connection_factor.cast()?,
                    current_branch.debt_factor.cast()?,
                )?
                .cast()?;

                // Check if user is ~100% liquidated
                if current_connection_factor == MAX_MASK_DEBT_FACTOR.cast()? {
                    break;
                }

                branch_id = current_branch.connected_branch_id;
                current_branch = branch_accounts.load(branch_id)?;
            }

            // Check if branch is closed or user is ~100% liquidated
            if current_branch.is_closed()
                || current_connection_factor == MAX_MASK_DEBT_FACTOR.cast()?
            {
                // Branch got closed (or user liquidated ~100%). Hence make the user's position 0
                // Rare cases to get into this situation
                // Branch can get close often but once closed it's tricky that some user might come iterating through there
                // If a user comes then that user will be very mini user like some cents probably
                self.tick = COLD_TICK;
                position_raw_debt = 0;
            } else {
                let branch = branch_accounts.load(branch_id)?;
                // If branch is not merged, the main branch it's connected to then it'll have minima debt factor
                // position debt = debt * base branch minimaDebtFactor / connectionFactor
                let branch_min_debt_factor: u128 = branch.get_branch_debt_factor()?;

                position_raw_debt = mul_div_normal(
                    position_raw_debt.cast()?,
                    branch_min_debt_factor.cast()?,
                    current_connection_factor.cast()?,
                )?
                .cast()?;

                // Reduce debt by 0.01% if liquidated
                if position_raw_debt > initial_position_raw_debt.safe_div(100)? {
                    // Reducing user's liquidity by 0.01% if user got liquidated.
                    // As this will make sure that the branch always have some debt even if all liquidated user left
                    // This saves a lot more logics & consideration on Operate function
                    // if we don't do this then we have to add logics related to closing the branch and factor connections accordingly.
                    position_raw_debt = position_raw_debt
                        .safe_mul(FOUR_DECIMALS - 1)?
                        .safe_div(FOUR_DECIMALS)?;
                } else {
                    // If user debt reduced by more than 99% in liquidation then make user fully liquidated
                    position_raw_debt = 0;
                }

                if position_raw_debt > 0 {
                    // Set position tick to minima tick of branch
                    self.tick = branch.minima_tick;

                    // Calculate user's collateral
                    let ratio_at_tick: u128 = TickMath::get_ratio_at_tick(self.tick)?;
                    let ratio_one_less: u128 = ratio_at_tick
                        .safe_mul(FOUR_DECIMALS)?
                        .safe_div(TickMath::TICK_SPACING)?;

                    // Calculate final ratio with partials
                    let ratio_length: u128 = ratio_at_tick.safe_sub(ratio_one_less)?;
                    let final_ratio: u128 = ratio_one_less.safe_add(
                        ratio_length
                            .safe_mul(branch.minima_tick_partials.cast()?)?
                            .safe_div(X30)?,
                    )?;

                    // Calculate collateral from debt and ratio
                    position_raw_col = position_raw_debt
                        .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
                        .safe_div(final_ratio)?;
                } else {
                    self.tick = COLD_TICK;
                }
            }
        }

        self.debt_raw = position_raw_debt;
        self.col_raw = position_raw_col;

        Ok(branch_id)
    }

    pub fn get_net_debt_raw(&self) -> Result<u128> {
        Ok(self.debt_raw.safe_sub(self.dust_debt_raw)?)
    }

    pub fn get_new_ratio(&self, new_net_debt_raw: u128) -> Result<u128> {
        Ok(new_net_debt_raw
            .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
            .safe_div(self.col_raw)?)
    }

    // @dev we scale the debt by 1/billionth every time user interacts with the vault
    pub fn get_scaled_debt(&self, debt_raw: Option<u128>) -> Result<u128> {
        let debt_raw = debt_raw.unwrap_or(self.debt_raw);

        Ok(debt_raw
            .safe_mul(BILLION + 1)?
            .safe_div(BILLION)?
            .safe_add(1)?)
    }
}

#[derive(Default)]
#[repr(C, packed)]
#[zero_copy]
pub struct BranchState {
    pub minima_tick: i32,          // Minima tick of this branch Y
    pub minima_tick_partials: u32, // Partials of minima tick of branch this is connected to Y
    pub debt_liquidity: u64,       // Debt liquidity at this branch Y

    // For non-merged branches
    pub debt_factor: u64, // Debt factor coefficient, 35 coefficient | 15 exponent, Y

    // For all branches
    pub connected_branch_id: u32, // Branch's ID with which this branch is connected Y
    pub connected_minima_tick: i32, // Minima tick of branch this is connected to
}

#[derive(Default)]
#[repr(C, packed)]
#[zero_copy]
pub struct BranchMemoryVars {
    pub id: u32,
    pub data: BranchState,
    pub debt_factor: u64,
    pub minima_tick: i32, // tick until which liquidation happened
    pub base_branch_data: BranchState,
}

impl BranchMemoryVars {
    pub fn update_branch_to_base_branch(&mut self) {
        self.id = self.data.connected_branch_id;
        self.data = self.base_branch_data;
        self.minima_tick = self.base_branch_data.connected_minima_tick;
    }

    pub fn get_current_ratio_from_minima_tick(&self) -> Result<u128> {
        // Calculate ratios
        let ratio = TickMath::get_ratio_at_tick(self.minima_tick)?;
        let ratio_one_less = ratio
            .safe_mul(FOUR_DECIMALS)?
            .safe_div(TickMath::TICK_SPACING)?;

        let length = ratio.safe_sub(ratio_one_less)?;

        // Calculate current ratio with partials
        let current_ratio = ratio_one_less.safe_add(
            length
                .safe_mul(self.data.minima_tick_partials.cast()?)?
                .safe_div(X30)?,
        )?;

        Ok(current_ratio)
    }

    pub fn update_branch_debt_factor(&mut self, debt_factor: u128) -> Result<()> {
        self.debt_factor = mul_div_big_number(self.debt_factor, debt_factor)?;
        Ok(())
    }

    pub fn set_base_branch_data(&mut self, branch: &Branch) -> Result<()> {
        self.base_branch_data = BranchState {
            minima_tick: branch.minima_tick,
            minima_tick_partials: branch.minima_tick_partials,
            debt_liquidity: branch.debt_liquidity,
            debt_factor: branch.debt_factor,
            connected_branch_id: branch.connected_branch_id,
            connected_minima_tick: branch.connected_minima_tick,
        };

        Ok(())
    }

    pub fn set_branch_data_load(&mut self, branch: &AccountLoader<Branch>) -> Result<()> {
        let branch_data = branch.load()?;
        self.set_branch_data(&branch_data)?;

        Ok(())
    }

    fn set_branch_data(&mut self, branch: &Branch) -> Result<()> {
        self.data = BranchState {
            minima_tick: branch.minima_tick,
            minima_tick_partials: branch.minima_tick_partials,
            debt_liquidity: branch.debt_liquidity,
            debt_factor: branch.debt_factor,
            connected_branch_id: branch.connected_branch_id,
            connected_minima_tick: branch.connected_minima_tick,
        };

        Ok(())
    }

    pub fn reset_branch_data(&mut self) {
        self.data = BranchState::default();
        self.data.minima_tick = COLD_TICK;
        self.data.connected_minima_tick = COLD_TICK;
    }

    pub fn set_branch_data_in_memory(&mut self, branch: &Branch) -> Result<()> {
        self.id = branch.branch_id;

        self.set_branch_data(branch)?;

        self.debt_factor = self.data.debt_factor;

        if self.debt_factor == 0 {
            self.debt_factor = INITIAL_BRANCH_DEBT_FACTOR.cast()?;
        }

        self.minima_tick = self.data.connected_minima_tick;

        Ok(())
    }
}

#[derive(Default)]
#[repr(C, packed)]
#[zero_copy]
pub struct CurrentLiquidity {
    pub debt_remaining: u128, // Debt remaining to liquidate
    pub debt: u128,           // Current liquidatable debt before reaching next check point
    pub col: u128,            // Calculate using debt & ratioCurrent
    pub total_debt_liq: u128, // Total debt liquidated till now
    pub total_col_liq: u128,  // Total collateral liquidated till now
    pub tick: i32,            // Current tick to liquidate
    pub ratio: u128,          // Current ratio to liquidate
    pub tick_status: u8, // if 1 then it's a perfect tick, if 2 that means it's a liquidated tick
    pub ref_tick: i32,   // ref tick to liquidate
    pub ref_ratio: u128, // ratio at ref tick
    pub ref_tick_status: u8, // if 1 then it's a perfect tick, if 2 that means it's a liquidated tick, if 3 that means it's a liquidation threshold
}

impl CurrentLiquidity {
    pub fn is_perfect_tick(&self) -> bool {
        self.tick_status == 1
    }

    pub fn is_liquidated_tick(&self) -> bool {
        self.tick_status == 2
    }

    pub fn is_ref_tick_perfect(&self) -> bool {
        self.ref_tick_status == 1
    }

    pub fn is_ref_tick_liquidation_threshold(&self) -> bool {
        self.ref_tick_status == 3
    }

    pub fn is_ref_tick_liquidated(&self) -> bool {
        self.ref_tick_status == 2
    }

    fn get_normal_debt_liq(&self, borrow_ex_price: u128) -> Result<u128> {
        Ok(self
            .total_debt_liq
            .safe_mul(borrow_ex_price)?
            .safe_div_ceil(EXCHANGE_PRICES_PRECISION)?)
    }

    fn get_normal_col_liq(&self, supply_ex_price: u128) -> Result<u128> {
        Ok(self
            .total_col_liq
            .safe_mul(supply_ex_price)?
            .safe_div(EXCHANGE_PRICES_PRECISION)?)
    }

    pub fn get_actual_amounts(
        &self,
        borrow_ex_price: u128,
        supply_ex_price: u128,
        debt_amount: u128,
        vault_id: u16,
    ) -> Result<(u128, u128)> {
        let mut actual_debt_amt = self.get_normal_debt_liq(borrow_ex_price)?;
        let mut actual_col_amt = self.get_normal_col_liq(supply_ex_price)?;

        if actual_debt_amt > debt_amount {
            // Only revert if the difference is unexpectedly large (> MAX_LIQUIDATION_ROUNDING_DIFF).
            // For small differences, adjust proportionally to maintain liquidation flow.
            let diff = actual_debt_amt.safe_sub(debt_amount)?;

            if diff > MAX_LIQUIDATION_ROUNDING_DIFF {
                emit!(LogLiquidationRoundingDiff {
                    vault_id,
                    actual_debt_amt: actual_debt_amt.cast()?,
                    debt_amount: debt_amount.cast()?,
                    diff: diff.cast()?,
                });

                return Err(error!(ErrorCodes::VaultInvalidLiquidation));
            }

            actual_col_amt = actual_col_amt
                .safe_mul(debt_amount)?
                .safe_div(actual_debt_amt)?;

            actual_debt_amt = debt_amount;
        }

        if actual_debt_amt == 0 {
            return Err(error!(ErrorCodes::VaultInvalidLiquidation));
        }

        Ok((actual_debt_amt, actual_col_amt))
    }

    fn reduce_debt(&mut self, debt_liquidated: u128) -> Result<()> {
        self.debt = self.debt.safe_sub(debt_liquidated)?;
        Ok(())
    }

    pub fn reduce_debt_remaining(&mut self, debt_liquidated: u128) -> Result<()> {
        self.debt_remaining = self.debt_remaining.safe_sub(debt_liquidated)?;
        Ok(())
    }

    fn reduce_col(&mut self, col_liquidated: u128) -> Result<()> {
        self.col = self.col.safe_sub(col_liquidated)?;
        Ok(())
    }

    fn increase_total_debt_liq(&mut self, debt_liquidated: u128) -> Result<()> {
        self.total_debt_liq = self.total_debt_liq.safe_add(debt_liquidated)?;
        Ok(())
    }

    fn increase_total_col_liq(&mut self, col_liquidated: u128) -> Result<()> {
        self.total_col_liq = self.total_col_liq.safe_add(col_liquidated)?;
        Ok(())
    }

    pub fn update_totals(&mut self, debt_liquidated: u128, col_liquidated: u128) -> Result<()> {
        self.reduce_col(col_liquidated)?;
        self.increase_total_col_liq(col_liquidated)?;
        self.increase_total_debt_liq(debt_liquidated)?;
        self.reduce_debt(debt_liquidated)?;

        Ok(())
    }

    pub fn get_debt_factor(&self, debt_liquidated: u128) -> Result<u128> {
        // debtFactor = debtFactor * (liquidatableDebt - debtLiquidated) / liquidatableDebt
        // -> debtFactor * leftOverDebt / liquidatableDebt

        let debt_factor = TWO_POWER_64
            .safe_mul(self.debt.safe_sub(debt_liquidated)?.cast()?)?
            .safe_div(self.debt.cast()?)?;

        Ok(debt_factor)
    }

    pub fn check_is_ref_partials_safe_for_tick(
        &self,
        existing_partials: u128,
        partials: u128,
    ) -> Result<()> {
        if existing_partials > 0 && existing_partials >= partials {
            // If refTick is liquidated tick and hence contains partials then checking that
            // current liquidation tick's partial should not be less than last liquidation refTick

            // Not sure if this is even possible to happen but adding checks to avoid it fully
            // If it reverts here then next liquidation on next block should go through fine
            return Err(error!(ErrorCodes::VaultLiquidationReverts));
        }

        Ok(())
    }

    pub fn get_final_ratio(&self, col_liquidated: u128, debt_liquidated: u128) -> Result<u128> {
        let final_ratio: u128 = self
            .debt
            .safe_sub(debt_liquidated)?
            .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
            .safe_div(self.col.safe_sub(col_liquidated)?)?;

        Ok(final_ratio)
    }

    pub fn update_next_iterations_with_ref(&mut self) {
        self.tick = self.ref_tick;
        self.tick_status = self.ref_tick_status;
        self.ratio = self.ref_ratio;
    }

    pub fn get_debt_from_ratios(&self) -> Result<u128> {
        Ok(self.ref_ratio.safe_mul(self.debt)?.safe_div(self.ratio)?)
    }

    pub fn get_col_from_ratios(&self, col_per_debt: u128) -> Result<u128> {
        // in u128, this would be risky to overflow, use u256 to be safe even though col_per_debt and ref_ratio
        // are inversely proportional to each other and this should not be necessary.
        // col_per_debt can be up to 4.62e25 at oracle precision 1e15
        // ratio can be up to 13002088133096036565414295

        Ok(safe_multiply_divide(
            col_per_debt,
            self.ref_ratio,
            TickMath::ZERO_TICK_SCALED_RATIO,
        )?)
    }

    pub fn get_debt_liquidated(&self, col_per_debt: u128) -> Result<u128> {
        // Calculate the numerator
        let numerator = self
            .debt
            .safe_sub(self.get_debt_from_ratios()?)?
            .safe_mul(10u128.pow(RATE_OUTPUT_DECIMALS))?;

        // Calculate the denominator
        let denominator = 10u128
            .pow(RATE_OUTPUT_DECIMALS)
            .safe_sub(self.get_col_from_ratios(col_per_debt)?)?;

        let debt_liquidated = numerator.safe_div(denominator)?;

        Ok(debt_liquidated)
    }
}

#[derive(Default)]
#[repr(C, packed)]
#[zero_copy]
pub struct TickMemoryVars {
    pub tick: i32,
    pub partials: u128,
}

impl TickMemoryVars {
    pub fn set_partials(&mut self, partials: u128) -> Result<()> {
        if partials == 0 {
            self.partials = 1;
        } else if partials >= X30 {
            self.partials = X30 - 1;
        } else {
            self.partials = partials;
        }

        Ok(())
    }

    pub fn set_tick(&mut self, tick: i32) {
        self.tick = tick;
    }
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Clone)]

pub struct InitVaultConfigParams {
    pub supply_rate_magnifier: i16, // 10000 = 100%, -10000 = -100%
    pub borrow_rate_magnifier: i16, // 10000 = 100%, -10000 = -100%
    pub collateral_factor: u16,
    pub liquidation_threshold: u16,
    pub liquidation_max_limit: u16,
    pub withdraw_gap: u16,
    pub liquidation_penalty: u16,
    pub borrow_fee: u16,
    pub rebalancer: Pubkey,
    pub liquidity_program: Pubkey,
    pub oracle_program: Pubkey,
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateCoreSettingsParams {
    pub supply_rate_magnifier: i16, // 10000 = 100%, -10000 = -100%
    pub borrow_rate_magnifier: i16, // 10000 = 100%, -10000 = -100%
    pub collateral_factor: u16,
    pub liquidation_threshold: u16,
    pub liquidation_max_limit: u16,
    pub withdraw_gap: u16,
    pub liquidation_penalty: u16,
    pub borrow_fee: u16,
}
