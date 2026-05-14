# Trident Fuzz Tests

For Trident source code, check out the [Trident repository](https://github.com/Ackee-Blockchain/trident).

## How to run the tests:

### 1. Install trident-cli

```bash
cargo install trident-cli --version 0.13.0-rc.4
```

### 2. Build the programs

```bash
# execute from project root
cargo build-sbf --manifest-path programs/marginfi/Cargo.toml --arch v2
```

or just run the Makefile:

```bash
make build
```

> [!WARNING]
> Make sure the Solana-CLI version is 3.1.13.

### 3. Run the tests

```bash
trident fuzz run fuzz_0
# or another fuzz test by name
```


> [!WARNING]
> Due to the incident around the Drift protocol, the Drift integration tests are not present. From the onchain data, it looks like there is no current traffic, so there are no accounts to fork.


## Fuzz `fuzz_0` — invariant checklist

These are the high-level properties we assert during fuzzing. The checks live in `fuzz_0/invariants/` and are invoked from `fuzz_0/methods/`.

### Core token/balance invariants (`invariants/core.rs`)

- **Token account reads are well-defined**: every sampled SPL account must deserialize as a token account so balances are defined.
- **Failure is side-effect free (token leg)**: for core lending ops (deposit/withdraw/borrow/repay), if the tx fails then the touched user token account + touched bank liquidity vault do not change.
- **User–vault conservation**: for successful core lending ops, the user token delta and bank liquidity vault delta net to zero (no silent mint/burn on that leg).
- **Cross-bank isolation**: liquidity vaults of banks not involved in the operation stay unchanged.
- **Flashloan empty-body snapshot**: when the flashloan has no inner instructions, a snapshot of relevant token accounts remains unchanged.

### Share invariants (`invariants/shares.rs`)

- **Deposit success (shares)**: asset shares increase for `amount > 0`; liability shares unchanged. `amount == 0` may either be a no-op or open an empty active-balance row (`find_or_create` behavior).
- **Withdraw success (shares)**: asset shares decrease (or the row closes cleanly) when a non-trivial withdraw happens; liability shares unchanged.
- **Borrow success (shares)**: liability shares increase for `amount > 0`; asset shares unchanged. `amount == 0` follows the same “no-op vs open row” rule as deposit.
- **Repay success (shares)**: liability shares decrease (or the row closes cleanly) when a non-trivial repay happens; asset shares unchanged.

### Accrue invariants (`invariants/accrue.rs`)

- **Accrue advances `last_update`**: after a successful `LendingPoolAccrueBankInterest` batch, each fuzz bank’s `last_update.slot` advances (not a `time_delta == 0` no-op).

### Liquidation invariants (`invariants/liquidation.rs`)

- **Liability-vault accounting**: liability liquidity vault + insurance vault total is conserved; liquidity vault does not grow; insurance vault does not shrink.
- **Success (share direction)**: for non-zero liquidation, share directions match expectation (liquidatee asset/liability decrease; liquidator asset/liability increase for the touched banks).
- **Failure is state-unchanged**: sampled vault balances + sampled share fields remain bitwise unchanged.

### Receivership invariants (`invariants/receivership.rs`)

- **End-liquidation clears flags (success)**: marginfi account clears `ACCOUNT_IN_RECEIVERSHIP`, `ACCOUNT_IN_FLASHLOAN`, and `ACCOUNT_IN_ORDER_EXECUTION`, and receivership record fields are consistent.

### Flashloan invariants (`invariants/flashloan.rs`)

- **Closed loop**: when borrow amount equals repay amount and the tx succeeds, the user’s token balance for that asset is unchanged.
- **Mismatch must fail**: when borrow and repay amounts differ, the flashloan tx is expected to fail.

### Kamino integration (`invariants/kamino/*`)

- **Deposit (success)**: user outflow matches reserve supply inflow; marginfi liquidity vault is a “hop” (net-zero); collateral destination increases (or no movement for `amount == 0`).
- **Deposit (failure)**: user / marginfi vault / reserve supply / collateral destination balances remain unchanged.
- **Withdraw (success)**: reserve supply decreases and user increases consistently; marginfi liquidity vault is a “hop” (net-zero); collateral source decreases (handles `withdraw_all`).
- **Withdraw (failure)**: user / marginfi vault / reserve supply / collateral source balances remain unchanged.

### Juplend integration (`invariants/juplend/*`)

- **Deposit (success)**: user decreases; fToken vault increases; marginfi liquidity vault behaves as a hop (net-zero movement when applicable), with `amount == 0` treated as no movement.
- **Deposit (failure)**: user / marginfi vault / fToken vault remain unchanged.
- **Withdraw (success)**: user increases; fToken vault decreases; withdraw intermediary ATA is a hop (net-zero), with `withdraw_all` handled explicitly.
- **Withdraw (failure)**: user / withdraw intermediary ATA / marginfi vault / fToken vault remain unchanged.

