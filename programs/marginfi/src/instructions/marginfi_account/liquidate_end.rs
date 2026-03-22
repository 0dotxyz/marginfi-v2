use crate::{
    check,
    constants::{LIQUIDATION_BONUS_FEE_MINIMUM, LIQUIDATION_CLOSEOUT_DOLLAR_THRESHOLD},
    events::{DeleverageEvent, LiquidationReceiverEvent},
    ix_utils::{get_discrim_hash, validate_not_cpi_by_stack_height, Hashable},
    prelude::*,
    state::marginfi_account::{
        check_pre_liquidation_condition_and_get_account_health,
        clear_liquidation_price_cache_locks, get_health_components, HealthPriceMode,
        MarginfiAccountImpl, RiskRequirementType,
    },
};
use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{
        FeeState, HealthCache, LiquidationRecord, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
        ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_RECEIVERSHIP,
    },
};

/// (Permissionless) Ends a liquidation. Records the liquidation event in the user's record. Debits a
/// small flat sol fee to the global fee wallet.
/// * Fails if account is less healthy than it was at start
/// * Fails if liquidator earned too much profit (took more assets in exchange for repayment of
///   liabs that they were allowed)
pub fn end_liquidation<'info>(
    ctx: Context<'_, '_, 'info, 'info, EndLiquidation<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_mut()?;
    let fee_state = ctx.accounts.fee_state.load()?;

    validate_not_cpi_by_stack_height()?;

    let pre_assets_equity: I80F48 = liq_record.cache.asset_value_equity.into();

    // Note: We guarantee that liquidation improves health to at most 0, unless the account's net value is
    // below the threshold, then we can clear it regardless (or not).
    let ignore_healthy = pre_assets_equity < LIQUIDATION_CLOSEOUT_DOLLAR_THRESHOLD;

    let (seized, seized_f64, repaid, repaid_f64) = end_receivership(
        &mut marginfi_account,
        &mut liq_record,
        ctx.remaining_accounts,
        ignore_healthy,
    )?;

    // Liquidator's allowed fee cannot go lower than the bonus fee minimum
    let fee_state_max_fee: I80F48 = fee_state.liquidation_max_fee.into();
    let max_fee: I80F48 = I80F48::max(
        I80F48!(1) + fee_state_max_fee,
        I80F48!(1) + LIQUIDATION_BONUS_FEE_MINIMUM,
    );

    // Ensure seized asset‐value ≤ N% of repaid liability‐value, where N = 100% + the bonus fee
    if !ignore_healthy {
        check!(
            seized <= repaid * max_fee,
            MarginfiError::LiquidationPremiumTooHigh
        );
    }

    let liquidation_flat_sol_fee = fee_state.liquidation_flat_sol_fee;
    if liquidation_flat_sol_fee > 0 {
        anchor_lang::system_program::transfer(
            ctx.accounts.transfer_flat_fee(),
            liquidation_flat_sol_fee as u64,
        )?;
    }

    emit!(LiquidationReceiverEvent {
        marginfi_account: ctx.accounts.marginfi_account.key(),
        liquidation_receiver: ctx.accounts.liquidation_receiver.key(),
        liquidatee_assets_seized: seized_f64,
        liquidatee_liability_repaid: repaid_f64,
        lamps_fee_paid: liquidation_flat_sol_fee
    });

    Ok(())
}

/// (Permissioned) Ends a deleverage. Records the liquidation event in the user's record.
/// * Fails if account is less healthy than it was at start
/// Note: no fees taken.
pub fn end_deleverage<'info>(
    ctx: Context<'_, '_, 'info, 'info, EndDeleverage<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_mut()?;

    validate_not_cpi_by_stack_height()?;

    marginfi_account.unset_flag(ACCOUNT_IN_DELEVERAGE, false);
    let (_, seized_f64, _, repaid_f64) = end_receivership(
        &mut marginfi_account,
        &mut liq_record,
        ctx.remaining_accounts,
        true,
    )?;

    emit!(DeleverageEvent {
        marginfi_account: ctx.accounts.marginfi_account.key(),
        risk_admin: ctx.accounts.risk_admin.key(),
        deleveragee_assets_seized: seized_f64,
        deleveragee_liability_repaid: repaid_f64,
    });

    Ok(())
}

// Common logic for both liquidation and deleverage
pub fn end_receivership<'info>(
    marginfi_account: &mut MarginfiAccount,
    liq_record: &mut LiquidationRecord,
    remaining_ais: &'info [AccountInfo<'info>],
    ignore_healthy: bool,
) -> Result<(I80F48, f64, I80F48, f64)> {
    let pre_assets: I80F48 = liq_record.cache.asset_value_maint.into();
    let pre_liabs: I80F48 = liq_record.cache.liability_value_maint.into();
    let pre_assets_equity: I80F48 = liq_record.cache.asset_value_equity.into();
    let pre_liabs_equity: I80F48 = liq_record.cache.liability_value_equity.into();
    let pre_health: I80F48 = pre_assets - pre_liabs;

    let mut post_hc = HealthCache::zeroed();
    let (post_health, _post_assets, _post_liabs) =
        check_pre_liquidation_condition_and_get_account_health(
            marginfi_account,
            remaining_ais,
            None,
            &mut Some(&mut post_hc),
            HealthPriceMode::Cached,
            ignore_healthy,
        )?;
    let (post_assets_equity, post_liabilities_equity) = get_health_components(
        marginfi_account,
        remaining_ais,
        RiskRequirementType::Equity,
        &mut Some(&mut post_hc),
        HealthPriceMode::Cached,
    )?;

    clear_liquidation_price_cache_locks(marginfi_account, remaining_ais)?;
    marginfi_account.health_cache = post_hc;

    // health must not get worse
    if pre_health > post_health {
        msg!(
            "pre_health > post_health: {} >= {}",
            pre_health,
            post_health
        );
        return err!(MarginfiError::WorseHealthPostLiquidation);
    }

    let seized: I80F48 = pre_assets_equity - post_assets_equity;
    let repaid: I80F48 = pre_liabs_equity - post_liabilities_equity;

    // clear receivership
    marginfi_account.unset_flag(ACCOUNT_IN_RECEIVERSHIP, false);
    liq_record.liquidation_receiver = Pubkey::default();

    let seized_f64 = seized.to_num::<f64>();
    let repaid_f64 = repaid.to_num::<f64>();
    // record the entry in the liquidation record
    {
        // Rotate left to eject the oldest entry
        liq_record.entries.rotate_left(1);
        let entry = &mut liq_record.entries[3];

        entry.asset_amount_seized = seized_f64.to_le_bytes();
        entry.liab_amount_repaid = repaid_f64.to_le_bytes();
        entry.timestamp = Clock::get()?.unix_timestamp;
    }

    Ok((
        seized,
        seized.to_num::<f64>(),
        repaid,
        repaid.to_num::<f64>(),
    ))
}

#[derive(Accounts)]
pub struct EndLiquidation<'info> {
    // Note: Must be the first account for tx introspection, do not move.
    /// Account under liquidation
    #[account(
        mut,
        has_one = liquidation_record @ MarginfiError::InvalidLiquidationRecord,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && !acc.get_flag(ACCOUNT_IN_DELEVERAGE)
                && !acc.get_flag(ACCOUNT_IN_FLASHLOAN)
                && !acc.get_flag(ACCOUNT_DISABLED)
                && !acc.get_flag(ACCOUNT_IN_ORDER_EXECUTION)
        } @MarginfiError::UnexpectedLiquidationState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// The associated liquidation record PDA for the given `marginfi_account`
    #[account(
        mut,
        has_one = liquidation_receiver @ MarginfiError::InvalidLiquidationReceiver
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    // Note: mutable signer because it must pay the transfer fee
    #[account(mut)]
    pub liquidation_receiver: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_wallet @ MarginfiError::InvalidFeeWallet
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: The fee admin's native SOL wallet, validated against fee state
    #[account(mut)]
    pub global_fee_wallet: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> EndLiquidation<'info> {
    fn transfer_flat_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                // TODO Eventually, maybe support the fee being paid by a different account
                from: self.liquidation_receiver.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}

impl Hashable for EndLiquidation<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "end_liquidation")
    }
}

// Note: risk admin only
#[derive(Accounts)]
pub struct EndDeleverage<'info> {
    #[account(
        mut,
        has_one = liquidation_record,
        has_one = group,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && acc.get_flag(ACCOUNT_IN_DELEVERAGE)
                && !acc.get_flag(ACCOUNT_IN_FLASHLOAN)
                && !acc.get_flag(ACCOUNT_DISABLED)
                && !acc.get_flag(ACCOUNT_IN_ORDER_EXECUTION)
        } @MarginfiError::UnexpectedLiquidationState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        constraint = {
            let req = liquidation_record.load()?;
            req.liquidation_receiver == risk_admin.key()
        } @MarginfiError::Unauthorized
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    #[account(
        has_one = risk_admin,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub risk_admin: Signer<'info>,
}

impl Hashable for EndDeleverage<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "end_deleverage")
    }
}
