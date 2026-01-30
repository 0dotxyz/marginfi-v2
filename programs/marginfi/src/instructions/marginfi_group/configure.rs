use crate::events::{GroupEventHeader, MarginfiGroupConfigureEvent};
use crate::state::emode::{DEFAULT_INIT_MAX_EMODE_LEVERAGE, DEFAULT_MAINT_MAX_EMODE_LEVERAGE};
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{basis_to_u32, MarginfiGroup, WrappedI80F48};

/// Configure margin group.
///
/// Note: not even the group admin can configure `PROGRAM_FEES_ENABLED`, only the program admin can
/// with `configure_group_fee`
///
/// Admin only
pub fn configure(
    ctx: Context<MarginfiGroupConfigure>,
    new_admin: Pubkey,
    new_emode_admin: Pubkey,
    new_curve_admin: Pubkey,
    new_limit_admin: Pubkey,
    new_emissions_admin: Pubkey,
    new_metadata_admin: Pubkey,
    new_risk_admin: Pubkey,
    emode_max_init_leverage: Option<WrappedI80F48>,
    emode_max_maint_leverage: Option<WrappedI80F48>,
) -> MarginfiResult {
    let marginfi_group = &mut ctx.accounts.marginfi_group.load_mut()?;

    marginfi_group.update_admin(new_admin);
    marginfi_group.update_emode_admin(new_emode_admin);
    marginfi_group.update_curve_admin(new_curve_admin);
    marginfi_group.update_limit_admin(new_limit_admin);
    marginfi_group.update_emissions_admin(new_emissions_admin);
    marginfi_group.update_metadata_admin(new_metadata_admin);
    marginfi_group.update_risk_admin(new_risk_admin);

    // Update emode max leverage - if None, set to default max emode leverage
    let max_init_leverage =
        emode_max_init_leverage.unwrap_or_else(|| DEFAULT_INIT_MAX_EMODE_LEVERAGE.into());
    let max_maint_leverage =
        emode_max_maint_leverage.unwrap_or_else(|| DEFAULT_MAINT_MAX_EMODE_LEVERAGE.into());

    let init_leverage_value: I80F48 = max_init_leverage.into();
    let maint_leverage_value: I80F48 = max_maint_leverage.into();

    // Validate that (1 <= leverage <= 100) for both
    require!(
        init_leverage_value >= I80F48::ONE,
        MarginfiError::BadEmodeConfig
    );
    require!(
        init_leverage_value <= I80F48::from_num(100),
        MarginfiError::BadEmodeConfig
    );

    require!(
        maint_leverage_value >= I80F48::ONE,
        MarginfiError::BadEmodeConfig
    );
    require!(
        maint_leverage_value <= I80F48::from_num(100),
        MarginfiError::BadEmodeConfig
    );

    // Validate that init < maint
    require!(
        init_leverage_value < maint_leverage_value,
        MarginfiError::BadEmodeConfig
    );

    marginfi_group.emode_max_init_leverage = basis_to_u32(init_leverage_value);
    marginfi_group.emode_max_maint_leverage = basis_to_u32(maint_leverage_value);

    // The fuzzer should ignore this because the "Clock" mock sysvar doesn't load until after the
    // group is init. Eventually we might fix the fuzzer to load the clock first...
    #[cfg(not(feature = "client"))]
    {
        let clock = Clock::get()?;
        marginfi_group.fee_state_cache.last_update = clock.unix_timestamp;
    }

    msg!("flags set to: {:?}", marginfi_group.group_flags);

    emit!(MarginfiGroupConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
        admin: new_admin,
        flags: marginfi_group.group_flags
    });

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiGroupConfigure<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}
