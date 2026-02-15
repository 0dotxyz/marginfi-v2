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
their accounts such that active Orders are orphaned and can longer execute, it's up to users to make
sure they do not close out positions involved with their Orders without updating the Orders too.



# Program Level Information (for Developers and Integrators)

## Accounts

- `Order` - tracks information about a single take-profit and/or stop-loss order for an
  asset/liability pair on the user's account.
- `ExecuteOrderRecord` - an ephemeral account that is always closed in the same TX it is opened in,
  used to pass information between the start and end of order execution. None of these should exist
  in production. Note that the Keeper must have enough SOL to pay rent to open this account, even
  though it's returned at the end of the tx.

## Instructions

- `PlaceOrder` (user) - Place a new Stop Loss, Take Profit, or Both type Order on a pair of balances
  the user currently holds.
- `StartExecuteOrder` (Keeper) - Keepers run this to begin the execution of an Order. Must be at the
  start of the tx, and `EndExecuteOrder` must appear last in the tx. Withdraw/Repay of the involved
  balances typically follows this ix. Requires a risk check of just the balances involved in the
  Order.
- `EndExecuteOrder` (Keeper) - Must be the last tx in executing an Order. Requires a risk check of
  just the balances involved in the Order.
- `CloseOrder` (user) - Clear an unwanted Order, user gets their rent back.
- `SetKeeperCloseFlags` (user) - Enables the Keeper to close Orders via `KeeperCloserOrder`,
  typically use `CloseOrder` instead.
- `KeeperCloserOrder` (Keeper) - Close an Order on an account where neither of the original positions exists or all the tags have been cleared by the user
