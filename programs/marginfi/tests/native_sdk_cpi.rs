//! Exercises `programs/native-cpi-example`, an SBF program built with no anchor in its dependency
//! graph, CPI-ing into marginfi through `marginfi_type_crate::ix_builders`.

use anchor_lang::solana_program::system_program;
use fixtures::{prelude::*, test::NATIVE_CPI_EXAMPLE_ID};
use marginfi_type_crate::types::MarginfiAccount;
use solana_program_test::{tokio, BanksClientError};
use solana_sdk::{
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
    transaction::TransactionError,
};

/// The `.so` comes from `scripts/build-workspace.sh`; `ProgramAccountNotFound` names neither.
fn require_example_program() {
    let path = std::path::PathBuf::from(std::env::var("SBF_OUT_DIR").unwrap_or_default())
        .join("native_cpi_example.so");
    assert!(
        path.exists(),
        "{} is missing; run scripts/build-workspace.sh first",
        path.display()
    );
}

fn authority_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"authority"], &NATIVE_CPI_EXAMPLE_ID).0
}

/// Accounts mirror the example program's documented order.
fn example_ix(group: Pubkey, marginfi_account: Pubkey, fee_payer: Pubkey) -> Instruction {
    Instruction {
        program_id: NATIVE_CPI_EXAMPLE_ID,
        accounts: vec![
            AccountMeta::new_readonly(group, false),
            AccountMeta::new(marginfi_account, true),
            AccountMeta::new_readonly(authority_pda(), false),
            AccountMeta::new(fee_payer, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(marginfi::ID, false),
        ],
        data: vec![],
    }
}

#[tokio::test]
async fn native_program_creates_a_marginfi_account_via_cpi() -> anyhow::Result<()> {
    require_example_program();
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let account_key = Keypair::new();
    let (banks_client, payer, blockhash) = {
        let ctx = test_f.context.borrow();
        (
            ctx.banks_client.clone(),
            ctx.payer.insecure_clone(),
            ctx.last_blockhash,
        )
    };

    let tx = Transaction::new_signed_with_payer(
        &[example_ix(
            test_f.marginfi_group.key,
            account_key.pubkey(),
            payer.pubkey(),
        )],
        Some(&payer.pubkey()),
        &[&payer, &account_key],
        blockhash,
    );
    banks_client.process_transaction(tx).await?;

    let account: MarginfiAccount =
        load_and_deserialize(test_f.context.clone(), &account_key.pubkey()).await;
    assert_eq!(account.authority, authority_pda());
    assert_eq!(account.group, test_f.marginfi_group.key);

    Ok(())
}

/// The program checks the PDA before signing, so a caller-supplied authority is rejected.
#[tokio::test]
async fn native_program_rejects_a_foreign_authority() -> anyhow::Result<()> {
    require_example_program();
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let account_key = Keypair::new();
    let (banks_client, payer, blockhash) = {
        let ctx = test_f.context.borrow();
        (
            ctx.banks_client.clone(),
            ctx.payer.insecure_clone(),
            ctx.last_blockhash,
        )
    };

    let mut ix = example_ix(
        test_f.marginfi_group.key,
        account_key.pubkey(),
        payer.pubkey(),
    );
    ix.accounts[2] = AccountMeta::new_readonly(Keypair::new().pubkey(), false);

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &account_key],
        blockhash,
    );
    let err = banks_client.process_transaction(tx).await.unwrap_err();
    assert!(
        matches!(
            err,
            BanksClientError::TransactionError(TransactionError::InstructionError(
                0,
                InstructionError::InvalidSeeds
            )) | BanksClientError::SimulationError {
                err: TransactionError::InstructionError(0, InstructionError::InvalidSeeds),
                ..
            }
        ),
        "expected InvalidSeeds from the PDA check, got {err:?}"
    );

    Ok(())
}
