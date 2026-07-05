use crate::check;
use crate::events::{GroupEventHeader, LendingPoolGroupPremiumConfigureEvent};
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use marginfi_type_crate::types::{
    MarginfiGroup, PremiumEntry, MAX_PREMIUM_ENTRIES, PREMIUM_TAG_EMPTY,
};

/// (emode admin only) Replace the group's pairwise variable-borrow premium matrix.
///
/// Full-replace semantics: the passed entries become the entire matrix. Pass an empty vec to
/// disable the premium matrix. Entries are stored sorted by (collateral_tag, liability_tag).
pub fn lending_pool_configure_group_premium(
    ctx: Context<LendingPoolConfigureGroupPremium>,
    entries: Vec<PremiumEntry>,
) -> MarginfiResult {
    let mut group = ctx.accounts.group.load_mut()?;

    check!(
        entries.len() <= MAX_PREMIUM_ENTRIES,
        MarginfiError::PremiumMatrixFull
    );
    for entry in entries.iter() {
        check!(
            entry.collateral_tag != PREMIUM_TAG_EMPTY && entry.liability_tag != PREMIUM_TAG_EMPTY,
            MarginfiError::PremiumEntryInvalid
        );
    }
    // With at most 64 entries, the quadratic duplicate scan is trivially cheap and heap-free.
    for (i, a) in entries.iter().enumerate() {
        for b in entries.iter().skip(i + 1) {
            check!(
                (a.collateral_tag, a.liability_tag) != (b.collateral_tag, b.liability_tag),
                MarginfiError::PremiumEntryDuplicate
            );
        }
    }

    let mut sorted_entries = [PremiumEntry::zeroed(); MAX_PREMIUM_ENTRIES];
    sorted_entries[..entries.len()].copy_from_slice(&entries);
    sorted_entries[..entries.len()].sort_by_key(|e| (e.collateral_tag, e.liability_tag));

    group.premium_entries = sorted_entries;
    group.premium_settings.entry_count = entries.len() as u16;
    // Groups created before the premium feature have a zeroed capacity; configuring the matrix
    // brings them up to the current account capacity.
    group.premium_settings.entry_capacity = MAX_PREMIUM_ENTRIES as u16;
    group.premium_settings.timestamp = Clock::get()?.unix_timestamp;

    msg!(
        "premium matrix set: {:?} entries: {:?}",
        entries.len(),
        &sorted_entries[..entries.len()]
    );

    emit!(LendingPoolGroupPremiumConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(ctx.accounts.emode_admin.key()),
        },
        entries: sorted_entries[..entries.len()].to_vec(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureGroupPremium<'info> {
    #[account(
        mut,
        has_one = emode_admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub emode_admin: Signer<'info>,
}
