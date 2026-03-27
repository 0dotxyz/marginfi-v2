# IDL Distill Scripts Guide

## IDL layout

The repo now keeps two IDL sets:

- `idls/`: canonical macro IDs used by `declare_program!` in Rust mock crates. Don't change this
  folder name, because anchor looks for it explicitly in the macro.
  - `idls/kamino_lending.json` is the minimal Kamino lending IDL.
  - `idls/kamino_farms.json` is the minimal Kamino farms IDL.
- `idls-complete/`: full IDs used by TS tests.
  - `idls-complete/kamino_lending.json` and `idls-complete/kamino_farms.json` stay full.
  - Drift/JupLend files are currently the same in both folders.

## Kamino minimal distillation workflow

Use `distill_kamino_macro_idl.py` to generate minimal Kamino IDLs for macro generation.

### 1. Fetch raw Kamino lending IDL

From `scripts/raw_idl/` (or any folder):

```bash
anchor idl fetch KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD > k_lend_idl_xxx.json --provider.cluster mainnet
```

### 2. Refresh `idls-complete/` first (full IDs)

Make sure the full IDL is in `idls-complete/` before rewriting the minimal one in `idls/`.

Example copy:

```bash
cp scripts/raw_idl/k_lend_idl_xxx.json idls-complete/kamino_lending_raw.json
```

If needed, convert/fetch your final full file and write it to:

```bash
idls-complete/kamino_lending.json
```

Note: Kamino's IDL is in a legacy format with various compatibility issues: use
`distill_kamino_idl.sh` to create a useable version for the TS side.

### 3. Generate minimal canonical IDs in `idls/`

From repo root:

```bash
./scripts/distill_kamino_macro_idl.py \
  --input scripts/raw_idl/k_lend_idl_xxx.json \
  --output idls/kamino_lending.json \
  --profile lending
```

For farms:

```bash
./scripts/distill_kamino_macro_idl.py \
  --input idls-complete/kamino_farms.json \
  --output idls/kamino_farms.json \
  --profile farms
```

### 4. Validate macro compile path

`declare_program!` in `kamino-mocks` should compile directly against `idls/*.json`:

```bash
cargo check -p kamino-mocks
```

### Notes

- The distiller auto-detects raw IDLs (missing instruction discriminators) and runs `anchor idl
  convert` before pruning.
- Output keeps CPI-relevant instructions and required arg types only.
- Anchor reuses the first generated composite type when account layouts are identical, which then
  would cause TS to complaint about duplicate types. This is particularly relevant for Kamino, where
  there is more than one FarmsAccount. We sidestep this annoyance by splitting duplicates like this
  into one type per instruction, e.g. a `DepositFarmsAccounts`.
- TS code should import from `idls-complete/` when full/mainnet-like IDL behavior is required.

## Drift + JupLend minimal distillation workflow

Use `distill_drift_juplend_macro_idl.py` to generate minimal Drift/JupLend IDLs for macro
generation.

These inputs are expected to already be in modern Anchor-IDL JSON format. The script does not run
`anchor idl convert`.

### 1. Distill Drift (minimal canonical output)

```bash
./scripts/distill_drift_juplend_macro_idl.py \
  --input scripts/raw_idl/drift_idl_xxx.json \
  --output idls/drift.json \
  --profile drift
```

### 2. Distill JupLend Earn (minimal canonical output)

```bash
./scripts/distill_drift_juplend_macro_idl.py \
  --input scripts/raw_idl/juplend_earn_xxx.json \
  --output idls/juplend_earn.json \
  --profile juplend
```

### 3. Keep full IDs in `idls-complete/`

TS tests should continue importing full IDs from `idls-complete/`:

- `idls-complete/drift.json`
- `idls-complete/juplend_earn.json`

At the moment, some IDLs may still be identical between `idls/` and `idls-complete/` until they are
fully split by tooling.