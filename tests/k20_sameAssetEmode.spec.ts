import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import BigNumber from "bignumber.js";
import {
  bankrunContext,
  banksClient,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  kaminoGroup,
  klendBankrunProgram,
  MARKET,
  riskAdmin,
  USDC_RESERVE,
  oracles,
  users,
} from "./rootHooks";
import {
  addBankWithSeed,
  configureBank,
  configureBankOracle,
  groupConfigure,
} from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endDeleverageIx,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  repayIx,
  startDeleverageIx,
  startLiquidationIx,
} from "./utils/user-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import {
  ASSET_TAG_SOL,
  blankBankConfigOptRaw,
  CONF_INTERVAL_MULTIPLE_FLOAT,
  defaultBankConfig,
  HEALTH_CACHE_HEALTHY,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  defaultKaminoBankConfig,
  estimateCollateralFromDeposit,
  estimateLiquidityFromCollateral,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import { assertBankrunTxFailed } from "./utils/genericTests";
import {
  bigNumberToWrappedI80F48,
  WrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { Reserve } from "@kamino-finance/klend-sdk";
import {
  computeSameValueBorrowNative,
} from "./utils/same-asset-emode";

const USER_ACCOUNT_SA_K = "same_asset_kamino_account";
const KAMINO_USDC_SA_SEED = new BN(20_000);
const REGULAR_USDC_SEED = new BN(20_001);
const REGULAR_SOL_SEED = new BN(20_002);
const RECEIVERSHIP_KAMINO_WITHDRAW = new BN(5_000);
const RECEIVERSHIP_KAMINO_REPAY = new BN(5_000);
const SAME_ASSET_DEPOSIT = new BN(100 * 10 ** ecosystem.usdcDecimals);
const SAME_ASSET_INIT_LEVERAGE = 101;
const SAME_ASSET_MAINT_LEVERAGE = 102;
const SAME_ASSET_TIGHTENED_INIT_LEVERAGE = 99;
const SAME_ASSET_TIGHTENED_MAINT_LEVERAGE = 100;
const SAME_ASSET_BORROW_ORIGINATION_FEE_RATE = 0.01;
const SAME_ASSET_BOUNDARY_GAP_POSITION = 0.5;

type HealthCacheSnapshot = {
  assetValue: WrappedI80F48;
  liabilityValue: WrappedI80F48;
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
};

type TestUser = (typeof users)[number];

const getNetHealth = (cache: HealthCacheSnapshot) => {
  const init = wrappedI80F48toBigNumber(cache.assetValue).minus(
    wrappedI80F48toBigNumber(cache.liabilityValue)
  );
  const maint = wrappedI80F48toBigNumber(cache.assetValueMaint).minus(
    wrappedI80F48toBigNumber(cache.liabilityValueMaint)
  );
  return { init, maint };
};

type KaminoSameAssetBorrowWindow = {
  collateralLiquidityNative: BN;
  collateralLiquidityUi: BigNumber;
  lowerOracleFactor: BigNumber;
  upperOracleFactor: BigNumber;
  healthyInitWeight: BigNumber;
  tightenedMaintWeight: BigNumber;
  healthyInitBoundaryUi: BigNumber;
  tightenedMaintBoundaryUi: BigNumber;
  boundaryGapUi: BigNumber;
  borrowPrincipalUi: BigNumber;
  borrowLiabilityUi: BigNumber;
  borrowNative: BN;
};

const computeKaminoSameAssetBorrowWindow = (
  collateralLiquidityNative: BN
): KaminoSameAssetBorrowWindow => {
  const uiScale = new BigNumber(10).pow(ecosystem.usdcDecimals);
  const collateralLiquidityUi = new BigNumber(
    collateralLiquidityNative.toString()
  ).div(uiScale);
  const lowerOracleFactor = new BigNumber(1 - CONF_INTERVAL_MULTIPLE_FLOAT);
  const upperOracleFactor = new BigNumber(1 + CONF_INTERVAL_MULTIPLE_FLOAT);
  const healthyInitWeight = new BigNumber(SAME_ASSET_INIT_LEVERAGE - 1).div(
    SAME_ASSET_INIT_LEVERAGE
  );
  const tightenedMaintWeight = new BigNumber(
    SAME_ASSET_TIGHTENED_MAINT_LEVERAGE - 1
  ).div(SAME_ASSET_TIGHTENED_MAINT_LEVERAGE);
  const liabilityWithFeeFactor = new BigNumber(1).plus(
    SAME_ASSET_BORROW_ORIGINATION_FEE_RATE
  );

  const healthyInitBoundaryUi = collateralLiquidityUi
    .times(ecosystem.usdcPrice)
    .times(lowerOracleFactor)
    .times(healthyInitWeight)
    .div(upperOracleFactor);
  const tightenedMaintBoundaryUi = collateralLiquidityUi
    .times(ecosystem.usdcPrice)
    .times(lowerOracleFactor)
    .times(tightenedMaintWeight)
    .div(upperOracleFactor);
  const boundaryGapUi = healthyInitBoundaryUi.minus(tightenedMaintBoundaryUi);
  const borrowLiabilityUi = tightenedMaintBoundaryUi.plus(
    boundaryGapUi.times(SAME_ASSET_BOUNDARY_GAP_POSITION)
  );
  const borrowPrincipalUi = borrowLiabilityUi.div(liabilityWithFeeFactor);
  const borrowNative = new BN(
    borrowPrincipalUi
      .times(uiScale)
      .integerValue(BigNumber.ROUND_FLOOR)
      .toFixed(0)
  );
  const roundedBorrowPrincipalUi = new BigNumber(borrowNative.toString()).div(
    uiScale
  );
  const roundedBorrowLiabilityUi =
    roundedBorrowPrincipalUi.times(liabilityWithFeeFactor);

  assert.isTrue(
    roundedBorrowLiabilityUi.isLessThan(healthyInitBoundaryUi),
    "Kamino same-asset liability should stay below the healthy init boundary"
  );
  assert.isTrue(
    roundedBorrowLiabilityUi.isGreaterThan(tightenedMaintBoundaryUi),
    "Kamino same-asset liability should stay above the tightened maint boundary"
  );

  return {
    collateralLiquidityNative,
    collateralLiquidityUi,
    lowerOracleFactor,
    upperOracleFactor,
    healthyInitWeight,
    tightenedMaintWeight,
    healthyInitBoundaryUi,
    tightenedMaintBoundaryUi,
    boundaryGapUi,
    borrowPrincipalUi: roundedBorrowPrincipalUi,
    borrowLiabilityUi: roundedBorrowLiabilityUi,
    borrowNative,
  };
};

const getKaminoCollateralSnapshot = async (marginfiAccount: PublicKey) => {
  const account = await bankrunProgram.account.marginfiAccount.fetch(
    marginfiAccount
  );
  const accountedCollateralShares = wrappedI80F48toBigNumber(
    account.lendingAccount.balances[0].assetShares
  );
  const accountedCollateralNative = new BN(
    accountedCollateralShares.integerValue().toFixed(0)
  );

  return { accountedCollateralNative };
};

describe("k20: Kamino same-asset emode", () => {
  let kaminoUsdcBank: PublicKey;
  let usdcReserve: PublicKey;
  let market: PublicKey;
  let regularUsdcBank: PublicKey;
  let regularSolBank: PublicKey;
  let kaminoObligation: PublicKey;

  const getSameAssetRemainingGroups = () =>
    [
      [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
      [regularUsdcBank, oracles.usdcOracle.publicKey],
    ] as PublicKey[][];
  const getCollateralAndSolRemainingGroups = () =>
    [
      [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
      [regularSolBank, oracles.wsolOracle.publicKey],
    ] as PublicKey[][];

  const initFreshAccount = async (user: TestUser) => {
    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
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

  const configureSameAssetLeverage = async (
    initLeverage: number,
    maintLeverage: number,
    options?: {
      newRiskAdmin?: PublicKey;
    }
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupConfigure(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: kaminoGroup.publicKey,
          newRiskAdmin: options?.newRiskAdmin,
          sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(initLeverage),
          sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(maintLeverage),
        })
      ),
      [groupAdmin.wallet]
    );
  };

  const resetSameAssetLeverage = async (options?: {
    newRiskAdmin?: PublicKey;
  }) =>
    configureSameAssetLeverage(
      SAME_ASSET_INIT_LEVERAGE,
      SAME_ASSET_MAINT_LEVERAGE,
      options
    );

  const tightenSameAssetLeverage = async (options?: {
    newRiskAdmin?: PublicKey;
  }) =>
    configureSameAssetLeverage(
      SAME_ASSET_TIGHTENED_INIT_LEVERAGE,
      SAME_ASSET_TIGHTENED_MAINT_LEVERAGE,
      options
    );

  const buildKaminoRefreshIxs = async () => [
    await simpleRefreshReserve(
      klendBankrunProgram,
      usdcReserve,
      market,
      oracles.usdcOracle.publicKey
    ),
    await simpleRefreshObligation(
      klendBankrunProgram,
      market,
      kaminoObligation,
      [usdcReserve]
    ),
  ];

  const depositKaminoCollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount = SAME_ASSET_DEPOSIT
  ) => {
    const refreshIxs = await buildKaminoRefreshIxs();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ...refreshIxs,
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount,
            bank: kaminoUsdcBank,
            signerTokenAccount: user.usdcAccount,
            lendingMarket: market,
            reserve: usdcReserve,
          },
          amount
        )
      ),
      [user.wallet]
    );
  };

  const getKaminoAccountedLiquidityNative = async (
    marginfiAccount: PublicKey
  ) => {
    const { accountedCollateralNative } = await getKaminoCollateralSnapshot(
      marginfiAccount
    );
    const reserveRaw = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve
    );
    const reserve = { ...reserveRaw } as unknown as Reserve;
    const accountedLiquidityNative = estimateLiquidityFromCollateral(
      reserve,
      accountedCollateralNative
    );

    return { accountedCollateralNative, accountedLiquidityNative };
  };

  const borrowFromRegularUsdc = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount: BN,
    remainingGroups = getSameAssetRemainingGroups()
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: regularUsdcBank,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts(remainingGroups),
          amount,
        })
      ),
      [user.wallet]
    );
  };

  const depositRegularUsdcCollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount: BN
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: regularUsdcBank,
          tokenAccount: user.usdcAccount,
          amount,
          depositUpToLimit: false,
        })
      ),
      [user.wallet]
    );
  };

  const pulseKaminoSameAssetHealth = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    sameAssetRemaining: PublicKey[][]
  ) => {
    const refreshIxs = await buildKaminoRefreshIxs();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ...refreshIxs,
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount,
          remaining: composeRemainingAccounts(sameAssetRemaining),
        })
      ),
      [user.wallet]
    );

    const account = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
    );

    return { account, health: getNetHealth(account.healthCache) };
  };

  const setupSameAssetScenario = async () => {
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      groupAdmin.usdcAccount,
      new BN(1_000 * 10 ** ecosystem.usdcDecimals)
    );

    const kaminoAddTx = new Transaction().add(
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
          config: defaultKaminoBankConfig(oracles.usdcOracle.publicKey),
          seed: KAMINO_USDC_SA_SEED,
        }
      )
    );
    await processBankrunTransaction(bankrunContext, kaminoAddTx, [
      groupAdmin.wallet,
    ]);

    const initObligationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank: kaminoUsdcBank,
          signerTokenAccount: groupAdmin.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        new BN(100)
      )
    );
    await processBankrunTransaction(bankrunContext, initObligationTx, [
      groupAdmin.wallet,
    ]);

    const usdcAddTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        config: defaultBankConfig(),
        seed: REGULAR_USDC_SEED,
      })
    );
    await processBankrunTransaction(bankrunContext, usdcAddTx, [
      groupAdmin.wallet,
    ]);

    const usdcOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: regularUsdcBank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.usdcOracle.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, usdcOracleTx, [
      groupAdmin.wallet,
    ]);

    const solConfig = defaultBankConfig();
    solConfig.assetTag = ASSET_TAG_SOL;
    const solAddTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.wsolMint.publicKey,
        config: solConfig,
        seed: REGULAR_SOL_SEED,
      })
    );
    await processBankrunTransaction(bankrunContext, solAddTx, [
      groupAdmin.wallet,
    ]);

    const solOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: regularSolBank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.wsolOracle.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, solOracleTx, [
      groupAdmin.wallet,
    ]);

    await resetSameAssetLeverage();

    const discounted = blankBankConfigOptRaw();
    discounted.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    discounted.assetWeightMaint = bigNumberToWrappedI80F48(0.5);

    const regularTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: regularUsdcBank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, regularTx, [
      groupAdmin.wallet,
    ]);

    const kaminoTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: kaminoUsdcBank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, kaminoTx, [
      groupAdmin.wallet,
    ]);

    for (const user of users) {
      const accountKeypair = Keypair.generate();
      user.accounts.set(USER_ACCOUNT_SA_K, accountKeypair.publicKey);

      const tx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: kaminoGroup.publicKey,
          marginfiAccount: accountKeypair.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      );
      await processBankrunTransaction(bankrunContext, tx, [
        user.wallet,
        accountKeypair,
      ]);
    }

    for (const user of users) {
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        new BN(2_000 * 10 ** ecosystem.usdcDecimals)
      );
      await mintToTokenAccount(
        ecosystem.wsolMint.publicKey,
        user.wsolAccount,
        new BN(20 * 10 ** ecosystem.wsolDecimals)
      );
    }

    const seedUser = users[2];
    const seedMarginfiAccount = seedUser.accounts.get(USER_ACCOUNT_SA_K)!;
    const tx = new Transaction()
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularUsdcBank,
          tokenAccount: seedUser.usdcAccount,
          amount: new BN(1_000 * 10 ** ecosystem.usdcDecimals),
          depositUpToLimit: false,
        })
      )
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularSolBank,
          tokenAccount: seedUser.wsolAccount,
          amount: new BN(10 * 10 ** ecosystem.wsolDecimals),
          depositUpToLimit: false,
        })
      );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(seedUser.wallet);
    await banksClient.processTransaction(tx);
  };

  before(async () => {
    usdcReserve = kaminoAccounts.get(USDC_RESERVE)!;
    market = kaminoAccounts.get(MARKET)!;

    [kaminoUsdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      KAMINO_USDC_SA_SEED
    );
    [regularUsdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      REGULAR_USDC_SEED
    );
    [regularSolBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      REGULAR_SOL_SEED
    );

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      kaminoUsdcBank
    );
    [kaminoObligation] = deriveBaseObligation(liquidityVaultAuthority, market);
    await setupSameAssetScenario();
  });

  it("(user 0) Kamino USDC collateral is healthy only because same-asset emode lifts the weight", async () => {
    const user = users[0];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    const reserveBeforeRaw = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve
    );
    const reserveBefore = { ...reserveBeforeRaw } as unknown as Reserve;
    const expectedCollateral = estimateCollateralFromDeposit(
      reserveBefore,
      SAME_ASSET_DEPOSIT
    );

    await depositKaminoCollateral(user, marginfiAccount);

    const { accountedCollateralNative, accountedLiquidityNative } =
      await getKaminoAccountedLiquidityNative(
        marginfiAccount
      );
    const borrowWindow = computeKaminoSameAssetBorrowWindow(
      accountedLiquidityNative
    );
    const accountedCollateral = Number(accountedCollateralNative.toString());
    assert.equal(
      accountedCollateral,
      Number(expectedCollateral.toString())
    );

    // Kamino stores the deposit as reserve-collateral shares, so the first step is converting the
    // accounted shares back into liquidity-equivalent USDC under the live reserve exchange rate:
    // - collateral_liquidity = collateral_shares * liquidity_per_collateral_share
    //
    // The borrow window is then:
    // - healthy init boundary =
    //   collateral_liquidity * lower_oracle / upper_oracle * (100 / 101)
    // - tightened maint boundary =
    //   collateral_liquidity * lower_oracle / upper_oracle * (99 / 100)
    //
    // The 1% origination fee matters because the risk engine records liability as:
    // - liability = borrow_principal * 1.01
    //
    // So this test borrows the principal whose recorded liability sits halfway inside the window:
    // - liability = tightened boundary + 0.5 * (healthy boundary - tightened boundary)
    // - principal = liability / 1.01
    //
    // `computeKaminoSameAssetBorrowWindow(...)` asserts the rounded result still satisfies:
    // tightened maint boundary < recorded liability < healthy init boundary.
    await borrowFromRegularUsdc(user, marginfiAccount, borrowWindow.borrowNative);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount,
          remaining: composeRemainingAccounts(getSameAssetRemainingGroups()),
        })
      ),
      [user.wallet]
    );

    const accountBeforeTighten =
      await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
      );
    assert.ok((accountBeforeTighten.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    await tightenSameAssetLeverage();

    const tightened = await pulseKaminoSameAssetHealth(
      user,
      marginfiAccount,
      getSameAssetRemainingGroups()
    );
    const account = tightened.account;
    const health = tightened.health;

    assert.equal(account.healthCache.flags & HEALTH_CACHE_HEALTHY, 0);
    assert.isTrue(health.init.isLessThan(0));
    assert.isTrue(health.maint.isLessThan(0));
  });

  it("(user 1) repaying the same-mint borrow and switching to equal-value SOL debt removes the lift", async () => {
    const user = users[1];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    await depositKaminoCollateral(user, marginfiAccount);

    const { accountedLiquidityNative } = await getKaminoAccountedLiquidityNative(
      marginfiAccount
    );
    const borrowWindow = computeKaminoSameAssetBorrowWindow(
      accountedLiquidityNative
    );
    const differentMintSameValueBorrow = computeSameValueBorrowNative({
      sourceBorrowNative: borrowWindow.borrowNative,
      sourceDecimals: ecosystem.usdcDecimals,
      sourcePrice: ecosystem.usdcPrice,
      targetDecimals: ecosystem.wsolDecimals,
      targetPrice: ecosystem.wsolPrice,
      sourceOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
      targetOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
    });

    // The same-mint USDC borrow above uses the exact Kamino window from the previous test:
    // - reserve-collateral shares are converted back into liquidity-equivalent USDC
    // - collateral uses the lower oracle bound and liabilities use the upper oracle bound
    // - the recorded debt is principal * 1.01 because of the origination fee
    // - the liability is chosen between the 101x healthy-init boundary and the 100x
    //   tightened-maint boundary
    //
    // `computeSameValueBorrowNative(...)` preserves that same fee-adjusted debt value while
    // switching the liability mint from USDC into SOL at $10/SOL.
    // Once the liability mint is SOL instead of USDC, the Kamino USDC collateral loses the
    // same-asset lift and falls back to the plain 0.5 regular weight, so the equal-value SOL
    // borrow must be rejected.
    await borrowFromRegularUsdc(user, marginfiAccount, borrowWindow.borrowNative);

    const repayAllTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularUsdcBank,
        tokenAccount: user.usdcAccount,
        amount: new BN(0),
        repayAll: true,
        remaining: composeRemainingAccounts(getSameAssetRemainingGroups()),
      })
    );
    await processBankrunTransaction(bankrunContext, repayAllTx, [user.wallet]);

    const unrelatedBorrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularSolBank,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts(
          getCollateralAndSolRemainingGroups()
        ),
        amount: differentMintSameValueBorrow,
      })
    );
    unrelatedBorrowTx.recentBlockhash = await getBankrunBlockhash(
      bankrunContext
    );
    unrelatedBorrowTx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(unrelatedBorrowTx);
    assertBankrunTxFailed(result, "0x1779");
  });

  it("(admin) tightening same-asset leverage makes a Kamino/P0 position liquidatable", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];
    const liquidateeAccount = await initFreshAccount(liquidatee);
    const liquidatorAccount = await initFreshAccount(liquidator);
    const sameAssetRemaining = getSameAssetRemainingGroups();
    const startRemaining =
      composeRemainingAccountsWriteableMeta(sameAssetRemaining);
    const endRemaining =
      composeRemainingAccountsMetaBanksOnly(sameAssetRemaining);

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      liquidator.usdcAccount,
      new BN(300 * 10 ** ecosystem.usdcDecimals)
    );

    await resetSameAssetLeverage();
    await depositRegularUsdcCollateral(
      liquidator,
      liquidatorAccount,
      new BN(150 * 10 ** ecosystem.usdcDecimals)
    );
    await depositKaminoCollateral(liquidatee, liquidateeAccount);

    const { accountedLiquidityNative: liquidateeLiquidityNative } =
      await getKaminoAccountedLiquidityNative(liquidateeAccount);
    const borrowWindow = computeKaminoSameAssetBorrowWindow(
      liquidateeLiquidityNative
    );

    // The liquidation setup uses the same explicit window as the first Kamino test:
    // - collateral shares are converted back into liquidity-equivalent USDC
    // - the confidence haircut lowers collateral and raises liabilities
    // - the 1% origination fee means recorded debt is principal * 1.01
    // - the recorded debt is placed halfway between the healthy 101x init boundary and the
    //   tightened 100x maint boundary
    await borrowFromRegularUsdc(
      liquidatee,
      liquidateeAccount,
      borrowWindow.borrowNative
    );

    await tightenSameAssetLeverage();

    const refreshIxs = await buildKaminoRefreshIxs();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          feePayer: liquidator.wallet.publicKey,
        }),
        ...refreshIxs,
        await startLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          liquidationReceiver: liquidator.wallet.publicKey,
          remaining: startRemaining,
        }),
        await endLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          remaining: endRemaining,
        })
      ),
      [liquidator.wallet]
    );
  });

  it("(admin) same-asset deleverage can improve a tightened Kamino/P0 position", async () => {
    const deleveragee = users[3];
    const deleverageeAccount = await initFreshAccount(deleveragee);
    const sameAssetRemaining = getSameAssetRemainingGroups();
    const startRemaining =
      composeRemainingAccountsWriteableMeta(sameAssetRemaining);
    const endRemaining =
      composeRemainingAccountsMetaBanksOnly(sameAssetRemaining);

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      riskAdmin.usdcAccount,
      new BN(300 * 10 ** ecosystem.usdcDecimals)
    );

    await resetSameAssetLeverage({ newRiskAdmin: riskAdmin.wallet.publicKey });
    await depositKaminoCollateral(deleveragee, deleverageeAccount);

    const { accountedLiquidityNative: deleverageeLiquidityNative } =
      await getKaminoAccountedLiquidityNative(deleverageeAccount);
    const borrowWindow = computeKaminoSameAssetBorrowWindow(
      deleverageeLiquidityNative
    );

    // The deleverage setup uses the same explicit leverage-driven window:
    // - Kamino collateral shares are converted back into liquidity-equivalent USDC
    // - collateral is haircut by the oracle lower bound and liabilities by the upper bound
    // - the 1% origination fee is included in the recorded debt
    // - the resulting debt is halfway between the healthy 101x init boundary and the tightened
    //   100x maint boundary
    //
    // So the account is healthy before the tighten and becomes deleverage-eligible only after the
    // leverage move lands.
    await borrowFromRegularUsdc(
      deleveragee,
      deleverageeAccount,
      borrowWindow.borrowNative
    );

    await tightenSameAssetLeverage();
    const refreshIxs = await buildKaminoRefreshIxs();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          feePayer: riskAdmin.wallet.publicKey,
        }),
        ...refreshIxs,
        await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          riskAdmin: riskAdmin.wallet.publicKey,
          remaining: startRemaining,
        }),
        await makeKaminoWithdrawIx(
          riskAdmin.mrgnBankrunProgram,
          {
            marginfiAccount: deleverageeAccount,
            authority: riskAdmin.wallet.publicKey,
            bank: kaminoUsdcBank,
            mint: ecosystem.usdcMint.publicKey,
            destinationTokenAccount: riskAdmin.usdcAccount,
            lendingMarket: market,
            reserve: usdcReserve,
          },
          {
            amount: RECEIVERSHIP_KAMINO_WITHDRAW,
            isWithdrawAll: false,
            remaining: composeRemainingAccounts(sameAssetRemaining),
          }
        ),
        await repayIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          bank: regularUsdcBank,
          tokenAccount: riskAdmin.usdcAccount,
          amount: RECEIVERSHIP_KAMINO_REPAY,
          remaining: composeRemainingAccounts(sameAssetRemaining),
        }),
        await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          remaining: endRemaining,
        })
      ),
      [riskAdmin.wallet]
    );
  });
});
