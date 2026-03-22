# Bank State Guide

## Glossary

- **Operational State** - The primary state of a bank, controlling which user operations are
  allowed. Stored in `BankConfig.operational_state`.
- **Bank Flags** - A bitmask of flags (`Bank.flags`) that toggle specific bank behaviors such as
  emissions, permissionless bad debt settlement, and settings freeze.
- **Config Flags** - A separate byte (`BankConfig.config_flags`) reserved for configuration-level
  flags.

## Bank Operational States

Every bank has an operational state that determines which user operations are allowed. When a new
bank is created, it starts in the **Paused** state. The group admin must explicitly set it to
**Operational** before users can interact with it.

### Paused

All operations are halted. Users cannot deposit, borrow, withdraw, repay, or be liquidated. This is
the default state for newly created banks.

Use cases:
- Initial setup: configure the bank before allowing users to interact with it.
- Emergency: halt all activity on a bank while investigating an issue.

### Operational

Normal operations. All user actions are allowed: deposit, borrow, withdraw, repay, and liquidation.

### ReduceOnly

Only withdrawals and repayments are allowed. New deposits and borrows are blocked. This state is
intended for winding down a bank.

Important nuances for health calculations in ReduceOnly:
- **Initial margin**: assets in a ReduceOnly bank are valued at **$0**. This means users cannot
  open new borrows using ReduceOnly collateral.
- **Maintenance margin**: assets in a ReduceOnly bank retain their **full value**. This means
  existing positions are not immediately liquidatable just because a bank entered ReduceOnly.

This asymmetry is by design: the system prevents new risk from being taken on ReduceOnly assets,
while not force-liquidating users who already hold them.

### KilledByBankruptcy

The bank was killed by a bankruptcy event and is irrecoverable. All operations are blocked. This
state can only be set programmatically by the `handle_bankruptcy` instruction when a bankruptcy
event wipes out all remaining assets in the bank. It **cannot** be set manually by an admin.

## Summary Table

| State | Deposit | Borrow | Withdraw | Repay | Liquidate | Initial Margin | Maintenance Margin |
|-------|---------|--------|----------|-------|-----------|----------------|--------------------|
| **Paused** | No | No | No | No | No | N/A | N/A |
| **Operational** | Yes | Yes | Yes | Yes | Yes | Full value | Full value |
| **ReduceOnly** | No | No | Yes | Yes | Yes | $0 | Full value |
| **KilledByBankruptcy** | No | No | No | No | No | N/A | N/A |

## State Transitions

The group admin can transition a bank between Paused, Operational, and ReduceOnly using the
`configure_bank` instruction. The admin **cannot** set a bank to KilledByBankruptcy directly;
that transition only happens automatically during bankruptcy resolution.

```
          admin sets             admin sets              admin sets
Paused <───────────> Operational <───────────> ReduceOnly
                           │                       │
                           │   handle_bankruptcy    │
                           └───────────┬────────────┘
                                       ▼
                              KilledByBankruptcy
                              (irrecoverable)
```

## Bank Flags

The `Bank.flags` field is a 64-bit bitmask. Each bit controls a specific behavior:

### Emissions Flags (Bits 0-1)

- **Bit 0** (`EMISSIONS_FLAG_BORROW_ACTIVE`, value 1): Enables emissions rewards for borrowers.
- **Bit 1** (`EMISSIONS_FLAG_LENDING_ACTIVE`, value 2): Enables emissions rewards for lenders.

These flags control whether the bank distributes token incentives to users. When enabled, the bank
uses `emissions_rate`, `emissions_remaining`, and `emissions_mint` to distribute rewards
proportionally. For more details see the [Emissions Guide](../USER/EMISSIONS.md).

### Permissionless Bad Debt Settlement (Bit 2)

- **Bit 2** (`PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG`, value 4): When set, anyone can call the
  `handle_bankruptcy` instruction for this bank. When not set, only the `risk_admin` or `admin`
  can do so.

This is useful for banks where you want the community or bots to be able to settle bad debt without
waiting for the risk admin.

### Freeze Settings (Bit 3)

- **Bit 3** (`FREEZE_SETTINGS`, value 8): When set, most bank configuration parameters are frozen.
  Only `deposit_limit` and `borrow_limit` can still be changed. Note: `total_asset_value_init_limit`
  is also frozen (it can only be updated when the bank is NOT frozen).

This flag provides a credible commitment that the bank's risk parameters, oracle configuration,
interest rate curves, and other settings will not change. It can only be set through the
`configure_bank` instruction by the group admin. Once frozen, the admin can still adjust capacity
limits, but cannot change anything that affects the risk profile of the bank (such as weights,
oracle setup, interest rate curves, init limit, etc).

### Close Enabled (Bit 4)

- **Bit 4** (`CLOSE_ENABLED_FLAG`, value 16): Enables the bank to be closed. Banks cannot be
  closed unless this flag is set. This flag is **automatically set at creation** for all banks
  created in 0.1.4 or later. There is no instruction to toggle it after creation — if it is ever
  cleared (e.g. by the emissions flag bug), the bank can never be closed.

### Tokenless Repayments (Bits 5-6)

- **Bit 5** (`TOKENLESS_REPAYMENTS_ALLOWED`, value 32): When set, the risk admin can perform
  tokenless repayments (deleverage). This writes off debt without requiring actual token transfers.
- **Bit 6** (`TOKENLESS_REPAYMENTS_COMPLETE`, value 64): Signals that all tokenless repayments for
  this bank are complete.

These are used during forced deleveraging scenarios where the risk admin needs to unwind positions
without moving tokens.

## Flags Summary Table

| Bit | Name | Value | Who Sets It | Effect |
|-----|------|-------|-------------|--------|
| 0 | `EMISSIONS_FLAG_BORROW_ACTIVE` | 1 | Emissions admin | Enables borrow emissions |
| 1 | `EMISSIONS_FLAG_LENDING_ACTIVE` | 2 | Emissions admin | Enables lending emissions |
| 2 | `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` | 4 | Admin | Anyone can settle bad debt |
| 3 | `FREEZE_SETTINGS` | 8 | Admin | Freezes most bank config (only deposit/borrow limits changeable) |
| 4 | `CLOSE_ENABLED_FLAG` | 16 | Auto (at creation) | Allows bank closure. Cannot be toggled after creation. |
| 5 | `TOKENLESS_REPAYMENTS_ALLOWED` | 32 | Admin | Allows deleverage repayments |
| 6 | `TOKENLESS_REPAYMENTS_COMPLETE` | 64 | Auto or Risk admin | Signals deleverage complete. Auto-set when liabilities reach zero on a TOKENLESS_REPAYMENTS_ALLOWED bank. Can also be force-set by risk admin. |

## Typical Bank Lifecycle

1. **Creation**: Bank is created in the **Paused** state. The admin configures oracle, risk
   parameters, interest rate curve, and limits.
2. **Go Live**: Admin sets the state to **Operational**. Users can deposit, borrow, etc.
3. **Normal Operation**: The bank operates normally. The admin may adjust limits as needed. If
   `FREEZE_SETTINGS` is set, only limits can change.
4. **Wind Down** (if needed): Admin sets the state to **ReduceOnly**. Users can only withdraw and
   repay. No new positions can be opened.
5. **Closure** (if needed): Once all positions are closed and the bank is empty, the admin can
   close the bank (`CLOSE_ENABLED_FLAG` is already set from creation).
6. **Bankruptcy** (edge case): If a bankruptcy event wipes out all bank assets, the bank
   transitions to **KilledByBankruptcy** and is irrecoverable.
