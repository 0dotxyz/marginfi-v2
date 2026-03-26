# Integrator Quickstart Guide

Do you want to build on top of mrgnLendv2? Read on.

## Ecosystem Resources

Be aware of:

- Our TS packages:
  - https://www.npmjs.com/package/@mrgnlabs/marginfi-client-v2
  - https://www.npmjs.com/package/@mrgnlabs/mrgn-common
- Our example scripts: https://github.com/mrgnlabs/mrgn-ts-scripts/tree/master/scripts
- Rust and TS examples of all instructions are available in our test suites, just search this repo
  for the instruction name (or remember to change to camelCase if searching for TS examples)

## Be Aware of Breaking Changes!

* Please add https://github.com/mrgnlabs/marginfi-v2 to your "Watch" list and make sure you are notified when we create a release.
  * Releases (except hotfixes) will stay in "pre-release" for **at least seven days** before we merge them into mainnet
  * Only the last release candidate (rc) will go live on mainnet, e.g. if there is an rc1, rc2, and rc3, then rc3 will go to mainnet.
  * When there is a pre-release pending, the staging program
    (stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct) will be updated to match it, typically on the same
    day the pre-release is published. See
    https://github.com/mrgnlabs/mrgn-ts-scripts/blob/master/scripts/accounts_ref.md for various test
    groups/banks you might use on staging, or create your own group and use the clone_bank
    instruction to quickly copy mainnet banks into staging (Note: clone_bank only works on the
    staging program)
* Contact us on telegram to be added to our "Integrators" list. We will make a best effort to ping
  you at least one week before any program update goes live.

## Migration: Unified Integration Interface

This release intentionally removes the old protocol-specific user entrypoints for wrapped
integrations and replaces them with one shared interface.

### Old -> New instruction mapping

| Old instruction | New instruction |
|----------------|-----------------|
| `kamino_deposit` | `integration_deposit` |
| `kamino_withdraw` | `integration_withdraw` |
| `drift_deposit` | `integration_deposit` |
| `drift_withdraw` | `integration_withdraw` |
| `solend_deposit` | `integration_deposit` |
| `solend_withdraw` | `integration_withdraw` |
| `juplend_deposit` | `integration_deposit` |
| `juplend_withdraw` | `integration_withdraw` |

### What integrators must change

- Stop building protocol-specific user deposit/withdraw instructions. They no longer exist in the
  program interface.
- Select the protocol behavior from `bank.config.asset_tag`.
- For `integration_deposit`, pass the protocol-specific accounts in `remaining_accounts`.
- For `integration_withdraw`, pass protocol-specific accounts first in `remaining_accounts`, then
  append the usual health/risk accounts.
- Update any discriminator allowlists or transaction inspectors that referenced
  `*_withdraw` integration discriminators. There is now a single `integration_withdraw`
  discriminator for all wrapped integrations.
- Re-run all client-side account builders. This migration is not only a method rename; the account
  packing model changed as well.

### Exact account layouts

The exact per-protocol account layouts are enforced by the program and mirrored in our TS helpers.
Use the builders under:

- `tests/utils/kamino-instructions.ts`
- `tests/utils/drift-instructions.ts`
- `tests/utils/solend-instructions.ts`
- `tests/utils/juplend/user-instructions.ts`
- `tests/utils/integration-account-layouts.ts`

Those files are the current source of truth for the required ordering and optional-account padding.

## Important Instructions (click to learn more)

<details>
<summary> <b>marginfi_account_initialize_pda</b> - Create an Account</summary>

- Note: `marginfi_account_initialize` is not recommended for integrators because it uses an
  ephemeral keypair which must sign.
- Ask us for a unique `third_party_id`! Simply open a PR or reach out through support. With this,
you can quickly grab all the Accounts that belong to your program using a fetch with memCmp.
</details>

<details>
<summary> <b>lending_account_deposit</b> - deposit into any Bank EXCEPT integrator banks (Kamino, etc)</summary>

- Check `bank.config.asset_tag`, ASSET_TAG_DEFAULT (0) or ASSET_TAG_SOL (1) ASSET_TAG_STAKED (2)
  are allowed with this instruction. Others have their own deposit instruction.
- No Risk Engine check, always considered risk-free
- `amount` is in native token, in native decimal, e.g. 1 SOL = 1 \* 10^9
- Set `deposit_up_to_limit` to "true" to ignore your amount input if near the deposit cap and
deposit only what is available. For example, if the deposit cap is 10 SOL, there is 6 SOL in
the bank, and you attempt to deposit 10, `deposit_up_to_limit` = true will deposit 4,
`deposit_up_to_limit` = false will fail and abort.
</details>

<details>
<summary> <b>integration_deposit</b> - deposit into an integration Bank (Kamino, Drift, Solend, JupLend)</summary>

- Check `bank.config.asset_tag` and pass the protocol-specific accounts in `remaining_accounts`.
- Supported tags are the wrapped integration banks: Kamino (3), Drift (4), Solend (5), JupLend (6).
- No Risk Engine check, always considered risk-free
- `amount` is in native underlying token decimals for every supported integration deposit.
</details>

<details>
<summary> <b>lending_account_withdraw</b> - withdraw from any Bank EXCEPT integrator banks (Kamino, etc)</summary>

- Check `bank.config.asset_tag`, ASSET_TAG_DEFAULT (0) or ASSET_TAG_SOL (1) ASSET_TAG_STAKED (2)
  are allowed with this instruction. Others have their own deposit instruction.
- Requires a Risk Engine check (pass banks and oracles in remaining accounts)
- `amount` is in native token, in native decimal, e.g. 1 SOL = 1 \* 10^9
- Set `withdraw_all` to "true" to ignore your amount input and withdraw the entire balance. This
is the only way to close a Balance so it no longer appears on your Account, simply withdrawing
by configuring `amount` will always leave the Balance on your account, even with zero shares.
</details>

<details>
<summary> <b>integration_withdraw</b> - withdraw from an integration Bank (Kamino, Drift, Solend, JupLend)</summary>

- Check `bank.config.asset_tag` and pass protocol accounts first in `remaining_accounts`, followed
  by the usual risk-engine bank/oracle accounts.
- Requires a Risk Engine check for the post-withdraw health validation.
- The program splits `remaining_accounts` by protocol-specific account count, so ordering matters.
- `amount` semantics depend on the wrapped protocol:
  - Kamino / Solend: amount is in collateral-share units.
  - Drift / JupLend: amount is in native underlying token units.
- Can fail if the Bank doesn't have enough liquidity, or the Account after the action would fail the
  risk check.
</details>

<details>
<summary> <b>lending_account_repay</b> - repay a debt</summary>

- Check `bank.config.asset_tag`, ASSET_TAG_DEFAULT (0) or ASSET_TAG_SOL (1) are allowed with this
  instruction. Others cannot borrow and, therefore cannot repay.
- No Risk Engine check, always considered risk-free
- `amount` is in native token, in native decimal, e.g. 1 SOL = 1 \* 10^9
- Set `repay` to "true" to ignore your amount input and repay the entire balance. This is the only
way to close a Balance so it no longer appears on your Account, simply repaying by configuring
`amount` will always leave the Balance on your account, even with zero shares.
</details>


<details>
<summary> <b>lending_account_borrow</b> - borrow a liability</summary>

- Check `bank.config.asset_tag`, ASSET_TAG_DEFAULT (0) or ASSET_TAG_SOL (1) are allowed with this
  instruction. Others cannot borrow and.
- Requires a Risk Engine check (pass banks and oracles in remaining accounts)
- `amount` is in native token, in native decimal, e.g. 1 SOL = 1 \* 10^9
- Can fail if the Bank doesn't have enough liquidity, or the Account after the action would fail the
  risk check.
</details>

<details>
<summary> <b>marginfi_account_update_emissions_destination_account</b> - set an emissions destination</summary>

- Highly encouraged if the Account is owned by a PDA. All emissions will be sent here instead of
to the authority.
</details>

<details>
<summary> <b>transfer_to_new_account/transfer_to_new_account_pda</b> - Move to a new authority</summary>

- Points earned will (eventually) go to the new account/authority, but you will still see points on
the old account for book-keeping reasons, and emissions will still airdrop to the old account for
that week.
</details>

<details>
<summary> <b>lending_account_start_flashloan</b> - Start a flashloan</summary>

- `lending_account_end_flashloan` must appear at the end of the same tx.
- Within this tx, you can borrow as much as you want, with no risk check! Note: you must pass a risk
  check at the end of this tx.
- No fees are charged for using this service at this time.
- Cannot be called by CPI
</details>

<details>
<summary> <b>lending_account_end_flashloan</b> - End a flashloan</summary>

- Requires a Risk Engine check (pass banks and oracles in remaining accounts)
- Cannot be called by CPI
</details>
