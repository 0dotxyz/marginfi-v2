use fixed_macro::types::I80F48;
use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use solana_sdk::signer::Signer;

#[tokio::test]
async fn set_fixed_oracle_price_bank_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Fixed,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Fixed);
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    let original_admin = test_f.context.borrow().payer.insecure_clone();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let new_price = I80F48!(5.5).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_fixed_oracle_price_with_signer(&bank_admin_kp, &bank, new_price)
        .await;
    assert!(
        result.is_ok(),
        "bank_admin should be able to set_fixed_oracle_price"
    );

    let another_price = I80F48!(7.2).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_fixed_oracle_price_with_signer(&original_admin, &bank, another_price)
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to set_fixed_oracle_price when bank_admin != admin"
    );
    let err = result.unwrap_err();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    Ok(())
}
