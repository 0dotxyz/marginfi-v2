use fixed::types::I80F48;
use fixtures::{
    assert_custom_error,
    prelude::*,
    rebalance::{
        drive_utilization, rebalance_move, setup, setup_multi_venue_fixture, DEPOSIT_USDC,
        DRIFT_DST_BORROW_DEN, DRIFT_DST_BORROW_NUM, VENUE_DEPOSIT_NATIVE,
    },
};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::{
    pdas::derive_juplend_token_reserve,
    types::{BankConfig, WrappedI80F48},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::tokio;
use solana_sdk::{signature::Signer, transaction::Transaction};

/// The per-venue deposit (`VENUE_DEPOSIT_NATIVE`, 100 USDC of 6-decimal native) as USD value, at the
/// $1 test oracle.
const VENUE_DEPOSIT_VALUE: f64 = 100.0;

#[tokio::test]
async fn rebalance_native_to_native_moves_the_deposit() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.asset_shares(f.src_bank_f.key).await, I80F48::ZERO);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
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
    // Cooldown (2h) exceeds the settle-delay cap (1h), so the first execution's record can be settled
    // (unblocking the order for a re-run) while the cooldown still blocks a second rebalance.
    let f = setup(I80F48::from_num(0.0001), 7_200).await?;
    let base = 10_000i64; // >= cooldown so the first execution clears the gate
    f.pin_clock(base).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    // Settle the first execution after its (clamped) 1h settle delay to close the record.
    f.advance_clock(3_601).await;
    let settle = f.build_settle(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&[settle]).await?;

    // A second rebalance is still inside the 2h cooldown (only ~1h elapsed) and is rejected.
    let ixs2 = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    let res = f.process(&ixs2).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceCooldown);
    Ok(())
}

/// The keeper tip is escrowed at `end_rebalance` and paid at settlement only when the destination
/// realized more yield than the source over the window. Here the idle source (rate 0) is out-yielded
/// by the borrow-carrying destination, so the escrow is paid to the keeper (the pool is not refunded).
#[tokio::test]
async fn rebalance_settle_pays_keeper_on_realized_yield() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let tip = 200_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    f.pin_clock(1_000).await;

    let pool_before = f.lamports_of(f.fee_pool()).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;
    // The escrow that left the pool equals the tip the record will settle.
    let pool_after_end = f.lamports_of(f.fee_pool()).await;
    let pending_tip = f.record_pending_tip().await;
    assert_eq!(
        pool_before - pool_after_end,
        pending_tip,
        "escrow out of the pool equals the record's pending tip"
    );

    f.advance_clock(601).await; // settle delay = clamp(0, 600, 3600) = 600
    let settle = f.build_settle(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&[settle]).await?;

    assert_eq!(
        f.lamports_of(f.fee_pool()).await,
        pool_after_end,
        "realized settlement pays the keeper, leaving the pool untouched"
    );
    assert_eq!(
        f.lamports_of(f.record_pda).await,
        0,
        "record closed after settlement"
    );
    Ok(())
}

/// When the move did not realize its promised improvement (here the source is driven to out-yield the
/// destination over the window, standing in for a manipulated advantage that did not hold), settlement
/// refunds the escrowed tip to the fee pool instead of paying the keeper.
#[tokio::test]
async fn rebalance_settle_refunds_pool_when_not_realized() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let tip = 200_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    f.pin_clock(1_000).await;

    let pool_before = f.lamports_of(f.fee_pool()).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;
    let pool_after_end = f.lamports_of(f.fee_pool()).await;
    assert_eq!(
        pool_before - pool_after_end,
        f.record_pending_tip().await,
        "escrow out of the pool equals the record's pending tip"
    );

    // Drive the source to a decisively higher utilization (~90%) than the destination (~25% after the
    // move diluted its deposit), so the source out-yields the destination over the window. The
    // driver's borrower posts SOL collateral, so refresh the SOL oracle to the pinned clock first.
    f.test_f
        .set_pyth_oracle_timestamp(PYTH_SOL_FEED, 1_000)
        .await;
    drive_utilization(&f.test_f, &f.src_bank_f, 900.0, 300.0).await?;

    f.advance_clock(601).await;
    let settle = f.build_settle(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&[settle]).await?;

    assert_eq!(
        f.lamports_of(f.fee_pool()).await,
        pool_before,
        "unrealized settlement refunds the full escrow, restoring the pool"
    );
    assert_eq!(f.lamports_of(f.record_pda).await, 0, "record closed");
    Ok(())
}

/// Settlement is rejected before the settle delay elapses.
#[tokio::test]
async fn rebalance_settle_rejects_before_delay() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_keeper_tip(200_000).await?;
    f.pin_clock(1_000).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    // No clock advance: the settle delay has not elapsed.
    let settle = f.build_settle(f.src_bank_f.key, f.dst_bank_f.key).await;
    let res = f.process(&[settle]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceSettleTooEarly);
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
            None,                                             // keep keeper tip
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
    // A bank not present in `allowed_banks` (the SOL bank) as a source is rejected before any move.
    let outside = f.test_f.get_bank(&BankMint::Sol).key;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(outside), f.bank_meta(f.dst_bank_f.key)],
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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

#[tokio::test]
async fn rebalance_rejects_second_start_in_tx() -> anyhow::Result<()> {
    // A second start_rebalance in the same tx is rejected: an end clears only its own account's
    // ACCOUNT_IN_REBALANCE flag, so a second start would strand another account's flag set.
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let mut ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    ixs.insert(1, ixs[0].clone());
    let res = f.process(&ixs).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceMalformedSandwich);
    Ok(())
}

/// A borrowing account can rebalance: the per-withdraw health check is skipped while
/// ACCOUNT_IN_REBALANCE is set (the account is transiently uncollateralized between the withdraw and
/// deposit), and `end_rebalance` runs the real maintenance-health check over the post-move balance set.
#[tokio::test]
async fn rebalance_borrowing_account_passes_health() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let user_sol = f.test_f.sol_mint.create_empty_token_account().await;
    let sol_bank = f.test_f.get_bank(&BankMint::Sol);
    f.user.try_bank_borrow(user_sol.key, sol_bank, 10.0).await?;

    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.asset_shares(f.src_bank_f.key).await, I80F48::ZERO);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
    Ok(())
}

/// An unlimited order permits a partial fill (e.g. the destination is near its deposit cap): moving
/// part of the position succeeds, leaves the remainder in the source, and conserves value.
#[tokio::test]
async fn rebalance_unlimited_allows_partial_fill() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let half = DEPOSIT_USDC / 2.0;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, half)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
    // Source keeps its unmoved half, so it stays active in the post-move health observation set.
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    let src_after = f.asset_shares(f.src_bank_f.key).await;
    let dst_after = f.asset_shares(f.dst_bank_f.key).await;
    assert_eq!(src_after, dst_after, "the position split into equal halves");
    assert_eq!(src_after + dst_after, old_src, "same-mint shares conserved");
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

/// Strict conservation: withdrawing the full source but depositing only part of it into dst leaks
/// position value beyond the dust tolerance and is rejected.
#[tokio::test]
async fn rebalance_rejects_value_leak() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    // Declare a full move, but the keeper only delivers half — reconciliation catches the shortfall.
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// Strict value conservation with an explicit SOL tip: an honest keeper deposits the full withdrawn
/// amount, so the destination position equals the old source position to the atomic unit, and the
/// keeper's compensation is drawn from the account's SOL fee pool — a full move pays the full tip.
#[tokio::test]
async fn rebalance_conserves_value_and_pays_full_tip() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_balance = f.asset_shares(f.src_bank_f.key).await;

    let tip = 200_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    // The whole source position lands in dst; no position value is skimmed.
    let new_balance = f.asset_shares(f.dst_bank_f.key).await;
    assert_eq!(
        new_balance, old_balance,
        "same-mint shares conserved, no skim"
    );

    // A full move earns ~the full configured tip, drawn from the pool.
    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, tip, "full move pays the full tip");
    Ok(())
}

/// The tip is capped at the pool's spendable balance (lamports above the rent-exempt reserve): an
/// underfunded pool pays what it can, the move still executes, and the pool keeps exactly its
/// rent-exempt reserve rather than being drained or left rent-paying.
#[tokio::test]
async fn rebalance_tip_capped_by_pool_balance() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_keeper_tip(5_000_000).await?; // owed far exceeds the pool
    let funded = 2_000_000u64;
    f.top_up_pool(funded).await?;

    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    let rent_floor = solana_sdk::rent::Rent::default().minimum_balance(0);
    assert_eq!(f.lamports_of(f.fee_pool()).await, rent_floor);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
    Ok(())
}

/// A tip whose owed amount would otherwise strand the pool in a rent-paying state (0 < balance <
/// rent-exempt), which the runtime rejects, must not brick the rebalance: the payout is clamped to
/// the spendable balance so the pool keeps exactly its rent-exempt reserve and the move executes.
#[tokio::test]
async fn rebalance_tip_never_leaves_pool_rent_paying() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let rent_floor = solana_sdk::rent::Rent::default().minimum_balance(0);
    // Owe 500k against a pool holding only 300k of spendable budget above the reserve: an unclamped
    // payout would leave a ~690k sub-rent-exempt remainder and end_rebalance would fail.
    f.set_keeper_tip(500_000).await?;
    f.top_up_pool(300_000).await?;

    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.lamports_of(f.fee_pool()).await, rent_floor);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
    Ok(())
}

/// A zero-tip order needs no pool: it executes and pays nothing.
#[tokio::test]
async fn rebalance_zero_tip_needs_no_pool() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    assert_eq!(f.lamports_of(f.fee_pool()).await, 0);

    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.lamports_of(f.fee_pool()).await, 0);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
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
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC / 2.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
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
    f.set_amount(500_000_000).await?; // authorize 500 USDC of total moved value
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];

    // Keeper honestly declares (and moves) the full 1000 — more than the order's 500 budget.
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceExceedsAmount);
    Ok(())
}

/// A bounded order allows a partial fill (e.g. the destination is near its deposit cap): moving less
/// than the ordered amount succeeds, leaves the remainder in the source, and pays the tip pro rata to
/// the fraction of the target actually moved.
#[tokio::test]
async fn rebalance_partial_fill_pays_prorata_tip() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(500_000_000).await?; // order targets 500 USDC
    let tip = 500_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, 100.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    // Keeper moves only 100 of the 500 target (a fifth).
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
    // Source keeps its unmoved remainder, so it stays in the post-move health observation set.
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    // 100 of a 500 target = 20% of the tip; the pro-rata floors to one lamport below tip/5.
    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(
        paid,
        tip / 5 - 1,
        "partial fill pays the floored 20% pro-rata tip"
    );
    Ok(())
}

/// Atomic multi-destination: one sandwich drains the source into two same-mint banks (the best venue
/// plus a spillover). Value is conserved across the whole set and the full tip is paid once over the
/// aggregate moved — splitting across banks earns no more than a single-destination move.
#[tokio::test]
async fn rebalance_splits_across_two_destinations() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let dst2 = f.add_second_dst().await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;

    let tip = 400_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(f.dst_bank_f.key),
        f.bank_meta(dst2.key),
    ];
    let half = DEPOSIT_USDC / 2.0;

    // One source, two destination moves: 0->1 and 0->2, each half the position.
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, half), rebalance_move(0, 2, half)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
    // Split the withdrawn position across both destinations.
    let deposit_dst1 = f
        .user
        .make_deposit_ix_with_authority(f.keeper_usdc, &f.dst_bank_f, half, None, f.keeper.pubkey())
        .await;
    let deposit_dst2 = f
        .user
        .make_deposit_ix_with_authority(f.keeper_usdc, &dst2, half, None, f.keeper.pubkey())
        .await;
    // Source is emptied by the full move, so it drops from the health observation set.
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_dst1, deposit_dst2, end_ix])
        .await?;

    let dst1_after = f.asset_shares(f.dst_bank_f.key).await;
    let dst2_after = f.asset_shares(dst2.key).await;
    assert_eq!(
        dst1_after, dst2_after,
        "the position split into equal halves"
    );
    assert_eq!(
        f.asset_shares(f.src_bank_f.key).await,
        I80F48::ZERO,
        "source emptied"
    );
    // Value conserved across the whole set.
    assert_eq!(
        dst1_after + dst2_after,
        old_src,
        "aggregate value conserved"
    );
    // A full move (across both banks) pays ~the full tip, once.
    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, tip, "full multi-dst move pays the full tip once");
    Ok(())
}

/// Balances outside the referenced set must survive the move with the same side and shares.
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
    let old_src = f.asset_shares(f.src_bank_f.key).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    assert_eq!(f.asset_shares(sol_bank_key).await, sol_before);
    assert_eq!(f.asset_shares(f.dst_bank_f.key).await, old_src);
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
    let ref_banks = vec![
        RebalanceBankMeta::new(src, f.kamino_slice().await),
        RebalanceBankMeta::new(dst, f.drift_slice().await),
    ];
    let moves = vec![rebalance_move(0, 1, VENUE_DEPOSIT_VALUE)];

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            moves,
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            // Deposit what the keeper received: the kamino withdraw rounds down by a native unit or
            // two, so a small margin is left. Well within the conservation dust tolerance.
            VENUE_DEPOSIT_NATIVE - 100,
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
            ref_banks,
            vec![src],
            order_pda,
            record_pda,
            f.keeper.pubkey(),
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

    assert_eq!(f.asset_shares(src).await, I80F48::ZERO);
    // The Drift mock bank scales shares 1000x (share value 0.001): dst holds the deposited position,
    // 100 native short of the source.
    assert_eq!(
        f.asset_shares(dst).await,
        I80F48::from_num((VENUE_DEPOSIT_NATIVE - 100) * 1000)
    );
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
    let juplend_reserve = derive_juplend_token_reserve(&f.mint.key).0;
    let ref_banks = vec![
        RebalanceBankMeta::new(src, f.drift_slice().await),
        RebalanceBankMeta::with_reserve(dst, juplend_reserve, f.juplend_slice().await),
    ];

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, VENUE_DEPOSIT_VALUE)],
            order_pda,
            record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            // Deposit what the keeper received: the drift withdraw rounds down by a native unit or
            // two, so a small margin is left. Well within the conservation dust tolerance.
            VENUE_DEPOSIT_NATIVE - 100,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![src],
            order_pda,
            record_pda,
            f.keeper.pubkey(),
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

    assert_eq!(f.asset_shares(src).await, I80F48::ZERO);
    // JupLend shares track native tokens 1:1: dst holds the deposit, 100 native short of the source.
    assert_eq!(
        f.asset_shares(dst).await,
        I80F48::from_num(VENUE_DEPOSIT_NATIVE - 100)
    );
    Ok(())
}

// N->N coverage: reconciliation adversarial cases + consolidate (N->1)

/// Reconciliation catches a keeper moving MORE than declared: the per-bank delta mismatch fires
/// before the amount budget. Declares a 500 move but physically relocates the full 1000.
#[tokio::test]
async fn rebalance_rejects_moves_more_than_declared() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC / 2.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// A keeper withdraws the full source but deposits only half into the destination, pocketing the rest.
/// Conservation is proven on underlying token count, so the missing tokens surface as a per-bank
/// shortfall regardless of any oracle price: the skim cannot be masked by divergent same-mint oracles.
#[tokio::test]
async fn rebalance_rejects_principal_skim() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ixs = f
        .build_skim_sandwich(f.src_bank_f.key, f.dst_bank_f.key, DEPOSIT_USDC / 2.0)
        .await;
    let res = f.process(&ixs).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// A keeper references an unused decoy bank at index 0 and declares only `src -> dst` (indices 1 -> 2).
/// The decoy participates in no move, so `start_rebalance` rejects it rather than parsing and storing a
/// bank that contributes nothing to the relocation.
#[tokio::test]
async fn rebalance_rejects_unreferenced_bank() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let decoy = f.add_second_dst().await?;
    let ref_banks = vec![
        f.bank_meta(decoy.key),
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(f.dst_bank_f.key),
    ];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks,
            vec![rebalance_move(1, 2, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceUnreferencedBank);
    Ok(())
}

/// A move declared as a split across two destinations cannot be routed entirely into one: per-bank
/// reconciliation catches the misattribution (dst1 over, dst2 under).
#[tokio::test]
async fn rebalance_rejects_misrouted_deposit() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let dst2 = f.add_second_dst().await?;
    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(f.dst_bank_f.key),
        f.bank_meta(dst2.key),
    ];
    let half = DEPOSIT_USDC / 2.0;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, half), rebalance_move(0, 2, half)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
    // Route the ENTIRE position into dst1; dst2 declared to receive half but gets nothing.
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
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// Consolidate crux: value conserves at the single destination, but the keeper lies about which
/// source funded it (declares both drained; only one actually is). Per-source reconcile rejects.
#[tokio::test]
async fn rebalance_consolidate_rejects_source_misattribution() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let src2 = f.add_second_src(500.0).await?;
    // Banks: src(0, holds 1000), src2(1, holds 500), dst(2, rate > 0).
    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(src2.key),
        f.bank_meta(f.dst_bank_f.key),
    ];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 2, 600.0), rebalance_move(1, 2, 400.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    // Actually drain only src (1000), nothing from src2; deposit 1000 into dst.
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
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// Headline consolidate (N->1): drain two same-mint sources into one higher-yield destination in one
/// sandwich. Value conserved across the whole set; full tip paid once over the summed source value.
#[tokio::test]
async fn rebalance_consolidates_two_sources_into_one() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let src2 = f.add_second_src(500.0).await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let old_src2 = f.asset_shares(src2.key).await;

    let tip = 200_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(src2.key),
        f.bank_meta(f.dst_bank_f.key),
    ];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![
                rebalance_move(0, 2, DEPOSIT_USDC),
                rebalance_move(1, 2, 500.0),
            ],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_src = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_src2 = f
        .user
        .make_withdraw_ix_with_authority(f.keeper_usdc, &src2, 500.0, Some(true), f.keeper.pubkey())
        .await;
    let deposit_dst = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC + 500.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key, src2.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_src, withdraw_src2, deposit_dst, end_ix])
        .await?;

    assert_eq!(
        f.asset_shares(f.src_bank_f.key).await,
        I80F48::ZERO,
        "src emptied"
    );
    assert_eq!(f.asset_shares(src2.key).await, I80F48::ZERO, "src2 emptied");
    // Both sources' value landed in dst, no skim.
    assert_eq!(
        f.asset_shares(f.dst_bank_f.key).await,
        old_src + old_src2,
        "value conserved"
    );
    // Full move relative to the summed source position -> ~full tip once.
    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, tip, "consolidate pays the full tip once");
    Ok(())
}

/// Consolidate leak: both sources drained but the keeper under-deposits into the single destination;
/// the destination's summed declared inflow exceeds its actual gain.
#[tokio::test]
async fn rebalance_consolidate_rejects_destination_shortfall() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let src2 = f.add_second_src(500.0).await?;
    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(src2.key),
        f.bank_meta(f.dst_bank_f.key),
    ];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![
                rebalance_move(0, 2, DEPOSIT_USDC),
                rebalance_move(1, 2, 500.0),
            ],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_src = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_src2 = f
        .user
        .make_withdraw_ix_with_authority(f.keeper_usdc, &src2, 500.0, Some(true), f.keeper.pubkey())
        .await;
    // Deposit only 1200 of the 1500 declared — pocket 300.
    let deposit_dst = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            1_200.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key, src2.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_src, withdraw_src2, deposit_dst, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// The per-move improvement gate is atomic over the batch: one move whose destination does not beat
/// its source reverts the entire start_rebalance.
#[tokio::test]
async fn rebalance_rejects_multi_move_when_one_not_improving() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    // src2 is a rate-0 bank (added as a "source" helper, but here used as a non-improving destination).
    let flat = f.add_second_src(1.0).await?;
    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(f.dst_bank_f.key),
        f.bank_meta(flat.key),
    ];
    let half = DEPOSIT_USDC / 2.0;
    // 0->1 improves (dst rate > 0); 0->2 does not (flat rate 0 == src rate 0).
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks,
            vec![rebalance_move(0, 1, half), rebalance_move(0, 2, half)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceNotImproving);
    Ok(())
}

// N->N coverage: untouched-balance guard, amount budget, tip denominator, dust

/// The only adversarial exercise of `verify_others_unchanged`: a keeper does an honest src->dst move
/// but also drains the user's UNREFERENCED SOL position to its own account. The snapshot catches it.
#[tokio::test]
async fn rebalance_rejects_touching_unreferenced_balance() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    // User holds a SOL deposit in the (unreferenced) SOL bank.
    let sol_bank = f.test_f.get_bank(&BankMint::Sol);
    let user_sol = f
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(10.0)
        .await;
    f.user
        .try_bank_deposit(user_sol.key, sol_bank, 10.0, None)
        .await?;
    let keeper_sol = f
        .test_f
        .sol_mint
        .create_empty_token_account_with_owner(&f.keeper.pubkey())
        .await
        .key;

    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_usdc = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let deposit_usdc = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    // Loot part of the unreferenced SOL position (partial, so SOL stays active and the snapshot
    // mismatch surfaces at `verify_others_unchanged` rather than the health-obs check).
    let steal_sol = f
        .user
        .make_withdraw_ix_with_authority(keeper_sol, sol_bank, 5.0, None, f.keeper.pubkey())
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_usdc, deposit_usdc, steal_sol, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    Ok(())
}

/// `order.amount` is a TOTAL-value budget across all moves: two sub-cap moves that sum past it are
/// rejected.
#[tokio::test]
async fn rebalance_amount_cap_sums_across_moves() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let dst2 = f.add_second_dst().await?;
    f.set_amount(500_000_000).await?; // $500 total budget
    let ref_banks = vec![
        f.bank_meta(f.src_bank_f.key),
        f.bank_meta(f.dst_bank_f.key),
        f.bank_meta(dst2.key),
    ];
    // 300 + 300 = 600 > 500, though each move is under the cap.
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, 300.0), rebalance_move(0, 2, 300.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            600.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_dst1 = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            300.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_dst2 = f
        .user
        .make_deposit_ix_with_authority(f.keeper_usdc, &dst2, 300.0, None, f.keeper.pubkey())
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_dst1, deposit_dst2, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceExceedsAmount);
    Ok(())
}

/// A move with amount 0 is rejected by the record initializer.
#[tokio::test]
async fn rebalance_rejects_zero_amount_move() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)],
            vec![rebalance_move(0, 1, 0.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    Ok(())
}

/// An empty move list is rejected up front.
#[tokio::test]
async fn rebalance_rejects_empty_moves() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)],
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    Ok(())
}

/// A bank appearing twice in the referenced-account stream is rejected (indices must be unambiguous).
#[tokio::test]
async fn rebalance_rejects_duplicate_referenced_bank() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.src_bank_f.key)],
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::SameAssetAndLiabilityBanks);
    Ok(())
}

/// `end_rebalance` requires the referenced banks in the same order the record recorded them; a
/// reordered end stream is rejected.
#[tokio::test]
async fn rebalance_end_rejects_reordered_banks() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks,
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    // End with the referenced banks reversed vs. the record's [src, dst].
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            vec![f.bank_meta(f.dst_bank_f.key), f.bank_meta(f.src_bank_f.key)],
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidBankAccount);
    Ok(())
}

/// When `order.amount` exceeds the held position, the tip denominator falls back to the source
/// position value, so a full move still pays ~the full tip (not half).
#[tokio::test]
async fn rebalance_amount_exceeds_source_tip_uses_source_denominator() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(2_000_000_000).await?; // $2000 budget, but only $1000 held
    let tip = 200_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ixs = f.build_sandwich(f.src_bank_f.key, f.dst_bank_f.key).await;
    f.process(&ixs).await?;

    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, tip, "full move pays the full tip");
    Ok(())
}

/// A bounded move that exactly fills the order's amount budget pays ~the full tip (fraction = 1),
/// even though only part of the position moved.
#[tokio::test]
async fn rebalance_bounded_move_at_cap_pays_full_tip() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(500_000_000).await?; // $500 budget
    let tip = 300_000u64;
    f.set_keeper_tip(tip).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, 500.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            500.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            500.0,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, tip, "at-cap move pays the full tip");
    Ok(())
}

/// A leak just over the dust tolerance is rejected.
#[tokio::test]
async fn rebalance_leak_just_over_dust_rejected() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
    // Deposit 0.02 short of the declared 1000 — beyond the $0.01 dust tolerance.
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC - 0.02,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f
        .process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceValueLeak);
    Ok(())
}

/// A shortfall within the dust tolerance passes (absorbs sub-unit venue rounding).
#[tokio::test]
async fn rebalance_leak_just_under_dust_passes() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let old_src = f.asset_shares(f.src_bank_f.key).await;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
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
    // 0.005 short — within the $0.01 dust tolerance.
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            DEPOSIT_USDC - 0.005,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;
    // dst holds the deposit: the full source minus the 0.005 USDC (5_000 native) tolerated dust.
    assert_eq!(
        f.asset_shares(f.dst_bank_f.key).await,
        old_src - I80F48::from_num(5_000)
    );
    Ok(())
}

// N->N coverage: structural guards + tip rounding

/// Fewer than 2 referenced banks is rejected up front.
#[tokio::test]
async fn rebalance_rejects_single_referenced_bank() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(f.src_bank_f.key)],
            vec![rebalance_move(0, 0, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceBankNotAllowed);
    Ok(())
}

/// A referenced bank of a different mint than the order is rejected.
#[tokio::test]
async fn rebalance_rejects_mint_mismatch() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let sol = f.test_f.get_bank(&BankMint::Sol).key;
    // Allowlist the SOL bank so it passes the allowlist check and reaches the mint check.
    let payer = f.test_f.context.borrow().payer.pubkey();
    let update_ix = f
        .user
        .make_update_rebalance_order_ix(
            f.order_pda,
            payer,
            Some(vec![f.src_bank_f.key, f.dst_bank_f.key, sol]),
            None,
            None,
            None,
            None,
        )
        .await;
    f.process_as_payer(&[update_ix]).await?;

    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(sol), f.bank_meta(f.dst_bank_f.key)],
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceMintMismatch);
    Ok(())
}

/// More moves than MAX_REBALANCE_MOVES is rejected.
#[tokio::test]
async fn rebalance_rejects_too_many_moves() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let moves = vec![rebalance_move(0, 1, 1.0); 9]; // MAX is 8
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)],
            moves,
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    Ok(())
}

/// A referenced bank supplied with no oracle account is rejected by the parser.
#[tokio::test]
async fn rebalance_rejects_missing_oracle_account() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            vec![
                f.bank_meta(f.src_bank_f.key),
                RebalanceBankMeta::new(f.dst_bank_f.key, vec![]), // no oracle
            ],
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::WrongNumberOfOracleAccounts);
    Ok(())
}

/// The tip floors a fractional lamport (owed 1.5 -> paid 1), never rounds up.
#[tokio::test]
async fn rebalance_tip_floors_fractional_lamport() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    f.set_amount(2_000_000).await?; // $2 budget
    f.set_keeper_tip(3).await?;
    f.top_up_pool(5_000_000).await?;
    let pool_before = f.lamports_of(f.fee_pool()).await;

    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, 1.0)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw_ix = f
        .user
        .make_withdraw_ix_with_authority(f.keeper_usdc, &f.src_bank_f, 1.0, None, f.keeper.pubkey())
        .await;
    let deposit_ix = f
        .user
        .make_deposit_ix_with_authority(f.keeper_usdc, &f.dst_bank_f, 1.0, None, f.keeper.pubkey())
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    f.process(&[start_ix, withdraw_ix, deposit_ix, end_ix])
        .await?;

    // moved 1 of a 2 budget -> fraction 1/2 -> owed 1.5 -> floor 1.
    let paid = pool_before - f.lamports_of(f.fee_pool()).await;
    assert_eq!(paid, 1, "tip must floor 1.5 to 1");
    Ok(())
}

// N->N coverage: end-side reject branches (health, overshoot)

/// Consolidating a borrower's collateral into a lower-maintenance-weight same-mint bank drops
/// maintenance health below zero and is rejected at end — even though value is conserved and the
/// move improves rate. The only exercise of the health REJECT branch.
#[tokio::test]
async fn rebalance_consolidate_rejected_when_destination_makes_unhealthy() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    // Low-maintenance-weight USDC destination (0.5), driven to a >0 rate so it clears the gate.
    let low_cfg = BankConfig {
        asset_weight_maint: I80F48::from_num(0.5).into(),
        asset_weight_init: I80F48::from_num(0.5).into(),
        ..*DEFAULT_USDC_TEST_BANK_CONFIG
    };
    let low_dst = f
        .test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(&f.test_f.usdc_mint, None, low_cfg, 104)
        .await?;
    let sol_bank = f.test_f.get_bank(&BankMint::Sol);
    // Drive low_dst utilization: a lender funds it, and a SEPARATE SOL-collateralized account borrows
    // from it (that SOL deposit also supplies the liquidity the user borrows against).
    let lender = f.test_f.create_marginfi_account().await;
    let lender_usdc = f
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, &low_dst, 1_000.0, None)
        .await?;
    let borrower = f.test_f.create_marginfi_account().await;
    let borrower_sol = f
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank, 1_000.0, None)
        .await?;
    let borrower_usdc = f.test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, &low_dst, 500.0)
        .await?;
    f.test_f
        .marginfi_group
        .try_accrue_interest(&low_dst)
        .await?;

    // User borrows 60 SOL ($600) against its 1000 USDC in src: healthy at weight 1 (buffer 400).
    let user_sol = f.test_f.sol_mint.create_empty_token_account().await;
    f.user.try_bank_borrow(user_sol.key, sol_bank, 60.0).await?;

    let payer = f.test_f.context.borrow().payer.pubkey();
    let update = f
        .user
        .make_update_rebalance_order_ix(
            f.order_pda,
            payer,
            Some(vec![f.src_bank_f.key, f.dst_bank_f.key, low_dst.key]),
            None,
            None,
            None,
            None,
        )
        .await;
    f.process_as_payer(&[update]).await?;

    // Consolidate the 1000 USDC into the low-weight dst -> collateral counts as 500 -> health -100.
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(low_dst.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, DEPOSIT_USDC)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            DEPOSIT_USDC,
            Some(true),
            f.keeper.pubkey(),
        )
        .await;
    let deposit = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &low_dst,
            DEPOSIT_USDC,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![f.src_bank_f.key],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix, withdraw, deposit, end_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::WorseHealthPostExecution);
    Ok(())
}

/// A move that clears the start gate but whose own market impact inverts the rate advantage is
/// rejected at end. src carries a small borrow, so draining it to the borrow floor spikes its
/// utilization (rate ~3%) above the destination, which the inflow drops. The only RebalanceOvershoot
/// coverage.
#[tokio::test]
async fn rebalance_rejects_overshoot() -> anyhow::Result<()> {
    let f = setup(I80F48::from_num(0.0001), 0).await?;
    let sol_bank = f.test_f.get_bank(&BankMint::Sol);
    // A SOL-collateralized borrower draws a small 50 USDC from src: start utilization ~5% (rate ~0),
    // but after the user's 1000 is drained down to the 50 borrow floor, src sits at ~100% util.
    let borrower = f.test_f.create_marginfi_account().await;
    let borrower_sol = f
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank, 1_000.0, None)
        .await?;
    let borrower_usdc = f.test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, &f.src_bank_f, 50.0)
        .await?;
    f.test_f
        .marginfi_group
        .try_accrue_interest(&f.src_bank_f)
        .await?;

    // Move the withdrawable 950 (leaving the 50 that backs the borrow) from src into dst.
    let moved = 950.0;
    let ref_banks = vec![f.bank_meta(f.src_bank_f.key), f.bank_meta(f.dst_bank_f.key)];
    let start_ix = f
        .user
        .make_rebalance_start_ix(
            ref_banks.clone(),
            vec![rebalance_move(0, 1, moved)],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
            f.keeper.pubkey(),
        )
        .await;
    let withdraw = f
        .user
        .make_withdraw_ix_with_authority(
            f.keeper_usdc,
            &f.src_bank_f,
            moved,
            None,
            f.keeper.pubkey(),
        )
        .await;
    let deposit = f
        .user
        .make_deposit_ix_with_authority(
            f.keeper_usdc,
            &f.dst_bank_f,
            moved,
            None,
            f.keeper.pubkey(),
        )
        .await;
    // src keeps its 50 remainder (active), so it stays in the observation set.
    let end_ix = f
        .user
        .make_rebalance_end_ix(
            ref_banks,
            vec![],
            f.order_pda,
            f.record_pda,
            f.keeper.pubkey(),
        )
        .await;
    let res = f.process(&[start_ix, withdraw, deposit, end_ix]).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RebalanceOvershoot);
    Ok(())
}
