# marginfi v2 CLI (`mfi`)

Production-oriented Rust CLI for interacting with the `marginfi` on-chain program. Replaces the legacy TypeScript scripts with a scalable, multisig-first workflow.

## Build

```bash
cargo build -p marginfi-v2-cli
```

Install locally:

```bash
cargo install --path clients/rust/marginfi-cli --locked --force
```

## Architecture

The CLI is split into clear layers:

- `src/commands/*` — clap command definitions + argument parsing
- `src/processor/*` — instruction builders/executors grouped by domain
- `src/config.rs` / `src/profile.rs` — runtime/profile loading and tx mode resolution
- `src/configs.rs` — JSON config file structs for complex instructions
- `src/utils.rs` — shared PDA/account-meta/tx helpers
- `src/output.rs` — human/JSON output formatters

Processor domains:

| Module | Scope |
|--------|-------|
| `account` | User actions (deposit/withdraw/borrow/repay/orders/liquidation) |
| `bank_ops` | Bank admin, oracle, rate-limit, curve, e-mode, metadata |
| `group_ops` | Group lifecycle, bank creation/clone, bankruptcy |
| `group` | Lookup table maintenance |
| `integrations` | Kamino, Drift, JupLend CPI integrations |
| `fee` | Fee-state, panic pause/unpause, staked settings |
| `oracle` | Oracle feed inspection utilities |
| `profile_ops` | Profile CRUD |

## Transaction Behavior

**Default (multisig mode):** Every transaction is simulated first. On success, the unsigned transaction is serialized as base58 and printed to stdout — ready for Squads or other multisig workflows.

**`--send-tx` flag:** After successful simulation, the transaction is signed and broadcast on-chain. Use this for staging/localnet or when operating without a multisig.

If simulation fails, the CLI aborts with program logs — it never silently swallows errors.

## Profiles

```bash
# Create a profile
mfi profile create \
  --name mainnet \
  --cluster mainnet \
  --keypair-path ~/.config/solana/id.json \
  --rpc-url https://api.mainnet-beta.solana.com

# Set active profile
mfi profile set mainnet
```

## Global Flags

| Flag | Description |
|------|-------------|
| `--send-tx` | Sign and broadcast (default: output unsigned base58) |
| `--skip-confirmation` / `-y` | Skip interactive confirmation prompts |
| `--compute-unit-price <u64>` | Priority fee in micro-lamports |
| `--compute-unit-limit <u32>` | Compute unit limit override |
| `--lookup-table <PUBKEY>` | Address lookup table (repeatable) |
| `--legacy-tx` | Force legacy transaction format |
| `--json` | JSON output mode |

## Command Groups

```
mfi group ...         # Group management
mfi bank ...          # Bank configuration
mfi account ...       # Account operations
mfi integration ...   # DeFi integrations (Kamino, Drift, JupLend)
mfi util ...          # Debug/utility commands
mfi profile ...       # Profile management
```

## JSON Config Files

Complex instructions accept `--config <path>` with a JSON file instead of many CLI flags. Use `--config-example` to print an example JSON template.

```bash
# Print example config
mfi group add-bank --config-example

# Use a config file
mfi group add-bank --config ./add-bank.json

# Also supported:
mfi group update --config ./group-update.json
mfi group init-fee-state --config ./fee-state.json
mfi group edit-fee-state --config ./fee-state.json
mfi bank update --config ./bank-config.json
```

## Group Commands

| Command | Description |
|---------|-------------|
| `get [group]` | Display group details |
| `get-all` | List all groups |
| `create` | Create a new group |
| `update` | Update admin roles (`--config` supported) |
| `add-bank` | Add a lending bank (`--config` supported) |
| `clone-bank` | Clone a mainnet bank for staging |
| `handle-bankruptcy` | Handle bankruptcy for accounts |
| `update-lookup-table` | Update address lookup table |
| `check-lookup-table` | Check ALT status |
| `init-fee-state` | Initialize global fee state (`--config` supported) |
| `edit-fee-state` | Edit global fee state (`--config` supported) |
| `config-group-fee` | Enable/disable program fee collection |
| `propagate-fee` | Propagate fee state to a group |
| `panic-pause` | Emergency pause all operations |
| `panic-unpause` | Admin unpause |
| `panic-unpause-permissionless` | Permissionless unpause after timeout |
| `init-staked-settings` | Initialize staked collateral settings |
| `edit-staked-settings` | Edit staked collateral settings |
| `propagate-staked-settings` | Propagate staked settings to a bank |
| `configure-rate-limits` | Group-level outflow rate limits |
| `configure-deleverage-limit` | Daily deleverage withdrawal limit |

## Bank Commands

| Command | Description |
|---------|-------------|
| `get <pubkey>` | Display bank details |
| `get-all [group]` | List all banks in a group |
| `update` | Full bank config update (`--config` supported) |
| `configure-interest-only` | Curve-admin interest rate update |
| `configure-limits-only` | Limit-admin deposit/borrow limits |
| `update-oracle` | Change oracle type and key |
| `force-tokenless-repay-complete` | Mark tokenless repay workflow complete |
| `inspect-price-oracle` | Show oracle price and metadata |
| `collect-fees` | Collect accrued protocol fees |
| `withdraw-fees` | Withdraw from fee vault |
| `withdraw-insurance` | Withdraw from insurance vault |
| `withdraw-fees-permissionless` | Permissionless fee withdrawal |
| `update-fees-destination` | Change fee destination address |
| `close` | Close a bank (must be empty) |
| `accrue-interest` | Trigger interest accrual |
| `set-fixed-price` | Override oracle with a fixed price |
| `configure-emode` | Set e-mode tag |
| `clone-emode` | Copy e-mode settings between banks |
| `migrate-curve` | Migrate legacy curve to 7-point format |
| `pulse-price-cache` | Refresh cached oracle price |
| `configure-rate-limits` | Bank-level outflow limits |
| `init-metadata` | Initialize on-chain metadata |
| `write-metadata` | Write ticker/description to metadata |

## Account Commands

| Command | Description |
|---------|-------------|
| `list` | List all accounts for the authority |
| `use <pubkey>` | Set default account |
| `get [pubkey]` | Display account with balances |
| `create` | Create new account |
| `create-pda` | Create PDA-based account |
| `close` | Close default account |
| `deposit` | Deposit tokens |
| `withdraw` | Withdraw tokens |
| `borrow` | Borrow tokens |
| `repay` | Repay borrowed tokens |
| `close-balance` | Close zero-balance position |
| `transfer` | Transfer account authority |
| `liquidate` | Liquidate an account |
| `liquidate-receivership` | Receivership liquidation bundle |
| `init-liq-record` | Initialize liquidation record PDA |
| `set-freeze` | Freeze/unfreeze account (admin) |
| `pulse-health` | Pulse health check |
| `place-order` | Place stop-loss/take-profit order |
| `close-order` | Close an existing order |
| `keeper-close-order` | Keeper permissionless close |
| `execute-order-keeper` | Keeper execute order in one tx |
| `set-keeper-close-flags` | Set keeper close flags |

## Integration Commands

All JupLend commands auto-derive CPI accounts from the bank — only the bank pubkey is required.

| Command | Description |
|---------|-------------|
| **Kamino** | |
| `kamino-init-obligation` | Initialize Kamino obligation |
| `kamino-deposit` | Deposit into Kamino reserve |
| `kamino-withdraw` | Withdraw from Kamino reserve |
| `kamino-harvest-reward` | Harvest Kamino farm rewards |
| **Drift** | |
| `drift-init-user` | Initialize Drift user account |
| `drift-deposit` | Deposit into Drift |
| `drift-withdraw` | Withdraw from Drift |
| `drift-harvest-reward` | Harvest Drift rewards |
| **JupLend** | |
| `juplend-init-position` | Initialize JupLend position |
| `juplend-deposit` | Deposit into JupLend |
| `juplend-withdraw` | Withdraw from JupLend |

## Unimplemented Program Instructions

The following marginfi program instructions are not yet exposed in the CLI:

- `lending_account_start_flashloan` / `lending_account_end_flashloan` — flash loan operations
- `lending_pool_add_bank_permissionless` — permissionless bank creation
- `lending_pool_configure_bank_oracle` (7-point curve variant)
- `set_account_flag` — account flag management
- `lending_account_authorize_address` / `lending_account_deauthorize_address` — delegated authority
- Emissions instructions (`setup_emissions`, `update_emissions`, `claim_emissions`) — intentionally removed
- Solend integration instructions — intentionally excluded

## Notes

- Banks are specified by pubkey only (no symbol shortcuts).
- Emissions mutation instructions were intentionally removed from the command surface.
- Token-2022 mints are auto-detected — the CLI uses the correct token program for fee/insurance vault operations.
- Multi-oracle setups (e.g., StakedWithPythPush) are handled correctly for bank observation keys.
