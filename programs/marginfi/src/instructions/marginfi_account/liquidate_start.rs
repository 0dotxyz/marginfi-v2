use crate::{
    check,
    constants::{ASSOCIATED_TOKEN_KEY, COMPUTE_PROGRAM_KEY, DRIFT_PROGRAM_ID, JUP_KEY, TITAN_KEY},
    ix_utils::{
        get_discrim_hash, load_and_validate_instructions, validate_ix_first, validate_ix_last,
        validate_ixes_exclusive, validate_not_cpi_by_stack_height, validate_not_cpi_with_sysvar,
        Hashable,
    },
    prelude::*,
    state::marginfi_account::{MarginfiAccountImpl, RiskEngine, RiskRequirementType},
};
use anchor_lang::{prelude::*, solana_program::sysvar};
use bytemuck::Zeroable;
use drift_mocks::drift::client::args as drift;
use kamino_mocks::kamino_lending::client::args as kamino;
use marginfi_type_crate::{
    constants::ix_discriminators,
    types::{
        HealthCache, LiquidationRecord, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
        ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_RECEIVERSHIP,
    },
};

/// (Permissionless) Begins a liquidation: snapshots the account and marks it in receivership. The
/// liquidator now has full control over the account until the end of the tx.
/// * Fails if account is healthy
/// * Fails if end liquidation instruction isn't at the end of this tx.
/// * Fails if the start liquidation instruction appears more than once in this tx.
/// * Fails if any mrgn instruction other than start, end, withdraw, or repay (or the equivalent
///   from a third party integration) are used within this tx.
pub fn start_liquidation<'info>(
    ctx: Context<'_, '_, 'info, 'info, StartLiquidation<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_mut()?;
    liq_record.liquidation_receiver = ctx.accounts.liquidation_receiver.key();
    start_receivership(
        &mut marginfi_account,
        &mut liq_record,
        ctx.remaining_accounts,
        false,
    )?;

    let sysvar = &ctx.accounts.instruction_sysvar;
    validate_instructions(
        sysvar,
        ctx.program_id,
        &ix_discriminators::START_LIQUIDATION,
        &ix_discriminators::END_LIQUIDATION,
    )
}

/// (Permissioned) Begins a forced deleverage: snapshots the account and marks it in receivership. The
/// risk admin now has full control over the account until the end of the tx.
/// * Fails if end deleverage instruction isn't at the end of this tx.
/// * Fails if the start deleverage instruction appears more than once in this tx.
/// * Fails if any mrgn instruction other than start, end, withdraw, or repay (or the equivalent
///   from a third party integration) are used within this tx.
pub fn start_deleverage<'info>(
    ctx: Context<'_, '_, 'info, 'info, StartDeleverage<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_mut()?;
    liq_record.liquidation_receiver = ctx.accounts.risk_admin.key();
    marginfi_account.set_flag(ACCOUNT_IN_DELEVERAGE, false);
    start_receivership(
        &mut marginfi_account,
        &mut liq_record,
        ctx.remaining_accounts,
        true,
    )?;

    let sysvar = &ctx.accounts.instruction_sysvar;
    validate_instructions(
        sysvar,
        ctx.program_id,
        &ix_discriminators::START_DELEVERAGE,
        &ix_discriminators::END_DELEVERAGE,
    )
}

// Common logic for both liquidation and deleverage
pub fn start_receivership<'info>(
    marginfi_account: &mut MarginfiAccount,
    liq_record: &mut LiquidationRecord,
    remaining_ais: &'info [AccountInfo<'info>],
    ignore_healthy: bool,
) -> MarginfiResult {
    // Note: the receiver can use the health cache state after this ix concludes to plan their
    // liquidation/deleverage strategy.
    let mut health_cache = HealthCache::zeroed();
    let risk_engine = RiskEngine::new(marginfi_account, remaining_ais)?;

    let (_pre_health, assets, liabs) = risk_engine
        .check_pre_liquidation_condition_and_get_account_health(
            None,
            &mut Some(&mut health_cache),
            ignore_healthy,
        )?;
    let (assets_equity, liabs_equity) = risk_engine
        .get_account_health_components(RiskRequirementType::Equity, &mut Some(&mut health_cache))?;
    marginfi_account.health_cache = health_cache;
    marginfi_account.set_flag(ACCOUNT_IN_RECEIVERSHIP, false);

    // Snapshot values to use in later checks
    liq_record.cache.asset_value_maint = assets.into();
    liq_record.cache.liability_value_maint = liabs.into();
    liq_record.cache.asset_value_equity = assets_equity.into();
    liq_record.cache.liability_value_equity = liabs_equity.into();

    Ok(())
}

// Common introspection logic for both liquidation and deleverage
pub fn validate_instructions(
    sysvar: &AccountInfo<'_>,
    program_id: &Pubkey,
    start_ix: &[u8],
    end_ix: &[u8],
) -> MarginfiResult {
    let allowed_program_ids = &[
        COMPUTE_PROGRAM_KEY,
        id_crate::ID,
        kamino_mocks::kamino_lending::ID,
        DRIFT_PROGRAM_ID,
        JUP_KEY,
        TITAN_KEY,
        ASSOCIATED_TOKEN_KEY,
        DRIFT_PROGRAM_ID,
    ];
    let ixes = load_and_validate_instructions(sysvar, Some(allowed_program_ids))?;
    validate_ix_first(
        &ixes,
        program_id,
        start_ix,
        &[
            (
                kamino_mocks::kamino_lending::ID,
                kamino::RefreshReserve::DISCRIMINATOR,
            ),
            (
                kamino_mocks::kamino_lending::ID,
                kamino::RefreshObligation::DISCRIMINATOR,
            ),
            (id_crate::ID, &ix_discriminators::INIT_LIQUIDATION_RECORD),
            (
                DRIFT_PROGRAM_ID,
                drift::UpdateSpotMarketCumulativeInterest::DISCRIMINATOR,
            ),
        ],
    )?;
    validate_ix_last(&ixes, program_id, end_ix)?;
    // Note: this only validates top-level instructions, all other instructions can still appear
    // inside a CPI. This list essentially bans any ix that's already banned inside CPI (e.g.
    // flashloan), but has limited utility otherwise.
    validate_ixes_exclusive(
        &ixes,
        program_id,
        &[
            start_ix,
            end_ix,
            &ix_discriminators::INIT_LIQUIDATION_RECORD,
            &ix_discriminators::LENDING_ACCOUNT_WITHDRAW,
            &ix_discriminators::LENDING_ACCOUNT_REPAY,
            &ix_discriminators::KAMINO_WITHDRAW,
            &ix_discriminators::DRIFT_WITHDRAW,
            // TODO add withdraw/repay from integrator as they are added to the program. Also
            // remember to add a test to ix_utils to validate you added the correct hash.

            // Note: At some point we may allow the liquidator to claim emissions too. Since we
            // currently don't allow this, liquidators can never fully close out an account that
            // has emissions active. This is not a priority since we are considering deprecating
            // the emissions feature in late 2025 and moving to a fully off-chain emissions
            // system anyways.
            // * &ix_discriminators::LENDING_SETTLE_EMISSIONS,
            // * &ix_discriminators::LENDING_WITHDRAW_EMISSIONS,
        ],
    )?;
    validate_not_cpi_by_stack_height()?;
    let start_ix = validate_not_cpi_with_sysvar(sysvar)?;
    // Sanity check: we have already verified start/end are first/last.
    check!(start_ix < ixes.len() - 1, MarginfiError::StartNotFirst);

    Ok(())
}

#[derive(Accounts)]
pub struct StartLiquidation<'info> {
    /// Account under liquidation
    #[account(
        mut,
        has_one = liquidation_record @ MarginfiError::InvalidLiquidationRecord,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && !acc.get_flag(ACCOUNT_IN_FLASHLOAN)
                && !acc.get_flag(ACCOUNT_DISABLED)
        } @MarginfiError::UnexpectedLiquidationState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// The associated liquidation record PDA for the given `marginfi_account`
    #[account(mut)]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    /// This account will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    ///
    /// CHECK: no checks whatsoever, liquidator decides this without restriction
    pub liquidation_receiver: UncheckedAccount<'info>,

    /// CHECK: validated against known hard-coded sysvar key
    #[account(
        address = sysvar::instructions::id()
    )]
    pub instruction_sysvar: AccountInfo<'info>,
}

impl Hashable for StartLiquidation<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "start_liquidation")
    }
}

// Note: risk admin only
#[derive(Accounts)]
pub struct StartDeleverage<'info> {
    /// Account to deleverage
    #[account(
        mut,
        has_one = liquidation_record,
        has_one = group,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && !acc.get_flag(ACCOUNT_IN_FLASHLOAN)
                && !acc.get_flag(ACCOUNT_DISABLED)
        } @MarginfiError::UnexpectedLiquidationState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// The associated liquidation record PDA for the given `marginfi_account`
    #[account(mut)]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    #[account(
        has_one = risk_admin
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// The risk admin will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    pub risk_admin: Signer<'info>,

    /// CHECK: validated against known hard-coded sysvar key
    #[account(
        address = sysvar::instructions::id()
    )]
    pub instruction_sysvar: AccountInfo<'info>,
}

impl Hashable for StartDeleverage<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "start_deleverage")
    }
}
