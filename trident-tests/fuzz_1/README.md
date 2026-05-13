# Fuzz `fuzz_1` — invariant checklist

This README lists properties asserted during Trident fuzzing (mostly in `invariants.rs`, wired from `core_methods.rs`).

- **Token account read**: every sampled SPL account must deserialize as a token account so balances are defined.
- **No balance change on failed lending tx**: user ATA and touched bank liquidity vault stay unchanged when deposit/withdraw/borrow/repay fails.
- **User–vault conservation**: any token movement between user ATA and the bank’s liquidity vault nets to zero (no silent mint/burn in that leg).
- **Per-account balance unchanged**: generic helper comparing a single u64 before vs after.
- **Flashloan empty-body vault snapshot**: records liquidity vault and optional user ATA balances so an empty inner bundle can be checked for no drift.
- **Token snapshot unchanged**: every account in a snapshot keeps the same token balance after the tx.
- **Successful deposit (token leg)**: user balance does not increase, vault does not decrease, and for `amount > 0` both move in the deposit direction with conservation.
- **Successful withdraw/borrow (token leg)**: user balance does not decrease, vault does not increase, and for `amount > 0` both move in the withdraw/borrow direction with conservation.
- **Successful repay (token leg)**: same directional rules as deposit (user pays, vault receives) with conservation.
- **Exact deposit amounts (success)**: for `amount > 0`, user loss and vault gain both equal the requested amount (Tokenkeg / fuzz mints without transfer fees).
- **Exact withdraw/borrow amounts (success)**: for `amount > 0`, user gain and vault loss both equal the requested amount.
- **Exact repay token leg (success)**: user outflow equals vault inflow and equals the post-fee repay amount when mints have no transfer fee (matches current fuzz setup).
- **Deposit share semantics (success)**: liability shares unchanged; asset shares strictly increase when `amount > 0`; `amount == 0` allows either no change or opening an empty active bank slot via `find_or_create`.
- **Withdraw share semantics (success)**: liability shares unchanged; asset shares decrease if the row stays open, or the row fully closes with prior positive assets when `amount > 0`; `amount == 0` implies no share snapshot change.
- **Borrow share semantics (success)**: asset shares unchanged; liability shares strictly increase when `amount > 0`; `amount == 0` allows either no change or opening an empty active bank slot via `find_or_create`.
- **Repay share semantics (success)**: asset shares unchanged; liability shares decrease if the row stays open, or the row fully closes with prior positive liabilities when `amount > 0`; `amount == 0` implies no share snapshot change.
- **Accrue interest**: after a successful time warp + `LendingPoolAccrueBankInterest` batch, each fuzz bank’s `last_update` strictly increases (so accrue is not a no-op `time_delta == 0` path).
- **Liquidation liability vault accounting**: liability-token liquidity vault plus insurance vault token total is conserved; liquidity vault does not grow; insurance vault does not shrink.
- **Liquidation success (share direction)**: for non-zero `asset_amount`, liquidatee USDC asset shares and ETH liability shares decrease while liquidator USDC asset and ETH liability shares increase.
- **Liquidation failure**: vault balances and the sampled marginfi share fields in the snapshot are bitwise unchanged.
- **Receivership end (success)**: marginfi account clears `ACCOUNT_IN_RECEIVERSHIP`, `ACCOUNT_IN_FLASHLOAN`, and `ACCOUNT_IN_ORDER_EXECUTION`; liquidation record still points at the same marginfi account; `liquidation_receiver` is cleared; newest history slot `entries[3]` has a non-zero timestamp.
- **Flashloan closed loop**: when borrow and repay amounts are equal and the transaction succeeds, the user’s token balance for that asset is unchanged.
- **Flashloan mismatch**: when borrow and repay amounts differ, the transaction is expected to fail.
- **Cross-bank isolation (lending ops)**: USDC/ETH/BTC liquidity vaults that are not the bank under test keep the same token balances after each single-bank deposit, withdraw, borrow, or repay.

