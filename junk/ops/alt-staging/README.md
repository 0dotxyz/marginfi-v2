# Alt Staging Ops

This folder contains **self-contained** scripts to:

1) Build your program with the **alt-staging** compile tag/features.
2) Deploy/upgrade the program to the **alt staging** cluster.
3) Initialize the on-chain state needed to list assets:
   - init global fee state (once per program deployment)
   - init group/pool (once per group)
   - init a user marginfi account (for manual testing)
4) Add banks (assets) one-by-one from JSON configs (oracle + limits).

These scripts intentionally live under `ops/` so they are easy to delete or move later.

---

## Prereqs

- `solana` CLI installed + on PATH
- `anchor` CLI installed + on PATH
- Node + your repo's root dependencies installed (`pnpm i` / `npm i`)
- A funded **admin keypair** (SOL for rent + fees) and (optionally) a funded user keypair

---

## Setup

1) Create an env file:

```bash
cp ops/alt-staging/.env.example ops/alt-staging/.env
```

2) Fill in at least:

- `ALT_STAGING_RPC_URL`
- `ALT_STAGING_PROGRAM_ID`
- `ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR`
- `ALT_STAGING_ADMIN_KEYPAIR`
- `ALT_STAGING_IDL_PATH`

---

## 1) Build the program for alt staging

```bash
bash ops/alt-staging/scripts/program-build-alt-staging.sh
```

By default this runs:

```bash
anchor build -p marginfi -- --no-default-features --features stagingalt
```

**IMPORTANT**: The `--no-default-features` flag is required because:
- Default feature is `mainnet-beta` (see `programs/marginfi/Cargo.toml`)
- Without it, both `mainnet-beta` AND `stagingalt` would be enabled
- This would compile in the wrong program ID

Valid feature values (see `id-crate/src/lib.rs`):
- `mainnet-beta` → `MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA`
- `staging` → `stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct`
- `stagingalt` → `5UDghkpgW1HfYSrmEj2iAApHShqU44H6PKTAar9LL9bY`
- `devnet` → `neetcne3Ctrrud7vLdt2ypMm21gZHGN2mCmqWaMVcBQ`

To use a different feature, set `ALT_STAGING_BUILD_FEATURES=...` in `ops/alt-staging/.env`.

---

## 2) Deploy / upgrade the program on alt staging

```bash
bash ops/alt-staging/scripts/program-deploy-alt-staging.sh
```

Notes:
- This uses `solana program deploy` directly to avoid Anchor.toml cluster mapping headaches.
- It will auto-detect the `.so` under `target/deploy/*.so` if you don't set `ALT_STAGING_PROGRAM_SO_PATH`.

---

## 3) One-time program state init

### 3a) Init global fee state (once per program id)

If this is a **fresh program deployment**, you must create the global fee state PDA before creating a group.

```bash
npx ts-node ops/alt-staging/scripts/init-global-fee-state.ts --send
```

Defaults are configured via env vars in `ops/alt-staging/.env` (all zero is fine for staging).

### 3b) Init the group/pool

```bash
npx ts-node ops/alt-staging/scripts/init-group.ts --send
```

This prints the new group pubkey. Copy it into your env:

```bash
# ops/alt-staging/.env
ALT_STAGING_GROUP=<printed group pubkey>
```

---

## 4) Init a user account (for local testing)

This creates a marginfi account PDA (no extra keypair needed).

```bash
npx ts-node ops/alt-staging/scripts/init-user.ts --send
```

---

## 5) Add banks (assets) one by one

Create a config file per asset under `ops/alt-staging/configs/banks/`.

Start from:

- `ops/alt-staging/configs/banks/example.json`

Then run:

```bash
npx ts-node ops/alt-staging/scripts/add-bank.ts ops/alt-staging/configs/banks/usdc.json --send
```

The script:
- derives the bank PDA from `{group, mint, seed}`
- auto-detects SPL vs Token-2022 mint
- simulates first (prints logs)
- sends only if you pass `--send`

---

## Did we forget anything?

For a brand-new program deployment, the usual order is:

1. **Deploy/upgrade** program
2. `init-global-fee-state` (once per program)
3. `init-group` (once per group)
4. Add banks (one per asset)
5. `init-user` (only needed for manual testing)

Other things you *might* need depending on what you test next:

- `marginfi_group_configure` (if your client expects non-default group settings)
- test mints / token faucets on alt staging for deposits
- emissions setup (only if you're testing emissions)

