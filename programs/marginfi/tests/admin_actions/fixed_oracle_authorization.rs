use fixed_macro::types::I80F48;
use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;

#[tokio::test]
async fn set_oracle_price_governance_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Fixed,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Fixed);
    let governance_admin = solana_sdk::signature::Keypair::new();

    let original_admin = test_f.context.borrow().payer.insecure_clone();

    test_f
        .marginfi_group
        .try_set_governance_admin(&governance_admin)
        .await?;

    let new_price = I80F48!(5.5).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_oracle_price_with_signer(&governance_admin, &bank, new_price)
        .await;
    assert!(
        result.is_ok(),
        "governance_admin should be able to set_oracle_price"
    );

    let another_price = I80F48!(7.2).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_oracle_price_with_signer(&original_admin, &bank, another_price)
        .await;
    assert!(
        result.is_err(),
        "fast admin should NOT be able to set_oracle_price when governance_admin differs"
    );
    let err = result.unwrap_err();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    Ok(())
}
