//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::SnapshotUpdateInput;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`monitor_archive_initialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorArchiveInitialize {
    pub payer: Pubkey,
    /// `MAX_ACCOUNT_DATA_LEN`. `MintSnapshotsArchive::initialize` rejects it unless it is
    /// large enough and its discriminator is still zero.
    pub archive: Pubkey,
}
impl MonitorArchiveInitialize {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::native::MONITOR_ARCHIVE_INITIALIZE;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new(self.archive, false),
        ]
    }
}
impl ToAccountMetas for MonitorArchiveInitialize {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (snapshot manager only) Initialize the monitor snapshot archive account metadata.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn monitor_archive_initialize(
    accounts: &MonitorArchiveInitialize,
    snapshot_manager: Pubkey,
) -> Instruction {
    monitor_archive_initialize_with_program_id(crate::ID, accounts, snapshot_manager)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn monitor_archive_initialize_with_program_id(
    program_id: Pubkey,
    accounts: &MonitorArchiveInitialize,
    snapshot_manager: Pubkey,
) -> Instruction {
    let mut data = MonitorArchiveInitialize::DISCRIMINATOR.to_vec();
    snapshot_manager
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`monitor_archive_upsert_batch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorArchiveUpsertBatch {
    /// Dedicated signer for monitor snapshot archive writes. Must match
    /// `ArchiveMeta.authority`.
    pub snapshot_manager: Pubkey,
    /// [8-byte discriminator][ArchiveMeta][index_map][payload].
    pub archive: Pubkey,
}
impl MonitorArchiveUpsertBatch {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::native::MONITOR_ARCHIVE_UPSERT_BATCH;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.snapshot_manager, true),
            AccountMeta::new(self.archive, false),
        ]
    }
}
impl ToAccountMetas for MonitorArchiveUpsertBatch {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (snapshot manager only) Upsert a batch of monitor snapshots into the archive account.
///
/// Mints are sourced positionally from `remaining_accounts`: `updates[i]` maps to
/// `remaining_accounts[i]`.
///
/// Remaining accounts: one mint account per update, in the order of the updates.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn monitor_archive_upsert_batch(
    accounts: &MonitorArchiveUpsertBatch,
    updates: Vec<SnapshotUpdateInput>,
) -> Instruction {
    monitor_archive_upsert_batch_with_program_id(crate::ID, accounts, updates)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn monitor_archive_upsert_batch_with_program_id(
    program_id: Pubkey,
    accounts: &MonitorArchiveUpsertBatch,
    updates: Vec<SnapshotUpdateInput>,
) -> Instruction {
    let mut data = MonitorArchiveUpsertBatch::DISCRIMINATOR.to_vec();
    updates
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
