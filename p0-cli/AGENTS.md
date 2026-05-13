# P0 CLI Agent Guide

This file is for AI agents or automation driving the `p0` binary. The CLI is user-facing only: there are no admin-only group/bank-creation/governance commands here.

## Scope

- Root: `p0-cli`
- Binary: `p0`
- Use built-in help for exact flags and examples:
  - `p0 -h`
  - `p0 <command> -h`
  - `p0 <command> <subcommand> -h`

## Core operating rules

1. Prefer the active profile for cluster, signer, group, and account defaults.
2. Use `--config` for integration flows that have many fields (Kamino, Drift). The CLI accepts both `--config` and direct flags.
3. Use `--config-example` before constructing a new JSON file.
4. Read current on-chain state before any state-changing command.
5. Treat bank pubkeys as canonical identifiers. There is no symbol resolver.

## Transaction and output rules

- Default behavior is send mode.
- `--no-send-tx` simulates and prints unsigned base58 for multisig or offline signing.
- `-y` skips confirmation prompts; only use after intent is already validated.
- Prefer `--json` when the downstream consumer is another program or agent.

## Profile rules

- Active profile lives in `~/.config/p0-cli/config.json`.
- One-shot override: `p0 --profile <NAME> ...`
- Persistent switch: `p0 profile set <NAME>`
- Profile-derived defaults:
  - `p0 group get` uses the active profile group.
  - `p0 group propagate-fee` uses the active profile group.
  - `p0 account get`, `p0 account close`, and related account flows use the active profile account.
  - `p0 util show-oracle-ages` uses the active profile group before its mainnet fallback.

## Command selection by intent

Integration deposits / withdrawals always go through the integration-specific commands:

- Standard bank deposit/withdraw: `p0 account deposit|withdraw`
- Kamino bank deposit/withdraw: `p0 kamino deposit|withdraw`
- Drift bank deposit/withdraw: `p0 drift deposit|withdraw`
- JupLend bank deposit/withdraw: `p0 juplend deposit|withdraw`

Do not use:

- `p0 account deposit` for an integration bank (Kamino, Drift, JupLend) — it will fail at the asset-tag check.
- an integration command for a standard bank.

## Command inventory

### `profile`

- `p0 profile create`
- `p0 profile show [NAME]`
- `p0 profile list`
- `p0 profile set <NAME>`
- `p0 profile update <NAME>`
- `p0 profile delete <NAME>`

### `group` (read + permissionless only)

- `p0 group get [GROUP_PUBKEY]`
- `p0 group get-all`
- `p0 group propagate-fee`
- `p0 group propagate-staked-settings <BANK_PUBKEY>`
- `p0 group panic-unpause-permissionless`

### `bank` (read + permissionless only)

- `p0 bank get <BANK_PUBKEY>`
- `p0 bank get-all [GROUP_PUBKEY]`
- `p0 bank inspect-price-oracle <BANK_PUBKEY>`
- `p0 bank collect-fees <BANK_PUBKEY>`
- `p0 bank accrue-interest <BANK_PUBKEY>`
- `p0 bank pulse-price-cache <BANK_PUBKEY>`
- `p0 bank withdraw-fees-permissionless <BANK_PUBKEY>`
- `p0 bank init-metadata <BANK_PUBKEY>`
- `p0 bank dump-metadata`

### `account`

- `p0 account list`
- `p0 account use <ACCOUNT_PUBKEY>`
- `p0 account get [ACCOUNT_PUBKEY]`
- `p0 account create`
- `p0 account create-pda <INDEX>`
- `p0 account close`
- `p0 account deposit <BANK_PUBKEY> <UI_AMOUNT>`
- `p0 account withdraw <BANK_PUBKEY> <UI_AMOUNT>`
- `p0 account borrow <BANK_PUBKEY> <UI_AMOUNT>`
- `p0 account repay <BANK_PUBKEY> <UI_AMOUNT>`
- `p0 account close-balance <BANK_PUBKEY>`
- `p0 account transfer <NEW_AUTHORITY_PUBKEY>`
- `p0 account liquidate`
- `p0 account init-liq-record`
- `p0 account close-liq-record`
- `p0 account liquidate-receivership`
- `p0 account place-order`
- `p0 account close-order <ORDER_PUBKEY>`
- `p0 account keeper-close-order`
- `p0 account execute-order-keeper`
- `p0 account set-keeper-close-flags`
- `p0 account pulse-health [ACCOUNT_PUBKEY]`

### `kamino`

- `p0 kamino init-obligation`
- `p0 kamino deposit`
- `p0 kamino withdraw`
- `p0 kamino harvest-reward`

### `drift`

- `p0 drift init-user`
- `p0 drift deposit`
- `p0 drift withdraw`
- `p0 drift harvest-reward`

### `juplend`

- `p0 juplend init-position <BANK_PUBKEY> --amount <NATIVE_AMOUNT>`
- `p0 juplend deposit <BANK_PUBKEY> <UI_AMOUNT>`
- `p0 juplend withdraw <BANK_PUBKEY> <UI_AMOUNT>`

### `util`

- `p0 util inspect-size`
- `p0 util make-test-i80f48`
- `p0 util show-oracle-ages`
- `p0 util inspect-pyth-push-oracle-feed <PUBKEY>`
- `p0 util find-pyth-push <FEED_ID_HEX>`
- `p0 util inspect-swb-pull-feed <PUBKEY>`

## Recommended workflow

1. Select the intended profile with `p0 profile show` or `p0 --profile <NAME> ...`.
2. Read target state first:
   - `p0 group get`
   - `p0 bank get <BANK_PUBKEY>`
   - `p0 account get`
3. For integration flows, print the template with `--config-example` if a config is being constructed.
4. For multi-sig or offline signing, use `--no-send-tx` first.
5. Only use `-y` after the target accounts and profile are verified.

## High-risk commands

Treat these as explicit-intent operations:

- `p0 account close` / `p0 account close-balance`
- `p0 account transfer`
- `p0 account liquidate*`
- `p0 group panic-unpause-permissionless`

## Retry guidance

Usually safe to retry:

- read-only `get*` / `inspect-*` / `dump-metadata`
- `p0 bank accrue-interest`
- `p0 bank pulse-price-cache`
- `p0 bank withdraw-fees-permissionless`
- `p0 group propagate-fee`

Retry only after checking what already succeeded:

- `p0 account create`
- `p0 account create-pda`
- `p0 kamino init-obligation`
- `p0 drift init-user`
- `p0 juplend init-position`
- receivership liquidation flows

Do not blindly retry:

- close flows
- authority transfer flows

## Current limitations

- Bank lookup is pubkey-first.
- Some output remains optimized for humans rather than strict machine schemas.
- Built-in help remains the authoritative source for exact flag spelling.
