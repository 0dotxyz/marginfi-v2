# P0 CLI (`p0`)

User-facing Rust CLI for the marginfi v2 program. Read state, manage your accounts, place orders, run integration deposits and withdrawals, and run permissionless ops (interest accrual, fee propagation, liquidations).

> **Scope:** this is the user-facing CLI only. Admin operations — group/bank creation, fee-state init, panic-pause, configuration updates, fee/insurance withdrawals, write-metadata, set-freeze, lookup-table management, etc. — are intentionally not exposed here and live in a separate admin tool.

## Build

```bash
cargo build -p p0-cli
```

Install locally:

```bash
cargo install --path p0-cli --locked --force
```

GitHub release builds are published for tagged versions:

```bash
gh release download p0-v0.1.8 --pattern 'p0-*'
```

## Help

```bash
p0 -h
p0 <command> -h
p0 <command> <subcommand> -h
```

Top-level command groups:

```text
p0 group       # read group state, run permissionless group ops
p0 bank        # read bank state, run permissionless bank ops
p0 profile     # CLI profile management
p0 account     # marginfi account ops (deposit, withdraw, borrow, repay, liquidate, orders)
p0 kamino      # Kamino integration (init-obligation, deposit, withdraw, harvest)
p0 drift       # Drift integration (init-user, deposit, withdraw, harvest)
p0 juplend     # JupLend integration (init-position, deposit, withdraw)
p0 util        # debug and oracle utilities
```

## Transaction behavior

- Default is send mode: simulate, then sign and broadcast.
- `--no-send-tx` simulates and prints an unsigned base58 transaction for external signing.
- `-y` / `--skip-confirmation` skips the interactive prompt for state-changing commands.
- Compute budget ixs are only added when `--compute-unit-price` and/or `--compute-unit-limit` are passed.
- `--json` emits machine-oriented output where supported.

## Profiles

```bash
p0 profile create \
  --name mainnet \
  --cluster mainnet \
  --keypair-path ~/.config/solana/id.json \
  --rpc-url https://api.mainnet-beta.solana.com

p0 profile set mainnet
```

One-shot override:

```bash
p0 --profile staging bank get <BANK_PUBKEY>
```

Profile-derived defaults:

- `p0 group get` uses the active profile group when omitted.
- `p0 group propagate-fee` uses the active profile group when omitted.
- `p0 account get` and most account flows use the active profile account.
- `p0 util show-oracle-ages` uses the active profile group, then falls back to the hardcoded mainnet group.

## Global flags

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Use a saved profile for this command only |
| `--no-send-tx` | Output unsigned base58 instead of signing and broadcasting |
| `-y`, `--skip-confirmation` | Skip interactive confirmation prompts |
| `--compute-unit-price <u64>` | Priority fee in micro-lamports |
| `--compute-unit-limit <u32>` | Compute unit limit override |
| `-l`, `--lookup-table <PUBKEY>` | Address lookup table (repeatable) |
| `--json` | JSON output mode |

## Config files

Integration commands accept `--config <path>`. Use `--config-example` to print a template.

```bash
p0 kamino deposit --config-example
p0 kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example
p0 drift withdraw --config ./configs/drift/withdraw/config.json.example
```

Config templates live under `p0-cli/configs/`.

## Command reference

### `profile`

| Command | Purpose |
|---------|---------|
| `p0 profile create` | Create a CLI profile |
| `p0 profile show [NAME]` | Show the active profile or `<NAME>` |
| `p0 profile list` | List saved profiles |
| `p0 profile set <NAME>` | Switch active profile |
| `p0 profile update <NAME>` | Update profile settings |
| `p0 profile delete <NAME>` | Delete a profile |

### `group`

| Command | Purpose |
|---------|---------|
| `p0 group get [GROUP_PUBKEY]` | Show one group and its banks |
| `p0 group get-all` | List every group |
| `p0 group propagate-fee` | (permissionless) Push the shared fee-state into a group |
| `p0 group propagate-staked-settings <BANK_PUBKEY>` | (permissionless) Push shared staked settings to one bank |
| `p0 group panic-unpause-permissionless` | (permissionless) Unpause after the pause window expires |

### `bank`

| Command | Purpose |
|---------|---------|
| `p0 bank get <BANK_PUBKEY>` | Show one bank |
| `p0 bank get-all [GROUP_PUBKEY]` | List banks in a group |
| `p0 bank inspect-price-oracle <BANK_PUBKEY>` | Show oracle state for a bank |
| `p0 bank collect-fees <BANK_PUBKEY>` | (permissionless) Move accrued fees into the fee vault |
| `p0 bank accrue-interest <BANK_PUBKEY>` | (permissionless) Trigger interest accrual |
| `p0 bank pulse-price-cache <BANK_PUBKEY>` | (permissionless) Refresh cached oracle price |
| `p0 bank withdraw-fees-permissionless <BANK_PUBKEY>` | (permissionless) Send fees to the bank's preset destination |
| `p0 bank init-metadata <BANK_PUBKEY>` | (permissionless) Pay rent to create the bank metadata PDA |
| `p0 bank dump-metadata` | Dump on-chain bank metadata to a local JSON file |

### `account`

| Command | Purpose |
|---------|---------|
| `p0 account list` | List marginfi accounts for the active authority |
| `p0 account use <ACCOUNT_PUBKEY>` | Set the default account on the current profile |
| `p0 account get [ACCOUNT_PUBKEY]` | Show one account and its balances |
| `p0 account create` | Create a new account |
| `p0 account create-pda <INDEX>` | Create a PDA-based account |
| `p0 account close` | Close the default account |
| `p0 account deposit <BANK_PUBKEY> <UI_AMOUNT>` | Deposit |
| `p0 account withdraw <BANK_PUBKEY> <UI_AMOUNT>` | Withdraw |
| `p0 account borrow <BANK_PUBKEY> <UI_AMOUNT>` | Borrow |
| `p0 account repay <BANK_PUBKEY> <UI_AMOUNT>` | Repay |
| `p0 account close-balance <BANK_PUBKEY>` | Close a zero-balance position |
| `p0 account transfer <NEW_AUTHORITY_PUBKEY>` | Transfer account authority |
| `p0 account liquidate` | Liquidate an undercollateralized account |
| `p0 account init-liq-record` | (permissionless) Initialize the liquidation record PDA |
| `p0 account close-liq-record` | (permissionless) Close the liquidation record PDA and return rent to the original payer |
| `p0 account liquidate-receivership` | Run the receivership liquidation flow |
| `p0 account place-order` | Place a stop-loss or take-profit order |
| `p0 account close-order <ORDER_PUBKEY>` | Close an order |
| `p0 account keeper-close-order` | Keeper: close a stale order |
| `p0 account execute-order-keeper` | Keeper: execute an order in one tx |
| `p0 account set-keeper-close-flags` | Clear keeper-close tags on balances |
| `p0 account pulse-health [ACCOUNT_PUBKEY]` | (permissionless) Refresh cached account health |

### `kamino`

| Command | Purpose |
|---------|---------|
| `p0 kamino init-obligation` | (permissionless) Initialize the Kamino obligation for a bank |
| `p0 kamino deposit` | Deposit through marginfi into Kamino |
| `p0 kamino withdraw` | Withdraw through marginfi from Kamino |
| `p0 kamino harvest-reward` | (permissionless) Harvest Kamino farm rewards |

`kamino harvest-reward` derives `user_state`, `farm_state`, `user_reward_ata`, `rewards_vault`, `rewards_treasury_vault`, and `farm_vaults_authority`; do not pass them by hand.

### `drift`

| Command | Purpose |
|---------|---------|
| `p0 drift init-user` | (permissionless) Initialize the Drift user for a bank |
| `p0 drift deposit` | Deposit through marginfi into Drift |
| `p0 drift withdraw` | Withdraw through marginfi from Drift |
| `p0 drift harvest-reward` | (permissionless) Harvest Drift spot-market rewards |

`drift withdraw` derives reward oracle and reward mint from each reward spot market; configs only need `drift_reward_spot_market` and optionally `drift_reward_spot_market_2`.

### `juplend`

| Command | Purpose |
|---------|---------|
| `p0 juplend init-position <BANK_PUBKEY> --amount <NATIVE_AMOUNT>` | (permissionless) Initialize the JupLend position |
| `p0 juplend deposit <BANK_PUBKEY> <UI_AMOUNT>` | Deposit through marginfi into JupLend |
| `p0 juplend withdraw <BANK_PUBKEY> <UI_AMOUNT>` | Withdraw through marginfi from JupLend |

### `util`

| Command | Purpose |
|---------|---------|
| `p0 util inspect-size` | Sizes of core on-chain types |
| `p0 util make-test-i80f48` | Random I80F48 test vectors |
| `p0 util show-oracle-ages` | Oracle ages for every bank in a group |
| `p0 util inspect-pyth-push-oracle-feed <PUBKEY>` | Inspect a Pyth push feed |
| `p0 util find-pyth-push <FEED_ID_HEX>` | Find Pyth push accounts by feed ID |
| `p0 util inspect-swb-pull-feed <PUBKEY>` | Inspect a Switchboard pull feed |

`find-pyth-push` keeps `find-pyth-pull` as an alias.
