# marginfi v2 CLI (`mfi`)

Production-oriented Rust CLI for interacting with the `marginfi` on-chain program.

## Build

```bash
cargo build -p marginfi-v2-cli
```

Install locally:

```bash
cargo install --path clients/rust/marginfi-cli --locked --force
```

GitHub release builds are published for tagged versions as archives containing the `mfi` binary:

```bash
gh release download mfi-v0.1.8 --pattern 'mfi-*'
```

## Help And Discovery

Use the built-in help as the live source of truth for flags and examples:

```bash
mfi -h
mfi <command> -h
mfi <command> <subcommand> -h
```

Main command groups:

```text
mfi group
mfi bank
mfi profile
mfi account
mfi kamino
mfi drift
mfi juplend
mfi util
```

## Transaction Behavior

- Default behavior is send mode: simulate first, then sign and broadcast on success.
- `--no-send-tx` simulates and prints an unsigned base58 transaction for external signing or multisig workflows.
- `-y` or `--skip-confirmation` skips the interactive confirmation prompt for state-changing commands.
- Compute budget instructions are only added when `--compute-unit-price` and/or `--compute-unit-limit` are passed.
- `--json` is useful for machine-oriented output where supported.

## Profiles

Typical setup:

```bash
mfi profile create \
  --name mainnet \
  --cluster mainnet \
  --keypair-path ~/.config/solana/id.json \
  --rpc-url https://api.mainnet-beta.solana.com

mfi profile set mainnet
```

Use a different saved profile for one command only:

```bash
mfi --profile staging bank get <BANK_PUBKEY>
```

Profile defaults matter for many commands:

- `mfi group get` uses the active profile group when omitted.
- `mfi group propagate-fee` can use the active profile group when omitted.
- `mfi account get` and some account-management flows can use the active profile account.
- `mfi util show-oracle-ages` defaults to the active profile group before falling back to the hardcoded mainnet group.

## Global Flags

| Flag | Description |
|------|-------------|
| `--profile <NAME>` | Use a saved profile for this command only |
| `--no-send-tx` | Output unsigned base58 instead of signing and broadcasting |
| `-y`, `--skip-confirmation` | Skip interactive confirmation prompts |
| `--compute-unit-price <u64>` | Priority fee in micro-lamports |
| `--compute-unit-limit <u32>` | Compute unit limit override |
| `-l`, `--lookup-table <PUBKEY>` | Address lookup table (repeatable) |
| `--json` | JSON output mode |

## Config Files

Complex workflows accept `--config <path>`. Use `--config-example` to print a template.

Examples:

```bash
mfi bank add --config-example
mfi bank add --config ./configs/bank/add/config.json.example
mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example
mfi group create --config ./configs/group/create/config.json.example
mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example
mfi drift withdraw --config ./configs/drift/withdraw/config.json.example
mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example
```

Config templates live under `clients/rust/marginfi-cli/configs/`.
See [configs/README.md](/Users/femi0x/Projects/marginfi-v2/clients/rust/marginfi-cli/configs/README.md) for the layout.

## Input Minimization Rules

The CLI now derives several deterministic accounts so JSON configs and direct inputs can stay minimal:

- Standard bank creation: `group` defaults to the active profile group, and `seed` can be omitted to auto-select the next free bank seed.
- Staked bank creation: `group` defaults to the active profile group, and `seed` can be omitted.
- `mfi juplend add-bank`: provide `mint` or `juplend_lending`; the CLI derives the other plus `f_token_mint`.
- `mfi kamino harvest-reward`: provide only the roots `bank_pk`, `reward_index`, `global_config`, `reward_mint`, and optional `scope_prices`; the CLI derives user and farm reward accounts.
- `mfi drift withdraw`: reward oracle and reward mint are derived from each reward spot market, so configs only need the reward spot market pubkeys.
- `mfi group propagate-fee` and `mfi util show-oracle-ages` can derive the target group from the active profile.

## Command Reference

### `profile`

| Command | Purpose |
|--------|---------|
| `mfi profile create` | Create a new CLI profile with cluster, keypair, RPC URL, and optional defaults |
| `mfi profile show [NAME]` | Show the active profile or a named profile |
| `mfi profile list` | List saved profiles |
| `mfi profile set <NAME>` | Switch the active profile |
| `mfi profile update <NAME>` | Update profile settings such as RPC URL, keypair, group, or account |
| `mfi profile delete <NAME>` | Delete a saved profile |

### `group`

| Command | Purpose |
|--------|---------|
| `mfi group get [GROUP_PUBKEY]` | Show one group and its banks |
| `mfi group get-all` | List all groups |
| `mfi group create` | Create a group, optionally updating the profile group with `--override` |
| `mfi group update` | Update group admins and e-mode caps |
| `mfi group handle-bankruptcy <ACCOUNT...>` | Settle bad debt for one or more accounts |
| `mfi group update-lookup-table` | Extend the group address lookup table |
| `mfi group check-lookup-table` | Inspect lookup table status |
| `mfi group init-fee-state` | Create the shared fee-state account |
| `mfi group edit-fee-state` | Edit fee-state parameters |
| `mfi group config-group-fee` | Enable or disable program fee collection |
| `mfi group propagate-fee` | Push the shared fee-state to a group |
| `mfi group panic-pause` | Emergency pause all group activity |
| `mfi group panic-unpause` | Admin unpause |
| `mfi group panic-unpause-permissionless` | Permissionless unpause after timeout |
| `mfi group init-staked-settings` | Create shared settings for staked collateral banks |
| `mfi group edit-staked-settings` | Update shared staked collateral settings |
| `mfi group propagate-staked-settings <BANK_PUBKEY>` | Push shared staked settings to one bank |
| `mfi group configure-rate-limits` | Set hourly and daily group outflow caps |
| `mfi group configure-deleverage-limit` | Set the daily deleverage withdrawal cap |

### `bank`

| Command | Purpose |
|--------|---------|
| `mfi bank add` | Create a standard bank |
| `mfi bank add-staked` | Create a staked collateral bank |
| `mfi bank clone` | Clone a source bank into a target group with a chosen seed |
| `mfi bank get <BANK_PUBKEY>` | Show one bank |
| `mfi bank get-all [GROUP_PUBKEY]` | List banks in a group |
| `mfi bank update <BANK_PUBKEY>` | Full config update for an existing bank |
| `mfi bank configure-interest-only <BANK_PUBKEY>` | Update only the interest-rate curve and fee APRs |
| `mfi bank configure-limits-only <BANK_PUBKEY>` | Update only deposit, borrow, and init limits |
| `mfi bank update-oracle <BANK_PUBKEY>` | Change oracle type and oracle account |
| `mfi bank force-tokenless-repay-complete <BANK_PUBKEY>` | Complete the tokenless repay workflow |
| `mfi bank inspect-price-oracle <BANK_PUBKEY>` | Show current oracle state for a bank |
| `mfi bank collect-fees <BANK_PUBKEY>` | Collect accrued fees into the fee vault |
| `mfi bank withdraw-fees <BANK_PUBKEY> <AMOUNT>` | Withdraw collected fees |
| `mfi bank withdraw-insurance <BANK_PUBKEY> <AMOUNT>` | Withdraw insurance funds |
| `mfi bank close <BANK_PUBKEY>` | Close an empty bank |
| `mfi bank accrue-interest <BANK_PUBKEY>` | Trigger interest accrual |
| `mfi bank set-fixed-price <BANK_PUBKEY>` | Override a bank with a fixed price |
| `mfi bank configure-emode <BANK_PUBKEY>` | Set the bank e-mode tag |
| `mfi bank clone-emode` | Copy e-mode settings between banks |
| `mfi bank pulse-price-cache <BANK_PUBKEY>` | Refresh cached price data |
| `mfi bank configure-rate-limits <BANK_PUBKEY>` | Set hourly and daily bank outflow caps |
| `mfi bank withdraw-fees-permissionless <BANK_PUBKEY>` | Permissionless fee withdrawal |
| `mfi bank update-fees-destination <BANK_PUBKEY>` | Change the fee destination |
| `mfi bank init-metadata <BANK_PUBKEY>` | Create the on-chain metadata account |
| `mfi bank write-metadata <BANK_PUBKEY>` | Initialize if needed, then write ticker and description metadata |
| `mfi bank sync-metadata` | Pull metadata from a source URL and write it on-chain |

### `account`

| Command | Purpose |
|--------|---------|
| `mfi account list` | List marginfi accounts for the active authority |
| `mfi account use <ACCOUNT_PUBKEY>` | Set the default account on the current profile |
| `mfi account get [ACCOUNT_PUBKEY]` | Show one account and its balances |
| `mfi account create` | Create a new account |
| `mfi account close` | Close the default account |
| `mfi account create-pda <INDEX>` | Create a PDA-based account |
| `mfi account deposit <BANK_PUBKEY> <UI_AMOUNT>` | Deposit tokens into a bank |
| `mfi account withdraw <BANK_PUBKEY> <UI_AMOUNT>` | Withdraw tokens from a bank |
| `mfi account borrow <BANK_PUBKEY> <UI_AMOUNT>` | Borrow from a bank |
| `mfi account repay <BANK_PUBKEY> <UI_AMOUNT>` | Repay borrowed tokens |
| `mfi account close-balance <BANK_PUBKEY>` | Close a zero-balance position |
| `mfi account transfer <NEW_AUTHORITY_PUBKEY>` | Transfer account authority |
| `mfi account liquidate` | Liquidate an undercollateralized account |
| `mfi account init-liq-record` | Initialize the liquidation record PDA |
| `mfi account liquidate-receivership` | Run the receivership liquidation flow |
| `mfi account place-order` | Place a stop-loss or take-profit order |
| `mfi account close-order <ORDER_PUBKEY>` | Close an order account |
| `mfi account keeper-close-order` | Keeper close an order |
| `mfi account execute-order-keeper` | Execute a keeper order with optional extra instructions |
| `mfi account set-keeper-close-flags` | Set or clear keeper close flags |
| `mfi account set-freeze <ACCOUNT_PUBKEY>` | Freeze or unfreeze an account |
| `mfi account pulse-health [ACCOUNT_PUBKEY]` | Refresh and print account health |

### `kamino`

Use the `kamino` command group for Kamino integration banks and reserve interactions:

| Command | Purpose |
|--------|---------|
| `mfi kamino add-bank` | Create a Kamino integration bank |
| `mfi kamino init-obligation` | Initialize the Kamino obligation used by the bank |
| `mfi kamino deposit` | Deposit through marginfi into Kamino |
| `mfi kamino withdraw` | Withdraw through marginfi from Kamino |
| `mfi kamino harvest-reward` | Harvest Kamino farm rewards |

Important note:

- `kamino harvest-reward` now expects only the root fields. Do not manually provide derived user-state, farm-state, or reward-vault accounts in config.

### `drift`

Use the `drift` command group for Drift integration banks and spot market interactions:

| Command | Purpose |
|--------|---------|
| `mfi drift add-bank` | Create a Drift integration bank |
| `mfi drift init-user` | Initialize the Drift user used by the bank |
| `mfi drift deposit` | Deposit through marginfi into Drift |
| `mfi drift withdraw` | Withdraw through marginfi from Drift |
| `mfi drift harvest-reward` | Harvest Drift rewards |

Important note:

- `drift withdraw` only needs reward spot market pubkeys for extra reward handling. Reward mint and oracle are derived from the spot market state.

### `juplend`

Use the `juplend` command group for JupLend integration banks and positions:

| Command | Purpose |
|--------|---------|
| `mfi juplend add-bank` | Create a JupLend integration bank |
| `mfi juplend init-position <BANK_PUBKEY> --amount <NATIVE_AMOUNT>` | Initialize the JupLend position PDA |
| `mfi juplend deposit <BANK_PUBKEY> <UI_AMOUNT>` | Deposit through marginfi into JupLend |
| `mfi juplend withdraw <BANK_PUBKEY> <UI_AMOUNT>` | Withdraw through marginfi from JupLend |

Important notes:

- For JupLend bank creation, the CLI accepts `mint` or `juplend_lending` and derives the other.
- `f_token_mint` is derived and should not be provided manually.
- Supported oracle setups are `juplendPythPull` and `juplendSwitchboardPull`.

### `util`

| Command | Purpose |
|--------|---------|
| `mfi util inspect-size` | Print sizes of key on-chain account types |
| `mfi util make-test-i80f48` | Generate test vectors for fixed-point numbers |
| `mfi util show-oracle-ages` | Inspect oracle ages for every bank in a group |
| `mfi util inspect-pyth-push-oracle-feed <PUBKEY>` | Inspect a Pyth push feed account |
| `mfi util find-pyth-push <FEED_ID_HEX>` | Search for Pyth push oracle accounts by feed ID |
| `mfi util inspect-swb-pull-feed <PUBKEY>` | Inspect a Switchboard pull feed account |

`find-pyth-push` keeps `find-pyth-pull` as an alias.

## Common Examples

```bash
mfi group get
mfi group create --config ./clients/rust/marginfi-cli/configs/group/create/config.json.example
mfi bank add --config ./clients/rust/marginfi-cli/configs/bank/add/config.json.example
mfi account deposit <BANK_PUBKEY> 10
mfi kamino harvest-reward --config ./clients/rust/marginfi-cli/configs/kamino/harvest-reward/config.json.example
mfi drift withdraw --config ./clients/rust/marginfi-cli/configs/drift/withdraw/config.json.example
mfi juplend add-bank --config ./clients/rust/marginfi-cli/configs/juplend/add-bank/config.json.example
```
