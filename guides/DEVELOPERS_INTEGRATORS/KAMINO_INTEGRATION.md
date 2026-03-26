# KAMINO INTEGRATION GENERAL GUIDE

Are you an administrator, risk manager, developer, or power user of the mrgn program's Kamino
integration and want to know more? Read on!

## Terms

* Klend - The main Kamino lending program.
* Market - A "group" in Kamino parlance.
* Reserve - A "bank" in Kamino parlance.
* Obligation - A "user account" in Kamino parlance.
* Farms - A rewards emission mechanism used by Kamino to incentivize reserve deposits/borrows. Built
  as a separate program from klend.
* Refreshing Reserves - Kamino requires reserves to cache price data from their oracles. This must
  happen for any tx that changes funds. A refresh is valid for one slot or one tx that changes
  funds where that reserve is involved, whichever comes first.
* Refreshing Obligations - Kamino requires the user's account to refresh before any tx that changes
  funds, after any reserves the account uses are refreshed. This syncs the "balances" or "health" of
  the user's account. Like refreshing reserves, this is valid for one slot or one tx that changes funds,
  whichever comes first. In practice, every Kamino related tx looks like: (refresh reserves, refresh
  obligation, everything else).
* Mrgn-wrapped or wrapped - We refer to mrgn banks that track Kamino collateral assets as
  "mrgn-wrapped" Kamino banks.
* Liquidity token or underlying token - the token users want to deposit, e.g. USDC, SOL, etc.
* Collateral tokens - When you deposit into Kamino, you are granted "collateral tokens" that
represent your position. These are equivalent to mrgn "shares". Although they are technically
tokenized, users rarely (never) hold them, the Kamino bank simply tracks these same way mrgn tracks
"shares" internally. Like mrgn shares, collateral tokens appreciate in value relative to the
liquidity token, so as interest accumulates, the exchange rate of collateral/liquidity token goes
up. Under the hood, mrgn banks are really storing collateral tokens.


## Big Picture Overview

Each mrgn-wrapped Kamino bank will correspond to exactly one Kamino reserve. It will have exactly
one obligation, with a single position, that holds all the deposits in that bank for all users.

Users always deposit the raw asset into this bank, users never transfer their Kamino position,
deposit collateral tokens, etc. For example, if the Reserve transacts in USDC, users are expected to
deposit USDC. To transfer an existing Kamino position in a mrgn-wrapped one, simply withdraw the
funds and deposit them again into the mrgn-wrapped bank.

User deposits in a wrapped bank are essentially a "share" of the obligation and its position in the
Reserve that bank is tracking. The asset share value of wrapped banks is always 1 (they never earn
interest in the mrgn program). All interest is earned due to appreciate of the collateral token on
Kamino's end.

The mrgn program never has custody of funds deposited into wrapped banks, it is simply the owner of
an obligation on that bank (caveat: the bank's `liquidity_vault_authority` actually owns the
obligation under the hood). In simple terms, Mrgn banks are like any other user/depositor of the
Kamino platform, enjoying no special access or privileges.

Wrapped banks now use the shared `integration_deposit` and `integration_withdraw` interface. They
can never borrow. The Kamino variant still requires Kamino-specific reserve/obligation accounts,
but those accounts are now passed through `remaining_accounts` rather than a dedicated Kamino user
instruction.

Remember to refresh reserves and the bank's obligation before any deposit/withdrawal.

## Developer Guide

It is useful to clone klend for debugging purposes (https://github.com/Kamino-Finance/klend).

Locally, we use a fixed dependencies located in fixtures (see kamino_lending.so, the idl, farms
program, etc, etc). Version 1.11.0 is the current version of klend we test against locally as of
June 2026. Update manually from
(https://github.com/Kamino-Finance/klend/releases/tag/release%2Fv1.12.0) as Kamino releases new
versions.


## Admin Guide

Groups do not have to opt into using Kamino, it is enabled by default. No special setup is required
beforehand.

### Process for adding a Kamino bank to a mrgnfi group
* Identify a reserve we want to use as collateral.
* Use the `add_pool` instruction to create a bank for the Reserve.
    * Multiple banks can exist for the same reserve
    * The bank's `mint` must match the reserve chosen
    * You must know the bank's `obligation` key at this time, even though it doesn't exist yet. The
      owner of the obligation is actually the bank's `liquidity_vault_authority`, not the bank
      itself. See `deriveBaseObligation`.
* use `init_obligation` to create the obligation for the bank you just created. If using a MS, note
  that this instruction is permissionless and may be done without your MS approval.
    * If the reserve has farms enabled, note that optional accounts related to the farms must be
      passed. Farms may be enabled even if no emissions are currently active!
    * You must deposit a small amount of the asset to keep the obligation open. These funds are not
      recoverable and are lost forever. We suggest using ~$1 in whatever the token is.

## Points and Rewards

The mrgn-wrapped Kamino obligation earns points and rewards like any other user. We (the group
admin) will distribute rewards from Kamino points back to mrgn users of that bank using a fair and
proportional method to be determined in the future.

Emissions can be harvested permissionlessly, and are sent to the global fee admin. Like rewards, we
(the group admin) will distribute emissions back to mrgn users of that bank using a fair and
proportional method to be determined in the future.

## Liquidators Guide

If a user has a wrapped Kamino position, remember to refresh any reserves/obligations that are
involved before attempting to liquidate.

Liquidators may claim a wrapped Kamino position, which must be withdrawn using
`integration_withdraw` on the wrapped bank.

The withdraw instruction requires refreshing the reserve/obligation and consumes a significant
amount of CU: it is recommended to do this in a separate tx to the liquidation ix in most cases.

## Migration Notes

If you previously built `kamino_deposit` or `kamino_withdraw`, migrate to:

- `integration_deposit` for user deposits
- `integration_withdraw` for user withdrawals

For Kamino withdraws, `remaining_accounts` must be packed as:

1. Kamino protocol accounts
2. Marginfi health/risk accounts

Do not interleave them. The shared integration withdraw flow uses the protocol account count to
split `remaining_accounts` before running the post-withdraw health check.

## Token Amount Types by Instruction

| Instruction | Token Amount Type | Notes |
|-------------|------------------|-------|
| Deposit | Liquidity token amount | Raw underlying token (e.g., USDC, SOL) |
| Withdraw | Collateral token amount | Must convert from collateral to liquidity token amount |
| Liquidate | Collateral token amount | Must convert from collateral to liquidity token amount |

**Important:** Deposit operations accept liquidity token amounts (the underlying asset), while
withdraw and liquidate operations work with collateral token amounts. Since collateral tokens
appreciate in value relative to the liquidity token as interest accumulates, liquidators and
withdrawers must manually convert from collateral token amounts to liquidity token amounts using the
current exchange rate.

## Ongoing Considerations

Integrating with another venue carries certain additional risks:
- **IDL updates**: Breaking changes to Kamino's instruction interfaces can break marginfi's Kamino
  operations
- **Account layout changes**: Changes to Reserve/Obligation structures can break deserialization and
  validation
- **Liquidity constraints**: Insufficient reserve liquidity or withdrawal halts prevent users from
  accessing funds despite marginfi allowing the operation
