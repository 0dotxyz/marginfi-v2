use anchor_lang::prelude::Pubkey;
use anchor_spl::token::spl_token;
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::prelude::*;
use fixtures::{assert_custom_error, native};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::{
    EMISSIONS_AUTH_SEED, EMISSIONS_FLAG_LENDING_ACTIVE, EMISSIONS_TOKEN_ACCOUNT_SEED,
};
use solana_program_test::tokio;
use solana_sdk::program_pack::Pack;

/// Set up emissions on a bank by directly injecting account state.
///
/// On mainnet, emissions were originally set up via a now-removed instruction.
/// This helper replicates that state: it creates the emissions vault PDA token
/// account and sets the bank's emissions fields.
async fn setup_bank_emissions(
    test_f: &TestFixture,
    bank_f: &BankFixture,
    emissions_mint: &MintFixture,
    emissions_amount: u64,
) {
    let (emissions_auth, _) = Pubkey::find_program_address(
        &[
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank_f.key.as_ref(),
            emissions_mint.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let (emissions_vault, _) = Pubkey::find_program_address(
        &[
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank_f.key.as_ref(),
            emissions_mint.key.as_ref(),
        ],
        &marginfi::ID,
    );

    {
        let mut ctx = test_f.context.borrow_mut();
        let rent = ctx.banks_client.get_rent().await.unwrap();
        let space = spl_token::state::Account::LEN;
        let lamports = rent.minimum_balance(space);

        let mut account_data = vec![0u8; space];
        spl_token::state::Account::pack(
            spl_token::state::Account {
                mint: emissions_mint.key,
                owner: emissions_auth,
                amount: emissions_amount,
                state: spl_token::state::AccountState::Initialized,
                ..Default::default()
            },
            &mut account_data,
        )
        .unwrap();

        let vault_account = solana_sdk::account::Account {
            lamports,
            data: account_data,
            owner: spl_token::id(),
            executable: false,
            rent_epoch: 0,
        };
        ctx.set_account(&emissions_vault, &vault_account.into());
    }

    bank_f
        .set_emissions(
            emissions_mint.key,
            1_000_000,
            I80F48::from_num(emissions_amount),
            EMISSIONS_FLAG_LENDING_ACTIVE,
        )
        .await;
}

// ─── lending_pool_reclaim_emissions_vault ───────────────────────────
// NOTE: The no-op path (bank.emissions_mint == default) cannot be tested
// locally because the emissions vault PDA never existed for such banks.
// On mainnet, the vault was created by a now-removed instruction, so the
// no-op early return works there even though we can't replicate it here.

#[tokio::test]
async fn reclaim_emissions_vault_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let emissions_mint = MintFixture::new(test_f.context.clone(), None, None).await;
    let vault_amount: u64 = native!(1000, "USDC");

    setup_bank_emissions(&test_f, usdc_bank_f, &emissions_mint, vault_amount).await;

    let bank = usdc_bank_f.load().await;
    assert_eq!(bank.emissions_mint, emissions_mint.key);
    assert_ne!(bank.emissions_rate, 0);

    let fee_wallet_ata = TokenAccountFixture::new_from_ata(
        test_f.context.clone(),
        &emissions_mint.key,
        &test_f.marginfi_group.fee_wallet,
        &spl_token::id(),
    )
    .await;

    usdc_bank_f
        .try_reclaim_emissions_vault(
            &emissions_mint,
            test_f.marginfi_group.fee_state,
            fee_wallet_ata.key,
        )
        .await?;

    // Bank emissions fields are zeroed
    let bank = usdc_bank_f.load().await;
    assert_eq!(bank.emissions_mint, Pubkey::default());
    assert_eq!(bank.emissions_rate, 0);
    assert_eq!(I80F48::from(bank.emissions_remaining), I80F48::ZERO);
    assert_eq!(bank.flags & EMISSIONS_FLAG_LENDING_ACTIVE, 0);

    // Tokens arrived at the fee wallet ATA
    let ata = TokenAccountFixture::fetch(test_f.context.clone(), fee_wallet_ata.key).await;
    assert_eq!(ata.token.amount, vault_amount);

    Ok(())
}

#[tokio::test]
async fn reclaim_emissions_vault_wrong_ata_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let emissions_mint = MintFixture::new(test_f.context.clone(), None, None).await;
    setup_bank_emissions(&test_f, usdc_bank_f, &emissions_mint, native!(100, "USDC")).await;

    // Token account that is NOT the canonical ATA of the global fee wallet
    let wrong_destination = emissions_mint.create_empty_token_account().await;

    let res = usdc_bank_f
        .try_reclaim_emissions_vault(
            &emissions_mint,
            test_f.marginfi_group.fee_state,
            wrong_destination.key,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidFeeAta);

    Ok(())
}

#[tokio::test]
async fn reclaim_emissions_vault_empty_vault() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let emissions_mint = MintFixture::new(test_f.context.clone(), None, None).await;
    setup_bank_emissions(&test_f, usdc_bank_f, &emissions_mint, 0).await;

    let fee_wallet_ata = TokenAccountFixture::new_from_ata(
        test_f.context.clone(),
        &emissions_mint.key,
        &test_f.marginfi_group.fee_wallet,
        &spl_token::id(),
    )
    .await;

    // Succeeds even with empty vault (no transfer, but still clears state)
    usdc_bank_f
        .try_reclaim_emissions_vault(
            &emissions_mint,
            test_f.marginfi_group.fee_state,
            fee_wallet_ata.key,
        )
        .await?;

    let bank = usdc_bank_f.load().await;
    assert_eq!(bank.emissions_mint, Pubkey::default());
    assert_eq!(bank.emissions_rate, 0);

    Ok(())
}
