use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::constants::{LIQUIDATION_TAG_DELAY_SECS, LIQUIDATION_TAG_FULL_PREMIUM_SECS};
use marginfi::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfigOpt, EmodeEntry, LiquidationRecord, MarginfiAccount},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

/// Sends `ixs` as a single payer-signed transaction.
async fn send_ixs(test_f: &TestFixture, ixs: &[Instruction]) -> Result<(), BanksClientError> {
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await
}

async fn init_record(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
) -> anyhow::Result<()> {
    let payer = test_f.payer();
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    send_ixs(test_f, &[init_ix]).await?;
    Ok(())
}

/// `nonce` varies the compute budget so repeated tags are distinct txs (BanksClient replays
/// cached results for byte-identical txs).
async fn send_tag(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    nonce: u32,
) -> Result<(), BanksClientError> {
    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000 + nonce);
    let tag_ix = liquidatee.make_tag_liquidation_record_ix().await;
    send_ixs(test_f, &[cu_ix, tag_ix]).await
}

/// Reads the account's premium-growth tag.
async fn load_tag(account: &MarginfiAccountFixture) -> i64 {
    account.load().await.liquidation_tagged_at
}

async fn load_record(test_f: &TestFixture, record_pk: Pubkey) -> LiquidationRecord {
    let ctx = test_f.context.borrow_mut();
    let account = ctx
        .banks_client
        .get_account(record_pk)
        .await
        .unwrap()
        .unwrap();
    *bytemuck::from_bytes::<LiquidationRecord>(&account.data[8..])
}

async fn current_time(test_f: &TestFixture) -> i64 {
    let clock: Clock = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await
        .unwrap();
    clock.unix_timestamp
}

/// Sets the clock's unix timestamp without warping slots (the fixture's clock starts at 0, and
/// slot warps recompute the timestamp from genesis, clobbering manual values).
async fn set_timestamp(test_f: &TestFixture, timestamp: i64) {
    let mut clock: Clock = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await
        .unwrap();
    clock.unix_timestamp = timestamp;
    test_f.context.borrow_mut().set_sysvar(&clock);
}

const T0: i64 = 1_000;

/// Re-publish the SOL and USDC feeds at the current clock time so warped tests don't hit oracle
/// staleness checks.
async fn refresh_oracles(test_f: &TestFixture) {
    let now = current_time(test_f).await;
    test_f.set_pyth_oracle_timestamp(PYTH_SOL_FEED, now).await;
    test_f.set_pyth_oracle_timestamp(PYTH_USDC_FEED, now).await;
}

/// Runs a full receivership liquidation in one tx: start, seize `withdraw_sol` SOL, repay
/// `repay_usdc` USDC, end.
async fn run_receivership_liquidation(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
    liquidator_usdc_acc: &TokenAccountFixture,
    withdraw_sol: f64,
    repay_usdc: f64,
) -> Result<TokenAccountFixture, BanksClientError> {
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.payer();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, withdraw_sol, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, repay_usdc, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;
    send_ixs(test_f, &[start_ix, withdraw_ix, repay_ix, end_ix]).await?;
    Ok(liquidator_sol_acc)
}

/// Sends a deleverage as one tx: start_deleverage, repay `repay_usdc`, end_deleverage.
async fn run_deleverage(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
    risk_admin_usdc_acc: &TokenAccountFixture,
    repay_usdc: f64,
) -> Result<(), BanksClientError> {
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let risk_admin = test_f.payer();
    let start_ix = liquidatee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(risk_admin_usdc_acc.key, usdc_bank, repay_usdc, None)
        .await;
    let end_ix = liquidatee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;
    send_ixs(test_f, &[start_ix, repay_ix, end_ix]).await
}

/// Liquidatee deposits $20 of SOL and borrows $10 of USDC, then SOL weights are cut so the
/// account is maintenance-unhealthy. The record PDA is initialized. Returns the liquidator's
/// marginfi account (holding a 100 USDC deposit) and USDC token account (still holding 100
/// USDC) for use in repayments.
async fn setup_unhealthy_liquidatee() -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    MarginfiAccountFixture,
    Pubkey,
    TokenAccountFixture,
    Keypair,
)> {
    setup_liquidatee_with(10.0, I80F48!(0.25), I80F48!(0.4)).await
}

/// As `setup_unhealthy_liquidatee`, but with the borrow size and the SOL weights the account is
/// left unhealthy at chosen by the caller.
async fn setup_liquidatee_with(
    borrow_usdc: f64,
    asset_weight_init: I80F48,
    asset_weight_maint: I80F48,
) -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    MarginfiAccountFixture,
    Pubkey,
    TokenAccountFixture,
    Keypair,
)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            borrow_usdc,
            0,
            &liquidatee_authority,
        )
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(asset_weight_init.into()),
                asset_weight_maint: Some(asset_weight_maint.into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    init_record(&test_f, &liquidatee, record_pk).await?;

    Ok((
        test_f,
        liquidatee,
        liquidator,
        record_pk,
        liquidator_usdc_acc,
        liquidatee_authority,
    ))
}

#[tokio::test]
async fn tag_sets_clears_and_rejects_double_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    set_timestamp(&test_f, T0 + 60).await;
    refresh_oracles(&test_f).await;
    let res = send_tag(&test_f, &liquidatee, 1).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountAlreadyTagged);
    assert_eq!(load_tag(&liquidatee).await, T0);

    // Restore weights so the account is healthy again; the tag can now be cleared
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(1).into()),
                asset_weight_maint: Some(I80F48!(1).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    set_timestamp(&test_f, T0 + 120).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 2).await?;
    assert_eq!(load_tag(&liquidatee).await, 0);

    // Healthy and untagged: nothing to do
    set_timestamp(&test_f, T0 + 180).await;
    refresh_oracles(&test_f).await;
    let res = send_tag(&test_f, &liquidatee, 3).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);
    Ok(())
}

/// Once the tag matures, a liquidation is allowed at the grown premium. A dust-sized one leaves the
/// tag (and the matured premium) alone; only one that erases at least
/// `LIQUIDATION_TAG_RESET_DEFICIT_FRACTION` of the health deficit restarts the growth clock.
#[tokio::test]
async fn receivership_dust_keeps_tag_material_restarts_clock() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    let matured = T0 + LIQUIDATION_TAG_FULL_PREMIUM_SECS;
    set_timestamp(&test_f, matured).await;
    refresh_oracles(&test_f).await;
    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // Repay $0.20 and seize $0.20 of SOL: erases $0.12 of the ~$2 deficit, under the 25% bar
    let liquidator_sol_acc = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.02,
        0.2,
    )
    .await?;
    // Liquidator seized exactly 0.02 SOL against a 0.2 USDC repayment.
    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(0.02, "SOL", f64)
    );
    assert_eq!(
        liquidator_usdc_acc.balance().await,
        native!(99.8, "USDC", f64)
    );

    assert_eq!(load_tag(&liquidatee).await, T0);
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.liquidation_receiver, Pubkey::default());
    assert_eq!(record.entries[3].timestamp, matured);

    // Repay $2 and seize $2 of SOL: erases $1.20 of the remaining ~$1.90 deficit, over the bar
    run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.2,
        2.0,
    )
    .await?;
    assert_eq!(load_tag(&liquidatee).await, matured);
    Ok(())
}

/// Walks the premium growth schedule on one fixture: within the delay the cap is still the 5%
/// base; halfway through the growth window the cap is 52.5% (halfway from 5% to 100%), so a $2
/// repayment allows just under $3.05 of collateral.
#[tokio::test]
async fn premium_growth_follows_schedule() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    // Within the delay: seizing .3 * 10 = $3 exceeds the base 5% cap on a $2 repayment
    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_DELAY_SECS / 2).await;
    refresh_oracles(&test_f).await;
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.3,
        2.0,
    )
    .await;
    assert!(res.is_err());
    assert_custom_error!(res.err().unwrap(), MarginfiError::LiquidationPremiumTooHigh);

    let growth_window = LIQUIDATION_TAG_FULL_PREMIUM_SECS - LIQUIDATION_TAG_DELAY_SECS;
    let midpoint = T0 + LIQUIDATION_TAG_DELAY_SECS + growth_window / 2;
    set_timestamp(&test_f, midpoint).await;
    refresh_oracles(&test_f).await;
    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // Mid-window: $3.50 exceeds the $3.05 cap...
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.35,
        2.0,
    )
    .await;
    assert!(res.is_err());
    assert_custom_error!(res.err().unwrap(), MarginfiError::LiquidationPremiumTooHigh);

    // ...while $3.04 is just inside it, and erases $0.78 of the ~$2 deficit, over the 25% bar
    let liquidator_sol_acc = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.304,
        2.0,
    )
    .await?;
    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(0.304, "SOL", f64)
    );
    assert_eq!(load_tag(&liquidatee).await, midpoint);
    Ok(())
}

#[tokio::test]
async fn tag_fails_on_account_with_no_liabilities() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = test_f.create_marginfi_account().await;

    // An account with no balances has exactly zero health but nothing to liquidate, so it must
    // not be taggable
    let res = send_tag(&test_f, &user, 0).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);
    Ok(())
}

/// The record can be closed while the account is tagged, and the tag is unaffected.
#[tokio::test]
async fn closing_the_record_keeps_the_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, test_f.payer())
        .await;
    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    send_ixs(&test_f, &[cu_ix, close_ix]).await?;

    let record_account = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_account(record_pk).await?
    };
    assert!(record_account.is_none());
    assert_eq!(load_tag(&liquidatee).await, T0);
    Ok(())
}

/// The ordinary repay path clears the tag once the last liability is gone.
#[tokio::test]
async fn repaying_all_debt_clears_the_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    let repay_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 20)
        .await;
    liquidatee
        .try_bank_repay_with_authority(
            repay_acc.key,
            usdc_bank,
            10.0,
            Some(true),
            &liquidatee_authority,
        )
        .await?;

    assert_eq!(load_tag(&liquidatee).await, 0);
    Ok(())
}

/// A pulse clears the tag once the account is maintenance-healthy again.
#[tokio::test]
async fn pulse_clears_the_tag_when_healthy() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    // A pulse while still unhealthy leaves the tag alone
    liquidatee.try_lending_account_pulse_health().await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(1).into()),
                asset_weight_maint: Some(I80F48!(1).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    set_timestamp(&test_f, T0 + 60).await;
    refresh_oracles(&test_f).await;
    liquidatee.try_lending_account_pulse_health().await?;
    assert_eq!(load_tag(&liquidatee).await, 0);
    Ok(())
}

/// A withdraw proves init health, which clears the tag while the account still carries debt.
#[tokio::test]
async fn withdraw_clears_the_tag_once_init_healthy() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    // Restore weights so the account is init-healthy while keeping its $10 USDC debt
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(1).into()),
                asset_weight_maint: Some(I80F48!(1).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    set_timestamp(&test_f, T0 + 60).await;
    refresh_oracles(&test_f).await;

    let sol_acc = test_f
        .sol_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_withdraw_with_authority(sol_acc.key, sol_bank, 0.1, None, &liquidatee_authority)
        .await?;

    assert_eq!(load_tag(&liquidatee).await, 0);
    Ok(())
}

/// A transfer carries the tag to the account it migrates to.
#[tokio::test]
async fn transfer_carries_the_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    let new_account = Keypair::new();
    liquidatee
        .try_transfer_account(
            new_account.pubkey(),
            liquidatee_authority.pubkey(),
            Some(clone_keypair(&liquidatee_authority)),
            None,
            &new_account,
            test_f.marginfi_group.fee_wallet,
        )
        .await?;

    let migrated: MarginfiAccount = test_f.load_and_deserialize(&new_account.pubkey()).await;
    assert_eq!(migrated.liquidation_tagged_at, T0);
    Ok(())
}

/// A deleverage that leaves the account underwater restarts the clock; one that leaves it healthy
/// clears the tag outright.
#[tokio::test]
async fn deleverage_restarts_clock_then_clears_when_healthy() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(10).await;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    // $1.50 repaid against $8 of maintenance collateral leaves a $0.50 deficit, down from $2
    set_timestamp(&test_f, T0 + 500).await;
    refresh_oracles(&test_f).await;
    run_deleverage(&test_f, &liquidatee, record_pk, &risk_admin_usdc_acc, 1.5).await?;
    assert_eq!(load_tag(&liquidatee).await, T0 + 500);
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.liquidation_receiver, Pubkey::default());

    // A further $1 leaves $7.50 of debt against $8 of maintenance collateral: healthy
    set_timestamp(&test_f, T0 + 1_000).await;
    refresh_oracles(&test_f).await;
    run_deleverage(&test_f, &liquidatee, record_pk, &risk_admin_usdc_acc, 1.0).await?;
    assert_eq!(load_tag(&liquidatee).await, 0);
    Ok(())
}

#[tokio::test]
async fn legacy_liquidate_dust_keeps_tag_material_restarts_clock() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator, _record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    // Seizing 0.01 SOL ($0.10) repays ~$0.095 and erases only ~$0.055 of the $2 deficit
    set_timestamp(&test_f, T0 + 1_800).await;
    refresh_oracles(&test_f).await;
    liquidator
        .try_liquidate(&liquidatee, sol_bank, 0.01, usdc_bank)
        .await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    // Seizing 0.2 SOL ($2) repays ~$1.90 and erases ~$1.10, above the 25% bar
    set_timestamp(&test_f, T0 + 3_600).await;
    refresh_oracles(&test_f).await;
    liquidator
        .try_liquidate(&liquidatee, sol_bank, 0.2, usdc_bank)
        .await?;
    assert_eq!(load_tag(&liquidatee).await, T0 + 3_600);
    Ok(())
}

/// Standard circuit-breaker config: 5%/10%/25% deviation tiers with 10m/1h/4h halt durations.
fn cb_config() -> BankConfigOpt {
    BankConfigOpt {
        circuit_breaker_enabled: Some(true),
        cb_deviation_bps_tiers: Some([500, 1000, 2500]),
        cb_tier_durations_seconds: Some([600, 3600, 14400]),
        cb_escalation_window_mult: Some(2),
        cb_ema_alpha_bps: Some(1000),
        ..Default::default()
    }
}

#[tokio::test]
async fn tag_allowed_while_cb_halted() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, _record_pk, _liquidator_usdc_acc, _liquidatee_authority) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Warm the SOL price cache, enable the breaker, then trip a halt with a +100% spike
    let warm_time: i64 = 100;
    let warm_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, warm_time)
        .await;
    test_f.set_clock(warm_slot, warm_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    sol_bank.update_config(cb_config(), None).await?;

    let trip_time = warm_time + 1;
    test_f.set_clock(warm_slot + 10, trip_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 20_000_000_000, 0, trip_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;

    // Restore the feed and refresh USDC so the tag's health check sees fresh, unremarkable prices
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, trip_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, trip_time)
        .await;

    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, trip_time);
    Ok(())
}

// The health check bounds the loss by the premium taken, so a premium grown past what a 0.96
// maintenance weight would fund is still reachable.
#[tokio::test]
async fn tag_grown_premium_is_reachable_at_high_leverage() -> anyhow::Result<()> {
    // $20 of SOL against $19.50 of USDC debt, weighted at a 25x maintenance leverage
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc, _liquidatee_authority) =
        setup_liquidatee_with(19.5, I80F48!(0.95), I80F48!(0.96)).await?;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;

    // Untagged the cap is the 5% minimum, so seizing $0.90 for $0.60 is refused
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.09,
        0.6,
    )
    .await;
    assert_custom_error!(
        res.map(|_| ()).unwrap_err(),
        MarginfiError::LiquidationPremiumTooHigh
    );

    send_tag(&test_f, &liquidatee, 0).await?;
    assert_eq!(load_tag(&liquidatee).await, T0);

    // Halfway through the growth window the cap is ~52.5%
    let growth_window = LIQUIDATION_TAG_FULL_PREMIUM_SECS - LIQUIDATION_TAG_DELAY_SECS;
    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_DELAY_SECS + growth_window / 2).await;
    refresh_oracles(&test_f).await;
    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // The same 50% premium now goes through, paid out of the account's remaining equity
    run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.09,
        0.6,
    )
    .await?;

    Ok(())
}

// Emode collateral holds the premium at the configured base no matter how long the tag has run.
#[tokio::test]
async fn tag_growth_does_not_apply_to_emode_collateral() -> anyhow::Result<()> {
    // SOL's own weight is 0.5; the emode entry lifts it to 0.94, still short of the $19.50 debt
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc, _liquidatee_authority) =
        setup_liquidatee_with(19.5, I80F48!(0.5), I80F48!(0.5)).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let collateral_tag = 1u16;
    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(sol_bank, collateral_tag, &[])
        .await?;
    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(
            usdc_bank,
            2,
            &[EmodeEntry {
                collateral_bank_emode_tag: collateral_tag,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48!(0.9).into(),
                asset_weight_maint: I80F48!(0.94).into(),
            }],
        )
        .await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    // Fully matured: an unboosted account would allow 100% here
    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_FULL_PREMIUM_SECS).await;
    refresh_oracles(&test_f).await;
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // Seizing $1.20 for $0.80 is a 50% premium, refused against the unchanged 5% base
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.12,
        0.8,
    )
    .await;
    assert_custom_error!(
        res.map(|_| ()).unwrap_err(),
        MarginfiError::LiquidationPremiumTooHigh
    );

    // The base 5% is still available: $1.04 seized against $1.00 repaid
    run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.104,
        1.0,
    )
    .await?;

    let account = liquidatee.load().await;
    assert!(account.health_cache.is_emode_boosted());
    Ok(())
}

// Growth is withheld once the account is in bad debt: past that point the premium is paid by the
// protocol, not the borrower, and the solvency floor does not apply.
#[tokio::test]
async fn tag_growth_does_not_apply_once_in_bad_debt() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc, _liquidatee_authority) =
        setup_liquidatee_with(19.5, I80F48!(0.5), I80F48!(0.5)).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, 0).await?;

    // Long enough for the tag to mature and for interest to carry the $19.50 debt past the $20 of
    // collateral, leaving the account in bad debt
    set_timestamp(&test_f, T0 + 60 * 24 * 60 * 60).await;
    refresh_oracles(&test_f).await;
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    liquidatee.try_lending_account_pulse_health().await?;
    let cache = liquidatee.load().await.health_cache;
    assert!(
        I80F48::from(cache.asset_value_equity) < I80F48::from(cache.liability_value_equity),
        "account should be in bad debt before the liquidation"
    );

    // Seizing $1.20 for $0.80 is a 50% premium, refused against the unchanged 5% base
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.12,
        0.8,
    )
    .await;
    assert_custom_error!(
        res.map(|_| ()).unwrap_err(),
        MarginfiError::LiquidationPremiumTooHigh
    );

    // The base 5% still clears: $1.04 seized against $1.00 repaid
    run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.104,
        1.0,
    )
    .await?;

    Ok(())
}
