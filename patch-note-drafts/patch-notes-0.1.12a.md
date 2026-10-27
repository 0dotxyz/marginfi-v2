# marginfi v0.1.12a

# Summary

## Lower Liquidation Tagging Premium Cap

The maximum receivership liquidation premium reached through tag growth is reduced from 100% to
25%. At the new ceiling, a liquidator may seize up to $1.25 of collateral for each $1 of debt repaid.

The timing stays the same: the premium remains at its base for the first hour after tagging, then
grows linearly to the ceiling at 7 days. With a 5% base, the premium is now 15% halfway through the
growth window.

Configured base premiums above 25% continue to apply. The existing exceptions for accounts in bad
debt and accounts boosted by e-mode are unchanged.

Existing tags use the new curve after the program upgrade. No account migration or instruction
changes are required.

## Order Execution Event

`end_execute_order` now emits `MarginfiAccountExecuteOrderEvent` after its execution checks pass,
so indexers and clients can track order executions. The event includes:

- The order address and an account header identifying the executor, marginfi account, account
  authority, and group.
- `created_at`: the configured Order's creation timestamp in Unix seconds (0 for legacy orders),
  and `max_slippage`: its configured limit, encoded as a u32 fraction where `u32::MAX` is 100%.
- `asset_bank` and `asset_shares_after`: the bank and remaining position shares of the tagged
  asset balance after execution. Shares are not token amounts or swap proceeds. Starting amounts
  and the closed liability bank are not retained by the execution record and are not emitted.
- `triggered`: the validated `StopLoss` or `TakeProfit` branch, including for orders configured
  with both triggers. This field never reports `Both`.
- `equity_before_usd` and `equity_after_usd`: the protocol-valued USD equity of the tagged order
  balances immediately before and after execution, encoded as `WrappedI80F48`.
- `closure_recipient` and `order_lamports_returned`: the closure recipient and the lamports
  returned from closing the order account. The execution record's returned lamports are not emitted.

Consumers must verify that the transaction succeeded before treating the event as a completed
execution. The equity difference is not realized trading P/L, and the returned lamports are account
closure proceeds, not trading fees.

Event consumers must use the updated IDL for the new event layout. The execution-record layout,
instruction arguments, and required accounts are unchanged.
