# Fee Administrator Guide

Are you an accountant or the person responsible for collecting bank fees? Read on.

## Glossary

- **Insurance fee** - Used to offset socialized losses in the event of bankruptcy. 
- **Group fee** - Goes to the group owner. For the main pool, this is marginfi.
- **Program fee** - Goes to the program owner (i.e. margin foundation).
- **Base rate** - The rate determined by the curve parameters and current utilization. A more detailed explanation is beyond the scope of this guide.
- **Ir rate** - A rate that scales off the base rate, for example an ir rate of 10% means you will pay 10% of the base rate. 
- **Fixed rate** - A rate of 10% here would mean users pay exactly 10%
- **Origination Fee** - A % of the borrow amount charged when a borrow is initiated.
- **Bank Init flat sol fee** - A fixed amount of SOL charged when creating a new bank.
- **Global fee wallet** - One per program, a wallet that receives all program fees. Can be changed
  by the global fee admin.
- **Legacy Liquidation fee** - When using lending_account_liquidation, 2.5% of the amount repaid by
  the liquidator goes to insurance, and 2.5% goes to the liquidator.
- **Liquidation flat sol fee** - A fixed amount of SOL charged when liquidating with receivership liquidation
- **Receivership liquidation fee** - The liquidator collects a profit when performing with
  receivership liquidation. The profit equals the amount withdrawn minus the amount repaid, in
  dollars, in equity terms.

## Kinds of Fees And How Earned

- **Interest** - When interest accrues, the borrow rate is calculated as 
```
base_rate * (1 + insurance_ir + group_ir + program_ir) + insurance_fixed + group_fixed + program_fixed.
```
For example, if the base rate is 10%, and the group_ir rate is 10%, and the group_fixed rate is 1%,
then the group gets 10% * 10% + 1% = 2% APR. Interest is only paid by borrowers, so banks without
borrowers (like Staked Collateral) never earn fees. Interest accumulates when any transaction that
changes funds occurs, so banks without much activity may have stale numbers until someone interacts
with them.
- **Origination fee** - Historically, always zero. The group admin configures this as a percentage
  of the borrow, and the program admin determine what portion of this fee goes to them. For example,
  if the origination fee is 1%, and the program_ir is 10%, then the group gets 0.9% and the program
  gets 0.1% of any borrow. A user borrowing \$100 would pay \$101, where 90 cents goes to the group
  and 10 cents to the program.
- **Bank Init flat sol fee** - Historically, always zero. Automatically debited to the global
  program fee wallet any time a bank is created.
- **Liquidation flat sol fee** - Historically, about $0.10 equivalent in SOL. Automatically debited
  when liquidating using receivership liquidation.
- **Receivership liquidation fee** - Historically no more than 10%, e.g. a liquidator withdrawing
  $10 in equity can repay $9 in debt and keep $1

## How Interest Accumulates

When any user interacts with marginfi in a way that changes the balance of a bank,
accrue_interest runs first. Based on the rate and how much time has elapsed, the program adds to the
following bank fields:
```
collected_insurance_fees_outstanding
collected_group_fees_outstanding
collected_program_fees_outstanding
```
This (along with the base rate itself) causes the `asset_share_value` and/or `liability_share_value`
to increase.

## How Fees Are Collected

(1) Interset accrues and increases the number in `fees_outstanding` fields. Optionally, origination
fees are charged and do the same.

(2) Anyone (the ix is permissionless) runs `LendingPoolCollectBankFees`. This moves:
* The amount specified in `collected_insurance_fees_outstanding` from the `liquidity_vault` to the
  `insurance_vault`
* The amount specified in `collected_group_fees_outstanding` from the `liquidity_vault` to the `fee_vault`
* The amount specified in `collected_program_fees_outstanding` from the `liquidity_vault` to the
    `fee_ata`, which is the cannonical ATA of the global fee wallet.

Then the `fees_outstanding` are all reset to zero. The vast majority of the time, we run this ix just before a withdraw.

(3) The group admin runs `LendingPoolWithdrawFees`. This moves:
* The amount in `fee_vault` into any account of the group admin's choosing. 

(3a) If the group admin set a destination of choice with `LendingPoolUpdateFeesDestinationAccount`,
then anyone can run `LendingPoolWithdrawFeesPermissionless`, which does the same thing as
`LendingPoolWithdrawFees` but doesn't require permission, and always sends to the account defined by
the group admin (`bank.fees_destination_account`)