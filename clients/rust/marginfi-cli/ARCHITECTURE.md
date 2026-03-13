# CLI Architecture

## Goal

Keep command parsing, instruction construction, and transaction execution separate so the CLI is maintainable across distro builds and protocol upgrades.

## Layers

1. `commands/*`

- Pure CLI surface.
- Parses args, resolves symbols/pubkeys, handles confirmations.
- Delegates to processor layer.

2. `processor/*`

- Builds Anchor instructions/account metas.
- Selects tx sending mode via config (`normal`, `multisig`, `simulate/dry-run`).
- Contains domain-specific actions:
  - `account`
  - `bank_ops`
  - `group_ops`
  - `group` (lookup tables)
  - `fee`
  - `integrations`
  - `oracle`
  - `profile_ops`

3. Core runtime utilities

- `config.rs`: global flags + tx-mode resolution.
- `profile.rs`: profile file loading and runtime config construction.
- `utils.rs`: PDA helpers, account-meta builders, tx sending implementation.
- `output.rs`: table/json output.

## Tx Modes

`Config::get_tx_mode` chooses one of:

- `Normal`: send and confirm.
- `Multisig`: print base58 tx bytes for external signing.
- `DryRun`: simulate and print logs.

Flags influencing mode:

- `--dry-run`
- `--simulate`
- `--multisig-tx`
- profile `multisig` value

## Design Rules

- No panics on user input paths.
- Prefer explicit `config.program_id` over hardcoded IDs.
- Keep command args protocol-version-aware but backward compatible where practical.
- Keep symbol resolution group-aware to avoid ambiguous token routing.
