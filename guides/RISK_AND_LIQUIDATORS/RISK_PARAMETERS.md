# Risk Parameters Reference

Want a comprehensive understanding of all the risk parameters available and how they interact?
Read on. This guide complements the [Risk Introduction](GETTING_STARTED_RISK.md) with specific
parameter details.

## Glossary

- **I80F48** - A 128-bit fixed-point number used throughout the protocol for precise decimal math.
- **WrappedI80F48** - The serialized form of I80F48 stored on-chain.
- **Asset Weight** - A discount factor (0 to 1 for init, 0 to 2 for maint) applied to collateral
  value. Lower weight = less borrowing power.
- **Liability Weight** - A markup factor (>= 1) applied to borrow value. Higher weight = more
  collateral needed.

## Risk Weights

Every bank has four weight parameters:

| Parameter | Range | Purpose |
|-----------|-------|---------|
| `asset_weight_init` | 0 to 1 | Discounts collateral for new borrows |
| `asset_weight_maint` | 0 to 2 (must be >= init) | Discounts collateral for liquidation threshold |
| `liability_weight_init` | >= 1 | Marks up debt for new borrows |
| `liability_weight_maint` | >= 1 (must be <= init) | Marks up debt for liquidation threshold |

The gap between initial and maintenance weights creates the "health buffer" or "liquidation
buffer": the range where a position is not borrowable-against but also not yet liquidatable.

### How Weights Affect Health

```
Initial Health  = sum(asset_value * asset_weight_init)  - sum(debt_value * liability_weight_init)
Maint Health    = sum(asset_value * asset_weight_maint) - sum(debt_value * liability_weight_maint)
Equity          = sum(asset_value)                      - sum(debt_value)  [no weights]
```

- **Initial Health >= 0**: Required to open new borrows or withdraw collateral.
- **Maint Health < 0**: Account is eligible for liquidation.
- **Equity < $0.10**: Account is eligible for bankruptcy handling.

### Confidence Adjustments

The protocol also adjusts asset prices based on oracle confidence intervals:

- **Assets**: Valued using `price * (1 - confidence_adjustment)` (conservative lower bound)
- **Liabilities**: Valued using `price * (1 + confidence_adjustment)` (conservative upper bound)

This means volatile assets with wide confidence intervals will have less borrowing power, even with
the same weights. The `oracle_max_confidence` parameter sets the maximum tolerated confidence
width.

## Oracle Configuration

Each bank has an oracle configuration that controls how prices are sourced.

### Oracle Types

| Type | Value | Description |
|------|-------|-------------|
| `None` | 0 | No oracle (disabled) |
| `PythLegacy` | 1 | Deprecated, do not use |
| `SwitchboardV2` | 2 | Deprecated, do not use |
| `PythPushOracle` | 3 | Pyth pull/push oracle (recommended for standard assets) |
| `SwitchboardPull` | 4 | Switchboard pull oracle |
| `StakedWithPythPush` | 5 | For staked assets using Pyth |
| `KaminoPythPush` | 6 | For Kamino vault positions using Pyth |
| `KaminoSwitchboardPull` | 7 | For Kamino vault positions using Switchboard |
| `Fixed` | 8 | Admin-set fixed price (no oracle needed) |
| `DriftPythPull` | 9 | For Drift integration positions using Pyth |
| `DriftSwitchboardPull` | 10 | For Drift integration positions using Switchboard |
| `SolendPythPull` | 11 | For Solend integration positions using Pyth |
| `SolendSwitchboardPull` | 12 | For Solend integration positions using Switchboard |

### Oracle Parameters

- **`oracle_keys`**: Up to 5 oracle account pubkeys. For Pyth, this is the price feed account. For
  wrapped assets (Kamino, Drift, Solend), additional oracle keys may be needed for the underlying
  asset.
- **`oracle_max_age`**: Maximum age (in seconds) of an oracle price before it's considered stale.
  Minimum enforced value is 10 seconds. Stale prices will cause transactions to fail.
- **`oracle_max_confidence`**: Maximum allowed confidence interval width. If set to 0, defaults to
  10% (0.10). If the oracle's confidence exceeds this threshold, the price is rejected.

### Price Bias

For health calculations, the protocol applies a price bias:
- Assets use a **Low** bias (lower bound of confidence interval), reducing their value.
- Liabilities use a **High** bias (upper bound), increasing their value.

This ensures health calculations are conservative even when oracle prices are uncertain.

## Risk Tiers

| Tier | Value | Effect |
|------|-------|--------|
| `Collateral` | 0 | Normal asset. Can be borrowed alongside other assets. |
| `Isolated` | 1 | Can only be borrowed in isolation. If an account borrows an isolated asset, it cannot have any other borrow positions. |

Isolated tier is used for higher-risk assets where cross-collateralization would be dangerous.
Isolated assets **must** have asset weights of 0 (both init and maint), meaning they contribute
no collateral value. They can be deposited but provide no borrowing power. Borrowing an isolated
asset requires dedicating the entire account to that single borrow (no other borrow positions
allowed). Emode may override isolated asset weights in the future, but this is not yet implemented.

## Capacity Limits

| Parameter | Description |
|-----------|-------------|
| `deposit_limit` | Maximum total deposits in native token units |
| `borrow_limit` | Maximum total borrows in native token units |
| `total_asset_value_init_limit` | Maximum USD value (in init-weight terms) of deposits across all accounts. This is an "oracle attack" mitigation: even if an oracle is manipulated to show a very high price, the total collateral value from this bank is capped. Set to 0 to disable. |

These limits are enforced at deposit/borrow time. Existing positions that exceed limits (due to
price movements) are not affected.

## Asset Tags

Banks can be tagged to identify their asset category. This is primarily used by the staked
collateral system and integration modules:

| Tag | Value | Meaning |
|-----|-------|---------|
| `ASSET_TAG_DEFAULT` | 0 | Standard asset |
| `ASSET_TAG_SOL` | 1 | SOL or native stake |
| `ASSET_TAG_STAKED` | 2 | Staked collateral (e.g. LSTs via staked settings) |
| `ASSET_TAG_KAMINO` | 3 | Kamino vault position |
| `ASSET_TAG_DRIFT` | 4 | Drift protocol position |
| `ASSET_TAG_SOLEND` | 5 | Solend protocol position |

## Liquidation Parameters

The protocol has two liquidation mechanisms, each with their own fee structure:

### Classic Liquidation

- **Liquidator fee**: 2.5% of the repaid amount (goes to the liquidator as profit)
- **Insurance fee**: 2.5% of the repaid amount (goes to the bank's insurance fund)
- Total discount to the liquidatee: ~5%

The liquidator chooses an asset to seize and a liability to repay. The exchange rate is the oracle
price adjusted by these fees. A liquidation cannot make an account healthy; the liquidator can only
bring maintenance health up to zero.

### Receivership Liquidation

- **Max fee**: Configurable via `FeeState.liquidation_max_fee` (historically ~10%)
- **Flat SOL fee**: A small SOL fee charged per liquidation

The receiver gets temporary control of the account and can withdraw collateral / repay debts. The
protocol enforces that the receiver does not extract more than the max fee as profit.

## Key Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `LIQUIDATION_LIQUIDATOR_FEE` | 2.5% | Classic liquidation: liquidator's share |
| `LIQUIDATION_INSURANCE_FEE` | 2.5% | Classic liquidation: insurance fund's share |
| `BANKRUPT_THRESHOLD` | $0.10 | Equity below this triggers bankruptcy |
| `EMPTY_BALANCE_THRESHOLD` | 1 (native unit) | Balances below this are considered empty |
| `ZERO_AMOUNT_THRESHOLD` | 0.0001 | Amounts below this are treated as zero |
| `CONF_INTERVAL_MULTIPLE` | 2.12 | Multiplier for oracle confidence intervals |
| `MAX_CONF_INTERVAL` | 5% | Default maximum confidence interval |
| `MAX_PYTH_ORACLE_AGE` | 60s | Maximum Pyth oracle age |
| `ORACLE_MIN_AGE` | 10s | Minimum allowed `oracle_max_age` setting |

## Emode Parameters

Emode (Efficiency Mode) allows overriding asset weights for specific borrowing pairs. See the
[Emode Guide](EMODE_ADMIN.md) for configuration details.

Group-level emode limits:
- **`emode_max_init_leverage`**: Maximum initial leverage allowed via emode (default: 15x)
- **`emode_max_maint_leverage`**: Maximum maintenance leverage allowed via emode (default: 20x,
  must be > init)

These prevent emode configurations from creating excessively leveraged positions.
