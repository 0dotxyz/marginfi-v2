// TODO deprecate in 1.8
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    centi_to_u32, make_points, milli_to_u32, Bank, RatePoint, INTEREST_CURVE_SEVEN_POINT,
};

use crate::{state::bank_config::BankConfigImpl, utils::wrapped_i80f48_to_f64};

#[derive(Accounts)]
pub struct MigrateCurve<'info> {
    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,
}

pub fn migrate_curve(ctx: Context<MigrateCurve>) -> Result<()> {
    {
        msg!("Migrating bank {:?}", &ctx.accounts.bank.key());
        let bank = ctx.accounts.bank.load()?;
        bank.config.validate()?;
        let irc = &bank.config.interest_rate_config;
        msg!(
            "Rate was optimal: {:?} max: {:?} plat: {:?}",
            wrapped_i80f48_to_f64(irc.optimal_utilization_rate),
            wrapped_i80f48_to_f64(irc.max_interest_rate),
            wrapped_i80f48_to_f64(irc.plateau_interest_rate)
        );
    }

    let mut bank = ctx.accounts.bank.load_mut()?;
    {
        let irc = &mut bank.config.interest_rate_config;

        if irc.curve_type == INTEREST_CURVE_SEVEN_POINT {
            msg!("Already migrated, doing nothing");
            return Ok(());
        }

        irc.zero_util_rate = 0;

        let hundred_rate: I80F48 = irc.max_interest_rate.into();
        irc.hundred_util_rate = milli_to_u32(hundred_rate);

        let util: I80F48 = irc.optimal_utilization_rate.into();
        let rate: I80F48 = irc.plateau_interest_rate.into();
        let point = RatePoint {
            util: centi_to_u32(util),
            rate: milli_to_u32(rate),
        };
        irc.points = make_points(&[point]);
        irc.curve_type = INTEREST_CURVE_SEVEN_POINT;

        // Zero out the old fields so they can be recycled
        irc.plateau_interest_rate = I80F48::ZERO.into();
        irc.max_interest_rate = I80F48::ZERO.into();
        irc.optimal_utilization_rate = I80F48::ZERO.into();

        msg!(
            "Migration complete. Rate at 0: {:?} points: {:?} rate at 100: {:?}",
            irc.zero_util_rate,
            irc.points,
            irc.hundred_util_rate
        );
    }

    bank.config.validate()?;

    Ok(())
}
