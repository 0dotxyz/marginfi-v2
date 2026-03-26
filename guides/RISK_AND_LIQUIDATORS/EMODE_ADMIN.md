# Emode Administrator Guide and General Overview

Are you interested on how Emode works and is adminstrated? Read on.

## Glossary

- **Emode/E-mode/EMode** - "Efficiency Mode", probably a backronym, popularized by Aave. Allows
  users of a lending protocol to enjoy more borrowing power when working with certain asset
  categories. For more information see [Aave's docs](https://aave.com/help/borrowing/e-mode) or
  [this article on emode benefits and
  risks](https://www.llamarisk.com/research/understanding-aave-v3-2-s-liquid-e-mode-a-deep-dive-into-enhanced-capital-efficiency).
- **Preferrential Asset** - When borrowing some asset A, a preferrential asset B is some asset that
  is sufficiently price correlated with A that for risk purposes we would be comfortable offering a
  better-than-usual collateral ratio on B.
- **Bank** - Each asset available to borrow and lend on mrgnlend has a single bank, this account
  controls all the settings for the asset.
- **Asset Weight** - Each asset available to lend has two rates: The "Initial" and "Maintenance"
  rates. The Maintenance rate is always higher. If attempting to execute a borrow, collateral is
  valued at (price x initial rate). If a liquidator is attempting a liquidation, collateral is
  valued at (price x maintenance rate). The range between these is sometimes called the health
  buffer. For example, if a user has collateral worth \$10, and init/maint rates are 50\% and 60\%
  respectively, the user can borrow \$10 x .5 = \$5 in collateral. For liquidation purposes, their
  collateral is worth \$10 x .6 = \$6.
- **Emode Tag** - an arbitrary number (1 - 65,535) assigned to each bank by the admin. Multiple
  banks can share the same number. By convention, these are the asset name in l337sp34k. For
  example, SOL is 501, LST is 157, etc. If the name doesn't fit into range (1 - 65,535), we use our
  imagination.
- **Emode Config** - A bank's collection of emode entries.
- **Emode Entry** - A tag we want to treat as a preferrential asset, and the initial/maintenance
  asset weights to use for that asset.

## Key Differences vs AAVE

- Opt-in by default: we don't require users to enable Emode, users automatically get the most
  preferrential Emode setting that applies to their account.
- Opt-out by borrowing something else - Aave requires you to disable E-mode before you can borrow a
  non-preferrential asset, in our system (at least at the program level), you can enter or exit
  emode simply by changing which assets you are borrowing. This means all assets are available to
  borrow, but also that users can significantly reduce their account health by borrowing a
  non-preferrential asset or significantly increase health by repaying one.

## Emode Configuration Overview

Emode can never reduce the weights of a bank, i.e. the bank's weight is always the minimum and
attempts to configure an emode entry below the bank's weights will simply do nothing.

Users who borrow multiple emode assets at the same time get the LEAST FAVORABLE treatment between
the borrowed assets, regardless of the amount of each asset borrowed. For example, if borrowing an
LST and USDC against SOL, the user would normally get an emode benefit for LST/SOL, but since they
are also borrowing USDC, they get only standard rates on the SOL they are lending. Note that a user
that is currently lending SOL and borrowing LST, but now wants to borrow USDT, must have enough
account health to support the LST and USDT borrow without any emode advantage.

One of the biggest naunces is that we always configure the BORROWING bank not the LENDING bank. For
example, if we want users to be able to borrow more SOL against some LST, we will configure the SOL
bank. If we want users to be able to borrow more WIF against BONK, we configure WIF, and so forth.

The first step to configuring the emode environment is to make a list of all the assets that will be
paired, for example, let's say LENDING is an asset users are lending, the NORMAL RATE is the initial
asset weight set on the bank, BORROWING is an asset they might borrowing against the LENDING asset,
and EMODE RATE is the initial asset weight users should enjoy when borrowing BORROWING and lending
LENDING.

```
LENDING   | NORMAL RATE  |  BORROWING  |  EMODE RATE
SOL            .8             LST_A           .9
SOL            .8             LST_B           .9
SOL            .8             LST_C           .85
SOL            .8             LST_D           .85
LST_A          .6             SOL             .8
LST_B          .6             SOL             .8
LST_C          .6             SOL             .8
LST_D          .6             SOL             .8
USDC           .9             USDT            .95
USDT           .9             USDC            .95
```

From the above we have four unique pairings and need at least four tags: the SOL, LST_AB, LST_CD,
STABLE. We also have 5 unique 5 rates: LEND_SOL_BORROW_LST_AB (.9), LEND_SOL_BORROW_LST_CD (.85),
LEND_LST_BORROW_SOL (.8), LEND_STABLE_BORROW_STABLE (.95).

Our emode config will look something like this:

```
bank: SOL_BANK,
tag: SOL,
entries: [
    tag: LST_AB,
    rate: LEND_LST_BORROW_SOL

    tag: LST_CD,
    rate: LEND_LST_BORROW_SOL
],

bank: LST_A_BANK,
tag: LST_AB,
entries: [
    tag: SOL,
    rate: LEND_SOL_BORROW_LST_AB
],
bank: LST_B_BANK,
tag: LST_AB,
entries: [
    tag: SOL,
    rate: LEND_SOL_BORROW_LST_AB
],

bank: LST_C_BANK,
tag: LST_CD,
entries: [
    tag: SOL,
    rate: LEND_SOL_BORROW_LST_CD
],
bank: LST_D_BANK,
tag: LST_CD,
entries: [
    tag: SOL,
    rate: LEND_SOL_BORROW_LST_CD
],

bank: USDC_BANK,
tag: STABLE,
entries: [
    tag: STABLE,
    rate: LEND_STABLE_BORROW_STABLE
],
bank: USDT_BANK,
tag: STABLE,
entries: [
    tag: STABLE,
    rate: LEND_STABLE_BORROW_STABLE
],
```

Now let's look at some examples of what rate users would get.

<details>
<summary>User lending SOL and borrowing LST_A.</summary>

- (1) We look at SOL_BANK to see what tag it has (SOL)
- (2) we look at LST_A_BANK to read the rate for tag SOL. The rate is LEND_SOL_BORROW_LST_AB (.9)

> The answer is .9

</details>

<details>
<summary>User lending SOL and borrowing LST_A and LST_C.</summary>

- (1) We look at SOL_BANK to see what tag it has (SOL)
- (2) We look at LST_A_BANK to read the rate for tag SOL (LEND_SOL_BORROW_LST_AB)
- (3) We look at LST_C_BANK and read the rate for tag SOL (LEND_SOL_BORROW_LST_CD)
- (4) We take the smallest of the two rates: min(AB, CD) = LEND_SOL_BORROW_LST_CD (.85)

> The answer is .85

</details>

<details>
<summary>User lending SOL and borrowing LST_A and USDT</summary>

- (1) We look at SOL_BANK to see what tag it has (SOL)
- (2) We look at LST_A_BANK to read the rate for tag SOL (LEND_SOL_BORROW_LST_AB)
- (3) We look at USDT and read the rate for tag SOL, but it doesn't have one.
- (4) Since one of the borrowing assets doesn't have an entry, we fall back to SOL bank's NORMAL RATE (.8).

> The answer is .8

</details>

<details>
<summary>User lending SOL and LST_A and borrowing LST_C</summary>

- (1) We look at SOL_BANK to see what tag it has (SOL)
- (2) We look at LST_A_BANK to see what tag it has (LST_AB)
- (3) We look at LST_C_BANK to read the rate for tag SOL (LEND_SOL_BORROW_LST_CD)
- (4) We look at LST_C_BANK and read the rate for tag LST_AB, but it doesn't have one.
- (5) The SOL is treated at LEND_SOL_BORROW_LST_CD (.85) while LST_A is treated at the NORMAL RATE (.6)

> The answer is .85 for SOL and .6 for LST_A

</details>

## Applying Emode to Isolated Assets (Not Yet Implemented)

Generally, an isolated asset is worth 0 for collateral purposes. Enabling Emode on an isolated asset
allows it count as collateral in preferrential borrowing situations. This is useful to enable
lending on assets that are highly correlated with some other asset but otherwise extremely risky.
For example, consider a betting market with a YES and NO token where we know one asset will converge
to zero eventually and the other will converge to $1. We may offer a limit emode advantage here, for
position hedging, when the event is in the distant future and gradually step the emode advantage
down to zero as the decision approaches.

## Ix Construction Examples

### Enable borrowing more SOL against LST and vice-versa

```
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: SOL_BANK,
    tag: SOL_TAG,
    entries: [
        newEmodeEntry(
            EMODE_LST_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_SOL_TO_LST),
            bigNumberToWrappedI80F48(MAINT_RATE_SOL_TO_LST)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: LST_BANK,
    tag: LST_TAG,
    entries: [
        newEmodeEntry(
            SOL_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_SOL_TO_LST),
            bigNumberToWrappedI80F48(MAINT_RATE_SOL_TO_LST)
        ),
    ],
})
```

### Enable borrowing more Stable vs any other Stable

```
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_A,
    tag: STABLE_TAG,
    entries: [
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_B,
    tag: STABLE_TAG,
    entries: [
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_C,
    tag: STABLE_TAG,
    entries: [
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
    ],
}),
```

- Note: A bank can have an emode entry for its own tag, but you can never borrow from a bank you are
  already lending into.
- Note: The `configBankEmode` is large, only two can fit in one TX. It is recommended to use a jito
  bundle of several txes when updating more than two banks.

### Enable borrowing more Stable vs any other Stable, with nuances

Let's say we now decide that stable C is considered somewhat riskier, we think the asset is prone to
depeg and may decline. We want A/B to continue to enjoy full emode advantage. We want lenders of C
to enjoy a lesser advantage when borrowing A or B. We want lenders of A or B to continue to enjoy
the full advantage when borrowing C.

```
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_A,
    tag: STABLE_TAG,
    entries: [
        // A favorable rate for lenders of B borrowing A
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
        // A less favorable rate for lenders of C borrowing A
        newEmodeEntry(
            REDUCED_STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_REDUCED_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_REDUCED_STABLE)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_B,
    tag: STABLE_TAG,
    entries: [
        // A favorable rate for lenders of A borrowing B
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
        // A less favorable rate for lenders of C borrowing B
        newEmodeEntry(
            REDUCED_STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_REDUCED_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_REDUCED_STABLE)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: STABLE_BANK_C,
    tag: REDUCED_STABLE_TAG, // a different tag than other stable banks
    entries: [
        // A favorable rate for lenders of A/B borrowing C
        newEmodeEntry(
            STABLE_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_STABLE_TO_STABLE),
            bigNumberToWrappedI80F48(MAINT_RATE_STABLE_TO_STABLE)
        ),
    ],
}),
```

### Enable borrowing more WIF against BONK

```
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: WIF_BANK,
    tag: WIF_TAG, // You could also leave this blank (0)
    entries: [
        newEmodeEntry(
            BONK_TAG,
            APPLIES_TO_ISOLATED,
            bigNumberToWrappedI80F48(INIT_RATE_LEND_BONK_BORROW_WIF),
            bigNumberToWrappedI80F48(MAINT_RATE_LEND_BONK_BORROW_WIF)
        ),
    ],
}),
await configBankEmode(emodeAdmin.mrgnProgram, {
    bank: BONK_BANK,
    tag: BONK_TAG,
    entries: [],
})
```
