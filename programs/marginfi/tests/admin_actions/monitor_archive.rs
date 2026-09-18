use anchor_lang::solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
};
use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::{assert_custom_error, prelude::*};
use marginfi::{
    errors::MarginfiError,
    instructions::marginfi_group::{
        SnapshotUpdateInput, MAX_ACCOUNT_DATA_LEN, MONITOR_INDEX_MAP_LEN,
    },
};
use marginfi_type_crate::types::{ArchiveMeta, ArchiveRecord, MintSnapshotRecords};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use solana_system_interface::instruction as system_instruction;

const ARCHIVE_HEADER_LEN: usize = 8 + ArchiveMeta::LEN + (MONITOR_INDEX_MAP_LEN * 64);

/// Archive sized for a handful of mints. The 10 MiB production size is only exercised by
/// `monitor_archive_initialize_success`, since program-test clones account data often.
fn archive_len_for(records: usize) -> usize {
    ARCHIVE_HEADER_LEN + (records * MintSnapshotRecords::LEN_V1)
}

/// Allocates the archive with a top-level `create_account` and initializes its metadata.
/// This mirrors what an off-chain client has to do: the program cannot allocate 10 MiB
/// itself because CPI account growth is capped at 10 KiB.
async fn create_and_init_archive(
    test_f: &TestFixture,
    size: usize,
    snapshot_manager: Pubkey,
) -> anyhow::Result<Keypair> {
    let archive = Keypair::new();
    let payer = test_f.payer();
    let rent = test_f.get_minimum_rent_for_size(size).await;
    let blockhash = test_f.get_latest_blockhash().await;

    let create_ix = system_instruction::create_account(
        &payer,
        &archive.pubkey(),
        rent,
        size as u64,
        &marginfi::ID,
    );

    let init_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::MonitorArchiveInitialize {
            payer,
            archive: archive.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MonitorArchiveInitialize { snapshot_manager }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&payer),
        &[&test_f.payer_keypair(), &archive],
        blockhash,
    );

    test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(archive)
}

fn upsert_batch_ix(
    snapshot_manager: Pubkey,
    archive: Pubkey,
    mints: &[Pubkey],
    updates: Vec<SnapshotUpdateInput>,
) -> Instruction {
    let mut accounts = marginfi::accounts::MonitorArchiveUpsertBatch {
        snapshot_manager,
        archive,
    }
    .to_account_metas(Some(true));
    accounts.extend(mints.iter().map(|m| AccountMeta::new_readonly(*m, false)));

    Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::MonitorArchiveUpsertBatch { updates }.data(),
    }
}

async fn archive_data(test_f: &TestFixture, archive: Pubkey) -> Vec<u8> {
    test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(archive)
        .await
        .unwrap()
        .unwrap()
        .data
}

/// Reads a mint record back through the same typed helper an on-chain caller would use.
fn read_record(archive: Pubkey, data: &mut [u8], mint: Pubkey) -> Option<MintSnapshotRecords> {
    let mut lamports = 0u64;
    let owner = marginfi::ID;
    let account_info = AccountInfo::new(&archive, false, false, &mut lamports, data, &owner, false);
    MintSnapshotRecords::from_archive_account::<MONITOR_INDEX_MAP_LEN>(&account_info, mint)
}

#[tokio::test]
async fn monitor_archive_initialize_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let snapshot_manager = Pubkey::new_unique();

    let archive = create_and_init_archive(&test_f, MAX_ACCOUNT_DATA_LEN, snapshot_manager).await?;

    let data = archive_data(&test_f, archive.pubkey()).await;
    assert_eq!(data.len(), MAX_ACCOUNT_DATA_LEN);
    assert_eq!(&data[0..8], &MintSnapshotRecords::TYPE_DISCRIMINATOR);

    let meta = ArchiveMeta::read(&data[8..8 + ArchiveMeta::LEN]).unwrap();
    assert_eq!(meta.version, 1);
    assert_eq!(meta.record_count, 0);
    assert_eq!(meta.authority, snapshot_manager);

    Ok(())
}

#[tokio::test]
async fn monitor_archive_initialize_twice_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let snapshot_manager = Pubkey::new_unique();

    let archive = create_and_init_archive(&test_f, archive_len_for(2), snapshot_manager).await?;
    let payer = test_f.payer();
    let blockhash = test_f.get_latest_blockhash().await;

    let init_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::MonitorArchiveInitialize {
            payer,
            archive: archive.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MonitorArchiveInitialize { snapshot_manager }.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&payer),
        &[&test_f.payer_keypair()],
        blockhash,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidConfig);

    Ok(())
}

#[tokio::test]
async fn monitor_archive_upsert_batch_appends_then_updates_in_place() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let snapshot_manager = Keypair::new();

    let archive =
        create_and_init_archive(&test_f, archive_len_for(4), snapshot_manager.pubkey()).await?;

    let mints: Vec<Pubkey> = (0..3).map(|_| Pubkey::new_unique()).collect();
    let payer = test_f.payer();

    for (batch, hour) in [(0u64, 100u64), (1, 101)] {
        let updates = (0..mints.len())
            .map(|i| SnapshotUpdateInput {
                snapshot_hour: hour,
                price: 1_000 + batch + i as u64,
                native_apy: 500 + batch + i as u64,
            })
            .collect();

        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[upsert_batch_ix(
                snapshot_manager.pubkey(),
                archive.pubkey(),
                &mints,
                updates,
            )],
            Some(&payer),
            &[&test_f.payer_keypair(), &snapshot_manager],
            blockhash,
        );

        test_f
            .context
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;
    }

    let mut data = archive_data(&test_f, archive.pubkey()).await;

    // Second batch updated the existing records rather than appending new ones.
    let meta = ArchiveMeta::read(&data[8..8 + ArchiveMeta::LEN]).unwrap();
    assert_eq!(meta.record_count, mints.len() as u64);

    for (i, mint) in mints.iter().enumerate() {
        let record = read_record(archive.pubkey(), &mut data, *mint).unwrap();
        assert_eq!(record.mint, *mint);
        assert_eq!(record.head, 0);
        assert_eq!(record.tail, 1);

        let latest = record.latest_snapshot().unwrap();
        assert_eq!(latest.snapshot_hour, 101);
        assert_eq!(latest.price, 1_001 + i as u64);
        assert_eq!(latest.native_apy, 501 + i as u64);

        assert_eq!(record.snapshots[0].snapshot_hour, 100);
        assert_eq!(record.snapshots[0].price, 1_000 + i as u64);
    }

    Ok(())
}

#[tokio::test]
async fn monitor_archive_upsert_batch_wrong_authority_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let snapshot_manager = Keypair::new();
    let attacker = Keypair::new();

    let archive =
        create_and_init_archive(&test_f, archive_len_for(2), snapshot_manager.pubkey()).await?;

    let mints = vec![Pubkey::new_unique()];
    let payer = test_f.payer();
    let blockhash = test_f.get_latest_blockhash().await;

    let tx = Transaction::new_signed_with_payer(
        &[upsert_batch_ix(
            attacker.pubkey(),
            archive.pubkey(),
            &mints,
            vec![SnapshotUpdateInput {
                snapshot_hour: 100,
                price: 1,
                native_apy: 2,
            }],
        )],
        Some(&payer),
        &[&test_f.payer_keypair(), &attacker],
        blockhash,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}
