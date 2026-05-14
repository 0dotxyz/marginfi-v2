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


## Known Issues and Scope Clarifications

### Solend Not Supported in ....

We are aware that e.g. Solend withdraw is not yet supported during e.g. receivership liquidation, this was a deliberate choice to limit cpi exposure while there are not yet any Solend banks in production.

The legacy liquidate instruction continues to support all bank types, including Solend, so there is no risk of bad debt even if Solend banks were to be added before we added Solend to the receivership allow list. We will add Solend instructions to the allowlist for other instructions if/when a Solend bank appears in production.

Any instances of Solend missing from a whitelist are out-of-scope.

### T22 Extensions

Adding banks is an administrator function, and we do not make program level assumptions about which (if any) of these T22 features the admin might tolerate. In cases where an asset is highly trusted (e.g. PYUSD), an admin may still determine listing is viable even though it has Transfer Fee and permanentDelegate extensions enabled. Regarding transfer hook, again it is on the admin to ensure the usage is safe (e.g. PUMP).

In summary, the program will not validate these extensions are disabled, we leave it to the admin to decide if they tolerate the associated risk, and the inclusion of these extensions is out-of-scope.

### Staked Collateral Price Confidence

Confidence bands on Staked Collateral oracles are currently priced incorrectly, slightly over-valuing Staked Collateral positions. Because Staked Collateral positions can only borrow SOL, they will never be liquidated unless the SOL borrow sustains above the native stake yield for a long period of time (weeks to months). Even if they are modestly over-valued during times of low SOL price confidence, these Staked Collateral positions would still be liquidated well before they went underwater. We have marked this Info/Low and expect a fix in ~1.9 (roughly late April).