# Important Notice
**DO NOT CREATE A GITHUB ISSUE** to report a security problem. Instead, please send an email to security@mrgn.group with a detailed description of the attack vector and security risk you have identified.

For bugs in unreleased features, such any PR that has yet to be merged into the main branch (which
reflects the code on mainnet at any given time), feel free to leave comments on the PR directly on
Github.

Due to the volume of scam reports, our spam filter is aggressive. Avoid sending links, and only
attach txt, md, or pdf files. If you have not received a reply within 3 business days, try sending
another email with no links or attachments. We do not open tar, zip, etc or click links.

# Bug Bounty Overview
Project 0 offers bug bounties for the on-chain marginlend program code. Bugs related to other parts of P0's infrastructure (networking, UI, SDK) are marked below.
​
|Severity|Bounty|
|-----------|-------------|
|Critical|2% of the value of the hack, up to $500,000. Minimum $50,000|
|High|$10,000 to $50,000 per bug, assessed on a case by case basis|
|Low-Medium|$1,000 to $5,000 per bug, assessed on a case by case basis|
|Info/WontFix|$50 minimum, assessed on a case by case basis|
​
The severity scale is based on [Immunefi's classification system](https://immunefi.com/immunefi-vulnerability-severity-classification-system-v2-3/). 
Note that these are simply guidelines for the severity of the bugs. Each bug bounty submission will be evaluated on a case-by-case basis.

Bounties are only valid if they affect mainnet. Bugs in the most recent code marked for "release" or "pre-release" (https://github.com/0dotxyz/marginfi-v2/releases) are also eligible for bounty. Bugs that are related to an upcoming update to an integration (e.g., a release-flagged or main-branch update to SVSP, Kamino, Juplend, etc that could lead to a bug on our end when deploying to mainnet) may also be valid, but are assessed on a case by case basis and strictly first-come. Bugs affecting a flag/feature that is not in use in production will always be considered Medium or below. Bugs that are already fixed on an upcoming release are not eligible for bounties even if they are still present on mainnet.

## Infrastructure Bug Bounties

Bug bounties for infrastructure components apply only to actively supported Project 0 production
assets. Deprecated marginfi applications and infrastructure are excluded as described below.
Eligible infrastructure reports are first-come, first-served, and bounty amounts are determined by
the team based on severity and demonstrated production impact.

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
You should expect a reply within 1 business day with additional questions or next steps regarding the bug bounty. For Medium severity and under our reply may take up to 1 week.
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
11. Vulnerabilities affecting only deprecated, archived, retired, staging, development, or
    otherwise unsupported applications and infrastructure (see below).

## Exclusions For Deprecated Marginfi App, SDK, and API Systems

This scope clarification applies to reports submitted on or after August 6, 2026.

The marginfi frontend application, website, sdk, and api were sunset in mid-2026. Note that this
does affect the program, which continues to power Project 0, and is eligible for bounties.

The following are explicitly out of scope:

- The deprecated marginfi web application (i.e. https://app.marginfi.com, marginfi.com, etc),
  including its legacy UI and API routes.
- Deprecated or archived marginfi client applications and source repositories, including `mrgn-ts`
  (see https://github.com/0dotxyz/p0-ts-sdk for the current, eligible sdk) and `marginfi-v2-ui` (the
  current P0 frontend app repository is private).
- Retired marginfi infrastructure, including legacy Vercel deployments, serverless functions,
  storage services, Supabase projects, GCP resources, APIs, and documentation sites.

Findings whose impact is limited to modifying, disclosing, or disrupting unused legacy data are out
of scope. An asset being publicly reachable, present in source control or Git history, or displaying
a deprecation notice does not make it eligible for a bounty.

A finding involving a deprecated asset may still be eligible if the reporter demonstrates a direct
impact on an active Project 0 production system, active user funds, non-public user data, or
currently valid production credentials. Reporters must not perform production writes, access user
data, or attempt to pivot from a deprecated system without prior written authorization.

If you are unsure whether an asset is actively supported, email security@mrgn.group before testing.

## Credits

Thank you to the following individuals for bug reports:

* https://github.com/mySebbe for identifying a bug where debts below the zero threshold can remain
  on the books after a borrow, enabling the extraction of assets where 1 satoshi/lamport/atom is
  worth more than ~1/10 of the Solana tx fee. 

* https://www.linkedin.com/in/tonmoy-bora-28861a384/ for identifying a bug where LST assets with low
  staked liquidity could be artificially manipulated to provide more borrowing value by donating SOL
  to their stake pools.

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

We are aware that pause state, global fees, etc can go out of sync if a group doesn't propagate the global
fee state in a timely fashion. It's incumbent on the group admin to propagate global fee state
settings. When we change fee state settings, we propagate to the main group in the same tx. Third
party groups can opt in to be included in that process (reach out to us if this interests you) or
propagate on their own. Any issues that deal with someone forgetting to propagate are out-of-scope.

The same applies to state we mirror from integrated protocols. For example, when Kamino puts a whole
lending market into emergency mode, our Kamino banks in that market keep full borrowing power until
someone calls the permissionless `propagate_kamino_market_emergency` (and likewise stay restricted
after Kamino resumes, until it is called again). Reading the market account on every risk action
would be too costly for this edge case, so our response time is bounded by our monitoring. Issues
that rely on this propagation window are out-of-scope.

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

### PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG Enables Attacks Around Bankruptcy Ordering

None of the 200+ production banks under group `4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8` have
this flag set (nor will they).

When `lending_pool_handle_bankruptcy` is permissionless, a party can benefit by ordering their
withdraw before handling bankruptcy for another user. This is by design: participants in a group
where banks use this flag are responsible for clearing other users' bankruptcies in a timely fashion
(for example, the design of Arena was to crank bankruptcies on the back of other user interactions).
The cranking party can, if they are greedy, "dodge" the socialization of the bankruptcy this way. 

In practice, we do not recommend setting this flag on any production bank unless the goal is
explicitly to perform no first-party risk management whatsoever.

### Bankruptcy Orphans Worthless Balances

Balances with no value (e.g. zero weight, Isolated positions) can be orphaned (becoming permanently
inaccessible) if bankruptcy is performed on an account. This is by design: assets that have no
collateral value should not block writing off bad debt, which is a high-priority task (when
necessary, which is rare). In a future update, we expect to add a "forced withdraw" instruction to
deal with e.g. forcing users to reclaim a zero-weight position. Users impacted by this edge case
before the implementation of that ix would be reimbursed for orphaned positions OTC.

### Dodging Liquidation Possible by Spamming New Accounts With Transfers

Attackers can try to avoid liquidation by repeatedly transferring their account, which requires
receivership liquidators to create a new record. 

Our mitigation is the transfer fee. While it is true that an attacker can grief liquidators by
repeatedly creating new accounts via transfers, which in turn requires liquidators to create a new
record each time, the attacker must themselves pay the transfer fee each time they do this. The
liquidation record creator can eventually reclaim the rent they paid for the record, so they
ultimately lose nothing, while the attacker loses the transfer fee forever, as well as locking up
considerable rent in accounts if someone does land a record/liquidation before they can transfer.

We don't want to block accounts that have an active liquidation record from initiating a transfer
because a transfer is often a "my wallet got hacked and I need to move my account right now" event
whereas closing a record requires the account to be in good standing for some time.

As such, we consider this issue sufficiently mitigated by economic realities.

### Breakers Use Spot Instead of EMA/TWAP Prices

This is by design: the live price is more reactive than the TWAP/EMA, and the breaker must fire as
soon as possible. A return to normal pricing (i.e., nearly flat TWAP) will allow the breaker to
reset before an escalation, causing minimal disruption. 

### PT Token Sy Rate Can Desync, and PT Emergency Mode May Block the Bank Entirely

We trust Exponent to make a timely update to their Vault's sy rate (triggering our oracle failure
state). Even if they fail to do so, the underlying oracle should react to the loss, so there is no
immediate solvency risk.

Ensuring a fresh sy rate for all PT tokens requires us to pass the extra sy account or to mandate an
Exponent refresh in the same tx: both of these are too cumbersome just to deal with this edge case,
so we make the tradeoff of allowing, in the event Exponent does not make a timely update, a
potentially stale sy rate.

We use the sy rate to determine when to trigger Exponent "emergency mode". Exponent's emergency mode
is a new feature (as of late Sept 2026) and our current implementation is proactively assuming
assets in this state cannot be priced from normal Oracles. The feature would only take affect if the
underlying asset has lost value. For a traditional LST this is generally impossible barring some
black swan even like loss of underlying stake. For something like Hyusd, this can occur if a loss in
Hyusd causes it to redeem for less than $1. 

The issue is that Exponent tokens are essentially locked until maturity. In the event of a loss, the
future value would likely be valued below the usual value, for example hysd now worth $0.7 might be
worth $0.5 as a PT, as holders price in the increased risk of further holding. Since PT liquidity is
limited, there is no good way to estimate the market's actual response. Our plan is to set a fixed
oracle price in the event this occurs, so we currently have the oracle hard-fail until we are able
to review. 

Our use of the feature is still evolving, so we may relax liquidations during the initial oracle
failure or enable them just for the risk manager.

### Pairs of Banks With Very High Leverage May Block Classic Liquidation

When a pair of banks has enabled high-leverage borrowing (usually due to emode boosts) it may put
them into a state where the liquidation premium + insurance fee is too high to liquidate that pair
using the classic liquidation instruction. As the default fees are 2.5% and 2.5% (5% total), this
typically occurs when the banks have a 0.95 weight ratio or roughly a 20x leverage.

Banks are only placed into this state if the risk is liquidation is exceedingly low or technically
impossible, for example LST/SOL, Stable/Stable,  PTToken/Underlying asset, etc.

Receivership liquidation is still functional in this state, so there is no risk to solvency. We also
anticipate lowering either the liquidation fee or insurance fee for some of these pairs to make
eligible for classic liquidation again.

### Interest Can Resume in CircuitBroken State

Interest is suspended during breakers from the start to the end of that breaker. After the "final"
T3 breaker trip, when the bank formally enters "CircuitBroken" state, the duration is now
indefinite, so there is no given end time. We pause interest only until the end of the last
fixed-duration breaker which triggered before the bank entered that state; interest then resumes.

Our rationale is that the admin should not completely block interest by merely being too slow to
unlock the breaker. Short and temporary interest suspension is fair, but letting interest be
suspended indefinitely during an unbounded break is not fair. If the admin is lax in resolving the
breaker, borrowers should not enjoy a large windfall. In normal operation, we do not anticipate
breakers being live for longer than a few hours at most, but the goal is to resolve all
"CircuitBroken" states before the final T3 breaker would expire anyways, which would render the
interest question moot. If we do not, then interest resumes, which is intentional.

### When PROGRAM_FEES_ENABLED, Turning Off the Flag Does Not Remove Fees

This minor bug is currently WONTFIX: there is only a single group with notable TVL in production,
and we manage it internally, so there have not historically been any program fees regardless. Even
if the fees were enabled, they would currently go to the same actual owner in practice (the current
program administrator).

### Healthy Accounts Do Not Automatically Clear Liquidation Tags

The liquidation tagging system must work this way by design because we cannot determine if an
account becomes healthy again unless someone runs a pulse, and we cannot compel liquidators to pulse
regularly, as that would be uneconomical. 

The result is indeed that even if the account becomes healthy again, the tag is not automatically
cleared; only a healthy pulse can do that. The tag continues to accumulate up to 100% even if the
account becomes healthy, unless someone pulses it. 

"Friendly" keepers (like the one we run) will try to pulse healthy accounts to remove their tags.
Users can also always pulse their own accounts. Liquidators indeed generally won't pulse, even if
the account is healthy, because they profit from the growing premium. The alternative is to require
periodic unhealthy "hearbeats" to maintain the tag, but again, this is probably uneconomical. We may
consider it if friendly keepers become vastly outnumbered by greedy liquidators and
healthy-but-tagged accounts consistently do not get cleared in a timely fashion.

The above also applies to repayments or new deposits: though they make the account healthy again,
they do not clear the tag, only a pulse can do that. Make sure to pulse if repaying/depositing on a
currently-unhealthy account.

### Keepers Can "Bump" Rates With Their Own Funds to Steal Rebalance Tips

A keeper moving funds from bank A -> B as part of a rebalance can manipulate rates by using their own money (not the user's) to  (1) withdraw from B, (2) borrow from B, (3) deposit into A, (4) repay debt from A. The user's rate then improves, and they keep the tip. This satisfies the covenant lenders make with Keepers from our point of view: the lender's rate genuinely improved to the level they approved. Doing this also isn't free to the Keeper (e.g., they might pay interest to B), and is likely not economical given the relatively small value of tips compared to the large amount of funds that would be required to actually affect rates.

### Tiny Keeper Caps Can Lead to Small Moves Earning the Entire Rebalance Tip

Users should not footgun themselves by offering, for example, a 0.001 SOL tip on a 0.001 USDC move. Make sure the tip is proportional to the smallest move you are willing to accept for that tip amount, and set the tip/cap accordingly. The program does not enforce rational tips, as it would be too cumbersome to check the exchange rate of SOL/rebalance asset.

### Kamino/Exponent Emergency Suspends All Interaction With Those Banks, Including Liquidation of Other Banks

This is a trade-off we have deliberately made because (1) we assume Kamino/PT emergencies are either
rare or transient, (2) there is no viable and safe automatic valuation of Kamino/PT banks in
emergency mode.

Not all emergencies are equal: if Kamino is hacked like Drift, then the appropriate price is zero.
If they merely paused out of caution, then the appropriate action is probably to wait and see.
Generally, we would not want to assign a price until it is manually reviewed, hence the emergency
pause creates a hard error. We therefore intentionally refuse to price the bank automatically and
allow the resulting hard error rather than treating a stale reserve state as trustworthy.

If, after manual review, governance determines that the bank should remain priceable, it can migrate
the bank away from the Kamino oracle path and configure a regular conservative Fixed price. This is
an explicit emergency-response decision, not an automated fallback.

### Staked Collateral Banks Have Unlimited Deposits

Because creating staked collateral banks is permissionless, the deposit caps on them are merely a placeholder, nothing prevents anyone from creating multiple banks for a single validator. The caps on staked collateral banks are merely a legacy artifact and not meant to enforce a limit.

### Circuit Breaker Halts Can be Late and Deposits May Continue Until a Risk Operation Executes

Deposits are blocked when a circuit breaker (CB) trips, however they do not themselves trigger breakers. This means if a breaker is priced to trip but hasn't tripped yet, deposits are allowed until the breaker is actually tripped by some risk-bearing operating. This is acceptable by design: deposits reduce risk, and an attacker cannot take advantage of a deposit -> inflate price -> borrow loop because the borrow will trip the breaker.

The same applies to repay.

### Classic Liquidation Can Generate Bad Debt

One report provided this example, noting that liquidation in this instance generates bad debt, even
accounting for insurance:

```
Scale 1x (Equity $20):
  Before price move: 200 SOL at $10 = $2,000 collateral vs $1,500 debt.
  After price move to $7.60: $1,520 collateral vs $1,500 debt (Equity
+$20, Maint Health -$132).
  Liquidation: Seize 150 SOL ($1,140). Liquidator pays $1,111.50
(profit $28.50). Insurance gets $28.50.
  After liquidation: $380 collateral vs $417 debt. Equity -$37.00 (Bad
debt). Net shortfall: $8.50.
```

Here we note that equity is lost when the price of SOL is 7.6: this is true. However, the position
was actually liquidatable at SOL ~= 8.333, it begins to generate bad debt when the price falls below
SOL ~= 7.895, and it begins to generate bad debt that exceeds insurance at SOL ~=7.692. Of course,
we prefer liquidators act as quickly as possible when the price is still higher; this prevents bad
debt from being generated.

If liquidators are failing to act between 8.3 -> 7.6, then one of two things is probably true (1)
the price moved too quickly for them to react, (2) liquidators did not want to take this position
because of market conditions. Because both of these signal a dangerous market for the asset, it is
preferable for a liquidator to clear debt from the books, even at the expense of minor bad debt,
because once the value of the SOL drops below 1500 (at SOL = 7.5), the protocol is exposed to bad
debt up to the full 1500 (if it continues on to $0)! This is indeed a net loss for lenders compared
to non-action if SOL were to briefly dip to 7.5 and then recover back to 7.692+, but that is not
typical. Putting it another way, it is certainly better for lenders to have the SOL liquidated at
7.6 with a net equity loss of $18 than if SOL drops to $7, resulting in $100 of bad debt that must
be socialized.

In other words, Liquidation does not necessarily guarantee protocol equity improves when the account
is very unhealthy, at those levels the service of liquidation is clearing bad exposure before it
becomes worse.

If wa/wl ~= our .95 limit, e.g. .9499, then the buffer zone before the liquidation incurs bad debt
is very small (0.01%), but here the insurance fee brings it up to 2.51%.

### Liquidators Can Leave Repay Dust

In some edge cases, classic liquidation can enable the liquidator to repay while leaving a small
amount of liability dust in their own account. This is extremely difficult to do on purpose, since
the repay amount is not known at call time. It is also not scalable since it requires a valid
liquidation target to achieve. Such dust may be settled into bad debt at account closure, but is
typically so nominal it does not even reach 1 token-atom.

### Receivership Liquidators Can Avoid Clearing Liquidation Tags By Firing In Small Clips

The liquidation tag doesn't reset if less than 25% of the unhealthy balance was repaid. Liquidators
might then try to liquidate a user in sub-25-percent-clips to avoid resetting their tag and allowing
the premium to keep growing.

This is by design and mitigated by market forces. There are many third party liquidators. Only one
liquidation (per sender) can typically land in a single tx due to tx limits (though bundles can land
several, at a higher cost). A "greedy" liquidator attempting to liquidate in sub-25% clips to avoid
tag-resetting will inherently race against a greedier liquidator that wants the entire eligible
amount. Most liquidators simply want to optimize their profit within one tx: liquidators attempting
to "save an account for later" will compete against that profit motive and likely fail.

We cannot allow the tag to be cleared by a liquidation of any size (or the eligible user would just
liquidate themselves for 1 lamport at regular intervals). Tracking cumulative progress is too
cumbersome because the amount eligible for liquidation also fluctuates with prices.

### Dust Balances Can Be Skipped During Orders/Liquidation/etc

When a Balance is below the EMPTY_BALANCE_THRESHOLD (1 share), start/end flows such as receivership
liquidation and Order fulfillment ignore that Balance. This means the executor/keeper can
theoretically withdraw them. However, balances below the dust threshold are usually worth less than
1-token-atom and are generally not transferable. We can consider the worst-case where the dust has
above-threshold value: for BTC (with 8 decimals), assuming EMPTY_BALANCE_THRESHOLD is 1 share,
assuming the user has .99999999 shares, and asset_share_value > 1, the attacker can claim 1 satoshi,
which is around 10 cents in October 2026. Essentially the keeper can take up to asset_share_value
satoshis, so if the asset_share_value is >2 then they can claim 2 and so forth. The
asset_share_value rarely exceeds 2 even on banks that have sustained interest for several years.


### Some Mathematical Protections Break When Share Value > 2^48

There are certain protections that break down when the asset or liability share_value exceed 2^48.
While reports regarding these are generally bounty eligible, they are capped to WONTFIX/INFO status
unless paired with a mechanism that can reasonably raise the share value to this threshold, which
currently would require centuries at max interest. One such bounty-eligible report involved emission
donation, which is now prevented from raising the asset_share_value beyond a certain threshold for
this reason.

### Large Confidence Bands Can Walk Circuit Breaker Windows Up Faster

The long-window deviation check subtracts the oracle confidence from the measured price move.
Assuming a confidence of just under 5%, one 24h window admits a move of up to 1.03 / (1 - 0.05) =
+8.42% above the window reference with zero circuit breaker trips. A clean pulse then re-anchors
this window.

We consider this an acceptable design tradeoff: sustained poor confidence is rare, and the effect is
minimal. The circuit breaker feature is intended to capture large moves, and walking up slightly
more than would perhaps be expected, only during times of poor confidence, is acceptable. 

### Empty Fee Pool Lets Keepers Incorrectly Earn Tips

Alice funds a fee account and opens a rebalance Order. Bob fills it, escrowing a tip T. Alice then
closes her fee account, returning the rent R. When Bob goes to settle his tip, let's assume he
didn't earn it (rebalance failed to get a better yield), it should now be returned to the fee pool.
The fee pool needs R, the rent, to be reopened. To avoid Alice griefing in this manner to steal R
from the Keeper, the Keeper gets to keep T instead -- this is Alice's fault for draining her fee
pool. At some point we may instead reopen the fee pool if T > R, but the current behavior
discourages trolls from closing their fee pool.