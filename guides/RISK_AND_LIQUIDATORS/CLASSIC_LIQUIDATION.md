### Disclaimer

Try Receivership Liquidation instead: the process is simpler, and the potential premium paid is higher. This page describes the "Legacy" or "Classic" liquidation approach, which we will support indefinitely.

## Summary

Classic liquidation uses the `lending_account_liquidate` instruction. To execute this kind of liquidation, the liquidator must have their own lending Account. The target, also called the "liquidatee", must be unhealthy. The liquidator will assume part of one of the liquidatee's assets, and in exchange, will also take on some of the liquidatee's debt in one of their liabilities.

If a liquidator reclaims \$A asset, they only have to assume (1-`liquidator_fee`)\*A in B liabilities. The liquidator keeps this premium.

The liquidatee only receives a repayment on their B debt of (1-`liquidator_fee`-`insurance_fee`)\*A, the additional `insurance_fee` goes to insurance funds.

Both `liquidator_fee` and `insurance_fee` are read from the **B (liability) bank**'s config (`liquidation_liquidator_fee_bps` / `liquidation_insurance_fee_bps`), each defaulting to 2.5% when left at `0`. So at the defaults a liquidator keeps a 2.5% premium and another 2.5% goes to insurance, but a bank can be configured to charge more or less per side (capped at 50% each).

### Common FAQs

<details>
<summary>How do I pack remaining accounts for this instruction?</summary>
See the tests in 10_liquidate, as well as e04, k10, m01, m02, s08 for various examples of liquidation with different account configurations.
</details>

<details>
<summary>Do I have to crank all of the Liquidatee AND Liquidator switchboard oracles prior to the liquidate instruction?</summary>
Yes, no stale oracles are permitted. Typically, you will send a crank tx just before the liquidate. Some operators prefer to use a bundle for this task, but sending a tx just prior to the liquidate (sometimes with a small delay) typically suffices.
</details>

<details>
<summary>How do I know when accounts are eligible to be liquidated?</summary>
Most liquidators will try to keep a running inventory of all the accounts that exist above some dollar threshold, and all the prices of the various assets they are involved with. This is a considerable endeavour with 500k accounts!
</details>

<details>
<summary>How do I know how much asset to claim?</summary>
Most liquidators will set this number conservatively, since prices may change by the time liquidation lands. Since liquidation cannot raise health above zero, you cannot seize too much, or the instruction will fail. Consider starting around 70-80% of the amount health is actually negative.
</details>

<details>
<summary>I landed a liquidation, now what?</summary>
Most liquidators will try to rebalance their inventory as soon as possible: withdraw the asset, swap to debt, and repay the debt. Some liquidators will even attempts to do this within the same transaction as the liquidation, though this isn't always possible with larger accounts due to TX size and compute limits. Note that taking on a liability carries risk: there's no guarantee that a profitable swap route exists to exit that liability.
</details>

<details>
<summary>Can I use a flashloan in liquidation?</summary>
Sure, the liquidator can use a flashloan to avoid actually keeping any collateral on hand. A liquidator taking this approach will typically try to repay the debt it takes on before the tx ends. The entire tx will looking something like:

```
start flashloan
liquidate user
withdraw asset A from liquidator
swap asset on e.g. jup, into the debt B
repay the debt B
end flashloan
```

In practice, this is considerably difficult to pack into a single tx.

</details>
