# marginfi v0.1.12a

# Summary

## Lower Liquidation Tagging Premium Cap

The maximum receivership liquidation premium reached through tag growth is reduced from 100% to
25%. At the new ceiling, a liquidator may seize up to $1.25 of collateral for each $1 of debt repaid.

The timing stays the same: the premium remains at its base for the first hour after tagging, then
grows linearly to the ceiling at 7 days. With a 5% base, the premium is now 15% halfway through the
growth window.

Configured base premiums above 25% continue to apply. The existing exceptions for accounts in bad
debt and accounts boosted by e-mode are unchanged.

Existing tags use the new curve after the program upgrade. No account migration or instruction
changes are required.
