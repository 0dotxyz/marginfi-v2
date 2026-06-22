use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixtures::{
    assert_custom_error,
    prelude::*,
    rebalance::{
        setup, setup_multi_venue_fixture, DEPOSIT_USDC, DRIFT_DST_BORROW_DEN, DRIFT_DST_BORROW_NUM,
        VENUE_DEPOSIT_NATIVE, VENUE_REDEPOSIT_NATIVE,
    },
};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::{pdas::derive_juplend_token_reserve, types::WrappedI80F48};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::tokio;
use solana_sdk::{signature::Signer, transaction::Transaction};

#[tokio::test]
async fn rebalance_native_to_native_moves_the_deposit() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert!(f.asset_shares(f.src_bank_f.key).await < I80F48::from_num(0.001));
    assert!(f.asset_shares(f.dst_bank_f.key).await > I80F48::from_num(0.001));
    Ok(())
}

#[tokio::test]
async fn rebalance_rejects_when_not_improving() -> anyhow::Result<()> {
    // A 100% required improvement can never be met by the small dst rate.
    let f = setup(I80F48::from_num(1.0), 0).await?;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    let res = f.process(&ixs).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceNotImproving);
    Ok(())
}

#[tokio::test]
async fn rebalance_enforces_cooldown() -> anyhow::Result<()> {
    // The harness pins `unix_timestamp`. Pin it to a fixed `now` (refreshing the oracle to match so
    // its price stays fresh) so the first execution clears the 10s cooldown and stamps
    // `last_exec_timestamp = now`. The clock does not advance between txs, so the immediate second
    // execution is rejected by the cooldown gate (checked before any oracle access).
    let f = setup(I80F48::from_num(0.0001), 10).await?;
    let now = 1_000i64;
    {
        let ctx = f.test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp = now;
        ctx.set_sysvar(&clock);
    }
    f.test_f
        .set_pyth_oracle_timestamp(f.oracle_metas[0].pubkey, now)
        .await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    let ixs2 = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    let res = f.process(&ixs2).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceCooldown);
    Ok(())
}

#[tokio::test]
async fn rebalance_order_update_takes_effect() -> anyhow::Result<()> {
    // Placed with a trivially-met 0.01% improvement; raise it to 100% via update, then the next
    // rebalance is rejected as not-improving — proving the update landed on-chain.
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let payer = f.test_f.context.borrow().payer.pubkey();
    let update_ix = f
        .user
        .make_update_rebalance_order_ix(
            f.order_pda,
            payer,
            None,                                             // keep allowlist
            Some(WrappedI80F48::from(I80F48::from_num(1.0))), // raise min improvement to 100%
            None,                                             // keep cooldown
            None,                                             // keep amount
        )
        .await;
    let blockhash = f.test_f.get_latest_blockhash().await;
    {
        let ctx = f.test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[update_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    let res = f.process(&ixs).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceNotImproving);
    Ok(())
}

#[tokio::test]
async fn rebalance_rejects_bank_outside_allowlist() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    // A bank not present in `allowed_banks` (the SOL bank) as src is rejected before any move.
    let outside = f.test_f.get_bank(&BankMint::Sol).key;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            outside,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceBankNotAllowed);
    Ok(())
}

#[tokio::test]
async fn rebalance_rejects_when_end_is_not_last() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let mut ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    // Append an allowed (compute-budget) ix after end_rebalance so end is no longer last.
    ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(1_400_000));
    let res = f.process(&ixs).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::EndNotLast);
    Ok(())
}

/// A borrowing account can rebalance: the per-withdraw health check is skipped while
/// ACCOUNT_IN_REBALANCE is set (the account is transiently uncollateralized between the withdraw and
/// deposit), and `end_rebalance` runs the real init-health check over the post-move balance set.
#[tokio::test]
async fn rebalance_borrowing_account_passes_health() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let user_sol = f.test_f.sol_mint.create_empty_token_account().await;
    let sol_bank = f.test_f.get_bank(&BankMint::Sol);
    f.user.try_bank_borrow(user_sol.key, sol_bank, 10.0).await?;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert!(f.asset_shares(f.src_bank_f.key).await < I80F48::from_num(0.001));
    assert!(f.asset_shares(f.dst_bank_f.key).await > I80F48::from_num(0.001));
    Ok(())
}

/// `end_rebalance` requires the full source balance to move; a partial move is rejected.
#[tokio::test]
async fn rebalance_rejects_partial_move() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let half = DEPOSIT_USDC / 2.0;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            half,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(f.keeper_usdc, &f.dst_bank_f, half, None, f.keeper.pubkey())
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceIncompleteMove);
    Ok(())
}

/// A negative min-improvement (which would permit moving into a worse venue) is rejected on update.
#[tokio::test]
async fn rebalance_update_rejects_negative_min_improvement() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let payer = f.test_f.context.borrow().payer.pubkey();
    let update_ix = f
        .user
        .make_update_rebalance_order_ix(
            f.order_pda,
            payer,
            None,
            Some(WrappedI80F48::from(I80F48::from_num(-0.01))),
            None,
            None,
        )
        .await;
    let blockhash = f.test_f.get_latest_blockhash().await;
    let ctx = f.test_f.context.borrow_mut();
    let tx =
        Transaction::new_signed_with_payer(&[update_ix], Some(&payer), &[&ctx.payer], blockhash);
    let res = ctx.banks_client.process_transaction(tx).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::RebalanceInvalidMinImprovement
    );
    Ok(())
}

/// Permissionless reclaim: once the account holds no position in any allowed venue, a keeper closes
/// the now-useless order and keeps the rent.
#[tokio::test]
async fn rebalance_keeper_close_when_no_position() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let dest = f.test_f.usdc_mint.create_empty_token_account().await;
    f.user
        .try_bank_withdraw(dest.key, &f.src_bank_f, DEPOSIT_USDC, Some(true))
        .await?;

    let close_ix = f
        .user
        .make_keeper_close_rebalance_order_ix(f.order_pda, f.keeper.pubkey())
        .await;
    let blockhash = f.test_f.get_latest_blockhash().await;
    {
        let ctx = f.test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&f.keeper.pubkey()),
            &[&f.keeper],
            blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }
    let order = f
        .test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(f.order_pda)
        .await?;
    assert!(order.map(|a| a.lamports).unwrap_or(0) == 0);
    Ok(())
}

/// The keeper cannot skim more than the flat fee: withdrawing the full source but depositing only
/// part of it into dst is rejected by the value-conservation floor.
#[tokio::test]
async fn rebalance_rejects_value_leak() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    // Deposit only half — the keeper tries to pocket the rest.
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC / 2.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// Exact value conservation: when the keeper skims a fee within the cap, the user's destination
/// position plus the keeper's fee equals the user's source position to the atomic unit —
/// `new_balance + keeper_fee == old_balance`, with no dust slack. (No time elapses in-tx, so both
/// banks' share value is 1 and the user's recorded balances equal native token amounts exactly.)
#[tokio::test]
async fn rebalance_conserves_value_exactly_minus_keeper_fee() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_balance = f.asset_shares(f.src_bank_f.key).await;
    let keeper_before = balance_of(f.test_f.context.clone(), f.keeper_usdc).await;

    // Keeper skims $0.40 (within the $0.50 flat-fee cap): withdraw the full source, deposit the rest.
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC - 0.4,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    let new_balance = f.asset_shares(f.dst_bank_f.key).await;
    let keeper_fee = balance_of(f.test_f.context.clone(), f.keeper_usdc).await - keeper_before;

    assert!(keeper_fee > 0, "keeper should have taken a fee");
    assert_eq!(new_balance + I80F48::from_num(keeper_fee), old_balance);
    Ok(())
}

/// A bounded order (amount < the deposited position) moves exactly that amount and leaves the
/// remainder in the source. With no skim the same-mint shares are conserved, so the destination
/// receives precisely the ordered amount and `src_after + dst_after == src_before`.
#[tokio::test]
async fn rebalance_partial_amount_moves_only_that_amount() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;
    // Manage half of the 1000 USDC deposit (6-decimal native), leaving the rest in src.
    f.set_amount(500_000_000).await?;

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC / 2.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC / 2.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    // Partial move leaves the source active, so it stays in the post-move observation set.
    let end_ix = f
        .user
        .make_rebalance_partial_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    let src_after = f.asset_shares(f.src_bank_f.key).await;
    let dst_after = f.asset_shares(f.dst_bank_f.key).await;
    assert_eq!(
        dst_after,
        I80F48::from_num(500_000_000),
        "exactly the ordered amount moved to dst"
    );
    assert_eq!(
        src_after + dst_after,
        old_src,
        "no skim -> same-mint shares conserved"
    );
    Ok(())
}

/// A bounded order caps the move: a keeper that withdraws the whole position when the order only
/// authorizes part of it is rejected, so the user keeps the unmanaged remainder where it is.
#[tokio::test]
async fn rebalance_partial_rejects_over_move() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(500_000_000).await?; // authorize 500 USDC

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    // Keeper moves the full 1000 — more than the order's 500.
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceExceedsAmount);
    Ok(())
}

/// A bounded order also has a floor: a keeper that moves less than the ordered amount is rejected.
/// This is the partial analog of the full-move "source not emptied" check — together with the
/// value-conservation floor it stops a keeper skimming the fee while under-delivering the move.
#[tokio::test]
async fn rebalance_partial_rejects_incomplete_move() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(500_000_000).await?; // order wants 500 USDC moved

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    // Keeper only moves 100 of the ordered 500.
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            100.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            100.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            f.src_bank_f.key,
            f.dst_bank_f.key,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.oracle_metas.clone(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceIncompleteMove);
    Ok(())
}

/// Balances outside {src, dst} must survive the move byte-identical.
#[tokio::test]
async fn rebalance_leaves_other_balances_unchanged() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let sol_bank_key = {
        let sol_bank = f.test_f.get_bank(&BankMint::Sol);
        let user_sol = f
            .test_f
            .sol_mint
            .create_token_account_and_mint_to(10.0)
            .await;
        f.user
            .try_bank_deposit(user_sol.key, sol_bank, 10.0, None)
            .await?;
        sol_bank.key
    };
    let sol_before = f.asset_shares(sol_bank_key).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.asset_shares(sol_bank_key).await, sol_before);
    assert!(f.asset_shares(f.dst_bank_f.key).await > I80F48::from_num(0.001));
    Ok(())
}

/// Kamino (src) -> Drift (dst): the user holds the position in a 0%-utilization Kamino bank (rate ~0),
/// the Drift bank carries borrow utilization (rate > 0). The keeper sandwich drains Kamino and deposits
/// the full balance into Drift; the move clears the improvement gate and conserves value.
#[tokio::test]
async fn rebalance_kamino_to_drift_moves_the_deposit() -> anyhow::Result<()> {
    let f = setup_multi_venue_fixture().await?;
    let src = f.kamino_bank.key;
    let dst = f.drift_bank.key;

    let user_token = f.mint.create_token_account_and_mint_to(1_000.0).await;
    f.test_f
        .run_kamino_deposit(
            &f.kamino_bank,
            &f.user,
            user_token.key,
            VENUE_DEPOSIT_NATIVE,
        )
        .await?;

    f.set_kamino_rate_zero().await;
    f.set_drift_borrow_utilization(DRIFT_DST_BORROW_NUM, DRIFT_DST_BORROW_DEN)
        .await;

    let (order_pda, record_pda) = f.place_order(src, dst, I80F48::from_num(0.0001)).await?;

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
    let refresh_reserve = f.user.make_kamino_refresh_reserve_ix(&f.kamino_bank).await;
    let refresh_obligation = f
        .user
        .make_kamino_refresh_obligation_ix(&f.kamino_bank)
        .await;
    let drift_crank = f
        .user
        .make_drift_update_spot_market_cumulative_interest_ix(&f.drift_bank)
        .await;
    let mut oracle_metas = f.kamino_slice().await;
    oracle_metas.extend(f.drift_slice().await);

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            src,
            dst,
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_kamino_withdraw_ix_with_authority(
            f.keeper_token,
            &f.kamino_bank,
            VENUE_DEPOSIT_NATIVE,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_drift_deposit_ix_with_authority(
            f.keeper_token,
            &f.drift_bank,
            VENUE_REDEPOSIT_NATIVE,
            f.keeper.pubkey(),
            None,
        )
        .await;
    // Re-refresh the Kamino reserve after the withdraw leg marks it stale, before end reads its rate.
    let refresh_reserve_end = f.user.make_kamino_refresh_reserve_ix(&f.kamino_bank).await;
    let refresh_obligation_end = f
        .user
        .make_kamino_refresh_obligation_ix(&f.kamino_bank)
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            src,
            dst,
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            oracle_metas,
        )
        .await;

    f.process(&[
        cu_ix,
        refresh_reserve,
        refresh_obligation,
        drift_crank,
        start_ix,
        withdraw_ix,
        deposit_ix,
        refresh_reserve_end,
        refresh_obligation_end,
        end_ix,
    ])
    .await?;

    assert!(f.asset_shares(src).await < I80F48::from_num(0.001));
    assert!(f.asset_shares(dst).await > I80F48::from_num(0.001));
    Ok(())
}

/// Drift (src) -> JupLend (dst): the user holds the position in a 0%-utilization Drift bank (rate ~0),
/// the JupLend bank reports a high supply rate. The keeper sandwich drains Drift and deposits the full
/// balance into JupLend. JupLend's `TokenReserve` is passed via the start/end `dst_token_reserve` arg.
#[tokio::test]
async fn rebalance_drift_to_juplend_moves_the_deposit() -> anyhow::Result<()> {
    let f = setup_multi_venue_fixture().await?;
    let src = f.drift_bank.key;
    let dst = f.juplend_bank.key;

    let user_token = f.mint.create_token_account_and_mint_to(1_000.0).await;
    f.test_f
        .run_drift_deposit(&f.drift_bank, &f.user, user_token.key, VENUE_DEPOSIT_NATIVE)
        .await?;

    // Drift src stays at 0% utilization (rate ~0) after a deposit-only history; only the dst is raised.
    f.set_juplend_rate_high().await;

    let (order_pda, record_pda) = f.place_order(src, dst, I80F48::from_num(0.0001)).await?;

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(2_000_000);
    let drift_crank = f
        .user
        .make_drift_update_spot_market_cumulative_interest_ix(&f.drift_bank)
        .await;
    let mut oracle_metas = f.drift_slice().await;
    oracle_metas.extend(f.juplend_slice().await);
    let dst_token_reserve = Some(derive_juplend_token_reserve(&f.mint.key).0);

    let start_ix = f
        .user
        .make_rebalance_start_ix_with_reserves(
            src,
            dst,
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
            None,
            dst_token_reserve,
            oracle_metas.clone(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_drift_withdraw_ix_with_authority(
            f.keeper_token,
            &f.drift_bank,
            VENUE_DEPOSIT_NATIVE,
            Some(true),
            f.keeper.pubkey(),
            None,
        )
        .await;
    let deposit_ix = f
        .user
        .make_juplend_deposit_ix_with_authority(
            f.keeper_token,
            &f.juplend_bank,
            VENUE_REDEPOSIT_NATIVE,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix_with_reserves(
            src,
            dst,
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            None,
            dst_token_reserve,
            true,
            oracle_metas,
        )
        .await;

    f.process(&[
        cu_ix,
        drift_crank,
        start_ix,
        withdraw_ix,
        deposit_ix,
        end_ix,
    ])
    .await?;

    assert!(f.asset_shares(src).await < I80F48::from_num(0.001));
    assert!(f.asset_shares(dst).await > I80F48::from_num(0.001));
    Ok(())
}
