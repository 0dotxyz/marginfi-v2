use anchor_lang::{prelude::*, solana_program::instruction::Instruction, InstructionData};
use fixtures::{assert_custom_error, prelude::*};
use marginfi::errors::MarginfiError;
use solana_program_test::*;
use solana_sdk::{hash::Hash, signature::Keypair, signer::Signer, transaction::Transaction};
use solana_system_interface::instruction::{advance_nonce_account, create_nonce_account};

/// Serialized size of a nonce account (`solana_nonce::state::State::size()`).
const NONCE_ACCOUNT_LEN: usize = 80;
/// Byte range of the stored durable nonce hash inside a nonce account: `Versions` tag (4) +
/// `State` tag (4) + authority (32), then the 32-byte hash.
const NONCE_HASH_RANGE: std::ops::Range<usize> = 40..72;

/// Clones the banks client out of the fixture so no `RefCell` borrow is held across an await.
fn banks_client(test_f: &TestFixture) -> BanksClient {
    test_f.context.borrow().banks_client.clone()
}

/// Creates a nonce account authorized by `authority` and returns it with its durable nonce hash,
/// ready to be used as a transaction's `recent_blockhash`.
async fn create_nonce(
    test_f: &TestFixture,
    authority: &Keypair,
) -> anyhow::Result<(Keypair, Hash)> {
    let nonce = Keypair::new();
    let lamports = test_f.get_minimum_rent_for_size(NONCE_ACCOUNT_LEN).await;
    let tx = Transaction::new_signed_with_payer(
        &create_nonce_account(
            &authority.pubkey(),
            &nonce.pubkey(),
            &authority.pubkey(),
            lamports,
        ),
        Some(&authority.pubkey()),
        &[authority, &nonce],
        test_f.get_latest_blockhash().await,
    );
    banks_client(test_f).process_transaction(tx).await?;

    // The runtime refuses to consume a nonce until the chain has moved past the blockhash the
    // nonce was initialized against.
    test_f.advance_time(1).await;

    let account = banks_client(test_f)
        .get_account(nonce.pubkey())
        .await?
        .expect("nonce account exists");
    let hash = Hash::new_from_array(account.data[NONCE_HASH_RANGE].try_into()?);
    Ok((nonce, hash))
}

fn configure_bank_premium_ix(test_f: &TestFixture, admin: Pubkey) -> Instruction {
    Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::LendingPoolConfigureBankPremium {
            group: test_f.marginfi_group.key,
            admin,
            bank: test_f.get_bank(&BankMint::Usdc).key,
            instruction_sysvar: solana_instructions_sysvar::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankPremium {
            premium_tag: 1,
            active: true,
        }
        .data(),
    }
}

#[tokio::test]
async fn admin_instruction_rejects_durable_nonce_transaction() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let admin = test_f.payer_keypair();
    let (nonce, nonce_hash) = create_nonce(&test_f, &admin).await?;

    let tx = Transaction::new_signed_with_payer(
        &[
            advance_nonce_account(&nonce.pubkey(), &admin.pubkey()),
            configure_bank_premium_ix(&test_f, admin.pubkey()),
        ],
        Some(&admin.pubkey()),
        &[&admin],
        nonce_hash,
    );
    let res = banks_client(&test_f)
        .process_transaction_with_preflight(tx)
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::DurableNonceNotAllowed);
    Ok(())
}

/// The scan is not limited to instruction 0 (the only position the runtime treats as a durable
/// nonce marker): an `AdvanceNonceAccount` anywhere in an admin transaction is rejected, even
/// when the transaction itself uses a recent blockhash.
#[tokio::test]
async fn admin_instruction_rejects_advance_nonce_at_any_index() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let admin = test_f.payer_keypair();
    let (nonce, _) = create_nonce(&test_f, &admin).await?;

    let tx = Transaction::new_signed_with_payer(
        &[
            configure_bank_premium_ix(&test_f, admin.pubkey()),
            advance_nonce_account(&nonce.pubkey(), &admin.pubkey()),
        ],
        Some(&admin.pubkey()),
        &[&admin],
        test_f.get_latest_blockhash().await,
    );
    let res = banks_client(&test_f)
        .process_transaction_with_preflight(tx)
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::DurableNonceNotAllowed);
    Ok(())
}

#[tokio::test]
async fn admin_instruction_accepts_recent_blockhash_transaction() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let admin = test_f.payer_keypair();

    let tx = Transaction::new_signed_with_payer(
        &[configure_bank_premium_ix(&test_f, admin.pubkey())],
        Some(&admin.pubkey()),
        &[&admin],
        test_f.get_latest_blockhash().await,
    );
    banks_client(&test_f).process_transaction(tx).await?;

    Ok(())
}
