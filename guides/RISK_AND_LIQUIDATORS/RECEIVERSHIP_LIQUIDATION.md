## Summary

Receivership liquidation is what we call the `start_liquidation` and `end_liquidation` instructions.
Classic liquidation uses `lending_account_liquidate` instead.

In this approach, the liquidator takes control of an unhealthy account, enabling them to withdraw
and repay as if they are the user. At the end of liquidation we simply validate that they actually
improved account health.

Within the tx, the liquidator can withdraw, perform e.g. a jup swap, and repay the debt, keeping
whatever is leftover for themselves. This enables to the liquidator to operate without holding any
funds or creating a mrgn account at all.

We use transaction introspection to ensure that every liquidate_start has a corresponding
liquidate_end.

The process generally looks like this:

```
* ComputeBudgetProgram (optional)
* Liquidate Start Instruction
* Withdraw A
* Swap A to B via jup or similar
* Repay B
* Liquidate End Instruction
```

## Why Use This Instead of Classic Liquidation?

Classic liquidation requires the liquidator themselves to have an account, and maintain collateral
in it. This approach requires no account and no collateral: you can start liquidating with zero
dollars (as long as you have SOL to pay gas fees).

Maximum profits are higher with receivership liquidation, up to 10% as of November 2025, where
classic only nets the liability bank's configured liquidator fee (2.5% by default). The maximum
profit is defined as:

```
Seized <= Repaid * (1 + max_fee)
```

Where Seized is the equity value withdrawn, in \$, Repaid is the equity value repaid, in \$, and max
fee is the maximum allowed profit currently configured at 10%. Note that equity value is the price
of the token without any weights applied, but inclusive of oracle confidence interval adjustments.

No exposure to slippage or illiquid markets if swapping within the same tx: if there is no
profitable route between A and B, simply abort the entire tx.

## Notable Rules:

- ComputeBudgetProgram ixes and Kamino refreshes are the only kind of instructions that can come
  before Start.

- There must be exactly one liquidate Start, and it must come first in the tx (other than Compute
  Budget)

- There must be exactly one liquidate End, and it must come last in the tx. No other instruction can
  come after End.

- The only other mrgn instructions that can be called are Withdraw and Repay.

- Account health must improve between Start and End. All health checks are ignored during withdraw
  and repay: only the final health matters. This means you can withdraw BEFORE repaying.

- The liquidator cannot profit more than percentage configured by the global fee admin (default
  10%), see `FeeState.liquidation_max_fee` for the current max profit.

- The liquidator pays a nominal fee in lamports to use this service. This is automatically billed to
  the global fee wallet at End. See `FeeState.liquidation_flat_sol_fee` to see the fee in lamps.

- The first liquidator to use this service for any account first inits a "liquidation record", and
  pays a nominal amount of rent to create that record (records 1:1 for that account). This record
  stores historical information about the last few liquidation events on that account. Currently, the
  rent for this account cannot be recovered.

- The liquidator can freely fully withdraw or full repay positions, but cannot close them. A
  position withdrawn or repaid in full therefore still requires passing the oracle for that position
  at End, unlike a `withdraw_all` or `repay_all` in normal circumstances.

- The liquidator can withdraw/repay from as many positions as they'd like, but cannot open new
  positions

- Isolated positions, positions with an asset weight of zero, and positions with a price of 0 cannot
  be withdrawn during liquidation.

- Liquidators can opt to withdraw or repay from as many positions as they wish, provided the above
  rules are adhered to.

- Liquidations are not inherently profitable: a liquidator can repay a debt without claiming any
  asset! They can also do nothing at all!

- If liquidating an account with Kamino positions, don't forget to include the refreshes. Also
  remember that Kamino positions use a different withdraw instruction.

  ## Tips and Tricks

  Maintaining a small balance of various assets reduces the need to swap within every liquidate tx.
  It can be more slippage efficient to swap outside the liquidate tx, when the liquidator can wait
  for a more favorable rate.

  A large pool of LUTs is typically required to complete a liquidate and swap within the tx size
  limits, especially when the account is large.

  Profit is ultimately defined by slippage: a liquidator looking to optimize profits will pick asset
  A and liability B such that swapping A->B, and swapping A->liquidator's preferred currency, incurs
  minimal slippage. For example, SOL -> LST and SOL -> USDC are typically highly liquid and have
  minimal slippage. A liquidator that prefers to keep holdings in USDC might prefer this pair over
  more volatile assets the user is holding.
