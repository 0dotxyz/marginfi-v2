import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  users,
  verbose,
  kaminoGroup,
} from "./rootHooks";
import {
  defaultKaminoBankConfig,
  getCollateralExchangeRate,
  getLiquidityAvailableAmount,
  getLiquidityExchangeRate,
  getTotalSupply,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  accountInit,
  depositIx,
  borrowIx,
  composeRemainingAccounts,
  healthPulse,
  pulseBankPrice,
  repayIx,
} from "./utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  setFixedPrice,
} from "./utils/group-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { assert } from "chai";
import { assertBankrunTxFailed, getTokenBalance } from "./utils/genericTests";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { logHealthCache, processBankrunTransaction } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  CONF_INTERVAL_MULTIPLE_FLOAT,
  defaultBankConfig,
  ORACLE_SETUP_FIXED_KAMINO,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import { Reserve } from "@kamino-finance/klend-sdk";

let ctx: ProgramTestContext;
let market: PublicKey;
let usdcReserve: PublicKey;
let tokenAReserve: PublicKey;
let fixedKaminoBank: PublicKey;
let fixedKaminoObligation: PublicKey;
let userAccount: PublicKey;
let borrowBank: PublicKey;
let adminAccount: PublicKey;
let userUsdcStart = 0;

const FIXED_SEED = new BN(7778);
const BORROW_SEED = new BN(8888);
// Note: USDC is not worth $2, so this test is silly
const FIXED_PRICE = 2;
const BORROW_AMOUNT = new BN(10 * 10 ** ecosystem.tokenADecimals);

describe("kx: Fixed Kamino price bank", () => {
  before(async () => {
    ctx = bankrunContext;
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  });

  it("(user 3) initialize marginfi account for main group", async () => {
    const user = users[3];
    const accountKeypair = Keypair.generate();
    userAccount = accountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);
  });

  it("(admin) add fixed Kamino USDC bank + init obligation", async () => {
    const defaultConfig = defaultKaminoBankConfig(oracles.usdcOracle.publicKey);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      FIXED_SEED,
    );
    fixedKaminoBank = bankKey;

    const addBankTx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: usdcReserve,
          kaminoMarket: market,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: FIXED_SEED,
        },
      ),
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const [authority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      fixedKaminoBank,
    );
    const [obligation] = deriveBaseObligation(authority, market);
    fixedKaminoObligation = obligation;

    const initObligationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: users[3].wallet.publicKey,
          bank: fixedKaminoBank,
          signerTokenAccount: users[3].usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        new BN(100),
      ),
    );
    await processBankrunTransaction(ctx, initObligationTx, [users[3].wallet]);

    const setFixedTx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: fixedKaminoBank,
        price: FIXED_PRICE,
        remaining: [usdcReserve],
      }),
    );
    await processBankrunTransaction(ctx, setFixedTx, [groupAdmin.wallet]);

    if (verbose) {
      console.log("Fixed Kamino bank:", fixedKaminoBank.toString());
    }
  });

  it("(admin) configure_bank_oracle rejects FixedKamino setup - use set_fixed_oracle_price", async () => {
    const tx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: fixedKaminoBank,
        type: ORACLE_SETUP_FIXED_KAMINO,
        oracle: oracles.usdcOracle.publicKey,
      }),
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true,
    );
    // UseSetFixedOraclePrice
    assertBankrunTxFailed(result, 6132);
  });

  it("(admin) add throwaway regular Token A bank + seed liquidity", async () => {
    const adminAccountKeypair = Keypair.generate();
    adminAccount = adminAccountKeypair.publicKey;

    const initAdminTx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: adminAccount,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(
      ctx,
      initAdminTx,
      [groupAdmin.wallet, adminAccountKeypair],
      false,
      true,
    );

    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      BORROW_SEED,
    );
    borrowBank = bankKey;

    const config = defaultBankConfig();
    config.interestRateConfig.protocolOriginationFee =
      bigNumberToWrappedI80F48(0);

    const addBankTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        config,
        seed: BORROW_SEED,
      }),
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const configOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: borrowBank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.tokenAOracle.publicKey,
      }),
    );
    await processBankrunTransaction(ctx, configOracleTx, [groupAdmin.wallet]);

    const seedAmount = new BN(100 * 10 ** ecosystem.tokenADecimals);
    const seedTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: borrowBank,
        tokenAccount: groupAdmin.tokenAAccount,
        amount: seedAmount,
      }),
    );
    await processBankrunTransaction(ctx, seedTx, [groupAdmin.wallet]);
  });

  it("(attacker) pulse bank price with wrong reserve - should fail", async () => {
    const user = users[3];
    const tx = new Transaction().add(
      await pulseBankPrice(user.mrgnBankrunProgram, {
        group: kaminoGroup.publicKey,
        bank: fixedKaminoBank,
        remaining: [tokenAReserve],
      }),
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [user.wallet],
      true,
    );
    // KaminoReserveValidationFailed
    assertBankrunTxFailed(result, 6210);
  });

  it("(user 3) deposit into fixed Kamino bank - happy path", async () => {
    const user = users[3];
    const depositAmount = new BN(1_000 * 10 ** ecosystem.usdcDecimals);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    userUsdcStart = userUsdcBefore;

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        fixedKaminoObligation,
        [usdcReserve],
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: fixedKaminoBank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        depositAmount,
      ),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    const diff = userUsdcBefore - userUsdcAfter;
    console.log("deposited: " + diff.toLocaleString());
    assert.equal(userUsdcBefore - userUsdcAfter, depositAmount.toNumber());

    const reserveAfterDepositRaw =
      await klendBankrunProgram.account.reserve.fetch(usdcReserve);
    const reserveAfterDeposit = { ...reserveAfterDepositRaw } as Reserve;
    console.log(
      "Kamino reserve after deposit:",
      "\n available",
      getLiquidityAvailableAmount(reserveAfterDeposit).toString(),
      "\n total",
      getTotalSupply(reserveAfterDeposit).toString(),
      "\n liq/coll",
      getLiquidityExchangeRate(reserveAfterDeposit).toString(),
      "\n coll/liq",
      getCollateralExchangeRate(reserveAfterDeposit).toString(),
    );
  });

  it("(user 3) borrow Token A against fixed Kamino collateral - happy path", async () => {
    const user = users[3];

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount,
    );

    const remaining = composeRemainingAccounts([
      [fixedKaminoBank, usdcReserve],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        remaining,
        amount: BORROW_AMOUNT,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount,
    );
    assert.equal(userTokenAAfter - userTokenABefore, BORROW_AMOUNT.toNumber());
  });

  it("(user 3) health pulse reports expected valuation", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const remaining = composeRemainingAccounts([
      [fixedKaminoBank, usdcReserve],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const cache = accAfter.healthCache;
    logHealthCache("cache after deposit", cache);

    const actualAssetValue = wrappedI80F48toBigNumber(
      cache.assetValue,
    ).toNumber();
    const actualLiabilityValue = wrappedI80F48toBigNumber(
      cache.liabilityValue,
    ).toNumber();

    // Note: The way this actually works is convoluted: Before the Fixed Price update to Kamino, the
    // internal value that would be reported is liqudity * (col/liq exchange rate, e.g. 1000 USDC
    // value * (1 / 1.00258) here). When we now multiply by the (liq/col) ratio in
    // `try_from_bank_with_max_age`, we're just undoing that to get back to the actual value of your
    // 1000 USDC deposit.
    const expectedAssetValue = FIXED_PRICE * 1000;
    // 10 tokens (at high price bias)
    const expectedLiabilityValue =
      oracles.tokenAPrice * (1 + CONF_INTERVAL_MULTIPLE_FLOAT) * 10;

    const assetTolerance = Math.max(0.01, expectedAssetValue * 0.005);
    const liabTolerance = Math.max(0.01, expectedLiabilityValue * 0.005);

    assert.approximately(actualAssetValue, expectedAssetValue, assetTolerance);
    assert.approximately(
      actualLiabilityValue,
      expectedLiabilityValue,
      liabTolerance,
    );
  });

  it("(user 3) withdraw from fixed Kamino bank - happy path", async () => {
    const user = users[3];
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    const reserveBeforeWithdrawRaw =
      await klendBankrunProgram.account.reserve.fetch(usdcReserve);
    const reserveBeforeWithdraw = { ...reserveBeforeWithdrawRaw } as Reserve;
    const exchangeRateBeforeWithdraw = getLiquidityExchangeRate(
      reserveBeforeWithdraw,
    );

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );

    const remaining = composeRemainingAccounts([
      [fixedKaminoBank, usdcReserve],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        fixedKaminoObligation,
        [usdcReserve],
      ),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          bank: fixedKaminoBank,
          mint: ecosystem.usdcMint.publicKey,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        {
          amount: withdrawAmount,
          isWithdrawAll: false,
          remaining,
        },
      ),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    const diff = userUsdcAfter - userUsdcBefore;
    console.log("withdrew: " + diff.toLocaleString());

    const expectedWithdraw = exchangeRateBeforeWithdraw
      .mul(withdrawAmount.toString())
      .toNumber();
    assert.approximately(diff, expectedWithdraw, 2);

    const reserveAfterWithdrawRaw =
      await klendBankrunProgram.account.reserve.fetch(usdcReserve);
    const reserveAfterWithdraw = { ...reserveAfterWithdrawRaw } as Reserve;
    const availableDelta = getLiquidityAvailableAmount(
      reserveBeforeWithdraw,
    ).sub(getLiquidityAvailableAmount(reserveAfterWithdraw));
    const totalDelta = getTotalSupply(reserveBeforeWithdraw).sub(
      getTotalSupply(reserveAfterWithdraw),
    );

    assert.approximately(availableDelta.toNumber(), diff, 2);
    assert.approximately(totalDelta.toNumber(), diff, 2);
    console.log(
      "Kamino reserve after withdraw:",
      "\n available",
      getLiquidityAvailableAmount(reserveAfterWithdraw).toString(),
      "\n total",
      getTotalSupply(reserveAfterWithdraw).toString(),
      "\n liq/coll",
      getLiquidityExchangeRate(reserveAfterWithdraw).toString(),
      "\n coll/liq",
      getCollateralExchangeRate(reserveAfterWithdraw).toString(),
    );
  });

  it("(user 3) repay borrow and withdraw all - gets initial deposit back", async () => {
    const user = users[3];

    const repayTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        amount: BORROW_AMOUNT,
        repayAll: true,
        remaining: composeRemainingAccounts([
          [fixedKaminoBank, usdcReserve],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      }),
    );
    await processBankrunTransaction(ctx, repayTx, [user.wallet]);

    const remaining = composeRemainingAccounts(
      [
        [fixedKaminoBank, usdcReserve],
        [borrowBank, oracles.tokenAOracle.publicKey],
      ].filter((group) => !group[0].equals(fixedKaminoBank))
    );

    const withdrawAllTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        fixedKaminoObligation,
        [usdcReserve],
      ),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          bank: fixedKaminoBank,
          mint: ecosystem.usdcMint.publicKey,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        {
          amount: new BN(0),
          isWithdrawAll: true,
          remaining,
        },
      ),
    );
    await processBankrunTransaction(ctx, withdrawAllTx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    // Note: you lose 1-2 lamports for Kamino withdraws
    assert.approximately(userUsdcAfter, userUsdcStart, 2);
    assert.isAtMost(userUsdcAfter, userUsdcStart);

    // has_kamino clears once the last Kamino position is withdrawn
    const userAccAfter =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    assert.equal(userAccAfter.indexerFlags.hasKamino, 0);
  });
});
