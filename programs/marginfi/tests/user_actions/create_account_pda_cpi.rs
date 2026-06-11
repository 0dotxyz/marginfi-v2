use anchor_lang::InstructionData;
use fixtures::test::TestFixture;
use marginfi_type_crate::types::MarginfiAccount;
use mocks::instructions::CpiCallLog;
use solana_program_test::tokio;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use solana_system_interface::program as system_program;

fn create_mock_account_metas(
    accounts: &mocks::accounts::CreateMarginfiAccountPdaViaCpi,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(accounts.marginfi_group, false),
        AccountMeta::new(accounts.marginfi_account, false),
        AccountMeta::new_readonly(accounts.authority, true),
        AccountMeta::new(accounts.fee_payer, true),
        AccountMeta::new_readonly(accounts.instructions_sysvar, false),
        AccountMeta::new_readonly(accounts.system_program, false),
        AccountMeta::new_readonly(accounts.marginfi_program, false),
        AccountMeta::new(accounts.call_log, true),
    ]
}

#[tokio::test]
async fn marginfi_account_create_pda_via_cpi_success() -> anyhow::Result<()> {
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

    // Create a call log account for the CPI mock
    let call_log_keypair = Keypair::new();

    let mock_accounts = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair.pubkey(),
    };

    let mock_cpi_ix = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[mock_cpi_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &call_log_keypair],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok(), "CPI transaction failed: {:?}", res.err());

    // Verify the marginfi account was created correctly
    let marginfi_account: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda).await;

    assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
    assert_eq!(marginfi_account.authority, authority);
    assert!(marginfi_account
        .lending_account
        .balances
        .iter()
        .all(|bank| !bank.is_active()));

    // Verify the CPI call log
    let call_log: CpiCallLog = test_f
        .load_and_deserialize(&call_log_keypair.pubkey())
        .await;

    assert_eq!(call_log.caller_program, mocks::id());
    assert_eq!(call_log.target_program, marginfi::ID);
    assert_eq!(call_log.marginfi_group, test_f.marginfi_group.key);
    assert_eq!(call_log.marginfi_account, marginfi_account_pda);
    assert_eq!(call_log.authority, authority);
    assert_eq!(call_log.account_index, account_index);
    assert_eq!(call_log.third_party_id, third_party_id);
    assert!(call_log.call_successful);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_via_cpi_with_third_party_id() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let account_index = 1;
    let third_party_id = Some(10_001);

    // Derive PDA for the marginfi account
    let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    // Create a call log account for the CPI mock
    let call_log_keypair = Keypair::new();

    let mock_accounts = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair.pubkey(),
    };

    let mock_cpi_ix = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[mock_cpi_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &call_log_keypair],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok(), "CPI transaction failed: {:?}", res.err());

    // Verify the marginfi account was created correctly
    let marginfi_account: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda).await;

    assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
    assert_eq!(marginfi_account.authority, authority);
    assert_eq!(marginfi_account.account_index, 1);
    assert_eq!(marginfi_account.third_party_index, 10_001);

    // Verify the CPI call log records the third party ID
    let call_log: CpiCallLog = test_f
        .load_and_deserialize(&call_log_keypair.pubkey())
        .await;

    assert_eq!(call_log.account_index, account_index);
    assert_eq!(call_log.third_party_id, third_party_id);
    assert!(call_log.call_successful);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_via_cpi_multiple_calls() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let third_party_id = None;

    // Test creating multiple accounts via CPI with different indices
    for account_index in 0..3 {
        let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
            &test_f.marginfi_group.key,
            &authority,
            account_index,
            third_party_id,
            &marginfi::ID,
        );

        let call_log_keypair = Keypair::new();

        let mock_accounts = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
            marginfi_group: test_f.marginfi_group.key,
            marginfi_account: marginfi_account_pda,
            authority: authority,
            fee_payer: authority,
            instructions_sysvar: solana_instructions_sysvar::id(),
            system_program: system_program::id(),
            marginfi_program: marginfi::ID,
            call_log: call_log_keypair.pubkey(),
        };

        let mock_cpi_ix = Instruction {
            program_id: mocks::id(),
            accounts: create_mock_account_metas(&mock_accounts),
            data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
                account_index,
                third_party_id,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[mock_cpi_ix],
            Some(&test_f.payer()),
            &[&test_f.payer_keypair(), &call_log_keypair],
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
            "CPI transaction failed for index {}: {:?}",
            account_index,
            res.err()
        );

        // Verify each account was created properly
        let marginfi_account: MarginfiAccount =
            test_f.load_and_deserialize(&marginfi_account_pda).await;

        assert_eq!(marginfi_account.group, test_f.marginfi_group.key);
        assert_eq!(marginfi_account.authority, authority);

        // Verify the call log for each account
        let call_log: CpiCallLog = test_f
            .load_and_deserialize(&call_log_keypair.pubkey())
            .await;

        assert_eq!(call_log.account_index, account_index);
        assert!(call_log.call_successful);
    }

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_via_cpi_different_authorities() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority1 = test_f.payer();
    let authority2_keypair = Keypair::new();
    let authority2 = authority2_keypair.pubkey();
    let account_index = 0;
    let third_party_id = None;

    // Create account for authority1 via CPI
    let (marginfi_account_pda1, _bump1) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority1,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    let call_log_keypair1 = Keypair::new();

    let mock_accounts1 = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda1,
        authority: authority1,
        fee_payer: authority1,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair1.pubkey(),
    };

    let mock_cpi_ix1 = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts1),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx1 = Transaction::new_signed_with_payer(
        &[mock_cpi_ix1],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &call_log_keypair1],
        test_f.get_latest_blockhash().await,
    );

    let res1 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx1)
        .await;

    assert!(
        res1.is_ok(),
        "First CPI transaction failed: {:?}",
        res1.err()
    );

    // Create account for authority2 via CPI
    let (marginfi_account_pda2, _bump2) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority2,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    // PDAs should be different even with same account_index
    assert_ne!(marginfi_account_pda1, marginfi_account_pda2);

    let call_log_keypair2 = Keypair::new();

    let mock_accounts2 = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda2,
        authority: authority2,
        fee_payer: test_f.payer(),
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair2.pubkey(),
    };

    let mock_cpi_ix2 = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts2),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx2 = Transaction::new_signed_with_payer(
        &[mock_cpi_ix2],
        Some(&test_f.payer()),
        &[
            &test_f.payer_keypair(),
            // NOTE: A more typical use here would be a PDA signer...
            &authority2_keypair,
            &call_log_keypair2,
        ],
        test_f.get_latest_blockhash().await,
    );

    let res2 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx2)
        .await;

    assert!(
        res2.is_ok(),
        "Second CPI transaction failed: {:?}",
        res2.err()
    );

    // Verify both accounts were created with correct authorities
    let marginfi_account1: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda1).await;
    let marginfi_account2: MarginfiAccount =
        test_f.load_and_deserialize(&marginfi_account_pda2).await;

    assert_eq!(marginfi_account1.authority, authority1);
    assert_eq!(marginfi_account2.authority, authority2);

    // Verify both call logs recorded the correct authorities
    let call_log1: CpiCallLog = test_f
        .load_and_deserialize(&call_log_keypair1.pubkey())
        .await;
    let call_log2: CpiCallLog = test_f
        .load_and_deserialize(&call_log_keypair2.pubkey())
        .await;

    assert_eq!(call_log1.authority, authority1);
    assert_eq!(call_log2.authority, authority2);
    assert!(call_log1.call_successful);
    assert!(call_log2.call_successful);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_create_pda_via_cpi_duplicate_should_fail() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let authority = test_f.payer();
    let account_index = 0u16;
    let third_party_id = None;

    // Derive PDA for the marginfi account
    let (marginfi_account_pda, _bump) = MarginfiAccount::derive_pda(
        &test_f.marginfi_group.key,
        &authority,
        account_index,
        third_party_id,
        &marginfi::ID,
    );

    // First CPI call should succeed
    let call_log_keypair1 = Keypair::new();

    let mock_accounts1 = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair1.pubkey(),
    };

    let mock_cpi_ix1 = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts1),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx1 = Transaction::new_signed_with_payer(
        &[mock_cpi_ix1],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &call_log_keypair1],
        test_f.get_latest_blockhash().await,
    );

    let res1 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx1)
        .await;

    assert!(res1.is_ok(), "First CPI transaction should succeed");

    // Second CPI call with same parameters should fail
    let call_log_keypair2 = Keypair::new();

    let mock_accounts2 = mocks::accounts::CreateMarginfiAccountPdaViaCpi {
        marginfi_group: test_f.marginfi_group.key,
        marginfi_account: marginfi_account_pda,
        authority,
        fee_payer: authority,
        instructions_sysvar: solana_instructions_sysvar::id(),
        system_program: system_program::id(),
        marginfi_program: marginfi::ID,
        call_log: call_log_keypair2.pubkey(),
    };

    let mock_cpi_ix2 = Instruction {
        program_id: mocks::id(),
        accounts: create_mock_account_metas(&mock_accounts2),
        data: mocks::instruction::CreateMarginfiAccountPdaViaCpi {
            account_index,
            third_party_id,
        }
        .data(),
    };

    let tx2 = Transaction::new_signed_with_payer(
        &[mock_cpi_ix2],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &call_log_keypair2],
        test_f.get_latest_blockhash().await,
    );

    let res2 = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx2)
        .await;

    assert!(
        res2.is_err(),
        "Duplicate account creation via CPI should fail"
    );

    Ok(())
}
