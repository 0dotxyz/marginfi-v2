use crate::events::{GroupEventHeader, MarginfiGroupConfigureEvent};
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    basis_to_u32, same_asset_leverage_to_u32, u32_to_same_asset_leverage, MarginfiGroup,
    WrappedI80F48,
};

/// Validate and apply an optional emode leverage value. If `Some`, validates it is in [1, 100] and
/// writes the basis-point encoding. If `None`, leaves the value as is.
fn validate_and_apply_emode_leverage(
    new_value: Option<WrappedI80F48>,
    current: &mut u32,
) -> MarginfiResult {
    if let Some(wrapped) = new_value {
        let leverage: I80F48 = wrapped.into();
        if leverage < I80F48::ONE {
            msg!("emode leverage {} must be >= 1", leverage);
            return Err(MarginfiError::BadEmodeConfig.into());
        }
        if leverage > I80F48::from_num(100) {
            msg!("emode leverage {} must be <= 100", leverage);
            return Err(MarginfiError::BadEmodeConfig.into());
        }
        *current = basis_to_u32(leverage);
    }
    Ok(())
}

/// Validate and apply an optional same-asset emode leverage value.
/// If `Some`, validates it is in [1, 1000] and writes the encoded `u32` value.
/// If `None`, leaves the stored value as is.
fn validate_and_apply_same_asset_emode_leverage(
    new_value: Option<WrappedI80F48>,
    current: &mut u32,
) -> MarginfiResult {
    if let Some(wrapped) = new_value {
        let leverage: I80F48 = wrapped.into();
        if leverage < I80F48::ONE {
            msg!("same-asset emode leverage {} must be >= 1", leverage);
            return Err(MarginfiError::BadEmodeConfig.into());
        }
        if leverage > I80F48::from_num(1000) {
            msg!("same-asset emode leverage {} must be <= 1000", leverage);
            return Err(MarginfiError::BadEmodeConfig.into());
        }
        *current = same_asset_leverage_to_u32(leverage);
    }
    Ok(())
}

/// Configure margin group.
///
/// Note: not even the group admin can configure `PROGRAM_FEES_ENABLED`, only the program admin can
/// with `configure_group_fee`
///
/// Admin only
pub fn configure(
    ctx: Context<MarginfiGroupConfigure>,
    new_admin: Option<Pubkey>,
    new_emode_admin: Option<Pubkey>,
    new_curve_admin: Option<Pubkey>,
    new_limit_admin: Option<Pubkey>,
    new_flow_admin: Option<Pubkey>,
    new_emissions_admin: Option<Pubkey>,
    new_metadata_admin: Option<Pubkey>,
    new_risk_admin: Option<Pubkey>,
    emode_max_init_leverage: Option<WrappedI80F48>,
    emode_max_maint_leverage: Option<WrappedI80F48>,
    same_asset_emode_init_leverage: Option<WrappedI80F48>,
    same_asset_emode_maint_leverage: Option<WrappedI80F48>,
) -> MarginfiResult {
    let marginfi_group = &mut ctx.accounts.marginfi_group.load_mut()?;
    if let Some(new_admin) = new_admin {
        marginfi_group.update_admin(new_admin);
    }
    if let Some(new_emode_admin) = new_emode_admin {
        marginfi_group.update_emode_admin(new_emode_admin);
    }
    if let Some(new_curve_admin) = new_curve_admin {
        marginfi_group.update_curve_admin(new_curve_admin);
    }
    if let Some(new_limit_admin) = new_limit_admin {
        marginfi_group.update_limit_admin(new_limit_admin);
    }
    if let Some(new_flow_admin) = new_flow_admin {
        marginfi_group.update_flow_admin(new_flow_admin);
    }
    if let Some(new_emissions_admin) = new_emissions_admin {
        marginfi_group.update_emissions_admin(new_emissions_admin);
    }
    if let Some(new_metadata_admin) = new_metadata_admin {
        marginfi_group.update_metadata_admin(new_metadata_admin);
    }
    if let Some(new_risk_admin) = new_risk_admin {
        marginfi_group.update_risk_admin(new_risk_admin);
    }

    validate_and_apply_emode_leverage(
        emode_max_init_leverage,
        &mut marginfi_group.emode_max_init_leverage,
    )?;
    validate_and_apply_emode_leverage(
        emode_max_maint_leverage,
        &mut marginfi_group.emode_max_maint_leverage,
    )?;

    // Validate that init < maint
    if marginfi_group.emode_max_init_leverage >= marginfi_group.emode_max_maint_leverage {
        msg!(
            "emode init leverage ({}) must be < maint leverage ({})",
            marginfi_group.emode_max_init_leverage,
            marginfi_group.emode_max_maint_leverage
        );
        return Err(MarginfiError::BadEmodeConfig.into());
    }

    validate_and_apply_same_asset_emode_leverage(
        same_asset_emode_init_leverage,
        &mut marginfi_group.same_asset_emode_init_leverage,
    )?;
    validate_and_apply_same_asset_emode_leverage(
        same_asset_emode_maint_leverage,
        &mut marginfi_group.same_asset_emode_maint_leverage,
    )?;

    let same_asset_init_leverage =
        u32_to_same_asset_leverage(marginfi_group.same_asset_emode_init_leverage);
    let same_asset_maint_leverage =
        u32_to_same_asset_leverage(marginfi_group.same_asset_emode_maint_leverage);

    let same_asset_disabled =
        same_asset_init_leverage <= I80F48::ONE && same_asset_maint_leverage <= I80F48::ONE;

    if !same_asset_disabled && same_asset_init_leverage >= same_asset_maint_leverage {
        msg!(
            "same-asset emode init leverage ({}) must be < maint leverage ({})",
            same_asset_init_leverage,
            same_asset_maint_leverage
        );
        return Err(MarginfiError::BadEmodeConfig.into());
    }
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
