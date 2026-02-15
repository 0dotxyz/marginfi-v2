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
  oracles,
  users,
  verbose,
  driftGroup,
  driftAccounts,
  DRIFT_USDC_SPOT_MARKET,
  DRIFT_TOKEN_A_SPOT_MARKET,
  driftBankrunProgram,
} from "./rootHooks";
import {
  accountInit,
  depositIx,
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsByBalances,
  healthPulse,
  pulseBankPrice,
  repayIx,
} from "./utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  setFixedPrice,
} from "./utils/group-instructions";
import { deriveBankWithSeed } from "./utils/pdas";
import { assert } from "chai";
import {
  assertBankrunTxFailed,
  assertBNApproximately,
  assertBNEqual,
  getTokenBalance,
} from "./utils/genericTests";
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
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  defaultDriftBankConfig,
  getDriftUserAccount,
  getSpotMarketAccount,
  scaledBalanceToTokenAmount,
  tokenAmountToScaledBalance,
  USDC_INIT_DEPOSIT_AMOUNT,
  USDC_SCALING_FACTOR,
} from "./utils/drift-utils";
import {
  makeAddDriftBankIx,
  makeDriftDepositIx,
  makeDriftWithdrawIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";

let ctx: ProgramTestContext;
let usdcSpotMarket: PublicKey;
let tokenASpotMarket: PublicKey;
let fixedDriftBank: PublicKey;
let userAccount: PublicKey;
let borrowBank: PublicKey;
let adminAccount: PublicKey;
let userUsdcStart = 0;

const FIXED_SEED = new BN(7742);
const BORROW_SEED = new BN(8842);
// Note: USDC is not worth $2, so this test is silly
const FIXED_PRICE = 2;
const BORROW_AMOUNT = new BN(10 * 10 ** ecosystem.tokenADecimals);

describe("dx: Fixed Drift price bank", () => {
  before(async () => {
    ctx = bankrunContext;
    usdcSpotMarket = driftAccounts.get(DRIFT_USDC_SPOT_MARKET);
    tokenASpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
  });

  it("(user 3) initialize marginfi account for main group", async () => {
    const user = users[3];
    const accountKeypair = Keypair.generate();
    userAccount = accountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);
  });

  it("(admin) add fixed Drift USDC bank + init user", async () => {
    const defaultConfig = defaultDriftBankConfig(oracles.usdcOracle.publicKey);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      driftGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      FIXED_SEED,
    );
    fixedDriftBank = bankKey;

    const addBankTx = new Transaction().add(
      await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: usdcSpotMarket,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: FIXED_SEED,
        },
      ),
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const initUserTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await makeInitDriftUserIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: users[3].wallet.publicKey,
          bank: fixedDriftBank,
          signerTokenAccount: users[3].usdcAccount,
        },
        {
          amount: USDC_INIT_DEPOSIT_AMOUNT,
        },
        0,
      ),
    );
    await processBankrunTransaction(ctx, initUserTx, [users[3].wallet]);

    const setFixedTx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: fixedDriftBank,
        price: FIXED_PRICE,
        remaining: [usdcSpotMarket],
      }),
    );
    await processBankrunTransaction(ctx, setFixedTx, [groupAdmin.wallet]);

    if (verbose) {
      console.log("Fixed Drift bank:", fixedDriftBank.toString());
    }
  });

  it("(admin) add throwaway regular Token A bank + seed liquidity", async () => {
    const adminAccountKeypair = Keypair.generate();
    adminAccount = adminAccountKeypair.publicKey;

    const initAdminTx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
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
      driftGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      BORROW_SEED,
    );
    borrowBank = bankKey;

    const config = defaultBankConfig();
    config.interestRateConfig.protocolOriginationFee =
      bigNumberToWrappedI80F48(0);

    const addBankTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
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

  it("(attacker) pulse bank price with wrong spot market - should fail", async () => {
    const user = users[3];
    const tx = new Transaction().add(
      await pulseBankPrice(user.mrgnBankrunProgram, {
        group: driftGroup.publicKey,
        bank: fixedDriftBank,
        remaining: [tokenASpotMarket],
      }),
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [user.wallet],
      true,
    );
    // DriftSpotMarketValidationFailed
    assertBankrunTxFailed(result, 6304);
  });

  it("(user 3) deposit into fixed Drift bank - happy path", async () => {
    const user = users[3];
    const depositAmount = new BN(1_000 * 10 ** ecosystem.usdcDecimals);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    userUsdcStart = userUsdcBefore;

    const tx = new Transaction().add(
      await makeDriftDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: fixedDriftBank,
          signerTokenAccount: user.usdcAccount,
        },
        depositAmount,
        0,
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

    const bank = await bankrunProgram.account.bank.fetch(fixedDriftBank);
    const driftUserAfterDeposit = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2,
    );
    const scaledBalanceAfterDeposit =
      driftUserAfterDeposit.spotPositions[0].scaledBalance;

    const spotMarket = await getSpotMarketAccount(driftBankrunProgram, 0);
    const scaledBalance = tokenAmountToScaledBalance(depositAmount.add(USDC_INIT_DEPOSIT_AMOUNT), spotMarket);

    assertBNApproximately(scaledBalanceAfterDeposit, scaledBalance, 1);
  });

  it("(user 3) borrow Token A against fixed Drift collateral - happy path", async () => {
    const user = users[3];

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount,
    );

    const remaining = composeRemainingAccounts([
      [fixedDriftBank, usdcSpotMarket],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
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
      [fixedDriftBank, usdcSpotMarket],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
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

    // Note: The way this actually works is convoluted: Before the Fixed Price update to Drift, the
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

  it("(user 3) withdraw from fixed Drift bank - happy path", async () => {
    const user = users[3];
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    const bank = await bankrunProgram.account.bank.fetch(fixedDriftBank);
    const driftUserBeforeWithdraw = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2,
    );
    const scaledBalanceBeforeWithdraw =
      driftUserBeforeWithdraw.spotPositions[0].scaledBalance;

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );

    const remaining = composeRemainingAccounts([
      [fixedDriftBank, usdcSpotMarket],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await makeDriftWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: fixedDriftBank,
          destinationTokenAccount: user.usdcAccount,
        },
        {
          amount: withdrawAmount,
          withdrawAll: false,
          remaining,
        },
        driftBankrunProgram,
      ),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    const diff = userUsdcAfter - userUsdcBefore;
    console.log("withdrew: " + diff.toLocaleString());

    const driftUserAfterWithdraw = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2,
    );
    const scaledBalanceAfterWithdraw =
      driftUserAfterWithdraw.spotPositions[0].scaledBalance;

    const spotMarket = await getSpotMarketAccount(driftBankrunProgram, 0);
    const scaledBalanceDiff = tokenAmountToScaledBalance(withdrawAmount, spotMarket);

    assertBNApproximately(scaledBalanceBeforeWithdraw.sub(scaledBalanceAfterWithdraw), scaledBalanceDiff, 1);
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
          [fixedDriftBank, usdcSpotMarket],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      }),
    );
    await processBankrunTransaction(ctx, repayTx, [user.wallet]);

    const accountStateAfterRepay =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const remaining = composeRemainingAccountsByBalances(
      accountStateAfterRepay.lendingAccount.balances,
      [
        [fixedDriftBank, usdcSpotMarket],
        [borrowBank, oracles.tokenAOracle.publicKey],
      ],
      fixedDriftBank,
    );

    const withdrawAllTx = new Transaction().add(
      await makeDriftWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: fixedDriftBank,
          destinationTokenAccount: user.usdcAccount,
        },
        {
          amount: new BN(0),
          withdrawAll: true,
          remaining,
        },
        driftBankrunProgram,
      ),
    );
    await processBankrunTransaction(ctx, withdrawAllTx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );

    // We lose 1 lamport when deposit and immediately withdraw all (see details in drift_withdraw instruction)
    assert.equal(userUsdcAfter, userUsdcStart - 1);
  });
});
