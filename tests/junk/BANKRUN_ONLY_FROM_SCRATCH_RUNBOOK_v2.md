# Bankrun-only test migration runbook (from scratch, minimal-diff, no regressions) — v2

This runbook is meant to be handed to an implementation agent **together with the original repo**. The goal is to migrate **all TypeScript tests** to run **bankrun-only** (no `solana-test-validator`) with the **least amount of code churn** and **no regressions** in test meaning.

It incorporates lessons learned from the prior migration attempt and the “Kamino change log” doc (direct IDL imports, genesis fixture loading, IDL address injection, etc.).

---

## Non-negotiables (read first)

### What you are allowed to change
- `Anchor.toml` scripts / `package.json` scripts (only to change how tests are launched).
- `tests/rootHooks.ts` (bootstrap, provider setup, fixture loading, program instantiation).
- `tests/utils/**` (helpers: bankrun tx sending, fixture loading, time/warp helpers).
- Small, mechanical changes inside spec files **only** to:
  - swap “send tx” implementation,
  - swap “get slot/time” implementation,
  - remove `anchor.workspace` usage.

### What you are NOT allowed to change
- **No test logic changes.** Do not change protocol parameters, accounts, oracle configs, weights, or instruction ordering.
- **No assertion changes** (except if an assertion was already broken in the baseline).
- **No “fixes” that skip tests**, comment out describes, reduce coverage, or loosen checks.
- **No mixing networks** (do not start local validator and then “copy accounts into bankrun”).
- **No large refactors** (do not “modernize” the code while migrating).

If a test fails after migration, assume the harness is wrong until proven otherwise.

---

## Top 12 cliff notes (do these and you avoid 90% of failures)

1. **Do not run `anchor test`.** It spawns a local validator. Use `anchor run <script>` for bankrun-only.
2. Start bankrun **first** in `rootHooks.ts` via `startAnchor(...)`. Everything else happens *after*.
3. Load all previously-validator-injected accounts via **bankrun genesis accounts** (pass them to `startAnchor`).
4. Load all external programs (Kamino, Drift, Solend, etc.) as **extra programs** in `startAnchor`.
5. **Never call `bankrunContext.setAccount()` after the test runtime begins** if you will later call `warpToSlot/warpToEpoch` (known crash class).
6. Use **only one** “send transaction” helper and make everyone use it.
7. Always set:
   - `tx.feePayer = ctx.payer.publicKey`
   - `tx.recentBlockhash = (await banksClient.getLatestBlockhash())[0]`
8. **Filter signers** to only required signers for the tx (prevents “unknown signer” regressions).
9. For Anchor **v0.30+ / v0.31+**, Program construction expects the program id in the IDL’s `address` field. **Inject it** into the IDL JSON before creating a `Program`.
10. Do not rely on `anchor.workspace` (it won’t be populated reliably under `anchor run`). Import IDLs directly.
11. LUTs require a **real slot advance** after `extendLookupTable` before being usable.
12. Any warp-heavy suite (staking, LUT activation) should be runnable as a **separate mocha invocation** (fresh bankrun context) to avoid warp crashes in huge account DBs.

---

## Phase 0 — Capture a baseline “truth set”

Before touching anything, capture how the repo behaves today (even if it’s slow):

1. Run the original suite exactly once in the original way.
2. Save:
   - list of spec files,
   - pass/fail counts,
   - failing test names + full logs (if any),
   - the exact command used.
3. Save `git status` and `git diff` must be empty before starting.

This baseline is the regression oracle.

---

## Phase 1 — Make tests runnable without a local validator

### 1.1 Add bankrun-only scripts in `Anchor.toml`

Add **new** scripts. Do not delete existing scripts yet.

Example (adapt globs to your repo):

```toml
[scripts]
# Bankrun-only: does NOT start solana-test-validator
bankrun-all = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/*.spec.ts --exit --require tests/rootHooks.ts"

# Helpful subsets (no more commenting/uncommenting):
bankrun-kamino  = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/k*.spec.ts --exit --require tests/rootHooks.ts"
bankrun-drift   = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/d*.spec.ts --exit --require tests/rootHooks.ts"
bankrun-solend  = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/sl*.spec.ts --exit --require tests/rootHooks.ts"
bankrun-staking = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/s*.spec.ts --exit --require tests/rootHooks.ts"

# Optional CI-safe script (includes build):
bankrun-all:build = "anchor build -p marginfi -- --no-default-features && anchor run bankrun-all"
```

Run with:

```bash
anchor build -p marginfi -- --no-default-features
anchor run bankrun-all
```

> Note: It’s OK to keep `anchor build` *out* of the default script for speed, **as long as** the runbook clearly says you must build first. Add a separate `:build` script for CI.

### 1.2 (Optional but recommended) Prevent accidental `anchor test`

If your team keeps typing `anchor test`, you can intentionally make it noisy:

```toml
[scripts]
test = "echo 'Do not use anchor test. Use: anchor run bankrun-all' && exit 1"
```

(Anchor may still start the validator before running scripts depending on CLI behavior, so the real fix is “don’t use anchor test”.)

---

## Phase 2 — Rewrite `tests/rootHooks.ts` to be bankrun-first

The change log doc got the most important rule correct:

> **Start bankrun FIRST.** No validator, no RPC.

### 2.1 Identify what used to be injected by Anchor.toml

Look in the original `Anchor.toml` for:

- `[[test.genesis]]` blocks → external programs to load (`.so` fixtures)
- `[[test.validator.account]]` blocks → JSON account fixtures to preload

You will recreate BOTH of these inside `startAnchor(...)`.

### 2.2 Implement a JSON fixture loader (AddedAccount)

Create a helper exactly like this (paths adapted to repo layout):

```ts
import fs from "fs";
import path from "path";
import { PublicKey } from "@solana/web3.js";
import type { AddedAccount } from "solana-bankrun";

export function loadJsonFixture(filepath: string): AddedAccount {
  const fullPath = path.resolve(__dirname, "..", filepath);
  const json = JSON.parse(fs.readFileSync(fullPath, "utf8"));

  return {
    address: new PublicKey(json.pubkey ?? json.address),
    info: {
      lamports: BigInt(json.account.lamports),
      owner: new PublicKey(json.account.owner),
      executable: Boolean(json.account.executable),
      rentEpoch: BigInt(json.account.rentEpoch ?? 0),
      data: Buffer.from(json.account.data[0], json.account.data[1]),
    },
  };
}
```

### 2.3 Define `extraPrograms` (AddedProgram) for all external `.so` files

Example shape:

```ts
import type { AddedProgram } from "solana-bankrun";

const extraPrograms: AddedProgram[] = [
  { name: "kamino_lending", programId: new PublicKey("KLend...") },
  { name: "kamino_farms", programId: new PublicKey("Farms...") },
  { name: "spl_single_pool", programId: new PublicKey("SVSP...") },
  { name: "drift_v2", programId: new PublicKey("dRift...") },
  { name: "solend", programId: new PublicKey("So1end...") },
];
```

**Naming rule:** bankrun resolves program bytes by name. If you add `{ name: "drift_v2", ... }`, you must have a file like:

- `tests/fixtures/drift_v2.so` (or the repo’s configured fixture directory)

### 2.4 Start bankrun before anything else

Inside `before(async () => { ... })` (or `beforeAll`), do:

```ts
import { startAnchor, BankrunProvider } from "anchor-bankrun";
import path from "path";

bankrunContext = await startAnchor(
  path.resolve(__dirname, ".."),
  extraPrograms,
  genesisAccounts, // array<AddedAccount>
);

bankrunProvider = new BankrunProvider(bankrunContext);
banksClient = bankrunContext.banksClient;
payer = bankrunContext.payer;
```

### 2.5 Set the global Anchor provider (this avoids getProvider() regressions)

A lot of existing test helpers call `getProvider()`.

In bankrun-only, that returns `undefined` **unless you set it**.

Do this once:

```ts
import * as anchor from "@coral-xyz/anchor";
import { AnchorProvider, Wallet } from "@coral-xyz/anchor";

const anchorProvider = new AnchorProvider(
  bankrunProvider.connection,
  new Wallet(bankrunContext.payer),
  {},
);

anchor.setProvider(anchorProvider);
```

Now `anchor.getProvider()` and any helper using it works again, *and* the wallet/mint authority matches `bankrunContext.payer` (important for “attempt to debit an account” style errors).

### 2.6 Create Programs WITHOUT anchor.workspace (direct IDL imports)

Do not use `workspace.Marginfi`.

Instead, import IDLs:

- workspace programs from `../target/idl/*.json` (generated by `anchor build`)
- external programs from `tests/fixtures/*.json` (checked in fixtures)

Example:

```ts
import { Program } from "@coral-xyz/anchor";
import type { Marginfi } from "../target/types/marginfi";
import marginfiIdl from "../target/idl/marginfi.json";
```

### 2.7 Inject `address` into IDL before Program construction (Anchor v0.30+)

Anchor TS removed the Program constructor’s `programId` parameter starting in Anchor 0.30.

That means IDLs used at runtime must include the program id in the IDL’s `address` field.

Create a tiny helper and use it everywhere:

```ts
function withAddress<T extends object>(idl: any, programId: PublicKey): T {
  return { ...idl, address: programId.toBase58() } as T;
}

const marginfi = new Program<Marginfi>(
  withAddress<Marginfi>(marginfiIdl, MARGINFI_PROGRAM_ID),
  anchor.getProvider(), // provider already set to bankrun anchorProvider
);
```

Do the same for Drift/Kamino/Solend IDLs you construct as `Program<T>`.

### 2.8 Create *everything else* via bankrun transactions (not validator, not RPC)

The change log is correct here: **all runtime-created objects must be created by transactions processed by bankrun**:

- mints
- token accounts / ATAs
- global fee states
- banks / groups / users
- oracles (see next section)

If you find code that does this against a real `Connection` (8899), replace it.

---

## Phase 3 — Oracles without regressions (and without bankrun crashes)

### 3.1 Preferred: seed oracle accounts via genesis fixtures

If the original repo already has Pyth accounts as JSON fixtures (e.g. `sol_pyth_oracle.json`, `sol_pyth_price_feed.json`), load them via `genesisAccounts`.

This is the least invasive and fastest.

### 3.2 If oracles must be mutable during tests, update them via instructions

Avoid `bankrunContext.setAccount()` for oracle writes if the suite uses warp later.

Instead:
- write oracle updates via a “mock oracle” program, or
- via the oracle program’s update instruction if available in fixtures.

If you absolutely must write arbitrary bytes:
- do it **before** any warp happens, or
- move warp-heavy suites to a separate run where no mid-run setAccount happens.

---

## Phase 4 — Bankrun transaction helper layer (must be centralized)

Create `tests/utils/bankrunTx.ts` and use it everywhere.

### 4.1 Legacy transactions

```ts
import { Transaction } from "@solana/web3.js";
import type { ProgramTestContext } from "solana-bankrun";
import { PublicKey } from "@solana/web3.js";
import type { Signer } from "@solana/web3.js";

export async function sendBankrunTx(
  ctx: ProgramTestContext,
  tx: Transaction,
  signers: Signer[] = [],
) {
  tx.feePayer ??= ctx.payer.publicKey;

  const [blockhash] = await ctx.banksClient.getLatestBlockhash();
  tx.recentBlockhash = blockhash;

  // Filter signers to ONLY those actually required by the message.
  // This prevents “unknown signer” crashes when code passes extra keypairs.
  const required = new Set(
    tx.compileMessage().accountKeys
      .filter((k, i) => tx.compileMessage().isAccountSigner(i))
      .map((k) => k.toBase58()),
  );

  const filtered = signers.filter((s) => required.has(s.publicKey.toBase58()));

  tx.sign(ctx.payer, ...filtered);
  await ctx.banksClient.processTransaction(tx);
}
```

### 4.2 Versioned transactions (for LUT)

Implement a parallel helper for `VersionedTransaction`, and also signer-filter using the message header.

---

## Phase 5 — LUT tests (Address Lookup Tables) in bankrun

### 5.1 Required behavior: slot warmup after extending LUT

After `extendLookupTable`, you must advance the **bank slot** by at least 1 before using that table in lookups, otherwise Solana will throw an invalid lookup index error.

### 5.2 Use warp carefully (bankrun crash class)

`warpToSlot/warpToEpoch` may crash bankrun in large suites if the accounts DB invariants are broken (common trigger: `setAccount()` mid-run).

Best practice:

- avoid setAccount after start, and
- warp **only +1 slot** for LUT warmup.

If the suite still crashes on warp, use a safer warp helper:

```ts
// Workaround pattern seen in the wild:
// warp 1 slot at a time and process a trivial tx each slot.
export async function warpForwardSafely(
  ctx: ProgramTestContext,
  slots: number,
) {
  for (let i = 0; i < slots; i++) {
    const cur = Number(await ctx.banksClient.getSlot());
    ctx.warpToSlot(BigInt(cur + 1));

    // Process a trivial tx (fee payer only) to help runtime stabilize.
    const tx = new Transaction();
    await sendBankrunTx(ctx, tx, []);
  }
}
```

If warp still crashes, do not “fix” LUT tests by removing LUT usage. Instead, run LUT specs in a fresh bankrun process (`anchor run bankrun-lut`) to keep the accounts DB smaller.

---

## Phase 6 — Staking tests (epoch advancement)

Staking flows often require epoch advancement. In bankrun that generally means `warpToEpoch` or slot warps.

Because warp is crash-prone in huge runs:

- **Run staking specs in their own command**, e.g. `anchor run bankrun-staking`.
- Keep their setup identical (they will still run `rootHooks.ts`, but in a fresh process).

Do not change the staking test logic unless you absolutely must, and only after exhausting harness fixes.

---

## Phase 7 — Mechanical spec-file conversion rules

Once the harness and utils exist, most spec edits should be mechanical:

- Replace `provider.sendAndConfirm(tx, signers)` with `sendBankrunTx(bankrunContext, tx, signers)`
- Replace `connection.getSlot()` with `banksClient.getSlot()`
- Replace any `new Connection("http://127.0.0.1:8899")` usage with bankrun provider/banksClient
- Replace any `anchor.workspace.*` usage with imports from `rootHooks.ts` exports (program instances)

### Compatibility trick (from the change log)

If tests reference both `user.mrgnProgram` and `user.mrgnBankrunProgram`, set both to the same bankrun program instance:

```ts
user.mrgnBankrunProgram = marginfiProgramForUser;
user.mrgnProgram = user.mrgnBankrunProgram;
```

This avoids touching test logic.

---

## Phase 8 — Regression guardrails & “tangent detection”

Before opening a PR / handing off:

### 8.1 Diff hygiene rules
- The number of modified files should be small and expected:
  - `Anchor.toml`
  - `tests/rootHooks.ts`
  - `tests/utils/*`
  - Mechanical edits in specs (mostly 1–3 lines per tx send)
- If you see edits to protocol constants, math, or instruction ordering → **revert them**.

### 8.2 Run suite subsets to bisect quickly
- `anchor run bankrun-kamino`
- `anchor run bankrun-drift`
- `anchor run bankrun-solend`
- `anchor run bankrun-staking`

If a subset fails:
- first check provider/signers/IDL address,
- then check token mint authority and funding,
- then check LUT warmup and time/clock.

### 8.3 Common regression patterns (and the correct response)
- **“Lamports mismatch”**: you’re mixing worlds (validator + bankrun) or mutating accounts incorrectly. Fix harness, don’t change tests.
- **“Attempt to debit an account but found no record of a prior credit”**: usually wrong mint authority or missing funding tx. Ensure payer created mint and payer is the mint authority used.
- **“Invalid public key input” when constructing Program**: IDL missing `address`. Inject it.
- **“Unknown signer” / signature verification**: signer-filtering missing in `sendBankrunTx`.
- **LUT invalid index**: slot warmup missing; advance bank slot by 1.

---

## Appendix A — How to derive extraPrograms + genesisAccounts from the old repo

1. Open `Anchor.toml`:
   - every `[[test.genesis]]` block becomes an `AddedProgram` in `extraPrograms`
   - every `[[test.validator.account]]` becomes a JSON fixture included in `genesisAccounts`
2. Ensure `.so` fixtures exist with matching names.
3. Ensure JSON fixtures follow the `solana account --output json` shape:
   - `pubkey`
   - `account.lamports`
   - `account.owner`
   - `account.data` as `[data, encoding]`

---

## Appendix B — Workspace access under `anchor run`

If a test harness depends on `anchor.workspace`, it will often fail under `anchor run` because the Anchor CLI test harness is not bootstrapping it the same way it does for `anchor test`.

**The robust solution** (and the one this runbook uses) is:

- avoid `anchor.workspace`,
- import IDLs directly,
- construct `Program` objects explicitly,
- export them from `rootHooks.ts`.

Setting `anchor.setProvider(...)` is still useful to preserve `getProvider()` compatibility.

---

## “Done” checklist

You are done when:

- ✅ `anchor run bankrun-all` passes (or, if warp instability forces it, the documented set of bankrun-only commands pass: `bankrun-all`, `bankrun-staking`, etc.)
- ✅ No tests were skipped or loosened
- ✅ Most changes are harness/utilities; spec diffs are mechanical
- ✅ No local validator is started anywhere
