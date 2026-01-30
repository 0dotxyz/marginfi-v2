# Bankruptcy Guide

Want to learn more about how bankruptcy works on mrgn? Read on.

## Key Terms and Kinds of Bankruptcy

* User bankruptcy - When a user's liabilities exceed their assets, before accounting for weights,
  they are eligible to be bankrupt. A user is technically bankrupt after all their remaining assets
  are liquidated, at which point they will have 0 assets and a non-zero amount of liabilities. All
  bankruptcies are technically triggered when USERS go bankrupt. Some corollaries and notable facts:
    * Any time a bank is bankrupt, at least one lender in that bank is bankrupt.
    * When taking an eligible user into bankruptcy, if a user has several different liabilities,
      liquidators get to pick which ones will stay on their books, which means they get to pick
      which banks will absorb the bad debt.
* Bank bankruptcy - Banks where at least one user has bad debt in the manner described above are in
  a light state of bankruptcy. 
* Bank super-bankruptcy - Banks where the amount of bad debt outstanding in the manner described
  above exceeds the bank's assets are in a state of super-bankruptcy.


## How Does it Happen?

Bankruptcy is rare. It can only occur in one of the following circumstances: (1) liquidators haven't
been running properly, e.g. due to congestion, etc, (2) an asset is too illiquid for liquidators to
safely clear, (3) an asset's price changes drastically, before liquidators are able to respond. 

## Discharging a Bankruptcy

First, liquidators consume all the remaining assets that the user has. If the user has A dollars in
assets and B dollars in liabilities (in equity value, i.e. excluding any weights), we know that B >
A. After liquidation is complete, A_new = 0, and B_new = B - A + X, where X is the liquidation
premium and insurance.

Next, the group administrator runs `handle_bankruptcy` on the user. For banks where
`PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` is enabled, anyone can do this. This will perform the
following logic:

* If bank's insurance fund > liabilities, then the insurance fund is used to repay the user's liability.
* If the bank's insurance fund is not sufficient, the remainder will be covered by taking liquidity
  out of the bank, reducing the asset share value. This socializes the loss to all remaining
  depositors.
* If the bank's insurance fund and liquidity are not sufficient (super-bankruptcy), the bank is
  killed. The asset share value is set to zero, wiping out all holdings for all other depositors.
  This state is irrecoverable, and the bank is permanently disabled.

## FAQ

### What Happens if it Doesn't Run?

If Bankruptcy isn't executed on a bankrupt user, then remaining depositors can never withdraw the
whole balance in the bank. The last few depositors who try to withdraw will find there are not
enough funds - proportional to the liabilities held by bankrupt users.

### When Does This Matter?

Ideally, never. As of November 2025, bankruptcy has never been executed in the main pool.