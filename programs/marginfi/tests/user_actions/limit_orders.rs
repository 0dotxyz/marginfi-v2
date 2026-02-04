use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::{
    assert_custom_error, assert_eq_noise, bank::BankFixture,
    marginfi_account::MarginfiAccountFixture, prelude::*, ui_to_native,
};
use marginfi::{constants::MAX_BPS, prelude::MarginfiError, state::bank::BankVaultType};
use marginfi_type_crate::types::{OrderTrigger, WrappedI80F48};
use solana_program_test::tokio;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program, sysvar,
    transaction::Transaction,
};

/// Helper to create an OrderTrigger with a stop-loss threshold.
fn stop_loss_trigger(threshold: I80F48, max_slippage: u16) -> OrderTrigger {
    OrderTrigger::StopLoss {
        threshold: WrappedI80F48::from(threshold),
        max_slippage,
    }
}

/// Helper to create an OrderTrigger with a take-profit threshold.
fn take_profit_trigger(threshold: I80F48, max_slippage: u16) -> OrderTrigger {
    OrderTrigger::TakeProfit {
        threshold: WrappedI80F48::from(threshold),
        max_slippage,
    }
}

async fn setup_limit_order_fixture(
    asset_mint: BankMint,
    asset_deposit: f64,
    liability_mint: BankMint,
    liability_borrow: f64,
    trigger: OrderTrigger,
) -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    BankMint, // asset mint
    BankMint, // liability mint
    Pubkey,   // order PDA
    Keypair,  // keeper
    Pubkey,   // keeper liability token account
    Pubkey,   // keeper asset token account
)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let asset_bank_f = test_f.get_bank(&asset_mint);
    let liability_bank_f = test_f.get_bank(&liability_mint);

    let borrower_mfi_account_f = create_borrower_with_positions(
        &test_f,
        asset_bank_f,
        asset_deposit,
        liability_bank_f,
        liability_borrow,
    )
    .await?;

    // set emissions destination to the authority before placing order
    let authority = borrower_mfi_account_f.load().await.authority;
    borrower_mfi_account_f
        .try_set_emissions_destination(authority)
        .await?;

    let bank_keys = vec![asset_bank_f.key, liability_bank_f.key];
    let order_pda = borrower_mfi_account_f
        .try_place_order(bank_keys, trigger)
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;

    let keeper_liab_account = liability_bank_f
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), liability_borrow * 10.0)
        .await
        .key;
    let keeper_asset_account = asset_bank_f
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    Ok((
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ))
}

fn estimate_withdraw_amount(liability_ui: f64, asset_price: f64) -> f64 {
    liability_ui / asset_price
}

fn default_price_for_mint(mint: &BankMint) -> f64 {
    get_mint_price(mint.clone())
}

fn withdraw_scale_for_profit_pct(
    asset_deposit: f64,
    liability_borrow: f64,
    asset_price: f64,
    liability_price: f64,
    profit_pct: f64,
) -> f64 {
    let asset_value = asset_deposit * asset_price;
    let liability_value = liability_borrow * liability_price;
    let start_health = asset_value - liability_value;
    1.0 + (start_health * profit_pct) / liability_value
}

async fn make_start_execute_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    order: Pubkey,
    executor: Pubkey,
    observation_metas: Option<Vec<AccountMeta>>,
) -> anyhow::Result<(Instruction, Pubkey)> {
    let marginfi_account = marginfi_account_f.load().await;
    let (execute_record, _) = find_execute_order_pda(&order);

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::StartExecuteOrder {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_f.key,
            fee_payer: executor,
            executor,
            order,
            execute_record,
            instruction_sysvar: sysvar::instructions::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountStartExecuteOrder {}.data(),
    };

    let observation_metas = match observation_metas {
        Some(metas) => metas,
        None => {
            marginfi_account_f
                .load_observation_account_metas(vec![], vec![])
                .await
        }
    };

    ix.accounts.extend_from_slice(&observation_metas);

    Ok((ix, execute_record))
}

async fn make_end_execute_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    order: Pubkey,
    execute_record: Pubkey,
    executor: Pubkey,
    fee_recipient: Pubkey,
    exclude_banks: Vec<Pubkey>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::EndExecuteOrder {
            group: marginfi_account.group,
            marginfi_account: marginfi_account_f.key,
            executor,
            fee_recipient,
            order,
            execute_record,
            fee_state: Pubkey::find_program_address(
                &[marginfi_type_crate::constants::FEE_STATE_SEED.as_bytes()],
                &marginfi::ID,
            )
            .0,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::MarginfiAccountEndExecuteOrder {}.data(),
    };

    ix.accounts.extend_from_slice(
        &marginfi_account_f
            .load_observation_account_metas(vec![], exclude_banks)
            .await,
    );

    Ok(ix)
}

async fn make_repay_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    bank_f: &BankFixture,
    authority: Pubkey,
    signer_token_account: Pubkey,
    ui_amount: f64,
    repay_all: Option<bool>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut accounts = marginfi::accounts::LendingAccountRepay {
        group: marginfi_account.group,
        marginfi_account: marginfi_account_f.key,
        authority,
        bank: bank_f.key,
        signer_token_account,
        liquidity_vault: bank_f.get_vault(BankVaultType::Liquidity).0,
        token_program: bank_f.get_token_program(),
    }
    .to_account_metas(Some(true));

    if bank_f.mint.token_program == anchor_spl::token_2022::ID {
        accounts.push(AccountMeta::new_readonly(bank_f.mint.key, false));
    }

    let ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountRepay {
            amount: ui_to_native!(ui_amount, bank_f.mint.mint.decimals),
            repay_all,
        }
        .data(),
    };

    Ok(ix)
}

async fn make_withdraw_ix(
    marginfi_account_f: &MarginfiAccountFixture,
    bank_f: &BankFixture,
    authority: Pubkey,
    destination: Pubkey,
    ui_amount: f64,
    withdraw_all: Option<bool>,
) -> anyhow::Result<Instruction> {
    let marginfi_account = marginfi_account_f.load().await;

    let mut accounts = marginfi::accounts::LendingAccountWithdraw {
        group: marginfi_account.group,
        marginfi_account: marginfi_account_f.key,
        authority,
        bank: bank_f.key,
        destination_token_account: destination,
        bank_liquidity_vault_authority: bank_f.get_vault_authority(BankVaultType::Liquidity).0,
        liquidity_vault: bank_f.get_vault(BankVaultType::Liquidity).0,
        token_program: bank_f.get_token_program(),
    }
    .to_account_metas(Some(true));

    if bank_f.mint.token_program == anchor_spl::token_2022::ID {
        accounts.push(AccountMeta::new_readonly(bank_f.mint.key, false));
    }

    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts,
        data: marginfi::instruction::LendingAccountWithdraw {
            amount: ui_to_native!(ui_amount, bank_f.mint.mint.decimals),
            withdraw_all,
        }
        .data(),
    };

    ix.accounts.extend_from_slice(
        &marginfi_account_f
            .load_observation_account_metas(vec![], vec![])
            .await,
    );

    Ok(ix)
}

async fn fund_keeper_for_fees(test_f: &TestFixture, keeper: &Keypair) -> anyhow::Result<()> {
    let mut ctx = test_f.context.borrow_mut();
    let rent = ctx.banks_client.get_rent().await?;
    let min_balance = rent.minimum_balance(0);
    let account = Account {
        lamports: min_balance + 1_000_000_000,
        data: vec![],
        owner: solana_sdk::system_program::ID,
        executable: false,
        rent_epoch: 0,
    };
    ctx.set_account(&keeper.pubkey(), &account.into());
    Ok(())
}

async fn create_borrower_with_positions(
    test_f: &TestFixture,
    asset_bank_f: &BankFixture,
    asset_deposit: f64,
    liability_bank_f: &BankFixture,
    liability_borrow: f64,
) -> anyhow::Result<MarginfiAccountFixture> {
    let liquidity_seed = (liability_borrow * 10.0).max(1_000.0);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account = liability_bank_f
        .mint
        .create_token_account_and_mint_to(liquidity_seed)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(
            lender_token_account.key,
            liability_bank_f,
            liquidity_seed,
            None,
        )
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_asset_account = asset_bank_f
        .mint
        .create_token_account_and_mint_to(asset_deposit)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(
            borrower_asset_account.key,
            asset_bank_f,
            asset_deposit,
            None,
        )
        .await?;

    let borrower_liability_account = liability_bank_f.mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_borrow(
            borrower_liability_account.key,
            liability_bank_f,
            liability_borrow,
        )
        .await?;

    Ok(borrower_mfi_account_f)
}

async fn execute_order_with_withdraw_scale(
    test_f: &TestFixture,
    borrower_mfi_account_f: &MarginfiAccountFixture,
    asset_mint: &BankMint,
    liability_mint: &BankMint,
    liability_borrow: f64,
    order_pda: Pubkey,
    keeper: &Keypair,
    keeper_liab_account: Pubkey,
    keeper_asset_account: Pubkey,
    withdraw_scale: f64,
) -> Result<(), solana_program_test::BanksClientError> {
    let price = default_price_for_mint(asset_mint);
    let asset_bank_f = test_f.get_bank(asset_mint);
    let liability_bank_f = test_f.get_bank(liability_mint);

    let (start_ix, execute_record) =
        make_start_execute_ix(borrower_mfi_account_f, order_pda, keeper.pubkey(), None)
            .await
            .unwrap();

    let repay_ix = make_repay_ix(
        borrower_mfi_account_f,
        liability_bank_f,
        keeper.pubkey(),
        keeper_liab_account,
        0.0,
        Some(true),
    )
    .await
    .unwrap();

    let withdraw_amt = estimate_withdraw_amount(
        default_price_for_mint(liability_mint) * liability_borrow,
        price,
    ) * withdraw_scale;
    let withdraw_ix = make_withdraw_ix(
        borrower_mfi_account_f,
        asset_bank_f,
        keeper.pubkey(),
        keeper_asset_account,
        withdraw_amt,
        None,
    )
    .await
    .unwrap();

    let end_ix = make_end_execute_ix(
        borrower_mfi_account_f,
        order_pda,
        execute_record,
        keeper.pubkey(),
        keeper.pubkey(),
        vec![liability_bank_f.key],
    )
    .await
    .unwrap();

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, repay_ix, withdraw_ix, end_ix],
        Some(&keeper.pubkey()),
        &[keeper],
        ctx.last_blockhash,
    );

    ctx.banks_client.process_transaction(tx).await
}

// Note: these tests have  a silly setup: the user deposits $500 and borrows $100, making the
// take-profit instantly eligible.

// Here the keeper earns no profit, the user keeps their entire $400 position: the keeper neatly
// repays $100 USDC and withdraws $100 SOL
#[tokio::test]
async fn limit_order_take_profit_happy_path() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 50.0;
    let liability_borrow = 100.0;
    let trigger = take_profit_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let price = default_price_for_mint(&asset_mint);
    let asset_bank_f = test_f.get_bank(&asset_mint);
    let _liability_bank_f = test_f.get_bank(&liability_mint);

    let order_before = borrower_mfi_account_f.load_order(order_pda).await;
    let mfi_before = borrower_mfi_account_f.load().await;

    execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        1.0,
    )
    .await?;

    let order_after = test_f.try_load(&order_pda).await?;
    assert!(
        order_after.is_none(),
        "order should be closed after execution"
    );

    let mfi_after = borrower_mfi_account_f.load().await;
    let asset_tag = order_before.tags[0];
    let liab_tag = order_before.tags[1];

    let pre_asset = mfi_before
        .lending_account
        .balances
        .iter()
        .find(|b| b.tag == asset_tag)
        .unwrap();
    let pre_liab = mfi_before
        .lending_account
        .balances
        .iter()
        .find(|b| b.tag == liab_tag)
        .unwrap();

    let post_asset = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == pre_asset.bank_pk);
    assert!(post_asset.is_some(), "asset balance should remain");

    let post_liab = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == pre_liab.bank_pk);
    assert!(post_liab.is_none(), "liability balance should be removed");

    // sanity: compare value to trigger
    let post_asset_shares: I80F48 = post_asset.unwrap().asset_shares.into();
    let asset_native =
        post_asset_shares.to_num::<f64>() / 10f64.powi(asset_bank_f.mint.mint.decimals as i32);
    let asset_value = asset_native * price;
    let max_bps: f64 = MAX_BPS.into();
    let max_slippage: f64 = 0.0;
    assert!(asset_value >= 100.0 * (1.0 - (max_slippage / max_bps)));
    assert_eq_noise!(asset_value, 400.0, 0.5);

    Ok(())
}

// Here the keeper earns the max profit (5% in this test's setup), the user keeps $380: the keeper
// neatly repays $100 USDC and withdraws $120 SOL
#[tokio::test]
async fn limit_order_take_profit_max_profit_allowed() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 50.0;
    let liability_borrow = 100.0;
    let trigger = take_profit_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_price = default_price_for_mint(&asset_mint);
    let liability_price = default_price_for_mint(&liability_mint);
    let withdraw_scale = withdraw_scale_for_profit_pct(
        asset_deposit,
        liability_borrow,
        asset_price,
        liability_price,
        0.049,
    );

    execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        withdraw_scale,
    )
    .await?;

    let order_after = test_f.try_load(&order_pda).await?;
    assert!(
        order_after.is_none(),
        "order should be closed after execution"
    );

    let asset_bank_f = test_f.get_bank(&asset_mint);
    let price = default_price_for_mint(&asset_mint);
    let mfi_after = borrower_mfi_account_f.load().await;
    let post_asset = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == asset_bank_f.key)
        .expect("asset balance should remain");
    let post_asset_shares: I80F48 = post_asset.asset_shares.into();
    let asset_native =
        post_asset_shares.to_num::<f64>() / 10f64.powi(asset_bank_f.mint.mint.decimals as i32);
    let asset_value = asset_native * price;
    assert_eq_noise!(asset_value, 380.0, 0.5);

    Ok(())
}

// Here the keeper gets greedy and tries to take 5.1%, which fails due to too much profit.
#[tokio::test]
async fn limit_order_take_profit_too_much_profit_fails() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 50.0;
    let liability_borrow = 100.0;
    let trigger = take_profit_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_price = default_price_for_mint(&asset_mint);
    let liability_price = default_price_for_mint(&liability_mint);
    let withdraw_scale = withdraw_scale_for_profit_pct(
        asset_deposit,
        liability_borrow,
        asset_price,
        liability_price,
        0.051,
    );

    let result = execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        withdraw_scale,
    )
    .await;

    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderTriggerNotMet);
    Ok(())
}

// In these stop loss tests, the user depoits 20 SOL ($200) and borrows 150 USDC ($150), then sets a
// stop loss at exactly $50. Since they are at exactly $50, the stop loss is triggerable, but
// keepers can't make any profit! In a real-world scenario, keepers that aren't friendly might sit
// on this until there is some profit to be made. Note that we also set max slippage to zero here,
// real keepers might not even bother picking this up since it's more or less impossible to serve.
// The happy path demonstrates a zero-profit keeper.
#[tokio::test]
async fn limit_order_stop_loss_happy_path() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 20.0;
    let liability_borrow = 150.0;
    let trigger = stop_loss_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        1.0,
    )
    .await?;

    let order_after = test_f.try_load(&order_pda).await?;
    assert!(
        order_after.is_none(),
        "order should be closed after execution"
    );

    Ok(())
}

// Here the keeper is even friendlier: they take a loss of 0.1%
#[tokio::test]
async fn limit_order_stop_loss_just_under_max_profit_allowed() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 20.0;
    let liability_borrow = 150.0;
    let trigger = stop_loss_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_price = default_price_for_mint(&asset_mint);
    let liability_price = default_price_for_mint(&liability_mint);
    let withdraw_scale = withdraw_scale_for_profit_pct(
        asset_deposit,
        liability_borrow,
        asset_price,
        liability_price,
        -0.001,
    );

    execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        withdraw_scale,
    )
    .await?;

    let order_after = test_f.try_load(&order_pda).await?;
    assert!(
        order_after.is_none(),
        "order should be closed after execution"
    );

    Ok(())
}

// Here the keeper ties to make a profit, but it's impossible when the trigger is just-met like
// this, which from the user PoV is exactly the right time to execute the stop loss.
#[tokio::test]
async fn limit_order_stop_loss_too_much_profit_fails() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 20.0;
    let liability_borrow = 150.0;
    let trigger = stop_loss_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_price = default_price_for_mint(&asset_mint);
    let liability_price = default_price_for_mint(&liability_mint);
    let withdraw_scale = withdraw_scale_for_profit_pct(
        asset_deposit,
        liability_borrow,
        asset_price,
        liability_price,
        0.001,
    );

    let result = execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        withdraw_scale,
    )
    .await;

    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderTriggerNotMet);
    Ok(())
}

// In this more realistic scenario, the user accepts a stop loss with up to 5% slippage. Keepers
// will naturally attempt to maximize slippage to keep it as profit. Here the keeper claims 4.99% of
// the up-to-5% slippage.
#[tokio::test]
async fn limit_order_stop_loss_max_profit_with_slippage() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 20.0;
    let liability_borrow = 150.0;
    let max_slippage: u16 = 500; // 5%
    let trigger = stop_loss_trigger(fp!(100), max_slippage);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_price = default_price_for_mint(&asset_mint);
    let liability_price = default_price_for_mint(&liability_mint);
    let max_bps: f64 = MAX_BPS.into();
    // 4.99% to avoid issues due to rounding
    let profit_pct = ((max_slippage - 1) as f64) / max_bps;
    let withdraw_scale = withdraw_scale_for_profit_pct(
        asset_deposit,
        liability_borrow,
        asset_price,
        liability_price,
        profit_pct,
    );

    execute_order_with_withdraw_scale(
        &test_f,
        &borrower_mfi_account_f,
        &asset_mint,
        &liability_mint,
        liability_borrow,
        order_pda,
        &keeper,
        keeper_liab_account,
        keeper_asset_account,
        withdraw_scale,
    )
    .await?;

    let order_after = test_f.try_load(&order_pda).await?;
    assert!(
        order_after.is_none(),
        "order should be closed after execution"
    );

    let asset_bank_f = test_f.get_bank(&asset_mint);
    let mfi_after = borrower_mfi_account_f.load().await;
    let post_asset = mfi_after
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == asset_bank_f.key)
        .expect("asset balance should remain");
    let post_asset_shares: I80F48 = post_asset.asset_shares.into();
    let asset_native =
        post_asset_shares.to_num::<f64>() / 10f64.powi(asset_bank_f.mint.mint.decimals as i32);
    let asset_value = asset_native * asset_price;
    let start_health =
        (asset_deposit * asset_price) - (liability_borrow * liability_price);
    let expected_min = start_health * (1.0 - profit_pct);
    assert!(asset_value >= expected_min);
    assert_eq_noise!(asset_value, expected_min, 0.5);

    Ok(())
}

#[tokio::test]
async fn limit_order_start_fails_when_trigger_not_met() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 10.0;
    let liability_borrow = 90.0;
    let trigger = take_profit_trigger(fp!(1_000), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        _asset_mint,
        _liability_mint,
        order_pda,
        keeper,
        _keeper_liab_account,
        _keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let (start_ix, _execute_record) =
        make_start_execute_ix(&borrower_mfi_account_f, order_pda, keeper.pubkey(), None).await?;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix],
        Some(&keeper.pubkey()),
        &[&keeper],
        ctx.last_blockhash,
    );

    let result = ctx.banks_client.process_transaction(tx).await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderTriggerNotMet);
    Ok(())
}

#[tokio::test]
async fn limit_order_start_succeeds_after_fixed_price_shift() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 10.0;
    let liability_borrow = 90.0;
    let trigger = take_profit_trigger(fp!(1_000), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        _liability_mint,
        order_pda,
        keeper,
        _keeper_liab_account,
        _keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // Switch asset bank to fixed price high enough to satisfy the trigger.
    let asset_bank_f = test_f.get_bank(&asset_mint);
    let fixed_price = WrappedI80F48::from(fp!(150));
    let set_fixed_ix = test_f
        .marginfi_group
        .make_lending_pool_set_fixed_oracle_price_ix(asset_bank_f, fixed_price);

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[set_fixed_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let (start_ix, execute_record) =
        make_start_execute_ix(&borrower_mfi_account_f, order_pda, keeper.pubkey(), None).await?;
    let end_ix = make_end_execute_ix(
        &borrower_mfi_account_f,
        order_pda,
        execute_record,
        keeper.pubkey(),
        keeper.pubkey(),
        vec![],
    )
    .await?;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, end_ix],
        Some(&keeper.pubkey()),
        &[&keeper],
        ctx.last_blockhash,
    );

    let result = ctx.banks_client.process_transaction(tx).await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderLiabilityNotClosed);
    Ok(())
}

#[tokio::test]
async fn limit_order_fails_keeper_overwithdraw() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 50.0;
    let liability_borrow = 100.0;
    let trigger = take_profit_trigger(fp!(100), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        keeper_liab_account,
        keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let price = default_price_for_mint(&asset_mint);
    let asset_bank_f = test_f.get_bank(&asset_mint);
    let liability_bank_f = test_f.get_bank(&liability_mint);

    let (start_ix, execute_record) =
        make_start_execute_ix(&borrower_mfi_account_f, order_pda, keeper.pubkey(), None).await?;

    let repay_ix = make_repay_ix(
        &borrower_mfi_account_f,
        liability_bank_f,
        keeper.pubkey(),
        keeper_liab_account,
        0.0,
        Some(true),
    )
    .await?;

    let withdraw_amt = estimate_withdraw_amount(
        default_price_for_mint(&liability_mint) * liability_borrow,
        price,
    ) * 1.25;
    let withdraw_ix = make_withdraw_ix(
        &borrower_mfi_account_f,
        asset_bank_f,
        keeper.pubkey(),
        keeper_asset_account,
        withdraw_amt,
        None,
    )
    .await?;

    let end_ix = make_end_execute_ix(
        &borrower_mfi_account_f,
        order_pda,
        execute_record,
        keeper.pubkey(),
        keeper.pubkey(),
        vec![liability_bank_f.key],
    )
    .await?;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, repay_ix, withdraw_ix, end_ix],
        Some(&keeper.pubkey()),
        &[&keeper],
        ctx.last_blockhash,
    );

    let result = ctx.banks_client.process_transaction(tx).await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::OrderTriggerNotMet);
    Ok(())
}

#[tokio::test]
async fn limit_order_fails_with_spoofed_oracle() -> anyhow::Result<()> {
    // ---------------------------------------------------------------------
    // Setup
    // ---------------------------------------------------------------------
    let asset_mint = BankMint::Sol;
    let liability_mint = BankMint::Usdc;
    let asset_deposit = 20.0;
    let liability_borrow = 10.0;
    let trigger = stop_loss_trigger(fp!(1_000), 0);

    let (
        test_f,
        borrower_mfi_account_f,
        asset_mint,
        liability_mint,
        order_pda,
        keeper,
        _keeper_liab_account,
        _keeper_asset_account,
    ) = setup_limit_order_fixture(
        asset_mint,
        asset_deposit,
        liability_mint,
        liability_borrow,
        trigger,
    )
    .await?;

    // ---------------------------------------------------------------------
    // Test
    // ---------------------------------------------------------------------
    let asset_bank_f = test_f.get_bank(&asset_mint);
    let liability_bank_f = test_f.get_bank(&liability_mint);
    let wrong_oracle = liability_bank_f.load().await.config.oracle_keys[0];

    let mut observation_metas = borrower_mfi_account_f
        .load_observation_account_metas(vec![], vec![])
        .await;
    let bank_index = observation_metas
        .iter()
        .position(|meta| meta.pubkey == asset_bank_f.key)
        .expect("asset bank should be present in observation metas");
    let oracle_index = bank_index + 1;
    observation_metas[oracle_index] = AccountMeta::new_readonly(wrong_oracle, false);

    let (start_ix, _execute_record) = make_start_execute_ix(
        &borrower_mfi_account_f,
        order_pda,
        keeper.pubkey(),
        Some(observation_metas),
    )
    .await?;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix],
        Some(&keeper.pubkey()),
        &[&keeper],
        ctx.last_blockhash,
    );

    let result = ctx.banks_client.process_transaction(tx).await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::WrongOracleAccountKeys);
    Ok(())
}

// TODO test that without repay_all enabled, always fails...