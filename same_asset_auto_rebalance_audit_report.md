# Same-Asset Auto-Rebalance Fixes — Static Security Review

**Reviewer:** Independent third-party style review  
**Target:** `same-asset-auto-rebalance-fixes` (`a51e3aa3`)  
**Merge base / intended target:** `0.1.12-main` (`a22a568d`)  
**Reference deployment branch:** `main`  
**Review type:** Static source analysis only; no builds or tests were run.

## Executive summary

The branch resolves the prior report's material same-asset-rebalance integrity issue: a keeper can no longer use a rebalance sandwich to clear an unrelated order tag and permissionlessly close the affected order. The new allowlist binding is complemented by tag snapshotting and whole-balance tag carry, which is the appropriate defence-in-depth design.

The branch also implements the prior report's native-rate, JupLend-rate, empty-balance, fee-pool, post-close-settlement, and health-cache improvements. I found no Critical or High severity issue, and no direct loss-of-principal path in the reviewed delta.

One Low-severity edge case from the aggregate destination/capacity work remains open: a high-rate candidate is considered even when its previously declared inflow plus the move under examination exceeds its capacity. This can reject an otherwise valid rebalance; it does not create partial execution or trap user assets because the sandwich is atomic. I would resolve it before describing P0 L01 as fully fixed.

## Scope and approach

Reviewed the seven commits between `0.1.12-main` and `HEAD`, concentrating on the same-asset rebalance flow, its interaction with order tags, fee-pool/account-close lifecycle, pricing, state layout, and integrator interfaces. The attached Adevar pre-fix report was used only for findings related to same-asset rebalance: M01, L01–L06, and E01.

This review did not compile the program, build IDLs, query chain state, or execute any test. Test coverage mentioned below is source-level evidence only.

## Findings

### SA-01 — Aggregate capacity comparison can reject a feasible rebalance

**Severity:** Low  
**Status:** Open; P0 L01 is only partially remediated.  
**Location:** `programs/marginfi/src/instructions/marginfi_account/rebalance.rs`, candidate selection around lines 940–951.

The branch correctly aggregates all inbound moves to price each selected destination at its final planned inflow. However, it excludes an alternative candidate only when:

```rust
inflow[i] >= capacity[i]
```

For the current move, the candidate is instead priced at `inflow[i] + amount_native`. If `inflow[i] < capacity[i]` but `inflow[i] + current_move > capacity[i]`, the candidate cannot take the whole counterfactual move and should be skipped. It is nevertheless compared, and a higher hypothetical rate can cause `RebalanceNotBestVenue`.

Example: candidate C has 1.0 token capacity and already receives 0.5 tokens elsewhere in the submitted batch. When assessing a 1.0-token move to destination D, C is priced at 1.5 tokens despite being unable to accept that move. If C's hypothetical rate exceeds D's, a valid move to D is rejected.

The selected destination is likewise not explicitly checked for `inflow[d] <= capacity[d]`; an oversized selected deposit should fail later in its deposit leg and revert the whole transaction. Therefore the impact is availability/routing quality and wasted keeper transaction fees, not a partial move or loss of user principal.

**Recommendation:**

- Reject the proposed destination at start if its aggregate planned inflow exceeds its capacity.
- Skip a candidate unless its remaining capacity covers the complete counterfactual amount: `inflow[i] + move_amount <= capacity[i]` (with checked arithmetic and the intended rounding policy).
- Add tests for both cases, especially a partially pre-filled high-rate candidate whose remaining capacity is smaller than the move.

### SA-02 — JupLend rate-model validation relies on an upstream uniqueness invariant

**Severity:** Informational / defence in depth  
**Status:** Open.  
**Location:** `programs/marginfi/src/state/rate.rs`, `load_juplend_rate_model` around lines 408–424.

The new JupLend pricing path verifies that the supplied account is owned by the liquidity program, has a `RateModel` discriminator, and embeds the reserve mint. The repository already has the canonical PDA derivation (`["rate_model", mint]`) in `type-crate/src/pdas.rs`, but the on-chain validator does not require the supplied key to equal that PDA.

Current safety therefore depends on the external liquidity program continuing to provide exactly one valid `RateModel` for a mint and never creating another program-owned, same-mint model. This is a reasonable current assumption, but the rate is supplied by a permissionless keeper and directly controls the rebalance eligibility gate. Binding the canonical PDA would eliminate this dependency and make the integration safer against external-program evolution or configuration mistakes.

**Recommendation:** derive and require the canonical JupLend rate-model PDA in `load_juplend_rate_model`, then add a negative test for a non-canonical model-shaped account.

### SA-03 — Integrator contract changes need explicit release documentation

**Severity:** Informational  
**Status:** Open documentation/release task.

Two existing instruction interfaces change materially:

- For every JupLend bank in `start_rebalance` and `end_rebalance`, the remaining-account block now requires `RateModel` after `[rewards_rate_model, f_token_mint]`. Existing custom keepers will fail account parsing or use the wrong account layout until updated.
- `admin_close_account` now requires the rebalance fee-pool PDA. This is a necessary protection against orphaning fee-pool SOL, but direct instruction builders must add the account.

The local test helpers and the TypeScript venue test have been updated. Direct `admin_close_account` builders nevertheless need a coordinated update. The public instruction comments remain stale in at least one location: `StartRebalance` still describes its remaining accounts as `[bank, (JupLend reserve), oracles]`.

**Recommendation:** publish the exact remaining-account layouts for start, end, and settlement; regenerate/publish IDL and client types as part of the release; include the new `admin_close_account` account in the migration note; and update the stale inline comment.

## Adevar same-asset-rebalance remediation assessment

| Prior item | Assessment | Evidence |
| --- | --- | --- |
| M01 — unrelated `withdraw_all` clears an order tag | Fixed | `validate_rebalance_instructions` binds every move leg to the account and full order allowlist. `RebalanceRecord` snapshots tags, verifies untouched tags, and carries a tagged source only as a whole move. `ExecuteOrderRecord` now also compares saved tags. |
| L01 — aggregate destination pricing | Partially fixed | Aggregate inflow is used for selected-destination and candidate pricing. SA-01 leaves the capacity half of the recommendation incomplete. |
| L02 — native deposit dilution | Fixed | `NativeRateModel` captures accrued totals and evaluates the configured lending curve at post-deposit utilization. |
| L03 — stale JupLend borrow-rate simulation | Fixed, subject to SA-02 hardening | The JupLend path now passes the mint `RateModel` and prices non-zero deposits on post-deposit utilization. |
| L04 — unrelated empty balance blocks start | Fixed | Initialization and unchanged-balance verification consistently skip active balances with no side; the same robustness improvement was applied to execute-order snapshots. |
| L05 — tip settlement after account close | Fixed | Settlement uses a checked manual loader that accepts a closed account and obtains identity from the record. |
| L06 — close orphaning the fee pool | Fixed | `admin_close_account` requires the fee-pool PDA and rejects a non-zero balance. |
| E01 — rebalance health-cache visibility | Fixed | `end_rebalance` and `end_execute_order` stamp cache timestamp, program version, and engine status. |

## Security and lifecycle observations

- The M01 remediation is strong: it prevents both the original unallowlisted-balance sandwich and the more subtle mutation of an otherwise referenced but untouched tagged balance. Tagged balances are only movable as a complete, one-source/one-empty-destination transfer, and the tag is reattached before the rebalance flag clears.
- The transaction-structure allowlist remains important. It permits only the start/end and move-leg Marginfi instructions, and only refresh/crank instructions for venues, preventing unrelated Marginfi tag-mutating instructions or venue utilization mutations in the sandwich.
- The fee-pool close fix is compatible with the post-close settlement design: a pending tip is escrowed in the record, and an account can be closed only after the authority has emptied the pool. Settlement can then close the record later.
- Error codes 6718–6720 are appended inside the dedicated rebalance range; this branch does not renumber existing errors.

## Layout and compatibility assessment

No state struct already deployed on `main` is modified by this branch. The rebalance sources and `RebalanceRecord` do not exist on `main`; accordingly, increasing `RebalanceRefBank` from 48 to 56 bytes and `RebalanceRecord` from 1,800 to 1,864 bytes does not violate mainnet deployed-layout compatibility for this release.

There is an important future release condition: do not introduce this exact record-size change into a cluster that already has live pending rebalance records without first ensuring they are settled/closed or providing a migration-compatible reader. A record persists while a tip is pending, and an old allocated record cannot safely be deserialized using the enlarged layout.

## Test and release recommendations

The source adds valuable regression coverage for tag preservation, allowlist-bound legs, aggregate destination rate effects, native dilution, JupLend curve pricing, empty slots, fee-pool closing, post-close settlement, and health-cache stamps. None was executed for this review.

Before merge/release, run the appropriate existing integration and TypeScript slices and add at least:

1. A partially pre-filled candidate-capacity test for SA-01.
2. An aggregate selected-destination capacity rejection test at `start_rebalance`.
3. A canonical JupLend RateModel PDA validation test if SA-02 is adopted.
4. Client/IDL smoke coverage that uses the documented JupLend remaining-account order and the amended `admin_close_account` account list.

## Conclusion

The branch materially improves the safety of same-asset rebalance and remediates the prior Medium tag-loss issue. Subject to addressing SA-01 and completing the release/interface documentation, I found the changes fit for their stated purpose and found no direct user-fund-loss issue in the reviewed delta.
