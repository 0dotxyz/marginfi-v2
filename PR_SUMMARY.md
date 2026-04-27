This PR adds a per-bank oracle circuit breaker to reduce the blast radius of bad or manipulated price moves. The goal is to stop a bank from continuing normal risk-taking when the live oracle price jumps too far from its cached reference, using tiered temporary halts similar to market circuit breakers.

The breaker compares each fresh oracle observation against an EMA-based reference price:

`new_reference = alpha * current_price + (1 - alpha) * old_reference`

To avoid fast re-anchoring on attacker-controlled moves, the reference update is clipped per pulse:

`clipped_shift = clamp(new_reference - old_reference, -max_shift, +max_shift)`

`reference_after_update = old_reference + clipped_shift`

where:

- `alpha = cb_ema_alpha_bps / 10_000`
- `max_shift = old_reference * CB_MAX_EMA_SHIFT_BPS_PER_PULSE / 10_000`

The breaker then measures the observed move in basis points after discounting oracle confidence:

`raw_delta = abs(current_price - reference_price)`

`effective_delta = max(raw_delta - confidence, 0)`

`deviation_bps = effective_delta * 10_000 / reference_price`

If `deviation_bps` crosses a configured tier threshold for enough consecutive counted observations, the bank trips into a temporary halt. Repeated re-breaches inside the escalation window ratchet the bank into longer halts, and repeated severe tier-3 trips can promote the bank into a full paused state.

## Changes

- Added configurable circuit-breaker settings to bank config: enable flag, 3 deviation tiers, 3 halt durations, sustain-observation count, escalation window multiplier, and EMA alpha for the reference price.
- Added bank-side circuit-breaker runtime/state machine logic that:
  - tracks an EMA reference price,
  - ignores duplicate/stale observations,
  - subtracts oracle confidence before evaluating the move,
  - requires repeated breach observations before tripping,
  - escalates halts across tiers on repeated re-breaches,
  - can auto-promote repeated severe tier-3 events into a full bank pause.
- Enforced halt behavior in instruction flows:
  - block borrow/withdraw and adapter flows that increase risk,
  - allow risk-reducing actions like deposit/repay to continue,
  - freeze interest accrual while a bank is circuit-breaker halted,
  - restrict direct liquidation to admin/risk-admin when a halted bank is involved.
- Added admin/risk-admin recovery path via `lending_pool_clear_circuit_breaker`, with optional reference-price reseed.
- Added new events and errors so trips, clears, observed breaches, and auto-pauses are visible on-chain.
- Wired the new fields through shared types, TypeScript helpers, test utils, and CLI config/update surfaces.
- Added Rust and TS coverage for config validation, halt triggering, blocked/allowed actions during halt, escalation behavior, and admin clear flow.
