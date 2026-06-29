/// Utility helpers for timelocked operations.
use crate::prelude::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiGroup, TimelockedOperation};

/// Close a timelocked operation account after execution or cancellation.
pub fn close_timelocked_account<'info>(
    account: &AccountLoader<'info, TimelockedOperation>,
    recipient: &AccountInfo<'info>,
) -> MarginfiResult {
    let account_info = account.to_account_info();

    // Transfer all lamports to recipient
    let lamports = account_info.lamports();
    **account_info.lamports.borrow_mut() = 0;
    **recipient.lamports.borrow_mut() += lamports;

    // Zero the entire account data to prevent re-initialization
    let mut data = account_info.data.borrow_mut();
    data.fill(0);

    Ok(())
}

/// Verify and extract risk tier from a u8 discriminator.
pub fn parse_risk_tier(tier_u8: u8) -> MarginfiResult<marginfi_type_crate::types::RiskTier> {
    use std::convert::TryFrom;
    marginfi_type_crate::types::RiskTier::try_from(tier_u8)
        .map_err(|_| MarginfiError::InvalidConfig.into())
}

/// Verify bank config matches what was stored in the timelocked operation.
/// Verifies critical fields: limits, weights, risk tier, asset tag, init limit.
pub fn assert_bank_config_matches_op(
    deposit_limit: u64,
    borrow_limit: u64,
    risk_tier: marginfi_type_crate::types::RiskTier,
    asset_tag: u8,
    total_asset_value_init_limit: u64,
    asset_weight_init: &marginfi_type_crate::types::WrappedI80F48,
    asset_weight_maint: &marginfi_type_crate::types::WrappedI80F48,
    liability_weight_init: &marginfi_type_crate::types::WrappedI80F48,
    liability_weight_maint: &marginfi_type_crate::types::WrappedI80F48,
    op: &TimelockedOperation,
) -> MarginfiResult {
    // Verify basic limits
    require!(
        deposit_limit == op.data.value_u64_1,
        MarginfiError::InvalidConfig
    );
    require!(
        borrow_limit == op.data.value_u64_2,
        MarginfiError::InvalidConfig
    );
    use std::convert::From;
    let risk_tier_u8: u8 = risk_tier.into();
    let expected_tier_tag = (risk_tier_u8 as u64) | ((asset_tag as u64) << 8);
    require!(
        op.data.value_u64_3 == expected_tier_tag,
        MarginfiError::InvalidConfig
    );
    require!(
        total_asset_value_init_limit == op.data.value_u64_4,
        MarginfiError::InvalidConfig
    );

    // Verify collateral weights (most critical for borrowing power)
    require!(
        &op.data.extra[0..16] == &asset_weight_init.value[..],
        MarginfiError::InvalidConfig
    );
    require!(
        &op.data.extra[16..32] == &asset_weight_maint.value[..],
        MarginfiError::InvalidConfig
    );

    // Verify liability weights
    require!(
        &op.data.extra_extended[0..16] == &liability_weight_init.value[..],
        MarginfiError::InvalidConfig
    );
    require!(
        &op.data.extra_extended[16..32] == &liability_weight_maint.value[..],
        MarginfiError::InvalidConfig
    );

    Ok(())
}

/// Initialize common fields for a timelocked operation.
pub fn init_timelocked_operation(
    timelocked_op: &mut TimelockedOperation,
    group: Pubkey,
    admin: Pubkey,
    op_type: u8,
    bank_mint: Pubkey,
    bump: u8,
    delay_seconds: u64,
    now: i64,
) -> MarginfiResult {
    let execution_available_at = now
        .checked_add(delay_seconds as i64)
        .ok_or(MarginfiError::InvalidConfig)?;

    timelocked_op.group = group;
    timelocked_op.created_at = now;
    timelocked_op.execution_available_at = execution_available_at;
    timelocked_op.admin = admin;
    timelocked_op.operation_type = op_type;
    timelocked_op.executed = 0;
    timelocked_op.validated = 0;
    timelocked_op.bank_mint = bank_mint;
    timelocked_op.bump = bump;
    Ok(())
}

/// Verify that a signer is authorized to execute or cancel this operation.
pub fn assert_signer_authorized(
    timelocked_op: &TimelockedOperation,
    signer: &Pubkey,
    group_admin: &Pubkey,
) -> MarginfiResult {
    require!(
        signer == &timelocked_op.admin || signer == group_admin,
        MarginfiError::Unauthorized
    );
    Ok(())
}

/// Verify this operation is ready for execution.
pub fn assert_ready_for_execution(
    timelocked_op: &TimelockedOperation,
    expected_group: &Pubkey,
    expected_op_type: u8,
    now: i64,
) -> MarginfiResult {
    require!(timelocked_op.executed == 0, MarginfiError::InvalidConfig);
    require!(
        timelocked_op.group == *expected_group,
        MarginfiError::InvalidConfig
    );
    require!(
        timelocked_op.operation_type == expected_op_type,
        MarginfiError::InvalidConfig
    );
    require!(
        now >= timelocked_op.execution_available_at,
        MarginfiError::InvalidConfig
    );
    Ok(())
}

/// Verify the bank matches what was scheduled in this operation.
pub fn assert_bank_matches(
    timelocked_op: &TimelockedOperation,
    bank_mint: &Pubkey,
) -> MarginfiResult {
    require!(
        timelocked_op.bank_mint == *bank_mint,
        MarginfiError::InvalidConfig
    );
    Ok(())
}

/// Verify that the timelocked admin feature is configured and the signer is authorized.
pub fn assert_timelocked_admin_authorized(
    marginfi_group: &MarginfiGroup,
    signer: &Pubkey,
) -> MarginfiResult {
    require!(
        marginfi_group.has_timelocked_admin(),
        MarginfiError::Unauthorized
    );
    require!(
        signer == &marginfi_group.timelocked_admin,
        MarginfiError::Unauthorized
    );
    Ok(())
}

/// Verify that timelocked admin is NOT configured. Used to gate legacy instructions.
pub fn assert_timelocked_admin_not_set(marginfi_group: &MarginfiGroup) -> MarginfiResult {
    require!(
        !marginfi_group.has_timelocked_admin(),
        MarginfiError::Unauthorized
    );
    Ok(())
}

pub mod security_model {}
