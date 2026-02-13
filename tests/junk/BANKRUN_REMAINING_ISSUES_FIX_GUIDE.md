# Bankrun remaining issues: fixes + troubleshooting guide

This guide is for an implementing agent that already has **bankrun-only** tests mostly working, but still has:

- tests that were temporarily **skipped** (emissions + raw tx)
- the **staked** test suite failing due to missing validator / stake-pool setup
- confusion around **`getAccountInfo` for missing accounts**

The goal is to fix these issues with **minimal, targeted changes** (prefer adapting the bankrun harness / root hooks, rather than rewriting test logic).

---

## 1) `getAccountInfo` in bankrun: make it behave like real `Connection`

### Symptom
Some bankrun connection proxies throw an exception like:

- `Could not find account <pubkey>`

where a normal `@solana/web3.js` connection would return:

- `null`

This is the root cause of the “get account info missing thing”, and it’s what encourages test code to start using `try/catch` for missing accounts.

### Fix
Patch the bankrun connection **once** (in `tests/rootHooks.ts`) so `getAccountInfo` returns `null` when the account is missing.

Minimal pattern:

- store the original `getAccountInfo`
- wrap it
- if the error message starts with `Could not find`, return `null`

This avoids sprinkling `try/catch` across tests.

Also update any helper that relied on exceptions (e.g., `depositToSinglePoolIxes`) to check for `null` instead.

---

## 2) Emissions tests: bankrun clock is FROZEN - `waitForTimeToPass` does NOT work

### Symptom
Emissions-related tests fail under bankrun because `dt = now - last_update = 0`, so no emissions accrue.

### Root Cause: Bankrun Clock is Decoupled from Wall Time

**CRITICAL**: Bankrun's Clock sysvar is **FROZEN** and does NOT automatically sync with wall time.

```
JavaScript wall time:  T=0 -----> T=1.2s -----> T=2.4s
Bankrun Clock sysvar:  T=1000     T=1000        T=1000  (FROZEN!)
```

When your program calls `Clock::get()?.unix_timestamp`, it reads from the **frozen** sysvar, not wall time.

### Why `waitForTimeToPass()` Does NOT Work

```ts
// THIS DOES NOT WORK FOR EMISSIONS:
const waitForTimeToPass = async (ms = 1200) =>
  new Promise((resolve) => setTimeout(resolve, ms));
await waitForTimeToPass();
```

This only advances JavaScript's `Date.now()`. The bankrun Clock sysvar remains frozen.

**Emissions calculation:**
```rust
let period = current_timestamp - balance.last_update;  // Both frozen at same value!
let emissions = period * rate * balance;               // period = 0, so emissions = 0
```

### Correct Fix: Use `setClock()` to Advance Blockchain Time

You **must** explicitly update the Clock sysvar:

```ts
import { Clock } from "solana-bankrun";

// Get current clock state
const clock = await banksClient.getClock();

// Advance by desired seconds (e.g., 2 seconds for emissions)
const newUnixTimestamp = clock.unixTimestamp + BigInt(2);

const newClock = new Clock(
  clock.slot + BigInt(1),        // slot
  clock.epochStartTimestamp,     // epochStartTimestamp
  clock.epoch,                   // epoch
  clock.leaderScheduleEpoch,     // leaderScheduleEpoch
  newUnixTimestamp               // unixTimestamp - THIS IS WHAT MATTERS
);

bankrunContext.setClock(newClock);
// NOW emissions will accrue on the next transaction
```

### Trade-off: Tests that Assert Against `Date.now()`

Some tests check that `lastUpdate` matches wall time:

```ts
const now = Math.floor(Date.now() / 1000);
assertBNApproximately(lastUpdate, new BN(now), 2);
```

If you use `setClock()` to jump forward, these assertions will fail because:
- `lastUpdate` = blockchain time (advanced via setClock)
- `Date.now()` = wall time (not advanced)

**Solutions:**
1. Increase tolerance in these assertions (e.g., from 2 to 10)
2. Or isolate emissions tests in their own suite that doesn't assert against `Date.now()`
3. Or update assertions to compare against blockchain time instead of wall time

### Example from s09_emissions.spec.ts

The staked emissions tests correctly use `setClock`:

```ts
it("three hours elapses", async () => {
  const THREE_HOURS_IN_SECONDS = 60 * 60 * 3;
  let clock = await banksClient.getClock();
  const targetUnix = clock.unixTimestamp + BigInt(THREE_HOURS_IN_SECONDS);
  const newClock = new Clock(
    BigInt(clock.slot + slotsToAdvance),
    0n,
    BigInt(epoch),
    0n,
    targetUnix
  );
  bankrunContext.setClock(newClock);
});
```

---

## 3) Raw transaction tests: `sendRawTransaction` / `confirmTransaction` shims

### Symptom
Some tests build a transaction manually, serialize it, and call:

- `connection.sendRawTransaction(rawTx)`
- `connection.confirmTransaction(sig)`

Bankrun connection proxies often don’t implement these methods.

### Fix
In `tests/rootHooks.ts`, add small shims:

- `sendRawTransaction`: deserialize to `Transaction.from(raw)` and forward to `banksClient.sendTransaction(tx)`
- `confirmTransaction`: return `{ value: { err: null } }`

That is sufficient for tests that only need “did it land” semantics.

---

## 4) Staked suite: create validators + SPL single pools inside bankrun

### Symptom
Staked tests crash early with errors like:

- `validators[0] is undefined`

because `validators` was never populated.

In the original local-validator flow, vote accounts + stake pools were created on the validator and then copied. In bankrun-only, you need to create them natively.

### Fix
In `tests/rootHooks.ts` (bankrun-only):

1) Create a vote account (Vote program)
   - `SystemProgram.createAccount(... VoteProgram.space, VoteProgram.programId)`
   - `VoteProgram.initializeAccount(...)`

2) Initialize an SPL single pool (classic)
   - `SinglePoolProgram.initialize(connection, voteAccount, payer, true)`

3) Derive + store pool PDAs:
   - `findPoolAddress`
   - `findPoolMintAddress`
   - `findPoolStakeAuthorityAddress`
   - `findPoolStakeAddress`

4) Push to exported `validators[]`.

### Avoid impacting other suites
Stake pool setup creates extra accounts and can increase test runtime. If you only need this for the staked suite, gate it behind an env var:

- set `RUN_STAKED_TESTS=1` in the `staked-tests` script
- in `rootHooks.ts`, only run validator setup if that env var is set

---

## 5) Quick checklist when a bankrun test fails

- Missing account errors?
  - Ensure `getAccountInfo` AND `getAccountInfoAndContext` return `null` for missing accounts.

- Emissions stuck at zero?
  - **DO NOT use `waitForTimeToPass()` / `setTimeout()`** - this does NOT advance bankrun clock!
  - Use `bankrunContext.setClock()` to explicitly advance the Clock sysvar.

- Oracle staleness after setClock?
  - After advancing the clock, refresh Pyth pull-oracle publish times to match the new timestamp.

- Tests failing with "expected X to be close to Y" after setClock?
  - The test is comparing blockchain time to `Date.now()` wall time.
  - Increase the tolerance, or change the assertion to compare against blockchain time.

- Staked suite failing immediately?
  - Confirm validators are created and `RUN_STAKED_TESTS=1` is set.

- "sendRawTransaction not a function"?
  - Add connection shims in root hooks.

