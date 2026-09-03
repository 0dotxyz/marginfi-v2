# Summary

An `Order` is a stop-loss and/or take-profit trigger that a `Keeper` can permissionlessly execute.
When creating an Order, users choose an asset pair (a lending asset and a borrowing asset), a
trigger point to execute the order, and the type of order (Stop Loss, Take Profit, or Both).

- A `Stop Loss` executes when the pair of assets falls below a certain value.
- A `Take Profit` executes when the pair of assets goes above a certain value.
- `Both` allows the user to set a Stop Loss and Take profit threshold in the same Order. (F1)

### Order Execution

The borrow-side position of an Order is always closed in full. The lending position is never closed
(F2). This means if you have a \$200 SOL lend and \$100 USDC borrow, and you would like to close
just half of your net LONG position with an order, you will have to create two accounts with \$100
SOL and \$50 USDC each!

### Example

```
A user is lending $100 in SOL and borrowing $50 in BONK. They set a take-profit at $70.
 * SOL goes to $120, the Keeper can execute, closing their BONK position and leaving them with $70 in SOL.
 * Alternatively, a Keeper can also execute if BONK falls to $30, likewise leaving the user with $70 in SOL.
 * Any combination of SOL going up or BONK going down that leads to a net-value of $70 can make the Order eligible to execute!
```

## Interest Triggers

An Order can also carry an **interest trigger**: exit when the position's carry turns negative and
stays negative. This is aimed at strategies, where the point of the position is the spread between
what the lend leg earns and what the borrow leg costs.

```
A user lends $1,000 USDC earning 5% and borrows $900 PYUSD costing 3%, keeping the 2% spread.
 * If the USDC lend rate falls or the PYUSD borrow rate climbs, the spread can flip negative
   and the position bleeds every day it stays open.
 * An interest trigger closes it for them.
```

Two things make this more than "exit when the spread goes negative".

**A blip is not a signal.** Rates are measured as the growth of each bank's share value across a
window you choose (`window_seconds`, 6 to 48 hours, 24 hours by default), which is the average
rate actually realized over that window. A spike lasting an hour barely moves a 24-hour
measurement, so a rate has to genuinely persist to trigger an exit.

**Leaving costs money.** Losing 1% a year does not justify paying 1% slippage today to escape. You
set `exit_budget_seconds` (14 days by default), and a Keeper may only execute when the unwind
costs less than what the position would lose to interest over that span. It is a spending limit
priced in days of loss, not a delay: nothing waits for it. If a Keeper finds a route with
no slippage they can exit the moment carry turns; on an expensive route they must wait for the rate
to worsen or find a better route. Your `max_slippage` still caps the exit regardless.

`min_negative_apr` optionally requires the loss to reach a given annual rate, measured against your
lend leg, before the trigger fires at all. Left unset, any negative carry qualifies.

The variable-borrow premium you pay counts toward the cost side, since it is a real charge that
pushes the spread negative.

### How the Measurement Stays Honest

Every bank keeps its own rolling history of where its share value stood, written by the protocol's
ordinary pricing activity (borrows, withdrawals, liquidations, and a permissionless pulse anyone can
run), at least three hours apart and reaching back at least 48 hours once the bank has been live
that long. Your Order records nothing of its own. When a Keeper tries to execute it, each leg's rate
is read from its bank's history, from the reading closest to your window that is at least a window
old.

Three things follow.

- **It works from the moment you place it.** If the banks already hold a window of history, the
  Order can execute right away. There is no priming step, and nothing for you or a Keeper to keep
  alive.
- **Nobody can move your measurement.** The history belongs to the bank and is shared by every
  Order on it. A Keeper cannot shorten your window, reset it, or pick a flattering starting point.
- **The measurement is exact.** It reads accrued share value, not a rate at an instant, so no single
  transaction can spike or hide anything. Whatever the bank actually charged or paid over the window
  is what the Order sees.

If a bank has been quiet for longer than your window, the nearest older reading is used, so the
measured span can be longer than you asked for, never shorter.

### Interest and Price Triggers Together

The interest trigger is independent of the Stop Loss / Take Profit threshold on the same Order, so
one Order can carry both. That matters for a leveraged staking loop: lend an LST, borrow SOL, and
you face a depeg (a price problem, wanting a Stop Loss) and a borrow rate climbing above the
staking yield (a carry problem, wanting this trigger). Either condition can execute the Order, and
each is bound by its own cost rule.

Before the banks hold a full window of history, your Stop Loss still works normally.

### Fees, and Who Keeps the Keepers

Project Zero will run Keepers initially upon feature public launch (ETA Q2/Q3 2026), but any
third-party can run a Keeper. Users configure their max slippage tolerance when setting the order,
Keepers are permitted to keep whatever is leftover after completing the order execution as profit,
and they also get to keep the rent from the Order (currently worth about $0.25). Keepers can expect
to claim a small profit, especially when executing a Take Profit. 

If you are an integrator expecting to use this feature, you are strongly recommended to run your own
Keepers. Keepers are permissionless, any wallet can be a Keeper.

There is no guarantee that any given Keeper, or any Keeper at all, will execute an Order. Users who
spam Orders or otherwise misuse the Order system may be excluded by Keepers without notice. No
Keeper assumes liability for failing to execute an Order in time. Users should be aware of the tax
implications of using this feature in their respective jurisdiction, Keepers are not obligated (nor
expected to) provide any tax information, receipts of transactions, etc.

## Borrow Orders

A **Borrow Order** borrows on your behalf while a bank's rate has been cheap, and can repay
that borrow once the rate turns expensive. You name the bank, how much you want in total, and the
rate you are willing to pay, and a Keeper fills the order whenever the rate the bank has actually
charged over your window sits under that level. It is persistent: it fills a piece at a time, keeps
count of what is left, and stays open until it has borrowed everything you asked for or you cancel
it. Give it a second, higher level and the same order closes the position again when the rate has
risen over it, drawing the repayment from the bank the borrow was parked in.

```
A user wants to short SOL, but only if the borrow costs under 4% a year.
 * They place a Borrow Order on the SOL bank for 100 SOL, open-below 4%.
 * The SOL bank's rate has sat at 6% for days. Nothing happens.
 * Utilization drops and the rate holds at 3% for the user's window. A Keeper fills what fits.
 * If only 40 SOL fits under 4% before the rate would climb back over it, the Keeper fills 40 and
   the order stays open for the other 60.

A looper wants USDC leverage only while it is cheap.
 * They place an order on the USDC bank for 50,000 USDC, open-below 5%, close-above 12%, redeploying
   into a second USDC bank.
 * The rate holds at 4% for a day: a Keeper borrows, and the USDC lands in the second bank earning.
 * Weeks later the rate spikes and holds at 15%: a Keeper pulls the USDC back out of the second bank
   and repays the borrow. The order stays, ready to open again when the rate falls.
```

This is aimed at strict shorts and basis trades that are sensitive to the borrow rate, and at
loopers who want to hold leverage only while it is cheap enough to be worth it.

### Partial Fills

Borrowing pushes a bank's rate up. A fill is only allowed if the rate it would leave behind is still
under your level, and it must take all that fits: a fill that leaves more than a granule (1% of the
order) of room under the level is refused, and so is one smaller than a granule unless that is all
the order had left. So a Keeper takes what fits, no more and no less. That is the whole
partial-fill mechanism: there is no separate "partial" mode, it simply falls out of the rule. If the
bank has plenty of room, one fill takes everything; if it has a little, the order fills in pieces as
room appears. A bank's borrow limit, its liquidity, and the destination's deposit limit each count
as "nothing more to give", so a fill they cap is still the full one.

One thing the fill rule does not see is your collateral: a fill your account cannot carry is refused
by the health check, and a smaller fill is not accepted in its place. Size `amount` to what your
collateral can support, or add collateral before the rate comes down.

Consecutive fills are spaced by a `cooldown_seconds` you choose (1 hour by default), so an order
does not chase the rate up in a burst of small fills. Opens and closes share the one cooldown, which
also keeps an order from flipping back and forth when the rate hovers around a level. Because every
fill has to be the full one and at least a granule, nobody can start your cooldown with a
token-sized fill.

### Where the Money Goes

You choose one of two destinations when you place the order:

- **A bank of your choosing** holding the same asset. The borrowed funds are deposited there in the
  same transaction, so they earn from the moment they arrive. Borrow USDC from one bank and lend it
  to another whose rate is higher, or use it as collateral for the next leg of a loop. The
  destination cannot be the bank you borrowed from, since one balance cannot hold both sides of the
  same bank, and both banks must be native marginfi banks.
- **Your wallet.** The funds land in your own token account for the asset (your associated token
  account, created for you if it does not exist yet) and the order has nothing further to do with
  them. A Keeper cannot direct them anywhere else: a fill that tries is refused before it borrows.

### Closing Again

Give the order a `close_above_apr` higher than its open level and it gains a close side. Once the
rate the bank has realized over your window has risen over that level, and the rate right now is
still over it, a Keeper may withdraw from the destination bank and repay the borrow with it, in one
transaction. A close must repay all the destination can cover: the whole debt when the deposit
covers it, otherwise everything the deposit holds or the destination bank can pay out at the time
(to within a granule, and all of it once less than a granule is left). It can only repay what the
order itself borrowed, interest included, never debt you took on the same bank yourself, and a
Keeper can never take more out of the destination bank than it pays. Whatever is repaid comes off
the order's count in proportion, so a fully closed order is back to its original amount and opens
again when the rate falls. If you repay some of the borrow yourself, a close still clears the rest
and leaves the order empty.

The close side needs somewhere to repay from, so it requires a destination bank; a wallet order has
handed its funds over and cannot be closed by a Keeper. Setting the close level to zero removes the
close side. The order counts what its own fills borrowed; repaying some of that yourself does not
shrink the count, so update or cancel the order if you no longer want a Keeper closing the rest.

### What a Keeper Can and Cannot Do

While a fill is running, the Keeper holds borrow authority on your account (withdraw and repay
authority for a close). That authority is fenced in on every side: the only instructions allowed in
the transaction are the fill's start and end and its two legs, a borrow from the order's bank plus
(for a redeploying order) a deposit into the destination bank, or a withdraw from the destination
bank plus a repay on the order's bank; every other balance on your account is proven untouched
before the fill is accepted; an open is proven to have left the rate under your level with no more
than a granule to spare, and a redeploying fill to have deposited what it borrowed, not a fraction
of it; a close's repayment is proven to cover what the destination could pay and to stay within
what the order owes, and its withdrawal to stay within what it paid. On a bank charging a variable
borrow premium, the premium a repayment settles counts as paid.

Your account's health is checked once, over the finished position. For a redeploying order that
means the deposit counts as collateral for the borrow that funded it, which is what lets a loop
lever up in a single fill. A close only shrinks the position, so it is held to the maintenance
requirement rather than the initial one.

A fill that moves funds from one marginfi bank to another (a redeploying open, any close) is not
counted against either bank's outflow limits, the same way an auto-rebalance is not: nothing leaves
the protocol. A wallet open is a real outflow and counts like any borrow.

### Paying the Keeper

Placing an order charges the same flat SOL fee as any other order. Each successful open and close
also pays the Keeper the order's `keeper_tip`, in lamports, out of your account's fee pool (the
pool auto-rebalance orders use; top it up with `TopUpRebalanceFeePool`), scaled by how much of the
order's amount the fill moved: a fill of half the order earns half the tip. A pool that runs short
pays what it has; a tip of zero is allowed, but an order that pays nothing depends on someone
running a Keeper for free.

One residual risk is accepted, the same one auto-rebalance accepts: a Keeper who is also a lender
in the bank can lower its rate with a deposit just before the fill and withdraw it just after. The
window check confines that to banks which were genuinely cheap over your window, and the damage is
bounded by your order's amount.

### The Rate Is the Bank's Own History

The rate the order watches is the same bank history that powers Interest Triggers: it reads how
much the bank's borrow index actually grew across your window (`window_seconds`, 6 hours to 48
hours, 24 hours by default), not a rate at an instant. A brief spike contributes only its own
duration, so an hour at a punishing rate inside a day's window barely registers, while the same rate
held all day is exactly what the order sees. Nobody can reset or shorten that measurement, and there
is nothing to keep alive: placing the order takes a reading of its own, so the order can fire one
window after placement at the latest, sooner on a bank that already has the history.

### Modifying and Cancelling

Because the order persists, it can be changed in place. `UpdateBorrowOrder` adjusts the amount,
either level, the window, the cooldown, or the tip, with two rules: the new amount cannot be below
what has already been borrowed, and every value has to stay inside the ranges above.
`CancelBorrowOrder` removes it and returns your rent. Neither touches anything already borrowed.

### Knowing When a Keeper Acted

Every open emits an event carrying how much was borrowed, how much remains, the rate that justified
the fill, and the rate it left behind; every close, how much was repaid, what the order still holds,
and the rate that justified it. Front ends read these to tell you a Keeper opened or closed part of
your position, the same way a Trigger notifies you of an exit.

### What Is Not Here Yet

Redeploying into Kamino, Drift or JupLend banks, and turning a filled borrow into a Stop Loss or
Take Profit, are to come.

## Using Orders With Multiple Positions

Although orders apply to an asset and liability pair, the user can also have other positions on
their account. For example, if a user has lending positions A, B, C, and borrowing positions D, E,
F, the user might have orders on A/D, A/E, and C/F at the same time.

Using orders with more than two positions is an advanced feature with many financial nuances! If a
user sets a take-profit on A/D and then separately sets a stop loss on A/E, then the A/D order
executes such that they no longer have enough of asset A to fulfill the A/E stop loss, then the A/E
stop loss will remain open but can't be executed, which could lead to losses. It's up to users to
make sure their various orders do not interfere. This is consistent with e.g. most perps platforms,
where executing a stop-loss or take-profit does not close the other open order.

### Proof of Maximum Possible Orders

The theoretical maximum number of Orders is 64, the simple Cartesian Product:

```
* Let A = number of asset balances, L = number of liability balances, with A + L = 16
* An Order is defined as exactly one asset and one liability, (a, l) where a ∈ A and l ∈ L
* No pair {a, l} can repeat
* Thus, for each {a, _}, we can pick every l. i.e. for each |A| we can pair every |L| choice
* The maximum is achieved when |A| * |L| is maximized
* Maximizing A * L leads to A = 8, L = 8, and max = 64.
```
You may also frame this problem as counting ordered pairs `{a, l}` picked from the two sets.

Opening this many orders would be a silly idea, but the program supports it. Do what you like!


## Footnotes

(F1) Already have a Stop Loss on some pair and want to open a Take Profit? The correct flow is to
close the Stop Loss and open an Order for Both. Send this in an atomic transaction to avoid being
unprotected between the close of the Stop Loss and the open of the Both order.

(F2) The lending position can be withdrawn down to $0, but must remain open. If the Balance is closed
by the user (e.g. by withdraw_all), and the same asset is deposited later to re-open it, Orders
created prior to the Balance being closed **will not work**. This means users are able to modify
their accounts such that active Orders are orphaned and can no longer execute, it's up to users to make
sure they do not close out positions involved with their Orders without updating the Orders too.



# Program Level Information (for Developers and Integrators)

## Accounts

- `Order` - tracks information about a single take-profit and/or stop-loss order for an
  asset/liability pair on the user's account.
- `ExecuteOrderRecord` - an ephemeral account that is always closed in the same TX it is opened in,
  used to pass information between the start and end of order execution. None of these should exist
  in production. Note that the Keeper must have enough SOL to pay rent to open this account, even
  though it's returned at the end of the tx.
- `BorrowOrder` - a persistent borrow order for one (account, bank) pair: the amount, what
  has filled so far and the debt shares it holds, the open and close levels, the window, the
  cooldown, the keeper tip and the destination.
- `BorrowOrderRecord` - the borrow-order fill's ephemeral counterpart to `ExecuteOrderRecord`: opened
  at `StartBorrowOrderOpen` / `StartBorrowOrderClose`, closed at the matching end, never present
  between transactions.

## Instructions

- `PlaceOrder` (user) - Place a new Stop Loss, Take Profit, or Both type Order on a pair of balances
  the user currently holds.
- `PlaceInterestOrder` (user) - The same, carrying an `InterestTriggerConfig`. Takes the same
  accounts; the rates are read from the banks' own history, so the order is live from placement.
- `StartExecuteOrder` (Keeper) - Keepers run this to begin the execution of an Order. Must be at the
  start of the tx. Withdraw/Repay of the involved balances typically follows this ix.
  Requires a risk check of just the balances involved in the Order.
- `EndExecuteOrder` (Keeper) - Must be the last tx in executing an Order. Requires a risk check of
  just the balances involved in the Order.
- `CloseOrder` (user) - Clear an unwanted Order, user gets their rent back.
- `SetKeeperCloseFlags` (user) - Enables the Keeper to close Orders via `KeeperCloserOrder`,
  typically use `CloseOrder` instead.
- `KeeperCloserOrder` (Keeper) - Close an Order on an account where neither of the original positions exists or all the tags have been cleared by the user
- `PlaceBorrowOrder` (user) - Place a borrow order on a bank, with an optional same-mint
  destination bank, an optional close level (which requires the destination) and a keeper tip.
  Charges the flat order-init fee and takes a rate reading of the bank.
- `UpdateBorrowOrder` (user) - Change a live order's amount, levels, window, cooldown or tip.
- `CancelBorrowOrder` (user) - Cancel a borrow order and reclaim rent.
- `StartBorrowOrderOpen` (Keeper) - Begin an open. Checks the bank's realized rate against the open
  level, then grants the Keeper borrow authority for the transaction. Must be followed by the
  ordinary `LendingAccountBorrow` (and `LendingAccountDeposit` when redeploying).
- `EndBorrowOrderOpen` (Keeper) - Must be last. Proves the borrow was at least a granule, left the
  rate under the level with no more than a granule to spare (or exhausted the order, the bank's
  liquidity or a bank limit), that a redeploying order deposited what was borrowed, that no other
  balance changed, and that the account is healthy over the finished position; records the fill
  and pays the tip.
- `StartBorrowOrderClose` (Keeper) - Begin a close. Checks the bank's realized and current rates
  against the close level and that the order holds debt, then grants the Keeper withdraw and repay
  authority for the transaction. Must be followed by `LendingAccountWithdraw` from the destination bank and
  `LendingAccountRepay` on the order's bank.
- `EndBorrowOrderClose` (Keeper) - Must be last. Proves the repayment covered what the destination
  held and could pay out (less a granule) and stayed within what the order owes, that the withdrawal
  stayed within what was paid (premium settled included), that no other balance changed, and that
  the account meets maintenance health; records the repayment and pays the tip.
