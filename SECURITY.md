# Important Notice
**DO NOT CREATE A GITHUB ISSUE** to report a security problem. Instead, please send an email to security@mrgn.group with a detailed description of the attack vector and security risk you have identified.
​
# Bug Bounty Overview
marginfi offers bug bounties for marginfi's on-chain program code. Bugs related to other parts of marginfi's infrastructure (networking, UI, SDK) are marked below.
​
|Severity|Bounty|
|-----------|-------------|
|Critical|2% of the value of the hack, up to $500,000. Minimum $50,000|
|High|$10,000 to $50,000 per bug, assessed on a case by case basis|
|Medium/Low|$1,000 to $5,000 per bug, assessed on a case by case basis|
​

The severity scale is based on [Immunefi's classification system](https://immunefi.com/immunefi-vulnerability-severity-classification-system-v2-3/). 
Note that these are simply guidelines for the severity of the bugs. Each bug bounty submission will be evaluated on a case-by-case basis.

## Infrastructure Bug Bounties
Bug bounties for infrastructure components (networking, UI, SDK) are first-come-first-serve. The bounty amount is at the discretion of the team based on severity.

|Severity|Bounty|
|-----------|-------------|
|Minor|$50|
|Medium|$50 to $500|
|Critical|Up to $5,000|
​
## Submission
Please email security@mrgn.group with a detailed description of the attack vector.
​
For critical- and high-severity bugs, we may require a proof of concept reproducible on a privately deployed mainnet contract or localnet (**NOT** our official deployment).
​
You should expect a reply within 1 business day with additional questions or next steps regarding the bug bounty.
​
## Bug Bounty Payment
Bug bounties will be paid in USDC or equivalent. Critical bounties may be paid in up to 80\% token, with the rest in USDC.
​
## Invalid Bug Bounties
A number of attacks are out of scope for the bug bounty, including but not limited to:
1. Attacks that the reporter has already exploited themselves, leading to damage.
2. Attacks requiring access to leaked keys/credentials.
3. Attacks requiring access to privileged addresses (governance, admin).
4. Incorrect data supplied by third party oracles (this does not exclude oracle manipulation/flash loan attacks).
5. Lack of liquidity.
6. Third party, off-chain bot errors (for instance bugs with an arbitrage bot running on the smart contracts).
7. Best practice critiques.
8. Sybil attacks.
9. Attempted phishing or other social engineering attacks involving marginfi contributors or users
10. Denial of service, or automated testing of services that generate significant traffic.


## Credits

Thank you to the following individuals for bug reports:

* https://github.com/mySebbe for identifying a bug where debts below the zero threshold can remain on the books after a borrow, enabling the extraction of assets where 1 satoshi/lamport/atom is worth more than ~1/10 of the Solana tx fee. 

## Known Issues and Scope Clarifications

### Solend Not Supported in ....

We are aware that e.g. Solend withdraw is not yet supported during e.g. receivership liquidation,
this was a deliberate choice to limit cpi exposure while there are not yet any Solend banks in
production.

The legacy liquidate instruction continues to support all bank types, including Solend, so there is
no risk of bad debt even if Solend banks were to be added before we added Solend to the receivership
allow list. We will add Solend instructions to the allowlist for other instructions if/when a Solend
bank appears in production.

Any instances of Solend missing from a whitelist are out-of-scope.

### T22 Extensions

Adding banks is an administrator function, and we do not make program level assumptions about which
(if any) of these T22 features the admin might tolerate. In cases where an asset is highly trusted
(e.g. PYUSD), an admin may still determine listing is viable even though it has Transfer Fee and
permanentDelegate extensions enabled. Regarding transfer hook, again it is on the admin to ensure
the usage is safe (e.g. PUMP).

In summary, the program will not validate these extensions are disabled, we leave it to the admin to
decide if they tolerate the associated risk, and the inclusion of these extensions is out-of-scope.

### Staked Collateral Price Confidence

Fixed in 1.9

Previously: confidence bands on Staked Collateral oracles were not priced incorrectly, slightly
over-valuing Staked Collateral positions.

### Flashloans May Not Affect Rate Limits

In some instances flashloans do not trigger flow control limits. This is a design choice: Billions
of dollars in notional volume flow through flashloans daily, including temporary A → B and B → A
position swaps that would distort the rate-limit windows. We also did not want to introduce a
breaking change requiring flashloans to pass risk accounts for flow limit accounting.

Flow controls primarily deal with economic issues, the group-level limiter in particular is not
designed to stop a dedicated attacker.

### Rounding Issues in Rate Limits

As above, the flow control limits are not intended to be canonical: we are aware that they round
down in some instances, which under-counts withdraws, this is also out-of-scope.

### Group Flow Control can be Bypassed Before it Updates

We are aware that many withdraws can be sent back to back to bypass the group-level flow controls
until the rate admin syncs them. Again, as above, the group level flow control limit is not intended
to be canonical: because of its async nature, it's a best-effort system, there will be mechanisms
for dedicated attackers to bypass group-level flow control under some circumstances (though there's
always a chance we move faster and they fail!). Flow control is intended to handle economic vectors,
not a dedicated attacker.

### Staked Collateral and Order Spam, Liquidation Record Spam

We are aware that an attacker can create a slight nuisance by spamming the creation of Staked
Collateral banks or by creating u16::MAX orders on their account(s). Neither of these affect the
protocol health in any way, and would cost prohibitive amounts in rent. These attacks are
out-of-scope.

Attackers can also grief an account by creating a Liquidation Record for them, which prevents the
account from closing until the Record is closed: again this costs the attacker non-trivial rent and
grants them no benefit, we consider this out-of-scope.

### Global Fee Admin Init

The global Fee State is deployed only once per program deployment. When we initially created it, the
init ix contained a hard-coded key check, which we have since removed. This removal allows
third-party deployments of marginfi (like our own staging deployment) to use the init ix. If you
re-deploy the program and an attacker hijacks the fee state admin after deployment, we suggest you
simply close the program and redeploy. Issues related to the initial deployment of the Fee State are
out-of-scope.

### Propagation-Related Issues

We are that pause state, global fees, etc can go out of sync if a group doesn't propagate the global
fee state in a timely fashion. It's incumbent on the group admin to propagate global fee state
settings. When we change fee state settings, we propagate to the main group in the same tx. Third
party groups can opt in to be included in that process (reach out to us if this interests you) or
propagate on their own. Any issues that deal with someone forgetting to propagate are out-of-scope.

### Bank Position Counts

The bank position count is usually canonical for banks created after it was added, but is not used
for anything except determining when a Bank can be closed to recover rent, which we do not typically
do unless the Bank was created by accident and never used. There are edge cases where a Bank may
miss a position count, we consider such findings INFO only.

### Interest Accrual During Borrow, Liquidate, Etc

We are aware that when borrowing, liquidating, etc, the user does not have accrue interest on all of
their positions, only the subset actually being manipulated. This leads to edge cases like a
liquidator harvesting accrued interest during liquidation, or a borrower exceeding initial health by
unaccrued debt interest.

For unaccrued interest to be meaningful (i.e., exceed the flat SOL fee for receivership
liquidations), it must either be very large, or have accumulated for some time. As liquidation is
highly competitive, liquidators will accrue debts to claim the full amount possible. Liquidators may
skip accruing assets and only accrue debts, which can lead to marginally unfair liquidations, but
this does not affect solvency and we tolerate that behavior at the margins since it is extremely
rare.

Attackers also do not control when interest accrues: anyone can accrue interest while they are
waiting. If a liquidator does pull off a profit using this trick, then they will have accrued
interest on the asset in the process, so they cannot do so again (without waiting), which
significantly limits their upside. Most banks with meaningful interest accrue very frequently
already.

Historically, CU constraints have made it impractical for us to accrue on all banks, though we are
considering it in the future if performance allows. We may also raise the flat fee if we find
liquidation is consistently more profitable than expected, though in practice most liquidators earn
far less than the theoretical max already, due to the real-world complexity involved in running a
liquidator. Missing valid liquidations because of unaccrued liability interest has not been a
concern due to the strong economic incentive associated with accruing debts.

In summary, issues related to part of a user's portfolio containing unaccrued interest at risk-check
time are out-of-scope.

### Order Slippage is too High, Isolated Orders

Users configure their own Slippage in Orders, in edge cases setting this slippage to a high value
can lead to unfavorable outcomes for the user. Because users opt-in in to Orders and pick their own
slippage, this is on the user to configure correctly! Likewise, users can create an Order on
Isolated assets, an Order on an Isolated asset enables the attacker to take the entire Isolated
asset when fulfilling (in exchange for the entire paired debt), since Isolated assets are worthless
in risk terms. Again, the user should not set such an Order if that is not their intent. We treat
poorly configured Orders that lead to bad outcomes for the user as INFO only. Our frontend platform
will limit the usage of Orders to ensure they are useful to end users, but at the program level,
users can largely make their own decisions.

### Orders Fail Due to Pause, Limits, etc

Many readers of the code assume that Orders are intended to have the same treatment as flashloans or
liquidations, where they can bypass certain mechanisms like flow control limits or protocol pause.
This is not the intent: generally speaking, Orders can fail for the same reason a generic withdraw
would fail. If an Order is failing under circumstances where a normal Withdraw would also fail, that
is out-of-scope.

### There's an Incoming Bug in XXXX When it Goes Live to Mainnet

If you see an incoming change on any program marginlend is integrated with that will lead to a bug
in the future, feel free to report it. Please be aware that such reports are STRICTLY first-come
first-served and will usually be denied if the program owner has already informed us of the update.

An example of such a bounty being paid is for the upcoming SVSP update (expected mainnet roughly
June 2026), which is already on SVSP main branch and would be breaking to our Staked Collateral
accounting.


### Flow Control Under-Counts for Kamino, etc

We are aware of an issue in prod where the flow control limiter under-counts for integrations like
Kamino under certain circumstances. We mark it INFO/LOW because flow control is primarily an
economic safeguard and at best a secondary defense against technical adversaries. At worst, if the
flow control is bypassable due to this issue, the bank is no worse off than if it were simply
disabled, no griefing, loss of funds, etc is enabled from this bug alone. Only integration banks are
affected (not native P0 banks), a similar bug affecting P0 banks would still be in scope. This will
be resolved in an upcoming update with low priority.


### Drift Bugs (Post April 1 Hack)

Because Drift (now Velocity) does not anticipate restarting the now-suspended Drift program and will
instead launch at a new address, issues in the current Drift implementation are out of scope (issues
that currently affect production are always in scope, but note that all Drift banks are in a
non-operational state). Once Velocity deploys the new program and makes it public, we will update
our integration accordingly, and the bounty becomes valid again as soon as the first new Velocity
bank is opened in production.

### Zero-weight Assets can be Seized in Classic Liquidation

In classic liquidation, the liquidator can seize assets with zero weight, which some may consider
unusual since they are "worthless" as collateral. From a solvency perspective, this is a non-issue,
since the liquidator must repay a real debt with non-zero liability weight, and in doing so improves
the platform's solvency. In most instances, this is a good economic outcome for the user being
liquidated too, as the liquidator must still repay asset_value * (1 - fee) in debt, but regardless,
liquidation is more concerned with solvency of the platform. Issues when liquidating assets with
zero (or negligible) weight are only in scope if they affect platform solvency.

Note that receivership liquidation cannot capture zero-weight assets, because it does not tie the
proportion of assets seized to the amount repaid like classic liquidation does, and this can lead to
more unfavorable outcomes for the user.

### Zero-weight Assets can be Seized in Orders

When a user places an Order with an Isolated bank (which are considered "worthless" as collateral):
the keeper taking the entire amount would be in line with our expectation. Here if the user is
lending $100 A (not isolated), lending $X B (not isolated) and borrowing $50 C (not isolated), then
places a B/C SL at $50, we expect the keeper will immediately fill if X > 50, and decline to fill if
X < 50. Isolated assets often have no valid price due to lack of market liquidity, so essentially
the user is letting the Keeper decide X. Unlike liquidation and deleverage, this is strictly opt-in.
We agree that the vast majority of the time this is a bad idea and users should not do this.


### Bad Fixed Price Settings

Though it is a bad idea, is is valid for the administrator can set a "Fixed" price of 0, which
carries various risk implications. For banks P0 administers and chooses to sunset by giving them a
"Fixed" price, we typically set a nominal price like 10^-6, and set the weights to 0 instead.
Likewise, the administrator setting a foolish price that deviates from the market price is out of
scope.


### Liquidation of Small-Value Accounts is Unfair

The ability to fully liquidate accounts worth **less than $5 net** (capturing unlimited profit) is a
design decision to encourage our liquidators to index and close out low-value accounts. We have
thousands of accounts on mainnet, many of which hold only a small value. Liquidators (like
ourselves) have significant expenses, such as gRPC. To optimize costs, some liquidators might chose
to observe only high-value accounts, which creates protocol risk when many low-value accounts that
are not being observed could go under. For example, liquidators might not hit accounts where their
profit would otherwise be denominated by the tx fee. This profit incentive encourages liquidators to
continue to index and process low-value accounts, and while it is somewhat unfair to the user, the
maximum loss is $5, which can trivially be avoided by simply depositing more than $5.

### Receivership Liquidation uses Equity Weight and TWAP Price

The premium liquidators can collect is TWAP based, since Equity pricing uses the TWAP. This is
intentional: we already cap the receiver's Spot profit by requiring them to improve health. As a
result, there is a profit and incentive implication for liquidators when the TWAP differs from the
Spot price. 

Liquidators must realize their gains at market price. In a market where the spot price is N and the
EMA is N * k (where k > 1), the liquidator cannot be confident they can actually redeem the token at
N, and should therefore be allowed to claim "more" relative to the spot price. Likewise if k<1, the
liquidator should be confident they can redeem the asset for N, and can claim less. Here they may
wait for TWAP to stabilize if they cannot realize a profit at N, which is expected in a down market,
and it becomes a race to see which liquidator can indeed profit at the actual redeemable price
(closer to N * k). The inverse is true for liabilities as well.

The "max fee" is intentionally represented in Equity terms for the above reasons. 

### Permissionless Fee Collection is Dangerous With T22 Fees

Administrators should take caution with T22 assets that have a transfer fee enabled. This can cause
various edge cases such as permissionless fee extraction rounding down, which allows a troll to spam
small-value fee collections that collect less fees than expected. Currently, there are no T22 assets
with fees in prod, and no plan to add any (as no asset with market traction has such fees), so there
is no plan to fix edge cases related to T22 transfer fees.

### Bankruptcy Ignores Uncollected Fees

When bankrupting a bank, insurance typically offsets losses, but uncollected insurance does not. The
admin must remember to run `collect_bank_fees` before bankrupting a bank, and failing to do so can
result in the bank becoming KILLED_BY_BANKRUPTCY slightly before expectations. Because bankruptcy is
rare, and `collect_bank_fees` is permissionless, we categorize this as WONTFIX.