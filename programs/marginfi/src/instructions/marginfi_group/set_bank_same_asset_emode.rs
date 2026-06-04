use crate::events::{GroupEventHeader, LendingPoolBankSetSameAssetEmodeEligibilityEvent};
use crate::state::bank::BankImpl;
use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS},
    types::{Bank, MarginfiGroup},
};

/// Opt a bank in or out of same-asset e-mode participation.
///
/// This records local bank intent. The risk engine still requires every participating liability
/// and collateral bank to be eligible and to share the same mint and `oracle_keys[0]`.
pub fn lending_pool_set_bank_same_asset_emode_eligibility(
    ctx: Context<LendingPoolSetBankSameAssetEmodeEligibility>,
    enabled: bool,
) -> MarginfiResult {
    let group = ctx.accounts.group.load()?;

    check!(
        ctx.accounts.signer.key() == group.admin || ctx.accounts.signer.key() == group.emode_admin,
        MarginfiError::Unauthorized
    );

    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(!bank.get_flag(FREEZE_SETTINGS), MarginfiError::Unauthorized);

    if enabled {
        check!(
            !bank.config.oracle_setup.is_fixed_price(),
            MarginfiError::BadEmodeConfig,
            "fixed-price banks cannot be same-asset e-mode eligible"
        );
        check!(
            bank.config.oracle_keys[0] != Pubkey::default(),
            MarginfiError::BadEmodeConfig,
            "same-asset e-mode eligible banks must have oracle_keys[0] set"
        );
    }

    bank.update_flag(enabled, BANK_SAME_ASSET_EMODE_ELIGIBLE);

    emit!(LendingPoolBankSetSameAssetEmodeEligibilityEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(ctx.accounts.signer.key()),
        },
        bank: ctx.accounts.bank.key(),
        mint: bank.mint,
        enabled,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolSetBankSameAssetEmodeEligibility<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub signer: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}
