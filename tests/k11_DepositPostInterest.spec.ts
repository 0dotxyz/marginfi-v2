// This test is similar to K06, see the difference in assertions to note how deposit values change
// when the Kamino bank has had activity and the liquid/collateral token ratio is no longer 1:1
import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  ecosystem,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  kaminoGroup,
  MARKET,
  oracles,
  USDC_RESERVE,
  users,
  verbose,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  bankRunProvider,
  banksClient,
  KAMINO_TOKEN_A_BANK as KAMINO_TOKEN_A_BANK,
  TOKEN_A_RESERVE,
} from "./rootHooks";
import {
  estimateCollateralFromDeposit,
  getTotalSupply,
  simpleRefreshObligation,
  simpleRefreshReserve,
  wrappedU68F60toBigNumber,
} from "./utils/kamino-utils";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";
import { MockUser, USER_ACCOUNT_K } from "./utils/mocks";
import { omitPadding, processBankrunTransaction } from "./utils/tools";
import { makeKaminoDepositIx } from "./utils/kamino-instructions";
import { ProgramTestContext } from "solana-bankrun";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  assertBNEqual,
  assertU68F60Approx,
  assertI68F60Equal,
  assertI80F48Approx,
  assertI80F48Equal,
  assertKeysEqual,
  getTokenBalance,
  assertBNApproximately,
} from "./utils/genericTests";
import { KLEND_PROGRAM_ID } from "./utils/types";
import {
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
} from "./utils/pdas";
import { getEpochAndSlot } from "./utils/stake-utils";
import { BalanceRaw } from "@mrgnlabs/marginfi-client-v2";
import { Reserve } from "@kamino-finance/klend-sdk";

let ctx: ProgramTestContext;
let bank: PublicKey;
let market: PublicKey;
let usdcReserve: PublicKey;
let usdcBankObligation: PublicKey;
let reserveLiquiditySupply: PublicKey;
let reserveCollateralSupply: PublicKey;

describe("k11: Kamino Deposit Tests After Interest Accrues", () => {
  before(async () => {
    ctx = bankrunContext;
    bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const bankKey = bank.toString();
    usdcBankObligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);

    [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.usdcMint.publicKey
    );
    [reserveCollateralSupply] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.usdcMint.publicKey
    );

    // Refresh to pick up any interest changes since the last test ended.
    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        usdcBankObligation,
        [usdcReserve]
      )
    );
    await processBankrunTransaction(ctx, tx, [users[0].wallet]);
  });

  async function executeDeposit(
    user: MockUser,
    amount: BN,
    userLabel: string
  ): Promise<void> {
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    /** Without decimals, e.g. 1.1 USDC = 1.1 */
    const amtFloat = amount.toNumber() / 10 ** ecosystem.usdcDecimals;

    if (verbose) {
      console.log(
        `Deposit for user ${userLabel} Account: ${marginfiAccount.toString()}`
      );
    }

    const [
      obBefore,
      resBefore,
      bankBefore,
      userAccBefore,
      liqVaultBefore,
      colVaultBefore,
      userUsdcBefore,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.usdcAccount),
    ]);

    const balanceMaybe = userAccBefore.lendingAccount.balances.find(
      (b) => b.bankPk.equals(bank) && b.active === 1
    );
    const balanceAmtBefore = balanceMaybe
      ? wrappedI80F48toBigNumber(balanceMaybe.assetShares).toNumber()
      : 0;
    if (verbose) {
      if (balanceAmtBefore > 0) {
        console.log("Deposit into existing position with: " + balanceAmtBefore);
      } else {
        console.log("Deposit into new position");
      }
    }

    const { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      // pass the USDC reserve since it's now part of the obligation
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        usdcBankObligation,
        [usdcReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        amount
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [
      obAfter,
      resAfter,
      bankAfter,
      userAccAfter,
      liqVaultAfter,
      colVaultAfter,
      userUsdcAfter,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.usdcAccount),
    ]);

    const userUsdcChange = userUsdcBefore - userUsdcAfter;
    const resSupplyAfter = getTotalSupply(resAfter as Reserve);
    if (verbose) {
      console.log(` Deposited ${amount} USDC actual change ${userUsdcChange}`);
      // Note: the net supply also includes available, borrowed, etc.
      console.log("res available:   " + resSupplyAfter.toString());
      console.log(
        "col mint supply: " + resAfter.collateral.mintTotalSupply.toString()
      );
    }
    assertBNEqual(amount, userUsdcChange);
    assertBNEqual(amount, liqVaultAfter - liqVaultBefore);
    // Note: The ratio between collateral and liquidity is no longer 1:1.
    const expectedCollateral = estimateCollateralFromDeposit(
      resAfter as Reserve,
      amount
    );
    assertBNEqual(expectedCollateral, colVaultAfter - colVaultBefore);

    const balancesAfter = userAccAfter.lendingAccount.balances;
    const balanceAfter: BalanceRaw = balancesAfter.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    assert.equal(balanceAfter.active, 1);
    assertI80F48Approx(
      balanceAfter.assetShares,
      // Note: the balance tracks your collateral token, NOT liquidity token!
      balanceAmtBefore + expectedCollateral.toNumber()
    );
    assertBNEqual(
      resAfter.liquidity.availableAmount,
      resBefore.liquidity.availableAmount.add(amount)
    );
    // * Note: the collateral mint will have the same raw value, even though it may use different
    // decimals (fixed to 6) in the token program, it actually always uses mint_decimals.
    // * Note: the collateral supply is larger than the liquidity here because of interest.
    const interestTolerance =
      resAfter.collateral.mintTotalSupply.toNumber() * 0.02;
    assertBNApproximately(
      resAfter.collateral.mintTotalSupply,
      resBefore.collateral.mintTotalSupply.add(amount),
      interestTolerance
    );
    // We know there are some borrows here, but we don't care how many.
    assert(
      wrappedU68F60toBigNumber(resAfter.liquidity.borrowedAmountSf).toNumber() >
        10
    );

    // Assert bank updated as expected
    const sharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    // No interest accumulates on Kamino banks, so the asset share value is always 1, and the
    // relationship between collateral tokens and shares is always 1:1
    assertI80F48Approx(
      bankAfter.totalAssetShares,
      sharesBefore + expectedCollateral.toNumber()
    );
    assertI80F48Equal(bankAfter.assetShareValue, 1);
    assertI80F48Equal(bankAfter.collectedInsuranceFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedGroupFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedProgramFeesOutstanding, 0);

    // Assert obligation/reserve deposit state recorded as expected
    const depositBefore = obBefore.deposits[0];
    const depositAfter = obAfter.deposits[0];
    assertKeysEqual(depositAfter.depositReserve, usdcReserve);
    assertBNEqual(
      depositAfter.depositedAmount,
      // Note: the balance tracks your collateral token, NOT liquidity token!
      depositBefore.depositedAmount.add(expectedCollateral)
    );

    // Oracle update assertions
    assert.equal(obAfter.lastUpdate.slot.toNumber(), slot);
    assert.equal(resAfter.lastUpdate.slot.toNumber(), slot);
    /*
        const PRICE_LOADED =        0b_0000_0001; // 1
        const PRICE_AGE_CHECKED =   0b_0000_0010; // 2
        const TWAP_CHECKED =        0b_0000_0100; // 4 
        const TWAP_AGE_CHECKED =    0b_0000_1000; // 8
        const HEURISTIC_CHECKED =   0b_0001_0000; // 16
        const PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32
    */
    // All flags clear = 32 + 16 + 8 + 4 + 2 + 1
    assert.equal(obAfter.lastUpdate.priceStatus, 63);
    assert.equal(resAfter.lastUpdate.priceStatus, 63);
    // Note: Deposit triggers `obligation.last_update.mark_stale()`, setting stale to true (1).
    assert.equal(obAfter.lastUpdate.stale, 1);
    // Note: Deposit also triggers `deposit_reserve.last_update.mark_stale()`
    assert.equal(resAfter.lastUpdate.stale, 1);

    // Note: Some values don't update until the NEXT refresh!
    let refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        usdcBankObligation,
        [usdcReserve]
      )
    );
    await processBankrunTransaction(ctx, refreshTx, [user.wallet]);

    // Market price float data doesn't update until the NEXT refresh
    const [obAfterRefresh, resAfterRefresh] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
    ]);
    const depositAfterRefresh = obAfterRefresh.deposits[0];
    const marketValueBefore = wrappedU68F60toBigNumber(
      depositBefore.marketValueSf
    ).toNumber();
    const marketValueAfter = wrappedU68F60toBigNumber(
      depositAfterRefresh.marketValueSf
    ).toNumber();
    const expectedDiff = amtFloat * oracles.usdcPrice;
    const expectedValue = marketValueBefore + expectedDiff;
    assert.approximately(
      marketValueAfter,
      expectedValue,
      marketValueAfter * 0.0001
    );
    // Note: the reserve, in a variable with the same name, records the token PRICE...
    assert.approximately(
      wrappedU68F60toBigNumber(
        resAfterRefresh.liquidity.marketPriceSf
      ).toNumber(),
      oracles.usdcPrice,
      oracles.usdcPrice * 0.0001
    );

    // After another refresh, the stale flag is removed and the oracle data can be used again. Note:
    // This forces multiple transactions in the same tx that consume oracle data (e.g. multiple
    // deposits) to refresh again after each tx is done.
    assert.equal(obAfterRefresh.lastUpdate.stale, 0);
    assert.equal(resAfterRefresh.lastUpdate.stale, 0);
  }

  const deposit1 = 1_000_000 * 10 ** ecosystem.usdcDecimals;
  it("(user 0) Deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[0], new BN(deposit1), "0");
  });

  const deposit2 = 200 * 10 ** ecosystem.usdcDecimals;
  it("(user 0) Second deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[0], new BN(deposit2), "0");
  });

  const deposit3 = 150 * 10 ** ecosystem.usdcDecimals;
  it("(user 1) Deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[1], new BN(deposit3), "1");
  });
});

// * Note: With token A, which has 8 decimals, everything is the same!
// * Note: This is basically a copy of the USDC test above, but they are sufficiently different
//   (because the token accounts, bank, reserve, etc are different) that refactoring to support both
//   would probably look worse than this copypasta.

describe("k11a: Kamino Token A Deposit Tests After Interest Accrues", () => {
  let bank: PublicKey;
  let market: PublicKey;
  let tokenAReserve: PublicKey;
  let tokenABankObligation: PublicKey;
  let reserveLiquiditySupply: PublicKey;
  let reserveCollateralSupply: PublicKey;

  before(async () => {
    ctx = bankrunContext;
    bank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);
    market = kaminoAccounts.get(MARKET);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);

    const [authority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      bank
    );
    [tokenABankObligation] = deriveBaseObligation(authority, market);
    [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.tokenAMint.publicKey
    );
    [reserveCollateralSupply] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.tokenAMint.publicKey
    );

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        tokenABankObligation,
        [tokenAReserve]
      )
    );
    await processBankrunTransaction(ctx, tx, [users[0].wallet]);
  });

  async function executeDepositTokenA(
    user: MockUser,
    amount: BN,
    userLabel: string
  ): Promise<void> {
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const amtFloat = amount.toNumber() / 10 ** ecosystem.tokenADecimals;

    if (verbose) {
      console.log(
        `Deposit for user ${userLabel} Account: ${marginfiAccount.toString()}`
      );
    }

    const [
      obBefore,
      resBefore,
      bankBefore,
      userAccBefore,
      liqVaultBefore,
      colVaultBefore,
      userTokenABefore,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(tokenABankObligation),
      klendBankrunProgram.account.reserve.fetch(tokenAReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.tokenAAccount),
    ]);

    const balanceMaybe: BalanceRaw = userAccBefore.lendingAccount.balances.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    const balanceAmtBefore = balanceMaybe
      ? wrappedI80F48toBigNumber(balanceMaybe.assetShares).toNumber()
      : 0;
    if (verbose) {
      if (balanceAmtBefore > 0) {
        console.log("Deposit into existing position with: " + balanceAmtBefore);
      } else {
        console.log("Deposit into new position");
      }
    }

    const { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        tokenABankObligation,
        [tokenAReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.tokenAAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        },
        amount
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [
      obAfter,
      resAfter,
      bankAfter,
      userAccAfter,
      liqVaultAfter,
      colVaultAfter,
      userTokenAAfter,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(tokenABankObligation),
      klendBankrunProgram.account.reserve.fetch(tokenAReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.tokenAAccount),
    ]);

    const userTokenAChange = userTokenABefore - userTokenAAfter;
    const resSupplyAfter = getTotalSupply(resAfter as Reserve);
    if (verbose) {
      console.log(
        ` Deposited ${amount} TokenA actual change ${userTokenAChange}`
      );
      console.log("res available:   " + resSupplyAfter.toString());
      console.log(
        "col mint supply: " + resAfter.collateral.mintTotalSupply.toString()
      );
    }
    assertBNEqual(amount, userTokenAChange);
    assertBNEqual(amount, liqVaultAfter - liqVaultBefore);
    const expectedCollateral = estimateCollateralFromDeposit(
      resAfter as Reserve,
      amount
    );
    assertBNEqual(expectedCollateral, colVaultAfter - colVaultBefore);

    const balancesAfter = userAccAfter.lendingAccount.balances;
    const balanceAfter: BalanceRaw = balancesAfter.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    assert.equal(balanceAfter.active, 1);
    assertI80F48Approx(
      balanceAfter.assetShares,
      balanceAmtBefore + expectedCollateral.toNumber()
    );
    assertBNEqual(
      resAfter.liquidity.availableAmount,
      resBefore.liquidity.availableAmount.add(amount)
    );
    // Note: Here with an 8 decimal mint, the mintTotalSupply still uses mint_decimals.
    const interestTolerance =
      resAfter.collateral.mintTotalSupply.toNumber() * 0.02;
    assertBNApproximately(
      resAfter.collateral.mintTotalSupply,
      resBefore.collateral.mintTotalSupply.add(amount),
      interestTolerance
    );
    assert(
      wrappedU68F60toBigNumber(resAfter.liquidity.borrowedAmountSf).toNumber() >
        10
    );

    const sharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    assertI80F48Approx(
      bankAfter.totalAssetShares,
      sharesBefore + expectedCollateral.toNumber()
    );
    assertI80F48Equal(bankAfter.assetShareValue, 1);
    assertI80F48Equal(bankAfter.collectedInsuranceFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedGroupFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedProgramFeesOutstanding, 0);

    const depositBefore = obBefore.deposits[0];
    const depositAfter = obAfter.deposits[0];
    assertKeysEqual(depositAfter.depositReserve, tokenAReserve);
    assertBNEqual(
      depositAfter.depositedAmount,
      depositBefore.depositedAmount.add(expectedCollateral)
    );

    assert.equal(obAfter.lastUpdate.slot.toNumber(), slot);
    assert.equal(resAfter.lastUpdate.slot.toNumber(), slot);
    assert.equal(obAfter.lastUpdate.priceStatus, 63);
    assert.equal(resAfter.lastUpdate.priceStatus, 63);
    assert.equal(obAfter.lastUpdate.stale, 1);
    assert.equal(resAfter.lastUpdate.stale, 1);

    let refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        tokenABankObligation,
        [tokenAReserve]
      )
    );
    await processBankrunTransaction(ctx, refreshTx, [user.wallet]);

    const [obAfterRefresh, resAfterRefresh] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(tokenABankObligation),
      klendBankrunProgram.account.reserve.fetch(tokenAReserve),
    ]);
    const depositAfterRefresh = obAfterRefresh.deposits[0];
    const marketValueBefore = wrappedU68F60toBigNumber(
      depositBefore.marketValueSf
    ).toNumber();
    const marketValueAfter = wrappedU68F60toBigNumber(
      depositAfterRefresh.marketValueSf
    ).toNumber();
    const expectedDiff = amtFloat * oracles.tokenAPrice;
    const expectedValue = marketValueBefore + expectedDiff;
    assert.approximately(
      marketValueAfter,
      expectedValue,
      marketValueAfter * 0.0001
    );
    assert.approximately(
      wrappedU68F60toBigNumber(
        resAfterRefresh.liquidity.marketPriceSf
      ).toNumber(),
      oracles.tokenAPrice,
      oracles.tokenAPrice * 0.0001
    );

    assert.equal(obAfterRefresh.lastUpdate.stale, 0);
    assert.equal(resAfterRefresh.lastUpdate.stale, 0);
  }

  const deposit1 = 10_000 * 10 ** ecosystem.tokenADecimals;
  it("(user 0) Deposit Token A to Kamino via Marginfi - happy path", async () => {
    await executeDepositTokenA(users[0], new BN(deposit1), "0");
  });

  const deposit2 = 200 * 10 ** ecosystem.tokenADecimals;
  it("(user 0) Second deposit Token A to Kamino via Marginfi - happy path", async () => {
    await executeDepositTokenA(users[0], new BN(deposit2), "0");
  });

  const deposit3 = 150 * 10 ** ecosystem.tokenADecimals;
  it("(user 1) Deposit Token A to Kamino via Marginfi - happy path", async () => {
    await executeDepositTokenA(users[1], new BN(deposit3), "1");
  });
});
