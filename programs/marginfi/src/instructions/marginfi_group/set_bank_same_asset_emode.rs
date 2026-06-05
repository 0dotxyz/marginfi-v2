use crate::events::{GroupEventHeader, LendingPoolBankSetSameAssetEmodeEligibilityEvent};
use crate::state::bank::BankImpl;
use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS, SAME_ASSET_EMODE_REGISTRY_SEED},
    types::{
        Bank, MarginfiGroup, SameAssetEmodeBank, SameAssetEmodeGroup, SameAssetEmodeRegistry,
        MAX_SAME_ASSET_EMODE_BANKS, MAX_SAME_ASSET_EMODE_GROUPS,
    },
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

    let mut registry = ctx.accounts.same_asset_emode_registry.load_mut()?;

    if enabled {
        add_bank_to_registry(
            &mut registry,
            ctx.accounts.bank.key(),
            bank.mint,
            bank.config.oracle_keys[0],
        )?;
    } else {
        remove_bank_from_registry(&mut registry, ctx.accounts.bank.key());
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

fn add_bank_to_registry(
    registry: &mut SameAssetEmodeRegistry,
    bank_key: Pubkey,
    mint: Pubkey,
    oracle_key: Pubkey,
) -> MarginfiResult {
    if registry.find_bank_index(bank_key).is_some() {
        return Ok(());
    }

    let group_index = match registry.find_group_index(mint, oracle_key) {
        Some(index) => index,
        None => {
            let index = registry.group_count as usize;
            check!(
                index < MAX_SAME_ASSET_EMODE_GROUPS,
                MarginfiError::BadEmodeConfig,
                "same-asset e-mode registry group table is full"
            );

            registry.groups[index] = SameAssetEmodeGroup {
                bank: bank_key,
                mint,
                oracle_key,
            };
            registry.group_count += 1;
            index
        }
    };

    let bank_index = registry.bank_count as usize;
    check!(
        bank_index < MAX_SAME_ASSET_EMODE_BANKS,
        MarginfiError::BadEmodeConfig,
        "same-asset e-mode registry bank table is full"
    );
    registry.banks[bank_index] = SameAssetEmodeBank {
        bank: bank_key,
        group_index: group_index as u8,
        _padding: [0; 7],
    };
    registry.bank_count += 1;

    Ok(())
}

fn remove_bank_from_registry(registry: &mut SameAssetEmodeRegistry, bank_key: Pubkey) {
    let Some(bank_index) = registry.find_bank_index(bank_key) else {
        return;
    };

    let removed_group_index = registry.banks[bank_index].group_index;
    let last_bank_index = registry.bank_count as usize - 1;
    registry.banks[bank_index] = registry.banks[last_bank_index];
    registry.banks[last_bank_index] = SameAssetEmodeBank::default();
    registry.bank_count -= 1;

    if registry.group_member_count(removed_group_index) > 0 {
        if registry.groups[removed_group_index as usize].bank == bank_key {
            if let Some(representative) = registry.first_bank_for_group(removed_group_index) {
                registry.groups[removed_group_index as usize].bank = representative;
            }
        }
        return;
    }

    let removed_group_index_usize = removed_group_index as usize;
    let last_group_index = registry.group_count as usize - 1;
    registry.groups[removed_group_index_usize] = registry.groups[last_group_index];
    registry.groups[last_group_index] = SameAssetEmodeGroup::default();
    registry.group_count -= 1;

    if removed_group_index_usize != last_group_index {
        for entry in registry.banks[..registry.bank_count as usize].iter_mut() {
            if entry.group_index as usize == last_group_index {
                entry.group_index = removed_group_index;
            }
        }
    }
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

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        seeds = [
            SAME_ASSET_EMODE_REGISTRY_SEED.as_bytes(),
            group.key().as_ref()
        ],
        bump,
    )]
    pub same_asset_emode_registry: AccountLoader<'info, SameAssetEmodeRegistry>,
}
