use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::test::TestFixture;
use marginfi_type_crate::types::{MarginfiAccount, ACCOUNT_DISABLED};
use solana_program_test::tokio;
use solana_sdk::{
    instruction::Instruction, signature::Keypair, signer::Signer, transaction::Transaction,
};
use solana_system_interface::program as system_program;

#[tokio::test]
async fn transfer_to_new_account_pda_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Create initial keypair-based account
    let old_authority = test_f.payer();
    let old_marginfi_account_key = Keypair::new();
    let new_authority_keypair = Keypair::new();
    let new_authority = new_authority_keypair.pubkey();

    // Create the old account using keypair method
    let create_old_account_accounts = marginfi::accounts::MarginfiAccountInitialize {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: old_marginfi_account_key.pubkey(),
        authority: old_authority,
        fee_payer: old_authority,
        system_program: system_program::id(),
    };

    let create_old_account_ix = Instruction {
        program_id: marginfi::ID,
        accounts: create_old_account_accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitialize {}.data(),
    };

    let create_tx = Transaction::new_signed_with_payer(
        &[create_old_account_ix],
        Some(&old_authority),
        &[&test_f.payer_keypair(), &old_marginfi_account_key],
        test_f.get_latest_blockhash().await,
    );

    let create_res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(create_tx)
        .await;

    assert!(
        create_res.is_ok(),
        "Failed to create old account: {:?}",
        create_res.err()
    );

    // Now transfer to a new PDA account
    let account_index = 0;
    let third_party_id = None;

    let (new_marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &new_authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let transfer_accounts = marginfi::accounts::TransferToNewAccountPda {
        group: test_f.marginfi_group.key,
        old_marginfi_account: old_marginfi_account_key.pubkey(),
        new_marginfi_account: new_marginfi_account_pda,
        authority: old_authority,
        fee_payer: old_authority,
        new_authority,
        global_fee_wallet: test_f.marginfi_group.fee_wallet,
        fee_state: test_f.marginfi_group.fee_state,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let transfer_ix = Instruction {
        program_id: marginfi::ID,
        accounts: transfer_accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::TransferToNewAccountPda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let transfer_tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&old_authority),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let transfer_res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(transfer_tx)
        .await;

    assert!(
        transfer_res.is_ok(),
        "Transfer failed: {:?}",
        transfer_res.err()
    );

    // Verify the new account
    let new_account: MarginfiAccount = test_f.load_and_deserialize(&new_marginfi_account_pda).await;

    assert_eq!(new_account.group, test_f.marginfi_group.key);
    assert_eq!(new_account.authority, new_authority);
    assert_eq!(new_account.migrated_from, old_marginfi_account_key.pubkey());

    // Verify the old account is disabled
    let old_account: MarginfiAccount = test_f
        .load_and_deserialize(&old_marginfi_account_key.pubkey())
        .await;

    assert_eq!(old_account.migrated_to, new_marginfi_account_pda);
    assert_eq!(old_account.account_flags, ACCOUNT_DISABLED);
    Ok(())
}

#[tokio::test]
async fn transfer_to_new_account_pda_with_third_party_id() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Create initial keypair-based account
    let old_authority = test_f.payer();
    let old_marginfi_account_key = Keypair::new();
    let new_authority_keypair = Keypair::new();
    let new_authority = new_authority_keypair.pubkey();

    // Create the old account using keypair method
    let create_old_account_accounts = marginfi::accounts::MarginfiAccountInitialize {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: old_marginfi_account_key.pubkey(),
        authority: old_authority,
        fee_payer: old_authority,
        system_program: system_program::id(),
    };

    let create_old_account_ix = Instruction {
        program_id: marginfi::ID,
        accounts: create_old_account_accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitialize {}.data(),
    };

    let create_tx = Transaction::new_signed_with_payer(
        &[create_old_account_ix],
        Some(&old_authority),
        &[&test_f.payer_keypair(), &old_marginfi_account_key],
        test_f.get_latest_blockhash().await,
    );

    let create_res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(create_tx)
        .await;

    assert!(create_res.is_ok(), "Failed to create old account");

    // Transfer to PDA with third party id
    let account_index = 0;
    let third_party_id = Some(100); // Non-restricted id

    let (new_marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &new_authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let transfer_accounts = marginfi::accounts::TransferToNewAccountPda {
        group: test_f.marginfi_group.key,
        old_marginfi_account: old_marginfi_account_key.pubkey(),
        new_marginfi_account: new_marginfi_account_pda,
        authority: old_authority,
        fee_payer: old_authority,
        new_authority,
        global_fee_wallet: test_f.marginfi_group.fee_wallet,
        fee_state: test_f.marginfi_group.fee_state,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let transfer_ix = Instruction {
        program_id: marginfi::ID,
        accounts: transfer_accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::TransferToNewAccountPda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let transfer_tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&old_authority),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let transfer_res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(transfer_tx)
        .await;

    assert!(
        transfer_res.is_ok(),
        "Transfer with third party id failed: {:?}",
        transfer_res.err()
    );

    // Verify the new account
    let new_account: MarginfiAccount = test_f.load_and_deserialize(&new_marginfi_account_pda).await;

    assert_eq!(new_account.authority, new_authority);

    Ok(())
}

#[tokio::test]
async fn transfer_double_migration_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Create initial keypair-based account
    let old_authority = test_f.payer();
    let old_marginfi_account_key = Keypair::new();
    let new_authority1_keypair = Keypair::new();
    let new_authority1 = new_authority1_keypair.pubkey();
    let new_authority2_keypair = Keypair::new();
    let new_authority2 = new_authority2_keypair.pubkey();

    // Note: new authorities will be funded through the test fixture payer

    // Create the old account
    let create_old_account_accounts = marginfi::accounts::MarginfiAccountInitialize {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: old_marginfi_account_key.pubkey(),
        authority: old_authority,
        fee_payer: old_authority,
        system_program: system_program::id(),
    };

    let create_old_account_ix = Instruction {
        program_id: marginfi::ID,
        accounts: create_old_account_accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitialize {}.data(),
    };

    let create_tx = Transaction::new_signed_with_payer(
        &[create_old_account_ix],
        Some(&old_authority),
        &[&test_f.payer_keypair(), &old_marginfi_account_key],
        test_f.get_latest_blockhash().await,
    );

    let create_res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(create_tx)
        .await;

    assert!(create_res.is_ok());

    // First transfer should succeed
    let account_index1 = 0;
    let (new_marginfi_account_pda1, _bump1) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &new_authority1,
        account_index1,
        None,
        &marginfi::ID,
    );

    let transfer_accounts1 = marginfi::accounts::TransferToNewAccountPda {
        group: test_f.marginfi_group.key,
        old_marginfi_account: old_marginfi_account_key.pubkey(),
        new_marginfi_account: new_marginfi_account_pda1,
        authority: old_authority,
        fee_payer: old_authority,
        new_authority: new_authority1,
        global_fee_wallet: test_f.marginfi_group.fee_wallet,
        fee_state: test_f.marginfi_group.fee_state,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let transfer_ix1 = Instruction {
        program_id: marginfi::ID,
        accounts: transfer_accounts1.to_account_metas(Some(true)),
        data: marginfi::instruction::TransferToNewAccountPda {
            account_index: account_index1,
            third_party_id: None,
        }
        .data(),
    };

    let transfer_tx1 = Transaction::new_signed_with_payer(
        &[transfer_ix1],
        Some(&old_authority),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let transfer_res1 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(transfer_tx1)
        .await;

    assert!(transfer_res1.is_ok());

    // Second transfer should fail
    let account_index2 = 1;
    let (new_marginfi_account_pda2, _bump2) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &new_authority2,
        account_index2,
        None,
        &marginfi::ID,
    );

    let transfer_accounts2 = marginfi::accounts::TransferToNewAccountPda {
        group: test_f.marginfi_group.key,
        old_marginfi_account: old_marginfi_account_key.pubkey(),
        new_marginfi_account: new_marginfi_account_pda2,
        authority: old_authority,
        fee_payer: old_authority,
        new_authority: new_authority2,
        global_fee_wallet: test_f.marginfi_group.fee_wallet,
        fee_state: test_f.marginfi_group.fee_state,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let transfer_ix2 = Instruction {
        program_id: marginfi::ID,
        accounts: transfer_accounts2.to_account_metas(Some(true)),
        data: marginfi::instruction::TransferToNewAccountPda {
            account_index: account_index2,
            third_party_id: None,
        }
        .data(),
    };

    let transfer_tx2 = Transaction::new_signed_with_payer(
        &[transfer_ix2],
        Some(&old_authority),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let transfer_res2 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(transfer_tx2)
        .await;

    assert!(transfer_res2.is_err(), "Second transfer should fail");

    Ok(())
}
