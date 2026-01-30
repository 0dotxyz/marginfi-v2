# Project Zero: Built on the Marginfi v2 Program

This is a general overview of the Project Zero ecosystem and key features. Want the latest details
for developers, emode, fees, bankruptcies, liquidation, etc? Check the [guides
folder](./guides)!

## Overview

The Marginfi program is a decentralized borrow-lending platform that enables undercollateralized
lending against a variety of assets, including cryptocurrencies such as SOL, USDC, USDT, etc,
natively staked SOL, Liquid Staking Tokens (LST), and even lending positions on other platforms such
as Kamino.

## Glossary And Important Terms

- **Group** - A collection of banks. Groups have a single administrator (typically a secure
  governance MS) who has broad authority over them, and several delegate admins (limit, emode, etc)
  that can perform less risky modifications. All the assets you see at 
  [app.0.xyz](https://app.0.xyz) are in a single group overseen by the foundation.
- **Bank** - Each asset available to borrow and lend on `mrgnlend` has a Bank, this account controls
  all the settings for the asset. Many banks might exist for the same asset. Every asset on
  [app.0.xyz](https://app.0.xyz) is a Bank.
- **Account** - Users can create as many Accounts as they want. Accounts are per `Group`, and each
  account can have up to 16 positions in any `Bank`. The Account contains various user-specific
  settings and cached values in addition to their `LendingAccount`, where `Balances` are stored.
- **Balance** - A Balance is an asset or liability on a single `Bank`. Users cannot have both an
  asset and liability on the same Bank, and can have at most one Balance per `Bank`. Each `Account`
  has a `LendingAccount`, which is a collection of 16 Balances. Balances can be blank/unused, and
  are always sorted in byte order by the corresponding `Bank`'s Public Key.
- **Asset Weight** - Each asset available to lend has two rates: The "Initial" and "Maintenance"
  rates. The Maintenance rate is always higher. If attempting to execute a borrow, collateral is
  valued at (price x initial weight). If a liquidator is attempting a liquidation, collateral is
  valued at (price x maintenance weight). The range between these is sometimes called the health
  buffer. For example, if a user has collateral worth \$10, and initial/maintenance rates are 50\%
  and 60\% respectively, the user can borrow \$10 x .5 = \$5 in collateral. For liquidation purposes,
  their collateral is worth \$10 x .6 = \$6. The LTV (loan-to-value) you see on
  [app.0.xyz](https://app.0.xyz) is the "Initial" Weight, while the health displayed on the
  portfolio page uses the "Maintenance" Weight.
- **Liability Weight** - Each asset also has a liability weight! Like the `Asset Weight`, this is
  split into "Initial" and "Maintenance", where the Maintenance rate is always lower. If attempting
  to execute a borrow, liabilities are valued at (price x initial weight). If a liquidator is
  attempting a liquidation, liabilities are valued at (price x maintenance weight). On the borrowing
  page, this value is displayed as "LTV", which is (1/Initial Liability Weight), i.e. the LTV you
  would get if lending an asset with an Asset Weight of 1.
- **Oracle** - Each `Bank` has an oracle it uses to determine the price of the asset it transacts
  in. The `Group` admin is responsible for picking and maintaining the Oracle. Typically,
  [Switchboard](https://switchboard.xyz/) is the oracle provider, but [Pyth](https://pyth.network/)
  is also supported, and some banks have a Fixed price.
  An Oracle may use multiple accounts, for example a Kamino bank uses a price source and the Kamino
  reserve.
- **Oracle Confidence Interval** - Some Oracles report a price with Confidence, e.g. P +/- c. When
  pricing assets, we use $P - P*c$, and when pricing liabilities, we use $P + P*c$. For example, if
  Oracles report A is \$20 +/- $1, assets in A are priced at \$19, while liabilities are priced at
  \$21. The confidence interval will be no higher than 5%. If the Oracle reports a confidence
  interval higher than 5%, we may clamp it to 5% or abort the transaction. We call this an "Oracle
  Confidence Interval Adjustment", and it is relatively unique to the borrow-lending space.
- **EMA vs Spot** - Some oracles report an Exponential Moving Average (EMA) price in a addition to
  their spot price. This is essentially a price-over-time instead of an instant price "right now".
  If available, borrowing operations will use the EMA price, while liquidation will continue to use
  the spot price. If unavailable, all operations will use the spot price.

## Architecture at a Glance

```
┌────────────┐       ┌───────────┐       ┌──────────┐
│ Marginfi   |       │           │       │          │
│ Group      │1─────n│ Bank      │1─────a│ Oracle   │
│            │       │           │       │          │
└────────────┘       └───────────┘       └──────────┘
      1                    1
      |                    │
      |                    │
      n                    1
┌───────────┐       ┌────────────┐
│ Margin    │       │            │
│ Account   │1──<=16│  Balance   │
│           │       │            │
└───────────┘       └────────────┘
```

## Interest Accumulation

Interest accumulates when users borrow an asset. It's updated when a user performs any transaction
that affects a Bank's balances. Borrowers are assessed interest, while lenders earn it. A portion of
interest paid by borrowers also goes to fees. Interest accumulates on your position, entirely within
the program: no interaction is necessary beyond depositing, borrowing, withdrawing, and repaying.

Example:

```
      * Sally deposits $20 in A
      * Sally borrows $10 in B
      * Sally's $10 debt today accrues interest and grows to $11.
      * The $1 difference is not debited until Sally repays: i.e. she will have to repay $11 to close this debt.
      * Sally's net balance and borrowing power has declined from 20-10 = $10 to 20-11 = $9
      * If she cannot repay, she may not be able to withdraw all of her A.

      * Bob deposits $10 in B
      * Thanks to Sally's interest, Bob's B is now worth $11.
      * Bob doesn't realize his $1 gain until he withdraws.
      * Bob's net balance and borrowing power has increased from $10 to $11
      * Bob can withdraw $11 any time
```

### Interest Rates, Curves, and Compounding

In general, Banks are configured to charge progressively higher interest as the "utilization rate"
(UR) increases. The UR for a Bank is simply borrows/deposits. Interest typically climbs slowly until
the "optimal utilization rate", which for most assets is around 80-90%. Beyond that level, interest
grows rapidly. At 100% UR, lenders cannot withdraw, because all funds have been borrowed out! High
interest rates at 90%+ UR drive market conditions to force borrowers to repay their debts.

For more details about the interest configuration for a particular Bank, see the `zero_util_rate`
(interest at 0% UR), `hundred_util_rate` (interest at 100% UR), and `points` (interest at all levels
in between). Note that we support up to seven points in our interest rate curves (0 UR, 100 UR, and
5 in between), but the majority of banks use only 2 (0 at 0 UR, one in between, and a value at 100
UR).

You can read a Bank's last spot interest rate from `bank.cache`. This updates any time a Bank has a
balance change, or send a (permissionless) accrue interest instruction to force it to update.

### Curve Details

The "Base Rate" or `r` is the rate linearly interpolated between the two nearest points
$(x_0, y_0)$ and $(x_1, y_1)$, where there is always a point at $(0, y_0)$ and $(100, y_n)$.


$r = (ur-x_0)/(x_1-x_0) * (y_1-y_0) + y_0$


Examples:

If the `zero_util_rate` is 10%, and there is a point at (50%, 100%), and the UR is
currently 25%, then

```
r = (25-0)/(50-0) * (100-10) + 10 = 55%
```

If there is a point at (50%, 100%) and (80%, 150%), and the UR is currently 60%,
then

```
r = (60-50)/(80-50) * (150-100) + 100 = 116.67%
```

### Rate Details

There are two other notable interest rates that follow from `r`, and all of three these are reported
in the bank cache:

```
lending_rate = r * UR
borrow_rate = (r * ir_fees + fixed_fees)
```

Where fees include insurance and protocol fees. Notice that borrowers always pay more than lenders
get, because there are fees, and more importantly because there are more lenders than there are
borrowers!

The difference between what borrowers pay and lenders receive is called the `spread`. When the
lending interest in some asset exceeds the borrow rate for the same asset on another venue, we call
this opportunity an `interest arbitrage` (or `arb` for short).

### APR vs APY

The rates we just discussed are the Annual Percentage Rate (APR), not the Annual Percentage Yield
(APY). The former assumes simple interest, i.e. assuming it's charged once a year. The latter uses
compounding: your interest is accumulated pro-rata over time, and you also earn interest on the
interest that has accumulated to date. How often you earn pro-rata interest is sometimes called the
"compounding rate". Most savings accounts compound daily, while some financial products compound
monthly or quarterly. "Compounding continuously" means as often as possible.

Realized rates on our banks are actually closer to the APY. Interest is computed just before any
balance change, so the rate at which different Banks compound actually varies. More popular Banks,
like SOL, compound every few minutes, or even every few seconds on more active days. Less popular
Banks might compound just a few times per week, but these Banks typically have very few borrows (and
thus a low APR to compound). Since interest compounds based on usage, the more popular our platform,
the more often interest compounds. Remember that interest accrues for all of a Bank's users at the
same time: that means if anyone transacts with a bank, the interest for *every* user compounds!

In practice, Banks that have meaningful borrows typically end up yielding within 1% of the
continuously compounded APY. The exceptions are rare cases where an asset has a high APR, but not a
lot of activity.

Different venues and providers handle the APR -> APY conversion in different ways, which can lead to
slightly different rates depending on the source of the data. The rates displayed on
[app.0.xyz](https://app.0.xyz) assume compounding roughly hourly for native banks. Some other venues
use on-chain APY compounded every 400ms (the Solana slot time) using a Taylor series approximation
instead of an APR, so they may have no APR to compare against. Those APYs are fetched using best-
practices as defined in the venue's SDK.

### Interest, Previewed Amounts, and Closing Positions

Because interest accumulates just before any transaction that affects a Bank's balances, when a user
goes to withdraw, the amount they withdraw can be slightly higher than what is previewed, likewise
for repayments, etc. This also means that to withdraw or repay all, the user must send a special
flag to the instruction to close the balance in full AFTER interest.

## Risk Engine

To maintain account health, Marginfi uses a deterministic risk engine that ensures that borrowing
and lending activities are within acceptable risk parameters. If a user's account falls below the
minimum required health factor, they may be subject to liquidation to protect the integrity of the
lending pool and other users' accounts.

In a nutshell, health is calculated as the sum of assets minus the sum of liabilities, after all
applicable weights and confidence intervals have been applied. An Account cannot borrow more once
its health is less than zero using Initial weights, and is subject to liquidation if health is less
than zero using Maintenance Weights.

Examples:

```
Token A is worth $10 with conf $0.212 (worth $9.788 low, $10.212 high)
USDC is worth $1 with conf $0.0212 (worth $0.9788 low, $1.0212 high)
Token A has a Initial Asset Weight of 50% (0.5)
USDC has a Initial Liability Weight of 100% (1)

User has:
ASSETS
   [0] 2 Token A (worth $20)
DEBTS
   [1] 5.05 USDC (worth $5.05)

Health calculation: (2 * 9.788 * .5) - (5.05 * 1.0212 * 1) = 4.63094
```

This account is healthy, and has $4.63094 in remaining borrowing power!

Now let's look at an account that's unhealthy, falling below its Maintenance requirements.

```
Token A has a Maintenance Asset Weight of 10% (0.1)
USDC has a Maintenance Liability Weight of 100% (1)

User has:
ASSETS
   [0] 2 Token A (worth $20)
DEBTS
   [1] 5.05 USDC (worth $5.05)

Health calculation: (2 * 9.788 * .1) - (5.05 * 1.0212 * 1) = -3.19946
```

In the above example, we see that the user still has more assets than debts, but due to weights,
their account is unhealthy: they are eligible to be liquidated. A partial liquidation can restore
their health:

```
Liquidator fee = 2.5%
Insurance fee = 2.5%

Liquidator seizes .2 token A (worth $2) from liquidatee's account
Liquidator must pay (in USDC):
 value of A minus liquidator fee (low bias): .2 * (1 - 0.025) * 9.788 = $1.90866
Liquidatee receives (in USDC debt repayment):
 value of A minus (liquidator fee + insurance) (low bias): .2 * (1 - 0.025 - 0.025) * 9.788 = $1.85972

Health after: ((2-0.2) * 9.788 * .1) - ((5.05-1.85972) * 1.0212 * 1) = -1.496073936
```

The user's health improved, but they are still eligible to partially liquidated.

### Checking Health

While you can always take the sum of assets minus the sum of debts, it can be difficult to know
exactly what price the protocol will use on-chain. Users can send a `lending_account_pulse_health`
to read the Risk Engine's assessment of their account at this moment for borrowing, liquidation, and
bankruptcy purposes. This instruction uses the same oracles that would be used in any other risk
check.

Whenever a user Borrows or Withdraws, the Risk Engine determines if the user would be within
acceptable risk parameters after the tx completes, rejecting the tx if not. Deposits and Repays
require no Risk Engine check, as they can only improve health. Accounts requiring the Risk Engine
check must pass all Banks and Oracles involved in the user's Balances in remaining accounts.

### Third Party Liquidation

Liquidation is open to third parties, and encouraged! An account that is unhealthy can be
liquidated, protecting the solvency of depositors and netting a small profit (2.5-10%) for the
liquidator in exchange for performing this service.

### Passing Risk Accounts and Cranking Oracles

Other borrow-lending protocols typically have a refresh system. When their risk system runs, a
series of "refresh" instructions must appear before the instruction that consumes the risk data. Our
Risk Engine runs Just In Time: no refresh instructions are needed, except for those required by
integrated venues.

Whenever the risk engine must run, the caller passes required accounts in the remaining_accounts.
The format is:

```
[bank0, oracle0_1],
[bank1, oracle1_1, oracle1_2],
[bank2, oracle2_1],
```

Noting that some banks have more than one Oracle account (for example, Kamino banks have two: the
price oracle, and the Kamino reserve). Oracles can always be read from `bank.config.oracle_keys`,
and are passed in the order they appear there. Each set of banks/oracles are passed in sorted
bitwise order by Bank key, the same way they appear in the user's Balances.

Each oracle must not be stale. For Pyth oracles, the caller typically has no obligations, Pyth
oracles are kept up-to-date by their administrator. For Switchboard Oracles, the caller must call a
crank instruction just before the tx consuming that price. Some callers prefer to use bundles for
this, but it typically suffices to send a crank instruction and briefly wait. When tx size permits,
callers might even prepend the Switchboard crank to the tx consuming the Oracle data, although this
runs into account and CU constraints for larger txes. For Kamino banks, `refresh_reserve` must also
execute within 1 slot.

In some instructions, limited Oracle staleness is permitted. For example, when borrowing, the caller
can pass enough non-stale oracle data to demonstrate collateral is sufficient. For example, if the
user is lending \$1 in A and \$1000 in B, and trying to borrow \$100 in C, the caller might pass a
stale oracle for A, because the collateral in B alone is sufficient to complete the borrow!

## Integrations With Other Venues

The protocol accepts collateral from third-party venues like Kamino, provided the Group
administrator has created a Bank for that asset. Typically, each third-party venue asset will have a
Bank, for example the Kamino Maple Market USDC reserve is one Bank, and the Kamino Main Market USDC
reserve is another.

Lenders into a third-party venue earn no interest on our platform, because there is no borrowing
from third-party venues on our platform! Instead, depositors earn interest like a normal lender into
THAT VENUE. For example, when depositing in the Kamino Main Market USDC Bank, lenders earn what any
other depositor would earn on Kamino. Another way of saying this is that lending interest is earned
by borrowers on Kamino.

## Native Staked Assets ("Staked Collateral")

The protocol accepts SOL natively staked to validators as collateral. This is useful for validators
who want to increase their leverage, or for parties interested in utilizing their natively staked
SOL without exiting their position for e.g. tax purposes (Not tax advice: always consult your local
tax authority). Your SOL earns all the yields it would normally earn, including MEV, etc. Currently,
only SOL can be borrowed against native stake positions.

Under the hood, this feature depends on the Single Validator Stake Pool, [published and maintained
by Solana Foundation](https://github.com/solana-program/single-pool)

Don't see your validator listed? Adding Staked Collateral is permissionless!

## Emode Benefits

Some assets support "Emode", which increases the Asset Weight ("Initial" and/or "Maintenance") of
that token when it is used to borrow a specific paired asset. For example, LST might have an Emode
benefit with SOL, so when LST is used to borrow SOL, it supports much higher LTVs than when it is
used to borrow e.g. USDC. An Account's Emode benefit for a given token being lent is always based on
the worst benefit across all the assets they are borrowing, this means in some instances it makes
more sense to break assets into multiple accounts.

## Notable Bank States

The Group administrator can create Banks (or place them into) several notable states:

### Isolated

Banks in an "Isolated" Risk Tier are worthless as collateral: their Asset Weight is always zero.
They can still earn interest, and can still be borrowed, i.e. their Liability Weight is still
active. Accounts can only borrow one Isolated asset at a time.

### Reduce-Only

Banks in a "Reduce Only" Operational Mode can only withdraw or repay, not borrow or deposit.
Liquidations continue to function as normal, and all other parameters also work as normal.

Banks are usually placed in this mode when they are going to be sunset. Users are encouraged to
withdraw from Reduce-Only banks as soon as possible before more drastic measures are taken to
deleverage these banks, such as reducing Asset Weights.

### Paused

Banks in a "Paused" Operational Mode cannot deposit, withdraw, borrow, or repay. This mode is
usually reserved for defunct Banks that are empty.

### Globally Paused

The Global Fee State Admin, which controls program-level fees, can pause the protocol for up to 30
minutes at a time, and up to twice per day. This is used only in extreme emergencies: it has never
been used once as of November 2025. During a global pause, all financial operations on all Groups
and Banks are suspended. 