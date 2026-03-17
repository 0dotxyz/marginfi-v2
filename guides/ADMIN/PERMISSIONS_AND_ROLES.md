# Permissions and Roles Guide

## Glossary

- **MarginfiGroup** - The top-level account that contains all admin role assignments. Every bank
  and user account belongs to a group.
- **Authority** - The owner of a user's `MarginfiAccount`. This is the keypair that can deposit,
  withdraw, borrow, and repay on that account.
- **PDA** - Program Derived Address. Used for vault authorities and other system-controlled
  accounts. These are not human-controlled keys.
- **FeeState** - A global singleton account that stores protocol-level fee configuration and the
  global fee admin.

## Admin Roles

The `MarginfiGroup` account defines seven distinct admin roles. Each role is a single `Pubkey`
that can be set to any address, including a multisig program. Setting a role to `Pubkey::default()`
(all zeros) effectively disables it.

### Admin (Group Admin)

The most powerful role. The admin has full control over the group and its banks.

**Can do:**
- Create new banks (`LendingPoolAddBank`, `LendingPoolAddBankWithSeed`)
- Configure any bank setting (`LendingPoolConfigureBank`)
- Configure the bank's oracle (`LendingPoolConfigureBankOracle`)
- Set a fixed oracle price (`LendingPoolSetFixedOraclePrice`)
- Set or remove all other admin roles
- Configure the group itself
- Freeze and unfreeze individual user accounts
- Handle bankruptcy (in addition to `risk_admin`)
- Close banks (when `CLOSE_ENABLED` flag is set)
- Collect and withdraw group fees

**Cannot do:**
- Set a bank to `KilledByBankruptcy` (only happens programmatically)
- Change global fee state (that's the `global_fee_admin`)

### Risk Admin

Responsible for risk management operations.

**Can do:**
- Handle bankruptcy / settle bad debt (for banks without `PERMISSIONLESS_BAD_DEBT_SETTLEMENT`)
- Start forced deleverage (`StartLiquidation` with deleverage mode)
- Force tokenless repayment completion

The risk admin is the day-to-day risk operations role, handling bad debt and liquidation scenarios
that require manual intervention.

### Emode Admin

Controls E-mode (Efficiency Mode) configuration.

**Can do:**
- Set emode tags on banks
- Configure emode entries (preferential collateral ratios for correlated asset pairs)

For more details see the [Emode Guide](../RISK_AND_LIQUIDATORS/EMODE_ADMIN.md).

### Delegate Curve Admin

A scoped admin that can modify interest rate configuration, including both curve parameters and
fee parameters within the interest rate config.

**Can do:**
- Modify curve parameters (`zero_util_rate`, `hundred_util_rate`, `points`) on any bank
- Modify interest rate fee parameters (`insurance_ir_fee`, `insurance_fee_fixed_apr`,
  `protocol_ir_fee`, `protocol_fixed_fee_apr`, `protocol_origination_fee`)
- All via `ConfigureBankLiteCurve` (which takes `InterestRateConfigOpt`)

Note: any update through this path forces the bank to the seven-point curve type. Changes are
blocked if the bank has `FREEZE_SETTINGS` enabled.

This role allows interest rate management to be delegated to a separate party (e.g. a rate
committee) without giving them access to weights, oracle config, or other bank settings.

### Delegate Limit Admin

A scoped admin that can only modify capacity limits.

**Can do:**
- Modify `deposit_limit`, `borrow_limit`, and `total_asset_value_init_limit` on any bank
  (via `ConfigureBankLiteLimit`)

Note: if the bank has `FREEZE_SETTINGS` enabled, only `deposit_limit` and `borrow_limit` can be
changed. The `total_asset_value_init_limit` is treated as a frozen field because reducing it can
affect the value of existing deposited assets.

This is useful for dynamically managing bank capacity, for example adjusting limits based on
demand, without exposing other configuration.

### Delegate Emissions Admin

A scoped admin that can only modify emissions settings.

**Can do:**
- Modify emissions flags (enable/disable borrow and lending emissions)
- Modify `emissions_rate` and `emissions_mint`
- Top up emissions funding

### Metadata Admin

A scoped admin for bank metadata only.

**Can do:**
- Write and update metadata for any bank in the group (via `WriteBankMetadata`)

Metadata is informational only and does not affect the behavior of the protocol. This role allows
a separate party (e.g. a front-end team) to manage display names, descriptions, and similar data.

## Global Fee Admin

The `global_fee_admin` is separate from the group-level admin roles. It is stored on the `FeeState`
account (a global singleton).

**Can do:**
- Edit global fee parameters (program fee rates, origination fee shares, init fees)
- Change the global fee wallet
- Panic-pause the entire protocol (with rate limiting: max 2 consecutive pauses, max 3 per day,
  each lasting 30 minutes)

This role is intended for the protocol operator (e.g. the foundation) and controls protocol-level
economics and emergency pause functionality.

## User-Level Access

### Account Authority

Every `MarginfiAccount` has an `authority` field: the keypair that controls it.

**Can do:**
- Deposit into the account
- Withdraw from the account
- Borrow against the account
- Repay debts on the account
- Perform flash loans
- Transfer the account to a new authority
- Close the account

### Permissionless Operations

Some instructions can be called by anyone:

- **`LendingPoolCollectBankFees`** - Moves accrued fees from the liquidity vault to the appropriate
  fee vaults. Anyone can call this.
- **`LendingPoolWithdrawFeesPermissionless`** - Sends fees to the admin's pre-configured
  destination account, if one has been set.
- **`LendingAccountLiquidate`** (classic liquidation) - Any signer can liquidate an unhealthy
  account.
- **`StartLiquidation`** (receivership liquidation) - Any signer can initiate receivership
  liquidation of an unhealthy account.
- **`HandleBankruptcy`** - Only permissionless if the bank has the
  `PERMISSIONLESS_BAD_DEBT_SETTLEMENT` flag set.
- **Interest accrual** - Happens automatically when any user interacts with a bank.

## Special Account States

### Frozen Accounts

The group admin can freeze any individual user account. When frozen:
- The account's authority is blocked from all operations.
- Only the group admin can operate on the account (e.g. to withdraw or rebalance).
- The account remains frozen until explicitly unfrozen by the admin.

This is used for compliance, investigations, or protecting accounts in unusual situations.

### Receivership

When an account enters receivership liquidation:
- The designated `liquidation_receiver` gets temporary authority over the account.
- The original authority is temporarily locked out.
- Operations like withdraw become permissionless during receivership (anyone with
  `allow_receivership=true` authorization can act).

For more details see the [Receivership Liquidation Guide](../RISK_AND_LIQUIDATORS/RECEIVERSHIP_LIQUIDATION.md).

## Access Control Matrix

| Instruction | Required Role |
|-------------|---------------|
| Configure group | `admin` |
| Add bank | `admin` |
| Configure bank (full) | `admin` |
| Configure bank oracle | `admin` |
| Set fixed oracle price | `admin` |
| Configure interest rate config | `admin` or `delegate_curve_admin` |
| Configure bank deposit/borrow/init limits | `admin` or `delegate_limit_admin` |
| Configure bank/group rate limits | `admin` or `delegate_limit_admin` |
| Configure deleverage withdraw daily limit | `admin` or `delegate_limit_admin` |
| Settle group rate limiter batches | `admin` or `delegate_limit_admin` |
| Settle deleverage withdraw batches | `admin` or `delegate_limit_admin` |
| Configure emissions | `admin` or `delegate_emissions_admin` |
| Configure emode | `emode_admin` |
| Write bank metadata | `metadata_admin` |
| Freeze/unfreeze account | `admin` |
| Handle bankruptcy | `risk_admin` or `admin` (or permissionless if flag set) |
| Start forced deleverage | `risk_admin` |
| Force tokenless repay complete | `risk_admin` |
| Edit global fee state | `global_fee_admin` |
| Panic-pause protocol | `global_fee_admin` |
| Collect bank fees | Anyone |
| Classic liquidation | Anyone (if account unhealthy) |
| Receivership liquidation | Anyone (if account unhealthy) |
| Deposit/Withdraw/Borrow/Repay | Account `authority` |

For the off-chain aggregation flow behind group rate limits and deleverage withdraw limits, see [RATE_LIMITS_AND_DELEVERAGE_WITHDRAW_LIMITS.md](./RATE_LIMITS_AND_DELEVERAGE_WITHDRAW_LIMITS.md).
