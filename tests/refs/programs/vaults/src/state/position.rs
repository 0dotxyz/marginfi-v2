use anchor_lang::prelude::*;

use crate::{
    constants::{EXCHANGE_PRICES_PRECISION, FOUR_DECIMALS},
    errors::ErrorCodes,
    state::{structs::OperateMemoryVars, COLD_TICK},
};

use library::math::{casting::*, safe_math::*, tick::TickMath};

/// Position data structure
#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct Position {
    pub vault_id: u16, // Vault ID

    pub nft_id: u32,           // Position index
    pub position_mint: Pubkey, // NFT address

    pub is_supply_only_position: u8, // Position type (0 => borrow position; 1 => supply position)
    pub tick: i32,                   // User's tick
    pub tick_id: u32,                // User's tick's id
    pub supply_amount: u64,          // User's supply amount
    pub dust_debt_amount: u64,       // User's dust debt amount
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct VaultExchangePrices {
    pub supply_ex_price: u128,
    pub borrow_ex_price: u128,
    pub borrow_fee: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AmountInfo {
    pub new_col: i128,
    pub new_debt: i128,
    pub col_raw: u128,
    pub debt_raw: u128,
}

impl Position {
    pub fn get_supply_amount(&self) -> Result<u128> {
        Ok(self.supply_amount.cast()?)
    }

    pub fn set_supply_amount(&mut self, amount: u128) -> Result<()> {
        self.supply_amount = amount.cast()?;
        Ok(())
    }

    pub fn get_dust_debt_amount(&self) -> Result<u128> {
        Ok(self.dust_debt_amount.cast()?)
    }

    pub fn set_dust_debt_amount(&mut self, amount: u128) -> Result<()> {
        self.dust_debt_amount = amount.cast()?;
        Ok(())
    }

    pub fn is_supply_only_position(&self) -> bool {
        self.is_supply_only_position == 1
    }

    pub fn set_is_supply_only_position(&mut self, is_supply_only_position: bool) -> Result<()> {
        self.is_supply_only_position = if is_supply_only_position { 1 } else { 0 };
        Ok(())
    }

    pub fn get_position_info(&self) -> Result<(u128, u128, u128, i32, u32)> {
        let col_raw: u128 = self.get_supply_amount()?;
        let dust_debt_raw: u128 = self.get_dust_debt_amount()?;

        let (tick, tick_id) = if self.is_supply_only_position() {
            (COLD_TICK, 0)
        } else {
            (self.tick, self.tick_id)
        };

        let debt_raw: u128 = if tick > COLD_TICK {
            // Round up the collateral for debt calculation
            let collateral_for_debt_calc: u128 = col_raw.safe_add(1)?;

            // Fetch current debt based on tick ratio and collateral
            // rounding up the debt
            TickMath::get_ratio_at_tick(tick)?
                .safe_mul(collateral_for_debt_calc)?
                .safe_shr(TickMath::SHIFT)?
                .safe_add(1)?
        } else {
            0
        };

        Ok((col_raw, dust_debt_raw, debt_raw, tick, tick_id))
    }

    pub fn get_new_position_info(
        &self,
        mut info: Box<AmountInfo>,
        exchange_prices: Box<VaultExchangePrices>,
    ) -> Result<(i128, i128, u128, u128)> {
        // supply or withdraw
        if info.new_col > 0 {
            // supply new col, rounding down
            info.col_raw = info.col_raw.safe_add(
                info.new_col
                    .cast::<u128>()?
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div(exchange_prices.supply_ex_price)?,
            )?;

            // final user's collateral should not be above 2**64
            if info.col_raw > u64::MAX.cast()? {
                return Err(error!(ErrorCodes::VaultUserCollateralDebtExceed));
            }
        } else if info.new_col < 0 {
            // if withdraw equals i128::MIN then max withdraw
            if info.new_col > i128::MIN {
                // partial withdraw, rounding up removing extra wei from collateral
                let withdraw_amount: u128 = (info.new_col.abs())
                    .cast::<u128>()?
                    .safe_mul(EXCHANGE_PRICES_PRECISION)?
                    .safe_div_ceil(exchange_prices.supply_ex_price)?;

                if withdraw_amount > info.col_raw {
                    return Err(error!(ErrorCodes::VaultExcessCollateralWithdrawal));
                }

                info.col_raw -= withdraw_amount;
            } else if info.new_col == i128::MIN {
                // max withdraw, rounding up:
                // adding +1 to negative withdrawAmount newCol_ for safe rounding (reducing withdraw)
                let withdraw_amount: i128 = info
                    .col_raw
                    .safe_mul(exchange_prices.supply_ex_price)?
                    .safe_div(EXCHANGE_PRICES_PRECISION)?
                    .cast::<i128>()?
                    .safe_mul(-1)?
                    .safe_add(1)?;

                info.new_col = withdraw_amount;
                info.col_raw = 0;
            } else {
                return Err(error!(ErrorCodes::VaultUserCollateralDebtExceed));
            }
        }

        // borrow or payback
        if info.new_debt > 0 {
            // borrow new debt, rounding up adding extra wei in debt
            let borrow_amount: u128 = info
                .new_debt
                .cast::<u128>()?
                .safe_mul(EXCHANGE_PRICES_PRECISION)?
                .safe_div_ceil(exchange_prices.borrow_ex_price)?;

            let borrow_amount_with_fee: u128 = borrow_amount.safe_add(
                borrow_amount
                    .safe_mul(exchange_prices.borrow_fee.cast()?)?
                    .safe_div_ceil(FOUR_DECIMALS)?,
            )?;

            // if borrow fee is 0 then it'll become borrow_amount + 0.
            // Only adding fee in debt_raw and not in new_debt as new_debt is debt that needs to be borrowed from Liquidity
            // as we have added fee in debtRaw hence it will get added in user's position & vault's total borrow.
            // It can be collected with rebalance function.
            info.debt_raw = info.debt_raw.safe_add(borrow_amount_with_fee)?;

            if info.debt_raw > u64::MAX.cast()? {
                return Err(error!(ErrorCodes::VaultUserCollateralDebtExceed));
            }
        } else if info.new_debt < 0 {
            // if payback equals i128::MIN then max payback
            if info.new_debt > i128::MIN {
                // partial payback.
                // safe rounding up negative amount to rounding reduce payback
                let payback_amount: u128 = (info.new_debt.abs())
                    .cast::<u128>()?
                    .safe_mul(EXCHANGE_PRICES_PRECISION.cast()?)?
                    .safe_div(exchange_prices.borrow_ex_price.cast()?)?
                    .safe_sub(1)?;

                if payback_amount > info.debt_raw.cast()? {
                    return Err(error!(ErrorCodes::VaultExcessDebtPayback));
                }

                info.debt_raw -= payback_amount;
            } else if info.new_debt == i128::MIN {
                // max payback, rounding up amount that will be transferred in to pay back full debt:
                // subtracting -1 of negative debtAmount newDebt_ for safe rounding (increasing payback)
                let payback_amount: i128 = (info.debt_raw)
                    .safe_mul(exchange_prices.borrow_ex_price)?
                    .safe_div_ceil(EXCHANGE_PRICES_PRECISION)?
                    .cast::<i128>()?
                    .safe_mul(-1)?;

                info.new_debt = payback_amount;
                info.debt_raw = 0;
            } else {
                return Err(error!(ErrorCodes::VaultUserCollateralDebtExceed));
            }
        }

        Ok((info.new_col, info.new_debt, info.col_raw, info.debt_raw))
    }

    pub fn update_position_after_operate(&mut self, memory_vars: &OperateMemoryVars) -> Result<()> {
        // Update user position
        if memory_vars.tick == COLD_TICK {
            self.set_is_supply_only_position(true)?;
        } else {
            self.set_is_supply_only_position(false)?;
        }

        self.tick = memory_vars.tick;

        self.tick_id = memory_vars.tick_id;
        self.set_supply_amount(memory_vars.col_raw)?;
        self.set_dust_debt_amount(memory_vars.dust_debt_raw)?;

        Ok(())
    }
}
