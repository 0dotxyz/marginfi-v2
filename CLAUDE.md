## Build Notes

- Toolchain: Rust `1.90.0` (`rust-toolchain.toml`), Anchor CLI `1.0.2`
  (`Anchor.toml`), Agave/Solana CLI `3.1.13`, Node `24.15.0`, Yarn `4.14.1`.
- Quick `cargo check` of the program: `cargo check -p marginfi --no-default-features --features custom-heap`
- Full SBF build for TS tests: `anchor build -p marginfi -- --no-default-features --features custom-heap` (produces `target/deploy/marginfi.so` with the **localnet** program ID and refreshes `target/idl/marginfi.json` + `target/types/marginfi.ts`)
- The TS test runner also needs the mocks IDL: `anchor build -p mocks --ignore-keys` (run once, or after touching `programs/mocks`)
- If `anchor build` fails with `found invalid metadata files for crate juplend_mocks`, delete the stale rlib and rebuild:
  ```sh
  find target/debug/deps -name "libjuplend_mocks*" -delete
  anchor build -p mocks --ignore-keys
  anchor build -p marginfi -- --no-default-features --features custom-heap
  ```

## Test Notes

There are two distinct test stacks. They require **two separate `.so` builds** because each stack expects a different program ID (the test binary's `marginfi::ID` constant must match the ID baked into the `.so`; mismatches surface as `DeclaredProgramIdMismatch` / Anchor error `0x1004`).

### TypeScript / anchor tests (LiteSVM, `tests/*.spec.ts`)

- Expects the **localnet** program ID (`2jGhuVUuy3umdzByFx8sNWUAaf5vaeuDm78RDPEnhrMr`) baked into `target/deploy/marginfi.so`.
- Build with: `anchor build -p marginfi -- --no-default-features --features custom-heap` (Build B above).
- Run a slice: `anchor run basic-tests` (or `kamino-tests` / `drift-tests` / `solend-tests` / `juplend-tests` / `emode-tests` / `pyth-tests` / `limits-tests` / `staked-tests` / `bankruptcy-tests` / `all-tests` — see `Anchor.toml [scripts]`).
- Run a single spec: `RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/16_orders.spec.ts --exit --require tests/rootHooks.ts`
- If a TS test fails in `AccountsResolver.resolve` with *"Unresolved accounts: ..."*, the on-disk IDL is stale — rerun `anchor build -p marginfi` to regenerate `target/idl/marginfi.json` and `target/types/marginfi.ts`.
- **CI-only `BigInt` failure (Linux V8).** `BigInt(someBN.toString())` and BN `.toString()` on multi-word values get miscompiled by V8 on the CI Linux runner, emitting a corrupt string like `"1000000000NaN"` (→ `SyntaxError: Cannot convert 1000000000NaN to a BigInt`). It is **invisible on macOS**, so it only ever surfaces in CI. Convert BNs with `bnToBigIntSafe` / `bnToDecimalStringSafe` from `tests/utils/bn-utils.ts` (they read the BN bytes directly, bypassing base-10 `toString()`); never feed `bn.toString()` into `BigInt()` or compare a BN's `.toString()` in assertions. Smaller (single-word) BNs don't trip it, so passing specs are still latent risks.
- **Test isolation: specs run together in one shared bankrun ecosystem.** A spec that passes alone can fail in `all-tests` (and vice-versa) because globals from `rootHooks` (the `marginfiGroup`, the JupLend protocol singleton, shared bank keypairs) are already initialized by earlier specs. Don't re-`groupInitialize` the shared `marginfiGroup` — use your own `Keypair` like `07_deposit.spec.ts` (`throwawayGroup`) / `jlr07` (`juplendGroup`); make global-singleton setup (e.g. JupLend `InitNewProtocol`) idempotent (skip if the account exists). Always validate a new spec against the **full** suite, not just in isolation.

### Rust integration tests (`programs/marginfi/tests/{admin_actions,user_actions,misc}/*.rs`)

- Expects the **mainnet** program ID (`MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA`) baked into the `.so` it loads from `SBF_OUT_DIR`.
- Build the SBF binaries into a dedicated dir: `./scripts/build-workspace.sh` (uses `CARGO_TARGET_DIR=target/sbf`, outputs to `target/sbf/deploy/`).
- Run the whole package: `./scripts/test-program.sh marginfi` (uses `SBF_OUT_DIR=target/sbf/deploy`, `CARGO_TARGET_DIR=target/host`, `--features=test,test-bpf` with default features kept).
- Run a single test by name: `./scripts/single-test.sh marginfi <test_name>`.
- Equivalent invocation by hand:
  ```sh
  SBF_OUT_DIR="$PWD/target/sbf/deploy" \
  CARGO_TARGET_DIR="$PWD/target/host" \
    cargo nextest run -p marginfi --features=test,test-bpf
  ```
- **Do not use** `docs/test-program-localnet.sh` — it claims to share a single `.so` between both test stacks but produces a `DeclaredProgramIdMismatch` because `test-utils/Cargo.toml:43` pulls in marginfi without `default-features = false`, forcing the mainnet program ID on the test binary regardless of CLI flags.

### Why "tries to compile"

`cargo nextest` always compiles a host-arch test binary that links the marginfi *crate* (to access `marginfi::ID`, struct types, etc.). The first run takes ~5-15 minutes. Subsequent runs reuse the `target/host/` cache as long as features don't change. The SBF `.so` is loaded separately by `solana-program-test` from `SBF_OUT_DIR` — that's not what's being compiled.

### Directory layout

```
target/sbf/deploy/marginfi.so   ← mainnet ID  → Rust integration tests
target/host/                     ← host-arch nextest build cache (must persist between runs)
target/deploy/marginfi.so        ← localnet ID → TS / anchor tests
target/idl/, target/types/       ← regenerated by anchor build, used by TS tests
```

Keep the two trees separate. Don't try to share `target/deploy/marginfi.so` between stacks — the program IDs are different.

### Lib unit tests (no SBF needed)

For the in-source `#[cfg(test)]` modules (e.g., `state::order::tests::*`, `ix_utils::tests::*`, `close_balance_accounting`):

```sh
cargo test -p marginfi --no-default-features --features custom-heap --lib
```

These don't need a `.so` and don't hit either of the two stacks above.

### Pre-PR check

```sh
# 1. Build both targets
./scripts/build-workspace.sh                                          # mainnet ID → target/sbf/deploy
anchor build -p mocks --ignore-keys
anchor build -p marginfi -- --no-default-features --features custom-heap   # localnet ID → target/deploy

# 2. Run both stacks
./scripts/test-program.sh all   # Rust integration
anchor run all-tests               # TS / anchor (add other slices as needed)
```

## TypeScript Error Checking

- To check TypeScript errors, use the MCP IDE diagnostics tool:
  - For a specific file: `mcp__ide__getDiagnostics` with `uri` parameter (e.g., `file:///root/projects/kamino-integration/tests/utils/types.ts`)
  - For all open files: `mcp__ide__getDiagnostics` without parameters
- Do NOT use `npx tsc --noEmit` as it's not configured correctly for this project
