# marginfi v0.1.11-rc1

# Summary

## Scope Oracles

Banks can now use prices from a Scope OraclePrices account by selecting one of its 512 entries.

Scope oracles are supported only for default and SOL-tagged banks. They cannot currently be used by integration or staked-asset banks.

Banks using Scope cannot opt into same-asset e-mode.

## Native mSOL Oracles

The program can now derive mSOL prices using a SOL/USD Pyth feed and the canonical Marinade mSOL/SOL exchange rate.

Three configurations are available:

- PythMSOL
- KaminoMSOL
- JuplendMSOL

The Kamino and Juplend variants additionally apply the integration venue's exchange rate.

## Generic LST Oracles

The program can now price SPL stake-pool LSTs using a SOL/USD Pyth feed and the pool's on-chain LST/SOL exchange rate.

Three configurations are available:

- PythLST
- KaminoLST
- JuplendLST

Supported stake-pool owners include the standard SPL Stake Pool program and the supported Sanctum stake-pool programs.

Integration variants additionally apply the Kamino or Juplend exchange rate.

## Exponent Principal-Token Oracles

The release introduces oracle configurations for Exponent principal tokens:

- PTPyth - uses a Pyth base price and the Exponent PT conversion rate
- PTFixed - uses an implicit fixed one-dollar base price and the Exponent PT conversion rate

The initial PT price is stored in BankConfig.fixed_price and must be greater than zero and no greater than one.

The PT conversion rate increases linearly from its configured start price toward par between the vault start and maturity timestamps. It is also capped by the vault's available redemption backing.

Exponent vault is rejected when emergency mode has reduced its last-seen SY exchange rate below its historical high.

# Breaking Changes

## Admin Instructions

### lending_pool_set_fixed_oracle_price removed

The instruction has been replaced by:

```
lending_pool_set_oracle_price(
    price: WrappedI80F48,
    setup: u8,
)
```

The new instruction supports classic fixed-price banks and the new PT oracle configurations.

This is an IDL-breaking rename: the instruction discriminator has changed. Admin clients must use the v0.1.11 instruction rather than sending the old instruction discriminator.

### lending_pool_configure_bank_oracle

The signature is unchanged, but the instruction now supports the new mSOL and LST oracle configurations and validates their additional multiplier accounts.

Fixed and PT configurations must use lending_pool_set_oracle_price. Scope configurations must use lending_pool_configure_bank_oracle_scope.

Changing to a non-fixed oracle clears any previously stored fixed_price.

## Event Changes

`LendingPoolBankSetFixedOraclePriceEvent` has been renamed to:

`LendingPoolBankSetOraclePriceEvent`

Its payload remains header, bank, and price, but its event discriminator has changed. Event indexers must update to the new event definition.

## Error Changes

Error 6132 has been renamed:

`UseSetFixedOraclePrice` → `UseSetOraclePrice`

New oracle errors:

```
6136  MarinadeStateValidationFailed
6137  ExponentVaultValidationFailed
6138  InvalidPtStartPrice
6139  StakePoolStale
```

New Scope errors:

```
6800  ScopeInvalidAccount
6801  ScopeInvalidEntry
6802  ScopeStalePrice
6803  UseConfigureBankOracleScope
```

# New Instructions

### Admin Instructions

- `lending_pool_configure_bank_oracle_scope(oracle, entry_index)` - configures a bank to use an entry from a Scope OraclePrices account. The Scope account is supplied for validation when configuring the bank.
- `lending_pool_set_oracle_price(price, setup)` - configures classic fixed-price banks or initializes an Exponent PT oracle with its starting price.

# Changes to Existing Accounts

## BankConfig

The existing two-byte `_padding0` field is now:

`scope_entry_index: u16`

This stores the selected Scope price entry. The byte layout and account size are unchanged.

## OracleSetup

Existing values 0–17 are unchanged. The following variants are added:

```
18  Scope
19  PythMSOL
20  KaminoMSOL
21  JuplendMSOL
22  PythLST
23  KaminoLST
24  JuplendLST
25  PTPyth
26  PTFixed
```

# Breaking Changes for marginfi-type-crate Consumers

## Changed Structures

`ReconciledSameAssetConfig` gains:

`pub fixed_price: I80F48`

Consumers constructing or destructuring this type must include the new field.

`BankConfig::_padding0` has been replaced by `scope_entry_index`. This preserves the serialized layout but is a Rust source-level change for consumers accessing the field directly.

## Added Enum Variants

OracleSetup gains the nine variants listed above.

OracleFeedFamily gains:

```
MSOLPythPull
LSTPythPull
PtPythPull
```

Consumers using exhaustive matches over either enum must handle the new variants.

## Added Constants

`SCOPE_PROGRAM_ID` is now exported from the type crate's PDA module.

## Same-asset E-mode Semantics

`Bank::is_same_asset_emode_eligible` and same-asset reconciliation now consider the PT fixed_price. Two PT banks with otherwise identical oracle configuration but different start prices will not be treated as the same asset.

# Other Information

### Consolidates

#631, #660, #661

### Audit Information

TBD

### Release Information

Mainnet - Sept 4 at ~2pm EST