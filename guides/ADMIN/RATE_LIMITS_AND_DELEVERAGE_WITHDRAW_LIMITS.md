# Group Rate Limits And Deleverage Withdraw Limits

This guide covers two related protections that now share the same admin surface:

- `admin`
- `delegate_limit_admin`

Either signer can configure the limits and post the aggregated admin updates.

## Why These Flows Exist

Both systems avoid making the group account writable in every user transaction.

- Bank-level rate limiting is updated inline because the bank account is already writable.
- Group-level rate limiting is checked read-only during user actions, then settled later from
  aggregated events.
- Deleverage withdraw limits are also checked read-only during the withdraw, then settled later from
  aggregated events.

This avoids serializing all activity in a group through one writable group account.

## Group Rate Limit Flow

### 1. Configuration

Two levels exist:

- Bank rate limits: `configure_bank_rate_limits`
  - Tracks native token net outflow on the bank account.
- Group rate limits: `configure_group_rate_limits`
  - Tracks USD net outflow across the whole group.

Each has hourly and daily windows.

- `0` disables that window.
- Deposits/repays release capacity.
- Withdraws/borrows consume capacity.

### 2. User transaction path

During a withdraw or borrow:

- The bank rate limiter is updated immediately on the writable bank account.
- The group rate limiter is only checked read-only.
- The protocol converts the flow to USD using the instruction price/oracle path.
- If the projected group hourly or daily capacity is exceeded, the user instruction fails.
- A `RateLimitFlowEvent` is emitted for off-chain aggregation.

Important details:

- Flashloans, liquidations, and deleverages skip the normal rate-limit accounting path.
- `RateLimitFlowEvent` is an indexing aid, not a source of truth. Solana log truncation can drop
  events, so the off-chain worker must reconcile gaps instead of assuming no event means no flow.

### 3. Admin settlement path

The off-chain worker batches the observed flows and calls `update_group_rate_limiter`.

Inputs:

- `outflow_usd`
- `inflow_usd`
- `update_seq`
- `event_start_slot`
- `event_end_slot`

Rules enforced on-chain:

- At least one of `outflow_usd` or `inflow_usd` must be present.
- `event_start_slot <= event_end_slot`
- `event_start_slot > last_admin_update_slot`
- `event_end_slot <= current slot`
- The batch must not be stale (`<= 1500` slots old)
- `update_seq` must equal `last_admin_update_seq + 1`

Application order matters:

- Inflow is applied first.
- Outflow is applied second.

That lets a single batch include both releases and consumptions of capacity.

## Deleverage Withdraw Limit Flow

This is separate from the normal borrow/withdraw rate limiter.

It exists to cap how much USD value can be withdrawn from a group through forced deleveraging in a
day, as a defense if the risk workflow is abused or compromised.

### 1. Configuration

The daily cap is set with `configure_deleverage_withdrawal_limit`.

- The configured limit is a USD integer (`u32`).
- The instruction requires a non-zero value.
- Internally, the runtime still treats `0` as unlimited for backward compatibility when checking
  cached state.

### 2. Deleverage withdraw path

During a deleverage withdraw:

- The protocol computes the withdrawn equity value in USD.
- It checks the projected day total read-only with `check_deleverage_withdraw_limit`.
- If the projected total exceeds the configured daily cap, the withdraw fails.
- A `DeleverageWithdrawFlowEvent` is emitted.

At this stage, the cached group counter is not yet mutated by the user instruction.

### 3. Admin settlement path

The off-chain worker aggregates those deleverage-only withdraw events and calls
`update_deleverage_withdrawals`.

Inputs:

- `outflow_usd`
- `update_seq`
- `event_start_slot`
- `event_end_slot`

Rules enforced on-chain:

- `outflow_usd` must be non-zero
- `event_start_slot <= event_end_slot`
- `event_start_slot > last_admin_update_slot`
- `event_end_slot <= current slot`
- The batch must not be stale (`<= 1500` slots old)
- `update_seq` must equal `last_admin_update_seq + 1`

When the update lands:

- The on-chain daily counter is advanced with `update_withdrawn_equity`
- Daily reset behavior is handled from timestamp
- The resulting `withdrawn_today` value must still remain within the configured daily cap

