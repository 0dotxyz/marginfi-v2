use fixtures::prelude::*;
use solana_sdk::signer::Signer;

#[tokio::test]
async fn bank_admin_rotation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let group_before = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_before.admin, group_before.bank_admin);

    let new_bank_admin_1 = solana_sdk::signature::Keypair::new();
    let new_bank_admin_2 = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&new_bank_admin_1)
        .await?;

    let group_after_rotate_1 = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after_rotate_1.bank_admin, new_bank_admin_1.pubkey());

    test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin_1, new_bank_admin_2.pubkey())
        .await?;

    let group_after_rotate_2 = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after_rotate_2.bank_admin, new_bank_admin_2.pubkey());

    let new_bank_admin_3 = solana_sdk::signature::Keypair::new();
    let result = test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin_1, new_bank_admin_3.pubkey())
        .await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn set_bank_admin_authorization_after_divergence() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let original_admin = test_f.context.borrow().payer.pubkey();
    let new_bank_admin = solana_sdk::signature::Keypair::new();
    let another_admin = solana_sdk::signature::Keypair::new();

    let group_before = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_before.admin, original_admin);
    assert_eq!(group_before.bank_admin, original_admin);

    test_f
        .marginfi_group
        .try_set_bank_admin(&new_bank_admin)
        .await?;

    let group_after = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after.bank_admin, new_bank_admin.pubkey());
    assert_ne!(group_after.bank_admin, group_after.admin);

    let result = test_f
        .marginfi_group
        .try_set_bank_admin(&another_admin)
        .await;

    assert!(
        result.is_err(),
        "Original admin should NOT be able to set_bank_admin after bank_admin diverged"
    );

    Ok(())
}

#[tokio::test]
async fn set_bank_admin_rejects_default_pubkey() -> anyhow::Result<()> {
    use fixtures::assert_custom_error;
    use marginfi::prelude::MarginfiError;
    use solana_program_test::BanksClientError;

    let test_f = TestFixture::new(None).await;
    let new_bank_admin = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&new_bank_admin)
        .await?;

    let group_after = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after.bank_admin, new_bank_admin.pubkey());

    let result = test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin, solana_sdk::pubkey::Pubkey::default())
        .await;

    assert!(
        result.is_err(),
        "set_bank_admin should reject Pubkey::default()"
    );
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::InvalidBankAdmin);

    let group_final = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_final.bank_admin, new_bank_admin.pubkey());

    Ok(())
}
