
# Risk Introduction

Want to learn more about risks in borrow-lending in general, and how we manage them? Read on!

## Under-Collateralized Lending

Borrow-lending platforms allow the user to borrow less than the amount they lend. For example, if
you deposit \$100, you will never be able to borrow more than \$100 against your deposit.

### LTV

The Loan-To-Value ratio (LTV) is simply borrows/deposits. For example, if you lend \$100 and borrow
\$50, your LTV is 50/100 = 50%.

Some Borrow-Lending platforms will refer to "Weight" as the LTV: these are similar concepts, but not
entirely the same. When you deposit \$100 of token A, an "Asset Weight" is applied. This weight
discounts your asset, your \$100 deposit with a weight of 90% is only worth \$90. Likewise, a
"Liability Weight" applies to the assets you borrow, the \$50 you borrowed with a liability weight
of 110% is actually worth \$55. 

Summarizing the above scenario, your \$100 deposit and \$50 borrow have a real LTV of 55/90 = 61%.

```
(Asset weight for A)/(Liability weight for B) = LTV for lending A, borrowing B
```

Most borrowing lending venues (including ours) publish only the "Asset" weight on the user-facing
front end. The "Liability" Weights, while publicly available, can be confusing. This means that
when lending/borrowing some pair A/B, you may end up with a HIGHER LTV on a platform that shows a
HIGHER asset weight, if the more-hidden liability weight is LOWER. 

For example, if venue K shows A has a "90%" weight, and platform P shows A has a "85%" weight, when
borrowing B against \$1 of A, you might assume Platform K will always win in LTV. Not neccessarily!
If K has a Liability Weight of 1.2 for B, while P has a Liability Weight of 1.05, P wins!
```
Venue K: 1.2/0.9 = 0.75 
Venue P: 0.85/1.05 = 0.81
```

### Maintenance vs Initial LTV

Each asset actually has two Asset and Liability Weights: The "Initial" weight and the "Maintenance"
weight. When you borrow, our platform checks that you have enough collateral using the "Initial"
weight. 

When your LTV falls below the "Maintenance" requirement, your account can be liquidated. The
"Maintenance" value of an account is called its "Health". We call the values between the Initial and
Maintenance weights the "Health Buffer" or "Liquidation Buffer".

Some borrow-lending venues publish only their "Maintenance" Weights, while others publish just their
"Initial" Weights. We prefer the latter, you can see the former reflected in your "Health" on the
portfolio page.

Example:
```
Token A: Asset Weight Initial = 90%, Asset Weight Maintenance = 95%
Token B: Liability Weight Initial = 110%, Liability Weight Maintenance = 105%

Susie deposits $100 of Token A
    Susie's deposit is worth: 100 * .9 = $90 for Initial (borrowing) purposes.

Susie wants to borrow as much B as possible
    The max borrow of B is: 90/1.1 = $81.82

For Maintenance purposes, Susie has 100 * .95 - 81.82 * 1.05 = $9.089 left in liquidation buffer.
````

Now let's imagine Asset A drops in value by 5%, and B stays the same:

```
Susie's deposit in A is now worth $95

For Maintenance purposes, Susie has 95 * .95 - 81.82 * 1.05 = $4.339 left in liquidation buffer.
````

Now let's imagine Asset A drops in value by 10% (net), and B stays the same:

```
Susie's deposit in A is now worth $90

For Maintenance purposes, Susie has 90 * .95 - 81.82 * 1.05 = -$0.411, Susie can be liquidated!
````


# Liquidation

Most borrow-lending platforms, including ours, offer Partial Liquidation. In a nutshell, this means
the liquidator repays some of your debts and withdraws some of your assets, making the account more
healthy. In exchange for performing this service, the liquidator gets to keep some of the user's
funds as profit: we call this the "Liquidator Premium". A portion may also be paid out to the
insurance fund. Typically, the liquidator premium and insurance fund payout is a few percent in
total. Let's go back to the example with Susie:

```
Token A: Asset Weight Initial = 90%, Asset Weight Maintenance = 95%
Token B: Liability Weight Initial = 110%, Liability Weight Maintenance = 105%

Susie's has $90 deposits in A, and $81.82 borrowed in B

For Maintenance purposes, Susie has 90 * .95 - 81.82 * 1.05 = -$0.411, Susie can be liquidated!

The liquidator withdraws $1 of A, repays $0.95 of B.
Susie now has $89 deposits in A, and $80.92 borrowed in B

For Maintenance purposes, Susie has 89 * .95 - 80.87 * 1.05 = -$0.3635
````

Notice that Susie's account is now less unhealthy. On our platform, liquidations cannot make an
account healthy again, i.e., a liquidator can only bring a user's health up to 0, at most.

At any point, Susie could withdraw/repay herself, avoiding paying the liquidation premium!

In the above example, the liquidator collected a premium of \$0.05 (\$1 withdrawn - \$0.95 repaid).
