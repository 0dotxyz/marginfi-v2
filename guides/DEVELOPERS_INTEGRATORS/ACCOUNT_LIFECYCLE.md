# Account Lifecycle and Flags

## Glossary

- **MarginfiAccount** - A user's account on the protocol. Holds references to up to 16 lending
  positions (called "balances"). Owned by a single `authority`.
- **Balance** - A single asset position within a `MarginfiAccount`. Can be a lending (deposit) or
  borrowing position in a specific bank.
- **Authority** - The keypair (or program) that controls a `MarginfiAccount`.

## Account Creation

A `MarginfiAccount` is created with `MarginfiAccountInitialize`. The signer becomes the
account's `authority`. The account is created empty, with no balances.

Each account is associated with a single `MarginfiGroup` and can hold positions in any bank that
belongs to that group.

## Account Flags

The `MarginfiAccount.account_flags` field is a 64-bit bitmask that controls the account's state.
Flags are set and cleared by the protocol under specific conditions.

### Disabled (Bit 0)

- **Flag**: `ACCOUNT_DISABLED` (value 1)
- **Set by**: Bankruptcy handling
- **Effect**: The account is disabled, typically after all positions have been zeroed out due to
  bankruptcy.

### In Flashloan (Bit 1)

- **Flag**: `ACCOUNT_IN_FLASHLOAN` (value 2)
- **Set by**: The flashloan instruction
- **Cleared by**: End of flashloan

While this flag is active, health checks are deferred. The protocol verifies account health only
at the end of the flashloan transaction. This allows operations that would temporarily leave the
account unhealthy (e.g. borrow then deposit in the same tx).

### Deprecated Flags (Bits 2-3)

- **Bit 2** (`ACCOUNT_FLAG_DEPRECATED`, value 4): Deprecated, reserved for future use.
- **Bit 3** (`ACCOUNT_TRANSFER_AUTHORITY_DEPRECATED`, value 8): Deprecated, was previously used
  for account transfers.

### In Receivership (Bit 4)

- **Flag**: `ACCOUNT_IN_RECEIVERSHIP` (value 16)
- **Set by**: `StartLiquidation` (receivership mode)
- **Cleared by**: `EndLiquidation`
- **Effect**: The account enters receivership. The `liquidation_receiver` gets temporary control.
  The original authority is locked out until receivership ends. During receivership, withdraw
  operations become available to the receiver.

For more details see the [Receivership Liquidation Guide](../RISK_AND_LIQUIDATORS/RECEIVERSHIP_LIQUIDATION.md).

### In Deleverage (Bit 5)

- **Flag**: `ACCOUNT_IN_DELEVERAGE` (value 32)
- **Set by**: `StartLiquidation` (deleverage mode, risk admin only)
- **Effect**: Similar to receivership, but specifically for forced deleverage scenarios where the
  risk admin is unwinding positions without token transfers.

### Frozen (Bit 6)

- **Flag**: `ACCOUNT_FROZEN` (value 64)
- **Set by**: Group admin via `MarginfiAccountSetFreeze`
- **Cleared by**: Group admin via `MarginfiAccountSetFreeze`
- **Effect**: The account's authority is completely blocked. Only the group admin can perform
  operations on the account. This is used for compliance, investigations, or protecting accounts.

A frozen account's positions continue to accrue interest and can still be liquidated if unhealthy.
The freeze only blocks the authority from interacting.

## Authorization Logic

When a user operation is attempted, the protocol checks authorization in this order:

1. **KilledByBankruptcy**: If the bank is killed, the operation is blocked regardless of who calls it.
2. **Receivership**: If the account is in receivership and the operation allows it, any signer is
   authorized.
3. **Frozen**: If the account is frozen, only the group admin is authorized.
4. **Normal**: The signer must match the account's `authority`.

```
Is account in receivership? ──Yes──> Any signer OK (for allowed operations)
         │
         No
         │
Is account frozen? ──Yes──> Only group admin OK
         │
         No
         │
Is signer == authority? ──Yes──> OK
         │
         No
         │
    Unauthorized
```

## Health Checks

Most operations that change an account's risk profile trigger a health check. The protocol
calculates two health values:

- **Initial Health** (for new positions): Uses `asset_weight_init` and `liability_weight_init`.
  Must be >= 0 after any deposit, borrow, or withdrawal.
- **Maintenance Health** (for liquidation eligibility): Uses `asset_weight_maint` and
  `liability_weight_maint`. If < 0, the account can be liquidated.

The "health buffer" is the gap between these two values, providing a cushion before liquidation.

## Account Lifecycle Stages

### 1. Active

The normal state. The authority can freely deposit, withdraw, borrow, repay, and perform flash
loans. Health checks apply to operations that increase risk.

### 2. Unhealthy

When maintenance health drops below zero, the account becomes eligible for liquidation. The
authority can still operate the account (e.g. to repay debt), but cannot take actions that would
further reduce health.

### 3. In Liquidation (Receivership)

If an account is unhealthy, anyone can call `StartLiquidation`. This puts the account in
receivership. The liquidator (receiver) can then withdraw collateral and repay debts to bring the
account back to health. See the
[Receivership Liquidation Guide](../RISK_AND_LIQUIDATORS/RECEIVERSHIP_LIQUIDATION.md).

### 4. Bankrupt

If an account's equity drops below the bankruptcy threshold ($0.10), it can be handled by the
`HandleBankruptcy` instruction. Bad debt is socialized across lenders via the insurance fund. The
account is effectively zeroed out and disabled.

### 5. Closed

An account with no remaining balances can be closed by its authority. The rent-exempt SOL is
returned to the authority.

## Position Limits

Each `MarginfiAccount` can hold up to **16 balances** (positions) simultaneously. This covers both
lending and borrowing positions. If you need more positions, you must create additional accounts.

An account can hold at most one position per bank: you cannot have both a lending and borrowing
position in the same bank simultaneously.

## Risk Tier Restrictions

Banks can be configured as either `Collateral` or `Isolated` risk tier:

- **Collateral**: Can be borrowed alongside other assets. No restrictions on combining positions.
- **Isolated**: Can only be borrowed in isolation. If you have a borrow in an isolated bank, you
  cannot have any other borrow positions. You can still have multiple lending positions. Isolated
  assets must have asset weights of 0, so they contribute no collateral value when deposited.

This restriction is checked at borrow time. If an account already has a non-isolated borrow and
attempts to borrow an isolated asset (or vice versa), the operation is rejected.
