# marginfi-cli Agent Guide

This file is for AI agents or automation operating `mfi`.

## Scope

- Root: `clients/rust/marginfi-cli`
- Binary: `mfi`
- Use built-in help for exact flags and examples:
  - `mfi -h`
  - `mfi <command> -h`
  - `mfi <command> <subcommand> -h`

## Core Operating Rules

1. Prefer the active profile for cluster, signer, group, and account defaults.
2. Prefer `--config` for create, update, and integration admin flows with many fields.
3. Use `--config-example` before constructing a new JSON file.
4. Read current on-chain state before destructive, irreversible, or high-risk commands.
5. Treat bank pubkeys as canonical identifiers. Do not assume a symbol resolver exists.
6. Do not infer bank type from mint alone. Choose the workflow from the bank family.

## Transaction And Output Rules

- Default behavior is send mode.
- `--no-send-tx` simulates and prints unsigned base58 for multisig or offline signing flows.
- `-y` skips confirmation prompts and should only be used after intent is already validated.
- Prefer `--json` when the downstream consumer is another program or agent.
- Success in send mode usually prints a transaction signature.
- Success in `--no-send-tx` mode usually prints an unsigned base58 transaction payload.

## Profile Rules

- Active profile lives in `~/.config/mfi-cli/config.json`.
- One-shot profile override: `mfi --profile <NAME> ...`
- Persistent switch: `mfi profile set <NAME>`
- Important profile-derived defaults:
  - `mfi group get` can use the active profile group.
  - `mfi group propagate-fee` can use the active profile group.
  - `mfi account get`, `mfi account close`, and related account flows can use the active profile account.
  - `mfi util show-oracle-ages` uses the active profile group before its mainnet fallback.

## Command Selection By Intent

Choose the bank creation path from the actual bank family:

- Standard bank: `mfi bank add`
- Staked collateral bank: `mfi bank add-staked`
- Kamino integration bank: `mfi kamino add-bank`
- Drift integration bank: `mfi drift add-bank`
- JupLend integration bank: `mfi juplend add-bank`

Do not use:

- `mfi bank add` for staked banks
- `mfi bank add` for Kamino, Drift, or JupLend banks
- an integration command for a standard bank

## Command Inventory

### `profile`

- `mfi profile create`
- `mfi profile show [NAME]`
- `mfi profile list`
- `mfi profile set <NAME>`
- `mfi profile update <NAME>`
- `mfi profile delete <NAME>`

### `group`

- `mfi group get [GROUP_PUBKEY]`
- `mfi group get-all`
- `mfi group create`
- `mfi group update`
- `mfi group handle-bankruptcy <ACCOUNT...>`
- `mfi group update-lookup-table`
- `mfi group check-lookup-table`
- `mfi group init-fee-state`
- `mfi group edit-fee-state`
- `mfi group config-group-fee`
- `mfi group propagate-fee`
- `mfi group panic-pause`
- `mfi group panic-unpause`
- `mfi group panic-unpause-permissionless`
- `mfi group init-staked-settings`
- `mfi group edit-staked-settings`
- `mfi group propagate-staked-settings <BANK_PUBKEY>`
- `mfi group configure-rate-limits`
- `mfi group configure-deleverage-limit`

### `bank`

- `mfi bank add`
- `mfi bank add-staked`
- `mfi bank clone`
- `mfi bank get <BANK_PUBKEY>`
- `mfi bank get-all [GROUP_PUBKEY]`
- `mfi bank update <BANK_PUBKEY>`
- `mfi bank configure-interest-only <BANK_PUBKEY>`
- `mfi bank configure-limits-only <BANK_PUBKEY>`
- `mfi bank update-oracle <BANK_PUBKEY>`
- `mfi bank force-tokenless-repay-complete <BANK_PUBKEY>`
- `mfi bank inspect-price-oracle <BANK_PUBKEY>`
- `mfi bank collect-fees <BANK_PUBKEY>`
- `mfi bank withdraw-fees <BANK_PUBKEY> <AMOUNT>`
- `mfi bank withdraw-insurance <BANK_PUBKEY> <AMOUNT>`
- `mfi bank close <BANK_PUBKEY>`
- `mfi bank accrue-interest <BANK_PUBKEY>`
- `mfi bank set-fixed-price <BANK_PUBKEY>`
- `mfi bank configure-emode <BANK_PUBKEY>`
- `mfi bank clone-emode`
- `mfi bank migrate-curve <BANK_PUBKEY>`
- `mfi bank pulse-price-cache <BANK_PUBKEY>`
- `mfi bank configure-rate-limits <BANK_PUBKEY>`
- `mfi bank withdraw-fees-permissionless <BANK_PUBKEY>`
- `mfi bank update-fees-destination <BANK_PUBKEY>`
- `mfi bank init-metadata <BANK_PUBKEY>`
- `mfi bank write-metadata <BANK_PUBKEY>`
- `mfi bank sync-metadata`

### `account`

- `mfi account list`
- `mfi account use <ACCOUNT_PUBKEY>`
- `mfi account get [ACCOUNT_PUBKEY]`
- `mfi account create`
- `mfi account close`
- `mfi account create-pda <INDEX>`
- `mfi account deposit <BANK_PUBKEY> <UI_AMOUNT>`
- `mfi account withdraw <BANK_PUBKEY> <UI_AMOUNT>`
- `mfi account borrow <BANK_PUBKEY> <UI_AMOUNT>`
- `mfi account repay <BANK_PUBKEY> <UI_AMOUNT>`
- `mfi account close-balance <BANK_PUBKEY>`
- `mfi account transfer <NEW_AUTHORITY_PUBKEY>`
- `mfi account liquidate`
- `mfi account init-liq-record`
- `mfi account liquidate-receivership`
- `mfi account place-order`
- `mfi account close-order <ORDER_PUBKEY>`
- `mfi account keeper-close-order`
- `mfi account execute-order-keeper`
- `mfi account set-keeper-close-flags`
- `mfi account set-freeze <ACCOUNT_PUBKEY>`
- `mfi account pulse-health [ACCOUNT_PUBKEY]`

### `kamino`

- `mfi kamino add-bank`
- `mfi kamino init-obligation`
- `mfi kamino deposit`
- `mfi kamino withdraw`
- `mfi kamino harvest-reward`

### `drift`

- `mfi drift add-bank`
- `mfi drift init-user`
- `mfi drift deposit`
- `mfi drift withdraw`
- `mfi drift harvest-reward`

### `juplend`

- `mfi juplend add-bank`
- `mfi juplend init-position <BANK_PUBKEY> --amount <NATIVE_AMOUNT>`
- `mfi juplend deposit <BANK_PUBKEY> <UI_AMOUNT>`
- `mfi juplend withdraw <BANK_PUBKEY> <UI_AMOUNT>`

### `util`

- `mfi util inspect-size`
- `mfi util make-test-i80f48`
- `mfi util show-oracle-ages`
- `mfi util inspect-pyth-push-oracle-feed <PUBKEY>`
- `mfi util find-pyth-push <FEED_ID_HEX>`
- `mfi util inspect-swb-pull-feed <PUBKEY>`

## Minimal Input Rules

These are important because older configs may still include redundant fields:

- `mfi bank add`:
  - `group` may be omitted if the active profile already has a group.
  - `seed` may be omitted and the CLI will search for the next free bank seed.
- `mfi bank add-staked`:
  - `group` may be omitted if the active profile already has a group.
  - `seed` may be omitted.
- `mfi group propagate-fee`:
  - `--marginfi-group` is optional when the active profile already has a group.
- `mfi util show-oracle-ages`:
  - `--group` is optional when the active profile already has a group.
- `mfi juplend add-bank`:
  - provide `mint` or `juplend_lending`
  - the CLI derives the other
  - `f_token_mint` is derived and should not be supplied
  - `oracle` is still a required marginfi-side oracle root
  - `oracle_setup` must be `juplendPythPull` or `juplendSwitchboardPull`
- `mfi kamino harvest-reward`:
  - provide `bank_pk`, `reward_index`, `global_config`, `reward_mint`
  - optionally provide `scope_prices`
  - do not provide `user_state`, `farm_state`, `user_reward_ata`, `rewards_vault`, `rewards_treasury_vault`, or `farm_vaults_authority`; the CLI derives them
- `mfi drift withdraw`:
  - reward mint and reward oracle are derived from each reward spot market
  - provide only `drift_reward_spot_market` and optional `drift_reward_spot_market_2`

## Recommended Workflow

1. Select the intended profile with `mfi profile show` or `mfi --profile <NAME> ...`.
2. Read target state first:
   - `mfi group get`
   - `mfi bank get <BANK_PUBKEY>`
   - `mfi account get`
3. For multi-field admin flows, print the template first with `--config-example`.
4. Fill the smallest valid config possible, relying on derived fields and profile defaults.
5. For high-risk or multisig flows, use `--no-send-tx` first.
6. Only use `-y` after the target accounts and profile are verified.

## High-Risk Commands

Treat these as explicit-intent operations:

- `panic-*`
- `close` flows
- account authority transfer flows
- fee and insurance withdrawals
- fixed-price overrides
- bankruptcy handling
- liquidation flows

## Retry Guidance

Usually safe to retry after checking state:

- `mfi bank sync-metadata`
- `mfi bank update`
- `mfi bank configure-interest-only`
- `mfi bank configure-limits-only`
- `mfi group update`
- `mfi group propagate-fee`
- read-only inspection commands

Retry only after checking what already succeeded:

- `mfi bank add`
- `mfi bank add-staked`
- `mfi kamino add-bank`
- `mfi drift add-bank`
- `mfi juplend add-bank`
- `mfi group create`
- `mfi account create`
- `mfi account create-pda`
- receivership liquidation flows

Do not blindly retry:

- create flows with an explicit seed
- `panic-*`
- close flows
- authority transfer flows

## Current Limitations

- Bank lookup is pubkey-first.
- Some output remains optimized for humans rather than strict machine schemas.
- Some integration flows still rely on config JSON for clarity even when the root field count is small.
- Built-in help remains the authoritative source for exact flag spelling and examples.
