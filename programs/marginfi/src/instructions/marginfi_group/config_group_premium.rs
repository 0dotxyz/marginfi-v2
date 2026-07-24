use crate::check;
use crate::events::{GroupEventHeader, LendingPoolGroupPremiumConfigureEvent};
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use marginfi_type_crate::types::{
    MarginfiGroup, PremiumEntry, MAX_PREMIUM_ENTRIES, PREMIUM_TAG_EMPTY,
};

/// (emode admin only) Set one pair of the group's variable-borrow premium matrix.
///
/// `rate > 0` inserts or updates the (collateral_tag, liability_tag) pair; `rate == 0` removes
/// it, erroring if the pair is not in the matrix so operator typos fail loudly. One pair per
/// instruction (like emode config) keeps every matrix change independently auditable.
/// Entries are stored sorted by (collateral_tag, liability_tag).
pub fn lending_pool_configure_group_premium(
    ctx: Context<LendingPoolConfigureGroupPremium>,
    collateral_tag: u16,
    liability_tag: u16,
    rate: u32,
) -> MarginfiResult {
    // Zero (untagged) never matches a lookup, so storing it would create a dead entry.
    check!(
        collateral_tag != PREMIUM_TAG_EMPTY && liability_tag != PREMIUM_TAG_EMPTY,
        MarginfiError::PremiumEntryInvalid
    );

    let mut group = ctx.accounts.group.load_mut()?;
    let mut count = (group.premium_settings.entry_count as usize).min(MAX_PREMIUM_ENTRIES);
    let position = group.premium_entries[..count]
        .binary_search_by_key(&(collateral_tag, liability_tag), |e| {
            (e.collateral_tag, e.liability_tag)
        });

    let old_rate = if rate > 0 {
        let entry = PremiumEntry {
            collateral_tag,
            liability_tag,
            rate,
        };
        match position {
            Ok(i) => {
                let old = group.premium_entries[i].rate;
                group.premium_entries[i] = entry;
                old
            }
            Err(i) => {
                check!(
                    count < MAX_PREMIUM_ENTRIES,
                    MarginfiError::PremiumMatrixFull
                );
                group.premium_entries[i..=count].rotate_right(1);
                group.premium_entries[i] = entry;
                count += 1;
                0
            }
        }
    } else {
        let Ok(i) = position else {
            return err!(MarginfiError::PremiumEntryNotFound);
        };
        let old = group.premium_entries[i].rate;
        group.premium_entries[i..count].rotate_left(1);
        group.premium_entries[count - 1] = PremiumEntry::zeroed();
        count -= 1;
        old
    };

    group.premium_settings.entry_count = count as u16;
    // Groups created before the premium feature have a zeroed capacity; configuring the matrix
    // brings them up to the current account capacity.
    group.premium_settings.entry_capacity = MAX_PREMIUM_ENTRIES as u16;
    group.premium_settings.timestamp = Clock::get()?.unix_timestamp;

    msg!(
        "premium pair ({:?}, {:?}) rate: {:?} -> {:?} ({:?} entries)",
        collateral_tag,
        liability_tag,
        old_rate,
        rate,
        count
    );

    emit!(LendingPoolGroupPremiumConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(ctx.accounts.emode_admin.key()),
        },
        collateral_tag,
        liability_tag,
        old_rate,
        new_rate: rate,
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
