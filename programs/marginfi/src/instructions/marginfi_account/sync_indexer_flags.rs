use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiAccount;

use crate::{prelude::MarginfiResult, state::marginfi_account::MarginfiAccountImpl};

/// (Permissionless) Batch-sync balance-derived indexer flags for existing accounts.
/// Pass MarginfiAccounts as remaining_accounts (all must be writable).
pub fn sync_indexer_flags<'info>(
    ctx: Context<'_, '_, 'info, 'info, SyncIndexerFlags<'info>>,
) -> MarginfiResult {
    for account_info in ctx.remaining_accounts.iter() {
        let loader = AccountLoader::<MarginfiAccount>::try_from(account_info)?;
        let mut account = loader.load_mut()?;
        account.sync_indexer_flags();
    }

    Ok(())
}

#[derive(Accounts)]
pub struct SyncIndexerFlags<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
}
