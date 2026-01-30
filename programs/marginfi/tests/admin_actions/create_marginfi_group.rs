use anchor_lang::solana_program::{instruction::Instruction, system_program};
use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::prelude::*;
use marginfi::state::marginfi_group::MarginfiGroupImpl;
use marginfi_type_crate::constants::FEE_STATE_SEED;
use marginfi_type_crate::types::MarginfiGroup;
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

#[tokio::test]
async fn marginfi_group_create_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Create & initialize marginfi group
    let marginfi_group_key = Keypair::new();

    let (fee_state_key, _bump) =
        Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::ID);

    let accounts = marginfi::accounts::MarginfiGroupInitialize {
        marginfi_group: marginfi_group_key.pubkey(),
        admin: test_f.payer(),
        fee_state: fee_state_key,
        system_program: system_program::id(),
    };
    let init_marginfi_group_ix = Instruction {
        program_id: marginfi::ID,
        accounts: accounts.to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiGroupInitialize {}.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[init_marginfi_group_ix],
        Some(&test_f.payer().clone()),
        &[&test_f.payer_keypair(), &marginfi_group_key],
        test_f.get_latest_blockhash().await,
    );
    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok());

    // Fetch & deserialize marginfi group account
    let marginfi_group: MarginfiGroup = test_f
        .load_and_deserialize(&marginfi_group_key.pubkey())
        .await;

    // Check basic properties
    assert_eq!(marginfi_group.admin, test_f.payer());
    // Program fees are always enabled by default (Note that mostly elsewhere in the test fixture,
    // we send a config to disable them, to simplify testing)
    assert_eq!(marginfi_group.program_fees_enabled(), true);
    assert_eq!(marginfi_group.fee_state_cache.last_update, 0);

    Ok(())
}
