import { BN } from "@coral-xyz/anchor";
import BigNumber from "bignumber.js";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import {
  configureBank,
  groupConfigure,
} from "./utils/group-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  EMODE_SEED,
  emodeGroup,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assertBankrunTxFailed } from "./utils/genericTests";
import {
  blankBankConfigOptRaw,
  CONF_INTERVAL_MULTIPLE_FLOAT,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
} from "./utils/types";
import { deriveBankWithSeed } from "./utils/pdas";
import {
  bigNumberToWrappedI80F48,
  WrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  withdrawIx,
} from "./utils/user-instructions";
import {
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  assertSameAssetBadDebtSurvivability,
  computeSameAssetBoundaryBorrowNative,
  setAssetShareValueHaircut,
  warpToNextBankrunSlot,
} from "./utils/same-asset-emode";

const seed = new BN(EMODE_SEED);
let usdcBankA: PublicKey;
let usdcBankB: PublicKey;
let solBank: PublicKey;

const DEFAULT_INIT_LEVERAGE = 99;
const DEFAULT_MAINT_LEVERAGE = 100;
// The boundary helper requires `tightenedRequirementLeverage < healthyInitLeverage` in the
// no-haircut path so the borrow lands inside a positive gap.
const BOUNDARY_TIGHTENED_LEVERAGE = DEFAULT_INIT_LEVERAGE - 1;
const DEFAULT_LIAB_WEIGHT = 1;
const RiskEngineInitRejected = "0x1779";

const getNetHealth = (cache: {
  assetValue: WrappedI80F48;
  liabilityValue: WrappedI80F48;
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
}) => {
  const init = wrappedI80F48toBigNumber(cache.assetValue).minus(
    wrappedI80F48toBigNumber(cache.liabilityValue)
  );
  const maint = wrappedI80F48toBigNumber(cache.assetValueMaint).minus(
    wrappedI80F48toBigNumber(cache.liabilityValueMaint)
  );
  return { init, maint };
};

const sameAssetRemaining = () =>
  [
    [usdcBankA, oracles.usdcOracle.publicKey],
    [usdcBankB, oracles.usdcOracle.publicKey],
  ] as PublicKey[][];

const sameAssetWithSolRemaining = () =>
  [
    [usdcBankA, oracles.usdcOracle.publicKey],
    [usdcBankB, oracles.usdcOracle.publicKey],
    [solBank, oracles.wsolOracle.publicKey],
  ] as PublicKey[][];

describe("Same-asset emode safety", () => {
  const setSameAssetLeverage = async (
    initLeverage: number,
    maintLeverage: number
  ) => {
    const tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(initLeverage),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(maintLeverage),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  };

  const setBankLiabilityWeights = async (
    bank: PublicKey,
    liabWeightInit: number,
    liabWeightMaint: number
  ) => {
    const bankConfigOpt = blankBankConfigOptRaw();
    bankConfigOpt.liabilityWeightInit =
      bigNumberToWrappedI80F48(liabWeightInit);
    bankConfigOpt.liabilityWeightMaint =
      bigNumberToWrappedI80F48(liabWeightMaint);
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank,
        bankConfigOpt,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  };

  const initFreshAccount = async (user: (typeof users)[number]) => {
    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
      accountKeypair,
    ]);
    return accountKeypair.publicKey;
  };

  const boundaryBorrow = (depositAmount: BN) =>
    computeSameAssetBoundaryBorrowNative({
      collateralNative: depositAmount,
      collateralDecimals: ecosystem.usdcDecimals,
      collateralPrice: ecosystem.usdcPrice,
      liabilityDecimals: ecosystem.usdcDecimals,
      liabilityPrice: ecosystem.usdcPrice,
      healthyInitLeverage: DEFAULT_INIT_LEVERAGE,
      tightenedRequirementLeverage: BOUNDARY_TIGHTENED_LEVERAGE,
    });

  before(async () => {
    [usdcBankA] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );
    [usdcBankB] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed.addn(1)
    );
    [solBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      seed
    );
    await warpToNextBankrunSlot(bankrunContext);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    for (const user of users) {
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        new BN(5_000 * 10 ** ecosystem.usdcDecimals)
      );
      await mintToTokenAccount(
        ecosystem.wsolMint.publicKey,
        user.wsolAccount,
        new BN(50 * 10 ** ecosystem.wsolDecimals)
      );
    }

    // Seed bank-B liquidity so the self-loop test has enough to draw against.
    const seedUser = users[2];
    const seedUserAccountSeed = Keypair.generate();
    const seedAccountInitTx = new Transaction().add(
      await accountInit(seedUser.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        marginfiAccount: seedUserAccountSeed.publicKey,
        authority: seedUser.wallet.publicKey,
        feePayer: seedUser.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, seedAccountInitTx, [
      seedUser.wallet,
      seedUserAccountSeed,
    ]);
    const seedDepositTx = new Transaction().add(
      await depositIx(seedUser.mrgnBankrunProgram, {
        marginfiAccount: seedUserAccountSeed.publicKey,
        bank: usdcBankB,
        tokenAccount: seedUser.usdcAccount,
        amount: new BN(4_000 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, seedDepositTx, [
      seedUser.wallet,
    ]);

    await setSameAssetLeverage(DEFAULT_INIT_LEVERAGE, DEFAULT_MAINT_LEVERAGE);
  });

  it("(user 0) opening a different-mint liability after a same-asset borrow is rejected", async () => {
    const user = users[0];
    const account = await initFreshAccount(user);
    const depositAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    const borrowAmount = boundaryBorrow(depositAmount);

    const openTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
        amount: borrowAmount,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
      })
    );
    await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);

    // Pre-state must be healthy AND boosted; otherwise the SOL-borrow rejection below could
    // be a pre-existing failure rather than the boost-collapse we want to detect.
    const preAcc = await bankrunProgram.account.marginfiAccount.fetch(account);
    assert.ok((preAcc.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    const preBoostRatio = wrappedI80F48toBigNumber(
      preAcc.healthCache.assetValueMaint
    ).div(wrappedI80F48toBigNumber(preAcc.healthCache.assetValueEquity));
    const expectedPreBoostRatio = new BigNumber(DEFAULT_MAINT_LEVERAGE - 1).div(
      DEFAULT_MAINT_LEVERAGE
    );
    assert.isTrue(
      preBoostRatio
        .minus(expectedPreBoostRatio)
        .abs()
        .lt(new BigNumber(0.0001)),
      `pre-borrow maint/equity should equal ${expectedPreBoostRatio.toFixed()}; got ${preBoostRatio.toFixed()}`
    );

    const breakTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts(sameAssetWithSolRemaining()),
        amount: new BN(1),
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      breakTx,
      [user.wallet],
      true,
      true
    );
    assertBankrunTxFailed(result, RiskEngineInitRejected);
  });

  it("(user 0) raising the liability-side weight of the single liab bank keeps a same-asset position healthy", async () => {
    const user = users[0];
    const account = await initFreshAccount(user);
    const depositAmount = new BN(200 * 10 ** ecosystem.usdcDecimals);
    const borrowAmount = boundaryBorrow(depositAmount);
    const elevatedLiabWeight = 2;

    try {
      const openTx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankA,
          tokenAccount: user.usdcAccount,
          amount: depositAmount,
          depositUpToLimit: false,
        }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankB,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
          amount: borrowAmount,
        }),
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
        })
      );
      await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);

      let acc = await bankrunProgram.account.marginfiAccount.fetch(account);
      const baselineAsset = wrappedI80F48toBigNumber(
        acc.healthCache.assetValue
      );
      const baselineLiab = wrappedI80F48toBigNumber(
        acc.healthCache.liabilityValue
      );
      assert.ok((acc.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
      assert.isTrue(getNetHealth(acc.healthCache).init.gt(0));

      await setBankLiabilityWeights(
        usdcBankB,
        elevatedLiabWeight,
        elevatedLiabWeight
      );
      const pulseTx = new Transaction().add(
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
        })
      );
      await processBankrunTransaction(bankrunContext, pulseTx, [user.wallet]);

      acc = await bankrunProgram.account.marginfiAccount.fetch(account);
      const adjustedAsset = wrappedI80F48toBigNumber(
        acc.healthCache.assetValue
      );
      const adjustedLiab = wrappedI80F48toBigNumber(
        acc.healthCache.liabilityValue
      );
      assert.ok((acc.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
      assert.isTrue(getNetHealth(acc.healthCache).init.gt(0));

      // Both sides scale by `elevatedLiabWeight / DEFAULT_LIAB_WEIGHT`. Tolerance absorbs
      // interest accrual between the two pulses and I80F48 quantization.
      const expectedRatio = new BigNumber(elevatedLiabWeight).div(
        DEFAULT_LIAB_WEIGHT
      );
      const ratioTolerance = new BigNumber(0.0001);
      const assetRatio = adjustedAsset.div(baselineAsset);
      const liabRatio = adjustedLiab.div(baselineLiab);
      assert.isTrue(
        assetRatio.minus(expectedRatio).abs().lt(ratioTolerance),
        `asset side should scale by exactly ${expectedRatio.toFixed()}; got ${assetRatio.toFixed()}`
      );
      assert.isTrue(
        liabRatio.minus(expectedRatio).abs().lt(ratioTolerance),
        `liab side should scale by exactly ${expectedRatio.toFixed()}; got ${liabRatio.toFixed()}`
      );
      assert.isTrue(
        assetRatio.minus(liabRatio).abs().lt(ratioTolerance),
        `asset and liab ratios should agree; asset=${assetRatio.toFixed()} liab=${liabRatio.toFixed()}`
      );
    } finally {
      await setBankLiabilityWeights(
        usdcBankB,
        DEFAULT_LIAB_WEIGHT,
        DEFAULT_LIAB_WEIGHT
      );
    }
  });

  it("(user 0) withdraw under same-asset boost succeeds up to the init boundary and rejects beyond it", async () => {
    const user = users[0];
    const account = await initFreshAccount(user);
    const collateralUi = new BigNumber(200);
    const depositAmount = new BN(
      collateralUi
        .times(10 ** ecosystem.usdcDecimals)
        .integerValue(BigNumber.ROUND_FLOOR)
        .toFixed(0)
    );
    const borrowAmount = boundaryBorrow(depositAmount);

    const openTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
        amount: borrowAmount,
      })
    );
    await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);

    // (C - W) * lower * (1 - 1/L_init) >= B * upper
    // → C_min = B * (1 + eps) / [(1 - eps) * (1 - 1/L_init)]
    const liabWithConfidence = new BigNumber(borrowAmount.toString())
      .div(new BigNumber(10).pow(ecosystem.usdcDecimals))
      .times(new BigNumber(1).plus(CONF_INTERVAL_MULTIPLE_FLOAT));
    const remainingCollateralUi = liabWithConfidence.div(
      new BigNumber(1)
        .minus(CONF_INTERVAL_MULTIPLE_FLOAT)
        .times(
          new BigNumber(DEFAULT_INIT_LEVERAGE - 1).div(DEFAULT_INIT_LEVERAGE)
        )
    );
    const maxWithdrawUi = collateralUi.minus(remainingCollateralUi);
    assert.isTrue(maxWithdrawUi.gt(0));
    // -1 native unit absorbs I80F48 rounding direction drift.
    const safeWithdrawNative = new BN(
      maxWithdrawUi
        .times(10 ** ecosystem.usdcDecimals)
        .integerValue(BigNumber.ROUND_FLOOR)
        .toFixed(0)
    ).subn(1);
    assert.isTrue(safeWithdrawNative.gtn(0));

    const successWithdraw = new Transaction().add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
        amount: safeWithdrawNative,
      })
    );
    await processBankrunTransaction(bankrunContext, successWithdraw, [
      user.wallet,
    ]);

    const pulseAtBoundary = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
      })
    );
    await processBankrunTransaction(bankrunContext, pulseAtBoundary, [
      user.wallet,
    ]);
    const acc = await bankrunProgram.account.marginfiAccount.fetch(account);
    assert.ok((acc.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    // 100 native units = $0.0001 — the precision the boundary is asserted to within.
    const overWithdraw = new Transaction().add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
        amount: new BN(100),
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      overWithdraw,
      [user.wallet],
      true,
      true
    );
    assertBankrunTxFailed(result, RiskEngineInitRejected);
  });

  it("(user 0) iterated deposit/borrow stops at the configured leverage cap", async () => {
    const user = users[0];
    const loopInit = 5;
    const loopMaint = 6;
    await setSameAssetLeverage(loopInit, loopMaint);

    try {
      const account = await initFreshAccount(user);
      const initialCollateralUi = new BigNumber(50);
      const initialDepositNative = new BN(
        initialCollateralUi
          .times(10 ** ecosystem.usdcDecimals)
          .integerValue(BigNumber.ROUND_FLOOR)
          .toFixed(0)
      );
      const seedTx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankA,
          tokenAccount: user.usdcAccount,
          amount: initialDepositNative,
          depositUpToLimit: false,
        })
      );
      await processBankrunTransaction(bankrunContext, seedTx, [user.wallet]);

      // Per-borrow constraint:  C * lower * (1 - 1/L_init) >= L * upper
      // Convergence:            L∞ = C0 * lower * boost / (upper - lower * boost)
      const lower = new BigNumber(1).minus(CONF_INTERVAL_MULTIPLE_FLOAT);
      const upper = new BigNumber(1).plus(CONF_INTERVAL_MULTIPLE_FLOAT);
      const boostWeight = new BigNumber(loopInit - 1).div(loopInit);
      const realizedMaxBorrowUi = initialCollateralUi
        .times(lower)
        .times(boostWeight)
        .div(upper.minus(lower.times(boostWeight)));

      let totalDepositedUi = initialCollateralUi;
      let totalBorrowedUi = new BigNumber(0);
      let iterationsRan = 0;

      // Geometric ratio per iteration is ~0.76 (lower*boost/upper), so 0.5 USDC headroom is
      // reached in ~15 iterations.
      const maxIterations = 20;
      const convergenceEpsilonUi = new BigNumber(0.5);
      for (let i = 0; i < maxIterations; i++) {
        // δ_max = (C * lower * boost - L * upper) / upper.  ×0.999 absorbs I80F48 drift.
        const maxAdditional = totalDepositedUi
          .times(lower)
          .times(boostWeight)
          .minus(totalBorrowedUi.times(upper))
          .div(upper)
          .times(0.999);
        if (maxAdditional.lte(convergenceEpsilonUi)) break;
        const stepBorrowNative = new BN(
          maxAdditional
            .times(10 ** ecosystem.usdcDecimals)
            .integerValue(BigNumber.ROUND_FLOOR)
            .toFixed(0)
        );
        if (stepBorrowNative.isZero()) break;
        const stepTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }),
          await borrowIx(user.mrgnBankrunProgram, {
            marginfiAccount: account,
            bank: usdcBankB,
            tokenAccount: user.usdcAccount,
            remaining: composeRemainingAccounts(sameAssetRemaining()),
            amount: stepBorrowNative,
          }),
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: account,
            bank: usdcBankA,
            tokenAccount: user.usdcAccount,
            amount: stepBorrowNative,
            depositUpToLimit: false,
          })
        );
        await processBankrunTransaction(bankrunContext, stepTx, [user.wallet]);
        const stepUi = new BigNumber(stepBorrowNative.toString()).div(
          10 ** ecosystem.usdcDecimals
        );
        totalBorrowedUi = totalBorrowedUi.plus(stepUi);
        totalDepositedUi = totalDepositedUi.plus(stepUi);
        iterationsRan++;
      }

      assert.isTrue(
        iterationsRan >= 3 && iterationsRan < maxIterations,
        `loop should converge within ${maxIterations} iterations; ran ${iterationsRan}`
      );

      // Off-chain tally is interest-free; assert it stayed under the cap and reached ≥95% of it.
      assert.isTrue(
        totalBorrowedUi.lt(realizedMaxBorrowUi),
        `total borrowed ${totalBorrowedUi.toFixed()} exceeded realised cap ${realizedMaxBorrowUi.toFixed()}`
      );
      assert.isTrue(
        totalBorrowedUi.gt(realizedMaxBorrowUi.times(0.95)),
        `loop converged short of the cap: total borrowed ${totalBorrowedUi.toFixed()} vs cap ${realizedMaxBorrowUi.toFixed()}`
      );

      // On-chain liability_value carries the upper-oracle factor; compare in matching units.
      const accAfterLoop =
        await bankrunProgram.account.marginfiAccount.fetch(account);
      const liabValueUi = wrappedI80F48toBigNumber(
        accAfterLoop.healthCache.liabilityValue
      ).div(ecosystem.usdcPrice);
      assert.isTrue(
        liabValueUi.lt(realizedMaxBorrowUi.times(upper)),
        `on-chain liability_value ${liabValueUi.toFixed()} exceeded cap-in-value-units ${realizedMaxBorrowUi.times(upper).toFixed()}`
      );

      // 1 USDC unambiguously overshoots the ≤0.5 USDC remaining headroom.
      const aggressiveBorrow = new BN(1 * 10 ** ecosystem.usdcDecimals);
      const overTx = new Transaction().add(
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankB,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
          amount: aggressiveBorrow,
        })
      );
      const overResult = await processBankrunTransaction(
        bankrunContext,
        overTx,
        [user.wallet],
        true,
        true
      );
      assertBankrunTxFailed(overResult, RiskEngineInitRejected);
    } finally {
      await setSameAssetLeverage(DEFAULT_INIT_LEVERAGE, DEFAULT_MAINT_LEVERAGE);
    }
  });

  it("(user 0) mint-match gate keeps SOL collateral at the plain weight while USDC is boosted", async () => {
    const user = users[0];
    const account = await initFreshAccount(user);
    const usdcDeposit = new BN(100 * 10 ** ecosystem.usdcDecimals);
    const solDeposit = new BN(10 * 10 ** ecosystem.wsolDecimals);
    const borrowAmount = boundaryBorrow(usdcDeposit);

    const openTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        amount: usdcDeposit,
        depositUpToLimit: false,
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: solDeposit,
        depositUpToLimit: false,
      }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(sameAssetWithSolRemaining()),
        amount: borrowAmount,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        remaining: composeRemainingAccounts(sameAssetWithSolRemaining()),
      })
    );
    await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(account);
    const assetValueInit = wrappedI80F48toBigNumber(acc.healthCache.assetValue);
    const lowerOracle = new BigNumber(1).minus(CONF_INTERVAL_MULTIPLE_FLOAT);
    const usdcUi = new BigNumber(usdcDeposit.toString()).div(
      10 ** ecosystem.usdcDecimals
    );
    const solUi = new BigNumber(solDeposit.toString()).div(
      10 ** ecosystem.wsolDecimals
    );
    // USDC: boosted weight (1 - 1/L_init).  SOL: plain bank-A asset weight (0.5).
    const expectedUsdcContribution = usdcUi
      .times(ecosystem.usdcPrice)
      .times(lowerOracle)
      .times(
        new BigNumber(DEFAULT_INIT_LEVERAGE - 1).div(DEFAULT_INIT_LEVERAGE)
      );
    const expectedSolContribution = solUi
      .times(ecosystem.wsolPrice)
      .times(lowerOracle)
      .times(0.5);
    const expectedAssetValue = expectedUsdcContribution.plus(
      expectedSolContribution
    );
    // Gate-broken counterfactual: SOL would also receive the USDC boost.
    const counterfactualSolContribution = solUi
      .times(ecosystem.wsolPrice)
      .times(lowerOracle)
      .times(
        new BigNumber(DEFAULT_INIT_LEVERAGE - 1).div(DEFAULT_INIT_LEVERAGE)
      );
    const counterfactualAssetValue = expectedUsdcContribution.plus(
      counterfactualSolContribution
    );

    const tolerance = expectedAssetValue.times(0.0001);
    assert.isTrue(
      assetValueInit.minus(expectedAssetValue).abs().lt(tolerance),
      `init asset_value ${assetValueInit.toFixed()} should be ~${expectedAssetValue.toFixed()}`
    );
    assert.isTrue(
      counterfactualAssetValue
        .minus(expectedAssetValue)
        .gt(tolerance.times(100)),
      "test setup must produce a clearly distinguishable counterfactual"
    );
  });

  for (const haircut of [
    { numerator: 999, denominator: 1000, label: "10bp" },
    { numerator: 399, denominator: 400, label: "25bp" },
  ]) {
    it(`(admin) same-asset survives a ${haircut.label} bad-debt haircut without consuming the equity buffer`, async () => {
      const user = users[3];
      const account = await initFreshAccount(user);
      const depositAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
      const borrowAmount = computeSameAssetBoundaryBorrowNative({
        collateralNative: depositAmount,
        collateralDecimals: ecosystem.usdcDecimals,
        collateralPrice: ecosystem.usdcPrice,
        liabilityDecimals: ecosystem.usdcDecimals,
        liabilityPrice: ecosystem.usdcPrice,
        healthyInitLeverage: DEFAULT_INIT_LEVERAGE,
        tightenedRequirementLeverage: DEFAULT_MAINT_LEVERAGE,
        haircut,
      });

      const openTx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankA,
          tokenAccount: user.usdcAccount,
          amount: depositAmount,
          depositUpToLimit: false,
        }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankB,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
          amount: borrowAmount,
        }),
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
        })
      );
      await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);

      let acc = await bankrunProgram.account.marginfiAccount.fetch(account);
      const originalAssetValueEquity = wrappedI80F48toBigNumber(
        acc.healthCache.assetValueEquity
      );
      assertSameAssetBadDebtSurvivability({
        healthCache: acc.healthCache,
        originalAssetValueEquity,
        label: `pre-haircut (${haircut.label})`,
        requireMaintenanceUnderwater: false,
      });

      let restoreShareValue: () => Promise<void> = async () => {};
      try {
        restoreShareValue = await setAssetShareValueHaircut(
          bankrunProgram,
          banksClient,
          bankrunContext,
          usdcBankA,
          haircut.numerator,
          haircut.denominator
        );
        await warpToNextBankrunSlot(bankrunContext);
        const pulseTx = new Transaction().add(
          await healthPulse(user.mrgnBankrunProgram, {
            marginfiAccount: account,
            remaining: composeRemainingAccounts(sameAssetRemaining()),
          })
        );
        await processBankrunTransaction(bankrunContext, pulseTx, [user.wallet]);

        acc = await bankrunProgram.account.marginfiAccount.fetch(account);
        assertSameAssetBadDebtSurvivability({
          healthCache: acc.healthCache,
          originalAssetValueEquity,
          label: `post-${haircut.label}-haircut`,
        });
      } finally {
        await restoreShareValue();
      }
    });
  }

  it("(user 0) same-asset reconciliation is unaffected by another user's bad-debt haircut", async () => {
    const userA = users[0];
    const userB = users[3];
    const userAAccount = await initFreshAccount(userA);
    const userBAccount = await initFreshAccount(userB);
    const depositAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    const borrowAmount = boundaryBorrow(depositAmount);

    for (const [user, account] of [
      [userA, userAAccount],
      [userB, userBAccount],
    ] as const) {
      const openTx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankA,
          tokenAccount: user.usdcAccount,
          amount: depositAmount,
          depositUpToLimit: false,
        }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: account,
          bank: usdcBankB,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
          amount: borrowAmount,
        })
      );
      await processBankrunTransaction(bankrunContext, openTx, [user.wallet]);
    }

    const baselinePulse = new Transaction().add(
      await healthPulse(userA.mrgnBankrunProgram, {
        marginfiAccount: userAAccount,
        remaining: composeRemainingAccounts(sameAssetRemaining()),
      })
    );
    await processBankrunTransaction(bankrunContext, baselinePulse, [
      userA.wallet,
    ]);
    let aAcc = await bankrunProgram.account.marginfiAccount.fetch(userAAccount);
    assert.ok((aAcc.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((aAcc.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((aAcc.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);
    const preAssetEquity = wrappedI80F48toBigNumber(
      aAcc.healthCache.assetValueEquity
    );
    const preAssetMaint = wrappedI80F48toBigNumber(
      aAcc.healthCache.assetValueMaint
    );
    // maint/equity = boost = (L_maint - 1) / L_maint. Plain fallback would give ~0.6/1.0.
    const expectedBoostRatio = new BigNumber(DEFAULT_MAINT_LEVERAGE - 1).div(
      DEFAULT_MAINT_LEVERAGE
    );
    const boostRatioTolerance = new BigNumber(0.0001);
    const preBoostRatio = preAssetMaint.div(preAssetEquity);
    assert.isTrue(
      preBoostRatio.minus(expectedBoostRatio).abs().lt(boostRatioTolerance),
      `pre-haircut maint/equity should equal ${expectedBoostRatio.toFixed()}; got ${preBoostRatio.toFixed()}`
    );

    let restoreShareValue: () => Promise<void> = async () => {};
    try {
      restoreShareValue = await setAssetShareValueHaircut(
        bankrunProgram,
        banksClient,
        bankrunContext,
        usdcBankA,
        199,
        200
      );
      await warpToNextBankrunSlot(bankrunContext);
      const haircutPulse = new Transaction().add(
        await healthPulse(userA.mrgnBankrunProgram, {
          marginfiAccount: userAAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining()),
        })
      );
      await processBankrunTransaction(bankrunContext, haircutPulse, [
        userA.wallet,
      ]);
      aAcc = await bankrunProgram.account.marginfiAccount.fetch(userAAccount);
      const postAssetEquity = wrappedI80F48toBigNumber(
        aAcc.healthCache.assetValueEquity
      );
      const postAssetMaint = wrappedI80F48toBigNumber(
        aAcc.healthCache.assetValueMaint
      );

      const postBoostRatio = postAssetMaint.div(postAssetEquity);
      assert.isTrue(
        postBoostRatio.minus(expectedBoostRatio).abs().lt(boostRatioTolerance),
        `post-haircut maint/equity should equal ${expectedBoostRatio.toFixed()}; got ${postBoostRatio.toFixed()}`
      );

      // Equity shrinks by exactly the haircut factor (199/200).
      const expectedEquityRatio = new BigNumber(199).div(200);
      const equityRatioTolerance = new BigNumber(0.0001);
      const equityRatio = postAssetEquity.div(preAssetEquity);
      assert.isTrue(
        equityRatio.minus(expectedEquityRatio).abs().lt(equityRatioTolerance),
        `equity value should reflect the 50bp haircut exactly (${expectedEquityRatio.toFixed()}); got ${equityRatio.toFixed()}`
      );
    } finally {
      await restoreShareValue();
    }
  });
});
