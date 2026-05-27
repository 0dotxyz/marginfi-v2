use fixed::types::I80F48 as FixedI80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use marginfi_type_crate::types::{BankConfig, BankConfigOpt};
use solana_sdk::{signer::Signer, transaction::Transaction};

pub(super) const KAMINO_ROUNDING_TOLERANCE_NATIVE: i128 = 1;
pub(super) const DRIFT_ROUNDING_TOLERANCE_NATIVE: i128 = 1;
pub(super) const JUPLEND_ROUNDING_TOLERANCE_NATIVE: i128 = 30;
pub(super) const SAME_ASSET_BANK_SEED: u64 = 4_497;
pub(super) const SAME_ASSET_KAMINO_BANK_SEED: u64 = 6_497;

pub(super) async fn add_same_asset_regular_bank(
    test_f: &TestFixture,
    mint: BankMint,
    seed: u64,
) -> anyhow::Result<BankFixture> {
    let bank = match mint {
        BankMint::Usdc => {
            test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &test_f.usdc_mint,
                    None,
                    *DEFAULT_USDC_TEST_BANK_CONFIG,
                    seed,
                )
                .await?
        }
        BankMint::Sol => {
            test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &test_f.sol_mint,
                    None,
                    *DEFAULT_SOL_TEST_BANK_CONFIG,
                    seed,
                )
                .await?
        }
        BankMint::Fixed => {
            test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &test_f.fixed_mint,
                    None,
                    *DEFAULT_FIXED_TEST_BANK_CONFIG,
                    seed,
                )
                .await?
        }
        BankMint::PyUSD => {
            test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &test_f.pyusd_mint,
                    None,
                    *DEFAULT_PYUSD_TEST_BANK_CONFIG,
                    seed,
                )
                .await?
        }
        _ => {
            return Err(anyhow::anyhow!(
                "unsupported same-asset helper mint {:?}",
                mint
            ));
        }
    };

    Ok(bank)
}

pub(super) async fn add_same_asset_regular_bank_with_mint_fixture(
    test_f: &TestFixture,
    mint_fixture: &MintFixture,
    bank_config: BankConfig,
    seed: u64,
) -> anyhow::Result<BankFixture> {
    Ok(test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(mint_fixture, None, bank_config, seed)
        .await?)
}

pub(super) async fn set_bank_asset_weights(
    bank: &BankFixture,
    asset_weight_init: f64,
    asset_weight_maint: f64,
) -> anyhow::Result<()> {
    bank.update_config(
        BankConfigOpt {
            asset_weight_init: Some(FixedI80F48::from_num(asset_weight_init).into()),
            asset_weight_maint: Some(FixedI80F48::from_num(asset_weight_maint).into()),
            ..Default::default()
        },
        None,
    )
    .await?;

    Ok(())
}

pub(super) async fn configure_same_asset_pair(
    test_f: &TestFixture,
    bank_a: &BankFixture,
    bank_b: &BankFixture,
    asset_weight_init: f64,
    asset_weight_maint: f64,
    same_asset_init_leverage: i64,
    same_asset_maint_leverage: i64,
) -> anyhow::Result<()> {
    set_bank_asset_weights(bank_a, asset_weight_init, asset_weight_maint).await?;
    set_bank_asset_weights(bank_b, asset_weight_init, asset_weight_maint).await?;
    reconfigure_same_asset_leverage(test_f, same_asset_init_leverage, same_asset_maint_leverage)
        .await?;

    Ok(())
}

pub(super) async fn reconfigure_same_asset_leverage(
    test_f: &TestFixture,
    init_leverage: i64,
    maint_leverage: i64,
) -> anyhow::Result<()> {
    let group = test_f.marginfi_group.load().await;
    test_f
        .marginfi_group
        .try_update_with_same_asset_emode_leverage(
            group.admin,
            group.emode_admin,
            group.delegate_curve_admin,
            group.delegate_limit_admin,
            group.delegate_emissions_admin,
            group.metadata_admin,
            group.risk_admin,
            Some(FixedI80F48::from_num(init_leverage).into()),
            Some(FixedI80F48::from_num(maint_leverage).into()),
        )
        .await?;
    Ok(())
}

pub(super) async fn add_same_asset_regular_kamino_bank(
    setup: &KaminoBankSetup,
) -> anyhow::Result<BankFixture> {
    add_same_asset_regular_bank_with_mint_fixture(
        &setup.test_f,
        &setup.bank_f.mint,
        *DEFAULT_USDC_TEST_BANK_CONFIG,
        SAME_ASSET_KAMINO_BANK_SEED,
    )
    .await
}

pub(super) async fn refresh_same_asset_kamino_position(
    setup: &KaminoBankSetup,
    user: &MarginfiAccountFixture,
) -> anyhow::Result<()> {
    let refresh_reserve_ix = user.make_kamino_refresh_reserve_ix(&setup.bank_f).await;
    let refresh_obligation_ix = user.make_kamino_refresh_obligation_ix(&setup.bank_f).await;

    let ctx = setup.test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[refresh_reserve_ix, refresh_obligation_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;

    Ok(())
}
