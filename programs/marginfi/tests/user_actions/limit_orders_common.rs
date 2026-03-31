use fixtures::{bank::BankFixture, marginfi_account::MarginfiAccountFixture, prelude::*};

pub async fn seed_liquidity(
    test_f: &TestFixture,
    bank_f: &BankFixture,
    ui_amount: f64,
) -> anyhow::Result<()> {
    let lender = test_f.create_marginfi_account().await;
    let lender_wallet_balance = get_max_deposit_amount_pre_fee(ui_amount);
    let lender_token_account = bank_f
        .mint
        .create_token_account_and_mint_to(lender_wallet_balance)
        .await;
    test_f.refresh_blockhash().await;
    lender
        .try_bank_deposit(lender_token_account.key, bank_f, ui_amount, None)
        .await?;
    Ok(())
}

pub async fn create_account_with_positions(
    test_f: &TestFixture,
    assets: &[(BankMint, f64)],
    liabilities: &[(BankMint, f64)],
) -> anyhow::Result<MarginfiAccountFixture> {
    // Seed liquidity for all liabilities so the borrower can borrow.
    for (mint, amount) in liabilities {
        let bank_f = test_f.get_bank(mint);
        let liquidity_seed = (*amount * 10.0).max(1_000.0);
        seed_liquidity(test_f, bank_f, liquidity_seed).await?;
    }

    let borrower = test_f.create_marginfi_account().await;

    for (mint, amount) in assets {
        let bank_f = test_f.get_bank(mint);
        let wallet_balance = get_max_deposit_amount_pre_fee(*amount);
        let token_account = bank_f
            .mint
            .create_token_account_and_mint_to(wallet_balance)
            .await;
        test_f.refresh_blockhash().await;
        borrower
            .try_bank_deposit(token_account.key, bank_f, *amount, None)
            .await?;
    }

    for (mint, amount) in liabilities {
        let bank_f = test_f.get_bank(mint);
        let borrow_account = bank_f.mint.create_empty_token_account().await;
        test_f.refresh_blockhash().await;
        borrower
            .try_bank_borrow(borrow_account.key, bank_f, *amount)
            .await?;
    }

    Ok(borrower)
}

pub fn test_settings_16_banks() -> TestSettings {
    TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::UsdcT22,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::FixedLow,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent7,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolSwbPull,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolSwbOrigFee,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent6,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Sol,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::Fixed,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent1,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent2,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent3,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent4,
                ..TestBankSetting::default()
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent5,
                ..TestBankSetting::default()
            },
        ],
        protocol_fees: false,
    }
}
