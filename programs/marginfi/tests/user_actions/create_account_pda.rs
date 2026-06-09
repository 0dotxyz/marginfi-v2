use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::test::TestFixture;
use marginfi_type_crate::types::MarginfiAccount;
use solana_program_test::tokio;
use solana_sdk::{
    instruction::Instruction, signature::Keypair, signer::Signer, transaction::Transaction,
};
use solana_system_interface::program as system_program;

#[tokio::test]
async fn marginfi_account_create_pda_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let account_index = 42;
    let third_party_id = None;

    // Derive PDA for the marginfi account
    let (marginfi_account_pda, bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let accounts = marginfi::accounts::MarginfiAccountInitializePda {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let init_marginfi_account_pda_ix = Instruction {
        program_id: marginfi::ID,
        accounts: accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_marginfi_account_pda_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok(), "Transaction failed: {:?}", res.err());

    let marginfi_account: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda).await;

    assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
    assert_eq!(marginfi_account.authority, authority);
    assert!(marginfi_account
        .lending_account
        .balances
        .iter()
        .all(|bank| !bank.is_active()));
    assert_eq!(marginfi_account.account_index, account_index);
    assert_eq!(marginfi_account.third_party_index, 0);
    assert_eq!(marginfi_account.bump, bump);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_with_third_party_id_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let account_index = 1;
    let third_party_id = Some(10); // Using a non-restricted id

    // Derive PDA for the marginfi account
    let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let accounts = marginfi::accounts::MarginfiAccountInitializePda {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority: authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let init_marginfi_account_pda_ix = Instruction {
        program_id: marginfi::ID,
        accounts: accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_marginfi_account_pda_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok(), "Transaction failed: {:?}", res.err());

    let marginfi_account: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda).await;

    assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
    assert_eq!(marginfi_account.authority, authority);
    assert_eq!(marginfi_account.third_party_index, 10);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_multiple_accounts_same_authority() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let third_party_id = None;

    // Create multiple accounts with different indices
    for account_index in 0..3 {
        let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
            &test_f.marginfi_group.key,
            &authority,
            account_index,
            third_party_id,
            &marginfi::ID,
        );

        let accounts = marginfi::accounts::MarginfiAccountInitializePda {
            marginfi_group: test_f.marginfi_group.key,
            marginfi_account: marginfi_account_pda,
            authority: authority,
            fee_payer: authority,
            instructions_sysvar: solana_instructions_sysvar::id(),
            system_program: system_program::id(),
        };

        let init_marginfi_account_pda_ix = Instruction {
            program_id: marginfi::ID,
            accounts: accounts.to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountInitializePda {
                account_index,
                third_party_id,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[init_marginfi_account_pda_ix],
            Some(&test_f.payer()),
            &[&test_f.payer_keypair()],
            test_f.get_latest_blockhash().await,
        );

        let res = test_f
            .context
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await;

        assert!(
            res.is_ok(),
            "Transaction failed for index {}: {:?}",
            account_index,
            res.err()
        );

        let marginfi_account: MarginfiAccount =
            test_f.load_and_deserialize(&marginfi_account_pda).await;

        assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
        assert_eq!(marginfi_account.authority, authority);
    }

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_different_authorities() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority1 = test_f.payer();
    let authority2_keypair = Keypair::new();
    let authority2 = authority2_keypair.pubkey();
    let account_index = 0;
    let third_party_id = None;

    // Create account for first authority
    let (marginfi_account_pda1, _bump1) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority1,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    // Create account for second authority
    let (marginfi_account_pda2, _bump2) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority2,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    // PDAs should be different even with same account_index
    assert_ne!(marginfi_account_pda1, marginfi_account_pda2);

    // Create account for authority1
    let accounts1 = marginfi::accounts::MarginfiAccountInitializePda {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda1,
        authority: authority1,
        fee_payer: authority1,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let init_ix1 = Instruction {
        program_id: marginfi::ID,
        accounts: accounts1.to_account_metas(None),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx1 = Transaction::new_signed_with_payer(
        &[init_ix1],
        Some(&test_f.payer()),
        // Note: authority 1 = test_f payer
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res1 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx1)
        .await;

    assert!(res1.is_ok());

    // Create account for authority2 (using same payer for simplicity)
    let accounts2 = marginfi::accounts::MarginfiAccountInitializePda {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda2,
        authority: authority2,
        fee_payer: test_f.payer(),
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let init_ix2 = Instruction {
        program_id: marginfi::ID,
        accounts: accounts2.to_account_metas(None),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx2 = Transaction::new_signed_with_payer(
        &[init_ix2],
        Some(&test_f.payer()),
        // Note: Here the authority also needs to sign separately
        &[&test_f.payer_keypair(), &authority2_keypair],
        test_f.get_latest_blockhash().await,
    );

    let res2 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx2)
        .await;

    assert!(res2.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_duplicate_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let account_index = 0;
    let third_party_id = None;

    // Derive PDA for the marginfi account
    let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let accounts = marginfi::accounts::MarginfiAccountInitializePda {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority: authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
    };

    let init_marginfi_account_pda_ix = Instruction {
        program_id: marginfi::ID,
        accounts: accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountInitializePda {
            account_index,
            third_party_id,
        }
        .data(),
    };

    // First transaction should succeed
    let tx1 = Transaction::new_signed_with_payer(
        &[init_marginfi_account_pda_ix.clone()],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res1 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx1)
        .await;

    assert!(res1.is_ok());

    // Second transaction with same parameters should fail
    let tx2 = Transaction::new_signed_with_payer(
        &[init_marginfi_account_pda_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair()],
        test_f.get_latest_blockhash().await,
    );

    let res2 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx2)
        .await;

    assert!(res2.is_err(), "Duplicate account creation should fail");

    Ok(())
}
