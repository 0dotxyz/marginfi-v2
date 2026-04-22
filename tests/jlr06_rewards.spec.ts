import { BN } from "@coral-xyz/anchor";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { createAssociatedTokenAccountIdempotentInstruction } from "@solana/spl-token";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertBNEqual,
  assertBNGreaterThan,
  assertI80F48Approx,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";
import {
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
} from "./utils/juplend/juplend-pdas";
import {
  initJuplendClaimAccountIx,
  startJuplendRewardsIx,
  stopJuplendRewardsIx,
} from "./utils/juplend/admin-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import type { JuplendPoolKeys } from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  advanceBankrunClock,
  buildHealthRemainingAccounts,
  getUserAssetShares,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import { accountInit, healthPulse } from "./utils/user-instructions";

const EXCHANGE_PRICES_PRECISION = new BN("1000000000000");
const USER_0_ACCOUNT_SEED = Buffer.from("JLR06_USER0_ACCOUNT_SEED_0000000");
const USER_1_ACCOUNT_SEED = Buffer.from("JLR06_USER1_ACCOUNT_SEED_0000000");
const userMarginfiAccounts = [
  Keypair.fromSeed(USER_0_ACCOUNT_SEED),
  Keypair.fromSeed(USER_1_ACCOUNT_SEED),
];

const usdc = (ui: number) => new BN(ui * 10 ** ecosystem.usdcDecimals);
const REWARD_DURATION_SECONDS = new BN(24 * 60 * 60);
const REWARD_ACCRUAL_SECONDS_TOTAL = 3 * 60 * 60;
const HALF_ACCRUAL_SECONDS = REWARD_ACCRUAL_SECONDS_TOTAL / 2;
// Note: this dwarfs deposits in previous tests so these users basically get all of the rewards
// minus a tiny tolerance
const USER_DEPOSIT_AMOUNT_BN = usdc(100000);
const USER_DEPOSIT_AMOUNT = USER_DEPOSIT_AMOUNT_BN.toNumber();
const REWARD_AMOUNT = usdc(24);

const oneUsdc = new BN(10 ** ecosystem.usdcDecimals);
const totalWindowRewards = REWARD_AMOUNT.mul(
  new BN(REWARD_ACCRUAL_SECONDS_TOTAL),
).div(REWARD_DURATION_SECONDS);
const halfWindowRewards = REWARD_AMOUNT.mul(new BN(HALF_ACCRUAL_SECONDS)).div(
  REWARD_DURATION_SECONDS,
);
assertBNEqual(totalWindowRewards, oneUsdc.mul(new BN(3)));
assertBNEqual(halfWindowRewards, oneUsdc.mul(new BN(3)).div(new BN(2)));

describe("jlr06: Juplend rewards on wrapped deposits (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let groupPk = PublicKey.default;
  let usdcJupBankPk = PublicKey.default;
  let usdcJupPool: JuplendPoolKeys;
  let liqVaultAuth = PublicKey.default;
  let withdrawIntermediaryAtaPk = PublicKey.default;
  const decimals = 10 ** ecosystem.usdcDecimals;
  const exchangePricesPrecisionBig = new BigNumber(
    EXCHANGE_PRICES_PRECISION.toString(),
  );

  let user0UsdcBeforeDeposit = new BN(0);
  let user0UsdcAfterDeposit = new BN(0);
  let user0Shares = new BigNumber(0);
  let exchangePriceBeforeAccrual = new BN(0);
  let exchangePriceMidAccrual = new BN(0);
  let user1HealthAssetBefore = 0;
  let user1UsdcAfterBaselinePulse = new BN(0);
  let cacheMultiplierBeforeAccrual = new BigNumber(0);
  let cacheMultiplierMidAccrual = new BigNumber(0);

  const stopRewards = async () => {
    const stopIx = await stopJuplendRewardsIx(juplendPrograms, {
      authority: groupAdmin.wallet.publicKey,
      pool: usdcJupPool,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(stopIx),
      [groupAdmin.wallet],
      false,
      true,
    );
  };

  const getPoolSupplyShares = async (): Promise<BN> => {
    const supplyPosition =
      await juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      );
    return supplyPosition.amount;
  };

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);
    usdcJupBankPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);

    const bank = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    usdcJupPool = deriveJuplendPoolKeys({
      mint: bank.mint,
      mrgnProgramId: bankrunProgram.programId,
      bank: usdcJupBankPk,
    });
    withdrawIntermediaryAtaPk = bank.integrationAcc3;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      usdcJupBankPk,
    );
    liqVaultAuth = liquidityVaultAuthority;

    assertKeysEqual(
      withdrawIntermediaryAtaPk,
      usdcJupPool.withdrawIntermediaryAta!,
    );

    for (let i = 0; i < 2; i++) {
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        users[i].usdcAccount,
        usdc(300000),
      );
      const marginfiAccount = userMarginfiAccounts[i];
      const initUserIx = await accountInit(users[i].mrgnBankrunProgram!, {
        marginfiGroup: groupPk,
        marginfiAccount: marginfiAccount.publicKey,
        authority: users[i].wallet.publicKey,
        feePayer: users[i].wallet.publicKey,
      });
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(initUserIx),
        [users[i].wallet, marginfiAccount],
        false,
        true,
      );
    }

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        users[0].wallet.publicKey,
        withdrawIntermediaryAtaPk,
        liqVaultAuth,
        usdcJupPool.mint,
        usdcJupPool.tokenProgram,
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [users[0].wallet],
      false,
      true,
    );

    const initClaimIx = await initJuplendClaimAccountIx(juplendPrograms, {
      signer: users[0].wallet.publicKey,
      mint: usdcJupPool.mint,
      accountFor: liqVaultAuth,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initClaimIx),
      [users[0].wallet],
      false,
      true,
    );
  });

  after(async () => {
    await stopRewards();
  });

  it("(user 0) deposits funds", async () => {
    const user0mrgnAcc = userMarginfiAccounts[0].publicKey;

    user0UsdcBeforeDeposit = new BN(
      await getTokenBalance(bankRunProvider, users[0].usdcAccount),
    );

    const depositUser0Ix = await makeJuplendDepositIx(
      users[0].mrgnBankrunProgram!,
      {
        marginfiAccount: user0mrgnAcc,
        signerTokenAccount: users[0].usdcAccount,
        bank: usdcJupBankPk,
        pool: usdcJupPool,
        amount: USER_DEPOSIT_AMOUNT_BN,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositUser0Ix),
      [users[0].wallet],
      false,
      true,
    );

    user0UsdcAfterDeposit = new BN(
      await getTokenBalance(bankRunProvider, users[0].usdcAccount),
    );
    assertBNEqual(
      user0UsdcBeforeDeposit.sub(user0UsdcAfterDeposit),
      USER_DEPOSIT_AMOUNT_BN,
    );

    user0Shares = await getUserAssetShares(user0mrgnAcc, usdcJupBankPk);
    // i.e. Share value > 1 at this point due to minor interest accrued
    assert.isAbove(USER_DEPOSIT_AMOUNT_BN.toNumber(), user0Shares.toNumber());

    const bankAfterDeposit = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    cacheMultiplierBeforeAccrual = wrappedI80F48toBigNumber(
      bankAfterDeposit.cache.priceMultiplier,
    );
    assertI80F48Approx(bankAfterDeposit.cache.lastOraclePrice, oracles.usdcPrice, 0.000001);
  });

  it("(admin) starts rewards - 24 USDC per day", async () => {
    const supplySharesPhase1 = await getPoolSupplyShares();
    const supplySharesPhase1Big = new BigNumber(supplySharesPhase1.toString());
    assert.isTrue(supplySharesPhase1Big.gte(user0Shares));

    const lendingBeforeAccrual =
      await juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending);
    exchangePriceBeforeAccrual = lendingBeforeAccrual.tokenExchangePrice;

    const startRewardsIx = await startJuplendRewardsIx(juplendPrograms, {
      authority: groupAdmin.wallet.publicKey,
      pool: usdcJupPool,
      rewardAmount: REWARD_AMOUNT,
      duration: REWARD_DURATION_SECONDS,
      startTime: new BN(0),
      startTvl: new BN(0),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(startRewardsIx),
      [groupAdmin.wallet],
      false,
      true,
    );
  });

  it("1.5 hours elapses, then (user 1) deposits the same amount of funds", async () => {
    await advanceBankrunClock(bankrunContext, HALF_ACCRUAL_SECONDS);
    // Note: we must refresh to accrue rewards, but makeJuplendDepositIx refreshes internally, this
    // is just to demonstrate that the exchange rate has increased.
    const refreshIX = await refreshJupSimple(juplendPrograms.lending, {
      pool: usdcJupPool,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshIX),
      [users[0].wallet],
    );
    const lendingMidAccrual =
      await juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending);
    exchangePriceMidAccrual = lendingMidAccrual.tokenExchangePrice;
    assertBNGreaterThan(exchangePriceMidAccrual, exchangePriceBeforeAccrual);

    const user1mrgnAcc = userMarginfiAccounts[1].publicKey;
    const user1UsdcBeforeDeposit = await getTokenBalance(
      bankRunProvider,
      users[1].usdcAccount,
    );
    const depositUser1Ix = await makeJuplendDepositIx(
      users[1].mrgnBankrunProgram!,
      {
        marginfiAccount: user1mrgnAcc,
        signerTokenAccount: users[1].usdcAccount,
        bank: usdcJupBankPk,
        pool: usdcJupPool,
        amount: USER_DEPOSIT_AMOUNT_BN,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositUser1Ix),
      [users[1].wallet],
      false,
      true,
    );

    const user1UsdcAfterDeposit = await getTokenBalance(
      bankRunProvider,
      users[1].usdcAccount,
    );
    assert.equal(
      user1UsdcBeforeDeposit - user1UsdcAfterDeposit,
      USER_DEPOSIT_AMOUNT,
    );

    const bankMidAccrual = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    const expectedMidCacheMultiplier =
      Number(exchangePriceMidAccrual.toString()) /
      Number(EXCHANGE_PRICES_PRECISION.toString());
    assertI80F48Approx(
      bankMidAccrual.cache.priceMultiplier,
      expectedMidCacheMultiplier,
      expectedMidCacheMultiplier / 10000, // .001%
    );
    cacheMultiplierMidAccrual = wrappedI80F48toBigNumber(
      bankMidAccrual.cache.priceMultiplier,
    );
    assert.isTrue(cacheMultiplierMidAccrual.gte(cacheMultiplierBeforeAccrual));
    assertI80F48Approx(bankMidAccrual.cache.lastOraclePrice, oracles.usdcPrice, 0.000001);

    const user1Shares = await getUserAssetShares(user1mrgnAcc, usdcJupBankPk);
    if (verbose) {
      console.log(
        "user 0 shares: " +
          user0Shares.toString() +
          "\nuser 1 shares " +
          user1Shares.toString(),
      );
    }
    // Note: Since B deposited after A, and some rewards were earned, between the deposits, B gets
    // *nominally* fewer shares. Here we show B < A * 99.99
    assert.isTrue(user0Shares.gt(user1Shares));
    assert.isTrue(
      user1Shares.multipliedBy(1000).gte(user0Shares.multipliedBy(999)),
    );

    const supplySharesPhase2 = await getPoolSupplyShares();
    const halfWindowRewardsBig = new BigNumber(halfWindowRewards.toString());
    const supplySharesPhase2Big = new BigNumber(supplySharesPhase2.toString());
    const user0ExpRewardHalf2 = halfWindowRewardsBig
      .multipliedBy(user0Shares)
      .dividedBy(supplySharesPhase2Big)
      .dividedBy(decimals)
      .toNumber();
    const user1ExpRewardHalf2 = halfWindowRewardsBig
      .multipliedBy(user1Shares)
      .dividedBy(supplySharesPhase2Big)
      .dividedBy(decimals)
      .toNumber();
    const rewardsDiff = user0ExpRewardHalf2 - user1ExpRewardHalf2;
    if (verbose) {
      console.log("AFTER FIRST HALF");
      console.log(
        " user 0 reward: $" +
          user0ExpRewardHalf2 +
          "\n user 1 reward: $" +
          user1ExpRewardHalf2,
        "\n  diff: $" + rewardsDiff,
      );
    }
    // Again we note that user 0 gets *slightly* more because they deposited earlier.
    assert.isAbove(user0ExpRewardHalf2, user1ExpRewardHalf2);
    assert.approximately(user0ExpRewardHalf2, user1ExpRewardHalf2, 0.001);
    // Both users are entitled to roughly half of the $1.5 in awards accrued so far.
    assert.approximately(user0ExpRewardHalf2, 0.75, 0.01);

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const user1RemainingBefore = await buildHealthRemainingAccounts(
      user1mrgnAcc,
    );
    const pulseUser1BeforeIx = await healthPulse(users[1].mrgnBankrunProgram!, {
      marginfiAccount: user1mrgnAcc,
      remaining: user1RemainingBefore,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseUser1BeforeIx),
      [users[1].wallet],
      false,
      true,
    );

    const user1BeforeSecondHalfPulse =
      await bankrunProgram.account.marginfiAccount.fetch(user1mrgnAcc);
    user1HealthAssetBefore = wrappedI80F48toBigNumber(
      user1BeforeSecondHalfPulse.healthCache.assetValue,
    ).toNumber();
    user1UsdcAfterBaselinePulse = new BN(
      await getTokenBalance(bankRunProvider, users[1].usdcAccount),
    );
  });

  it("1.5 hours elapses (3 hours total) - users earn expected amount", async () => {
    await advanceBankrunClock(bankrunContext, HALF_ACCRUAL_SECONDS);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const user0mrgnAccount = userMarginfiAccounts[0].publicKey;
    const user1mrgnAccount = userMarginfiAccounts[1].publicKey;
    const user1RemainingAfter = await buildHealthRemainingAccounts(
      user1mrgnAccount,
    );
    const refreshRateSecondHalfIx = await refreshJupSimple(
      juplendPrograms.lending,
      {
        pool: usdcJupPool,
      },
    );
    const pulseUser1AfterIx = await healthPulse(users[1].mrgnBankrunProgram!, {
      marginfiAccount: user1mrgnAccount,
      remaining: user1RemainingAfter,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshRateSecondHalfIx, pulseUser1AfterIx),
      [users[1].wallet],
      false,
      true,
    );

    const lendingAfterAccrual =
      await juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending);
    const exchangePriceAfterAccrual = lendingAfterAccrual.tokenExchangePrice;
    assertBNGreaterThan(exchangePriceAfterAccrual, exchangePriceMidAccrual);

    const bankAfterAccrual = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    const expectedAfterCacheMultiplier =
      Number(exchangePriceAfterAccrual.toString()) /
      Number(EXCHANGE_PRICES_PRECISION.toString());
    assertI80F48Approx(
      bankAfterAccrual.cache.priceMultiplier,
      expectedAfterCacheMultiplier,
      expectedAfterCacheMultiplier / 10000, // .001%
    );
    const cacheMultiplierAfterAccrual = wrappedI80F48toBigNumber(
      bankAfterAccrual.cache.priceMultiplier,
    );
    assert.isTrue(cacheMultiplierAfterAccrual.gte(cacheMultiplierMidAccrual));
    assertI80F48Approx(bankAfterAccrual.cache.lastOraclePrice, oracles.usdcPrice, 0.000001);

    const user1AccountAfterSecondHalfPulse =
      await bankrunProgram.account.marginfiAccount.fetch(user1mrgnAccount);
    const user1HealthAssetAfter = wrappedI80F48toBigNumber(
      user1AccountAfterSecondHalfPulse.healthCache.assetValue,
    ).toNumber();
    const healthDiff = user1HealthAssetAfter - user1HealthAssetBefore;
    if (verbose) {
      console.log("");
      console.log(
        " user 1 health before: " +
          user1HealthAssetBefore +
          "\n user 1 health after " +
          user1HealthAssetAfter,
        "\n  diff " + healthDiff,
      );
    }

    assert.isAbove(user1HealthAssetAfter, user1HealthAssetBefore);
    // Half of the $1.5 user 1 is entitled to is 75 cents, however a collateral discount applies
    // here, banks are weighted at 0.8, and the confidence discount applies:
    // * 0.75 * .8 * (1-.0212) ~= 0.58728
    assert.approximately(healthDiff, 0.58728, 0.03);

    const user1UsdcAfterSecondHalfPulse = new BN(
      await getTokenBalance(bankRunProvider, users[1].usdcAccount),
    );
    // No withdraw, ergo no USDC gain, just a paper gain
    assertBNEqual(user1UsdcAfterSecondHalfPulse, user1UsdcAfterBaselinePulse);

    const withdrawAllIx = await makeJuplendWithdrawSimpleIx(
      users[0].mrgnBankrunProgram!,
      {
        marginfiAccount: user0mrgnAccount,
        destinationTokenAccount: users[0].usdcAccount,
        bank: usdcJupBankPk,
        pool: usdcJupPool,
        amount: new BN(0),
        withdrawAll: true,
        remainingAccounts: await buildHealthRemainingAccounts(user0mrgnAccount),
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawAllIx),
      [users[0].wallet],
      false,
      true,
    );

    const user0UsdcAfterWithdraw = new BN(
      await getTokenBalance(bankRunProvider, users[0].usdcAccount),
    );
    const withdrawnAmount = user0UsdcAfterWithdraw.sub(user0UsdcAfterDeposit);

    assertBNGreaterThan(withdrawnAmount, USER_DEPOSIT_AMOUNT_BN);
    assertBNGreaterThan(user0UsdcAfterWithdraw, user0UsdcBeforeDeposit);

    const expectedWithdrawnFromRate = user0Shares
      .multipliedBy(exchangePriceAfterAccrual.toString())
      .dividedBy(exchangePricesPrecisionBig)
      .toNumber();
    // Note: in native USDC decimals
    assertBNApproximately(withdrawnAmount, expectedWithdrawnFromRate, 2);

    const realizedYield =
      withdrawnAmount.sub(USER_DEPOSIT_AMOUNT_BN).toNumber() / decimals;
    if (verbose) {
      console.log("AFTER 3 HOURS TOTAL");
      console.log("user 0 realized: $" + realizedYield);
    }
    // Note: $2.25 total: all of the $1.5 from the first half, and half of the $1.5 from the
    // second half after user 0 deposits.
    assert.approximately(realizedYield, 1.5 + (1 / 2) * 1.5, 0.01);
  });
});
