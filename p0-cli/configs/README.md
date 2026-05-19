# Config Layout

This directory is organized by command group and workflow.

Each workflow folder contains a `config.json.example` template. Copy that file to a real `.json` name for your environment instead of editing the shared example in place.

Examples:

```bash
cp configs/bank/add/config.json.example configs/bank/add/mainnet-usdc.json
cp configs/kamino/add-bank/config.json.example configs/kamino/add-bank/staging-jlp.json
cp configs/group/update/config.json.example configs/group/update/mainnet-admin-rotation.json
```

Current layout:

- `bank/add/` - standard bank creation
- `bank/add-staked/` - staked collateral bank creation
- `bank/update/` - bank config updates
- `group/fee-state/` - fee state init/edit
- `group/edit-staked-settings/` - partial staked collateral settings updates
- `group/staked-settings/` - staked collateral settings
- `group/update/` - group admin updates
- `kamino/add-bank/`, `kamino/init-obligation/`, `kamino/deposit/`, `kamino/withdraw/`, `kamino/harvest-reward/`
- `drift/add-bank/`, `drift/init-user/`, `drift/deposit/`, `drift/withdraw/`, `drift/harvest-reward/`
- `juplend/add-bank/`

Important note for `bank/add-staked/`:

- That config is intentionally small because the on-chain permissionless staked-bank flow does not take a full per-bank risk config payload.
- The bank's risk/oracle defaults come from the group's staked settings account.
- If you need to change those values, update `configs/group/staked-settings/config.json.example` and apply that workflow first.

Important note for integration `add-bank/` templates:

- Those templates now include the full accepted config shape.
- Some fields are still nullable because the CLI can derive them from the integration root account:
  - Kamino: `mint`, `kamino_market`
  - Drift: `mint`, `oracle`, `drift_oracle`
  - JupLend: `juplend_lending` from `mint`, or `mint` from `juplend_lending`

Important note for integration user-operation templates:

- User-operation templates now prefer the minimal root inputs for each workflow.
- `kamino/harvest-reward/` examples now show only the external farm inputs: `reward_index`, `global_config`, and `reward_mint`. The CLI derives the bank obligation, farm state, vault authority, reward vaults, and reward ATA.
- `drift/withdraw/` examples now show only reward spot markets. The CLI derives each reward asset's oracle and mint from `drift_reward_spot_market` and/or `drift_reward_spot_market_2`.
- `juplend/add-bank/` examples now show the mint-first flow. The CLI can also accept `juplend_lending` instead and derive the mint from it.
