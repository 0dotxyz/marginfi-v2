import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  banksClient,
  bankrunProgram,
  driftAccounts,
  driftBankrunProgram,
  driftGroup,
  ecosystem,
  groupAdmin,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  oracles,
  riskAdmin,
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
import { deriveBankWithSeed } from "./utils/pdas";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  makeAddDriftBankIx,
  makeDriftDepositIx,
  makeDriftWithdrawIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";
import { assertBankrunTxFailed } from "./utils/genericTests";
import {
  bigNumberToWrappedI80F48,
  WrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  defaultDriftBankConfig,
  fundAndDepositAdminReward,
  getSpotMarketAccount,
  TOKEN_A_MARKET_INDEX,
  scaledBalanceToTokenAmount,
  tokenAmountToScaledBalance,
} from "./utils/drift-utils";
import {
  computeSameAssetBoundaryBorrowNative,
  computeSameValueBorrowNative,
} from "./utils/same-asset-emode";

const USER_ACCOUNT_SA_D = "same_asset_drift_account";
const DRIFT_TOKEN_A_SA_SEED = new BN(16_000);
const REGULAR_TOKEN_A_SEED = new BN(16_001);
const REGULAR_USDC_SEED = new BN(16_002);
const RECEIVERSHIP_DRIFT_WITHDRAW = new BN(500_000);
const SAME_ASSET_DEPOSIT = new BN(100 * 10 ** ecosystem.tokenADecimals);
const SAME_ASSET_INIT_LEVERAGE = 101;
const SAME_ASSET_MAINT_LEVERAGE = 102;
const SAME_ASSET_TIGHTENED_INIT_LEVERAGE = 99;
const SAME_ASSET_TIGHTENED_MAINT_LEVERAGE = 100;
const SAME_ASSET_BORROW_ORIGINATION_FEE_RATE = 0.01;

type TestUser = (typeof users)[number];

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

const computeDriftSameAssetBorrow = (accountedUnderlyingNative: BN) =>
  computeSameAssetBoundaryBorrowNative({
    collateralNative: accountedUnderlyingNative,
    collateralDecimals: ecosystem.tokenADecimals,
    collateralPrice: ecosystem.tokenAPrice,
    liabilityDecimals: ecosystem.tokenADecimals,
    liabilityPrice: ecosystem.tokenAPrice,
    healthyInitLeverage: SAME_ASSET_INIT_LEVERAGE,
    tightenedRequirementLeverage: SAME_ASSET_TIGHTENED_MAINT_LEVERAGE,
    liabilityOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });

const computeSameValueUsdcBorrow = (sameAssetBorrowNative: BN) =>
  computeSameValueBorrowNative({
    sourceBorrowNative: sameAssetBorrowNative,
    sourceDecimals: ecosystem.tokenADecimals,
    sourcePrice: ecosystem.tokenAPrice,
    targetDecimals: ecosystem.usdcDecimals,
    targetPrice: ecosystem.usdcPrice,
    sourceOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
    targetOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });

const getDriftAccountedCollateralNative = async (
  marginfiAccount: PublicKey
) => {
  const account = await bankrunProgram.account.marginfiAccount.fetch(
    marginfiAccount
  );
  const accountedScaledBalance = new BN(
    wrappedI80F48toBigNumber(
      account.lendingAccount.balances[0].assetShares
    ).toString()
  );
  const spotMarket = await getSpotMarketAccount(
    driftBankrunProgram,
    TOKEN_A_MARKET_INDEX
  );
  const accountedUnderlying = scaledBalanceToTokenAmount(
    accountedScaledBalance,
    spotMarket
  );

  return { accountedScaledBalance, accountedUnderlying };
};

describe("d16: Drift same-asset emode", () => {
  let driftTokenABank: PublicKey;
  let driftTokenASpotMarket: PublicKey;
  let driftTokenAOracle: PublicKey;
  let regularTokenABank: PublicKey;
  let regularUsdcBank: PublicKey;

  const getSameAssetRemainingGroups = () =>
    [
      [driftTokenABank, oracles.tokenAOracle.publicKey, driftTokenASpotMarket],
      [regularTokenABank, oracles.tokenAOracle.publicKey],
    ] as PublicKey[][];
  const getSameAssetWithUsdcRemainingGroups = () =>
    [
      [driftTokenABank, oracles.tokenAOracle.publicKey, driftTokenASpotMarket],
      [regularUsdcBank, oracles.usdcOracle.publicKey],
    ] as PublicKey[][];

  const initFreshAccount = async (user: TestUser) => {
    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
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
          marginfiGroup: driftGroup.publicKey,
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

  const depositDriftCollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount = SAME_ASSET_DEPOSIT
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeDriftDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount,
            bank: driftTokenABank,
            signerTokenAccount: user.tokenAAccount,
            driftOracle: driftTokenAOracle,
          },
          amount,
          TOKEN_A_MARKET_INDEX
        )
      ),
      [user.wallet]
    );
  };

  const borrowFromRegularTokenA = async (
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
          bank: regularTokenABank,
          tokenAccount: user.tokenAAccount,
          remaining: composeRemainingAccounts(remainingGroups),
          amount,
        })
      ),
      [user.wallet]
    );
  };

  const pulseDriftSameAssetHealth = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    remainingGroups: PublicKey[][]
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount,
          remaining: composeRemainingAccounts(remainingGroups),
        })
      ),
      [user.wallet]
    );

    return bankrunProgram.account.marginfiAccount.fetch(marginfiAccount);
  };

  const depositRegularTokenACollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount: BN
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: regularTokenABank,
          tokenAccount: user.tokenAAccount,
          amount,
          depositUpToLimit: false,
        })
      ),
      [user.wallet]
    );
  };

  const setupSameAssetScenario = async () => {
    await mintToTokenAccount(
      ecosystem.tokenAMint.publicKey,
      groupAdmin.tokenAAccount,
      new BN(1_000 * 10 ** ecosystem.tokenADecimals)
    );

    const driftAddTx = new Transaction().add(
      await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: driftTokenASpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultDriftBankConfig(oracles.tokenAOracle.publicKey),
          seed: DRIFT_TOKEN_A_SA_SEED,
        }
      )
    );
    await processBankrunTransaction(bankrunContext, driftAddTx, [
      groupAdmin.wallet,
    ]);

    const initDriftUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank: driftTokenABank,
          signerTokenAccount: groupAdmin.tokenAAccount,
          driftOracle: driftTokenAOracle,
        },
        {
          amount: new BN(100 * 10 ** ecosystem.tokenADecimals),
        },
        TOKEN_A_MARKET_INDEX
      )
    );
    await processBankrunTransaction(bankrunContext, initDriftUserTx, [
      groupAdmin.wallet,
    ]);

    const tokenAAddTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        config: defaultBankConfig(),
        seed: REGULAR_TOKEN_A_SEED,
      })
    );
    await processBankrunTransaction(bankrunContext, tokenAAddTx, [
      groupAdmin.wallet,
    ]);

    const tokenAOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: regularTokenABank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.tokenAOracle.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, tokenAOracleTx, [
      groupAdmin.wallet,
    ]);

    const usdcAddTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
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

    await resetSameAssetLeverage();

    const discounted = blankBankConfigOptRaw();
    discounted.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    discounted.assetWeightMaint = bigNumberToWrappedI80F48(0.5);

    const driftTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: driftTokenABank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, driftTx, [
      groupAdmin.wallet,
    ]);

    const regularTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: regularTokenABank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, regularTx, [
      groupAdmin.wallet,
    ]);

    for (const user of users) {
      const accountKeypair = Keypair.generate();
      user.accounts.set(USER_ACCOUNT_SA_D, accountKeypair.publicKey);

      const tx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: driftGroup.publicKey,
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
        ecosystem.tokenAMint.publicKey,
        user.tokenAAccount,
        new BN(2_000 * 10 ** ecosystem.tokenADecimals)
      );
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        new BN(2_000 * 10 ** ecosystem.usdcDecimals)
      );
    }

    await fundAndDepositAdminReward(
      groupAdmin.wallet,
      driftTokenABank,
      ecosystem.tokenAMint.publicKey,
      TOKEN_A_MARKET_INDEX,
      new BN(500 * 10 ** ecosystem.tokenADecimals)
    );

    const seedUser = users[2];
    const seedMarginfiAccount = seedUser.accounts.get(USER_ACCOUNT_SA_D)!;
    const seedTx = new Transaction()
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularTokenABank,
          tokenAccount: seedUser.tokenAAccount,
          amount: new BN(200 * 10 ** ecosystem.tokenADecimals),
          depositUpToLimit: false,
        })
      )
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularUsdcBank,
          tokenAccount: seedUser.usdcAccount,
          amount: new BN(2_000 * 10 ** ecosystem.usdcDecimals),
          depositUpToLimit: false,
        })
      );
    seedTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    seedTx.sign(seedUser.wallet);
    await banksClient.processTransaction(seedTx);
  };

  before(async () => {
    driftTokenASpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET)!;
    driftTokenAOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE)!;

    [driftTokenABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      driftGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      DRIFT_TOKEN_A_SA_SEED
    );
    [regularTokenABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      driftGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      REGULAR_TOKEN_A_SEED
    );
    [regularUsdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      driftGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      REGULAR_USDC_SEED
    );
    await setupSameAssetScenario();
  });

  it("(user 0) Drift Token A collateral is healthy only because same-asset emode lifts the weight", async () => {
    const user = users[0];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    const preSpotMarket = await getSpotMarketAccount(
      driftBankrunProgram,
      TOKEN_A_MARKET_INDEX
    );
    const expectedScaledBalance = tokenAmountToScaledBalance(
      SAME_ASSET_DEPOSIT,
      preSpotMarket
    );

    // Deposit = 100 Token A at $10, so the nominal collateral value is $1,000 before confidence.
    // Drift stores the deposit as a scaled balance, and the helper converts that balance back into
    // `accountedUnderlying` Token A before sizing the borrow. The assertions below pin
    // `accountedUnderlying` to the nominal deposit within Drift's 1-native-unit rounding.
    // `computeDriftSameAssetBorrow(accountedUnderlying)` then applies:
    // - the oracle lower/upper confidence haircut used by the risk engine
    // - a 1% origination fee on the liability side
    // - a 25%-into-the-gap position between the healthy 101x init weight = 100 / 101 ~= 0.990099
    //   and the tightened 100x maint weight = 99 / 100 = 0.99
    await depositDriftCollateral(user, marginfiAccount);

    const { accountedScaledBalance, accountedUnderlying } =
      await getDriftAccountedCollateralNative(marginfiAccount);
    const sameAssetBorrow = computeDriftSameAssetBorrow(accountedUnderlying);
    assert.isTrue(
      accountedScaledBalance.sub(expectedScaledBalance).abs().lte(new BN(1)),
      `expected scaled balance ${accountedScaledBalance.toString()} to be within 1 unit of ${expectedScaledBalance.toString()}`
    );
    assert.isTrue(
      accountedUnderlying.sub(SAME_ASSET_DEPOSIT).abs().lte(new BN(1)),
      `expected redeemed underlying ${accountedUnderlying.toString()} to be within 1 unit of ${SAME_ASSET_DEPOSIT.toString()}`
    );

    const sameAssetRemaining = getSameAssetRemainingGroups();
    await borrowFromRegularTokenA(
      user,
      marginfiAccount,
      sameAssetBorrow,
      sameAssetRemaining
    );

    const accountBeforeTighten = await pulseDriftSameAssetHealth(
      user,
      marginfiAccount,
      sameAssetRemaining
    );
    const health = getNetHealth(accountBeforeTighten.healthCache);
    assert.ok(
      (accountBeforeTighten.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0
    );
    assert.ok(
      (accountBeforeTighten.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0
    );
    assert.ok(
      (accountBeforeTighten.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0
    );
    assert.equal(accountBeforeTighten.healthCache.internalErr, 0);
    assert.equal(accountBeforeTighten.healthCache.mrgnErr, 0);
    assert.isTrue(health.init.isGreaterThan(0));
    assert.isTrue(health.maint.isGreaterThan(0));

    await tightenSameAssetLeverage();

    const account = await pulseDriftSameAssetHealth(
      user,
      marginfiAccount,
      sameAssetRemaining
    );
    const tightenedHealth = getNetHealth(account.healthCache);
    assert.equal(account.healthCache.flags & HEALTH_CACHE_HEALTHY, 0);
    assert.isTrue(tightenedHealth.init.isLessThan(0));
    assert.isTrue(tightenedHealth.maint.isLessThan(0));
  });

  it("(user 1) repaying the same-mint borrow and switching to equal-value USDC debt removes the lift", async () => {
    const user = users[1];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    const preSpotMarket = await getSpotMarketAccount(
      driftBankrunProgram,
      TOKEN_A_MARKET_INDEX
    );
    const expectedScaledBalance = tokenAmountToScaledBalance(
      SAME_ASSET_DEPOSIT,
      preSpotMarket
    );

    // Deposit = 100 Token A at $10, so the nominal collateral is worth $1,000 before weighting.
    // `computeDriftSameAssetBorrow(accountedUnderlying)` sizes the Token A borrow from the live
    // underlying-equivalent collateral amount, using the oracle confidence haircut, the 1%
    // origination fee, and a 25%-into-the-gap position inside the 101x-init vs 100x-tightened
    // boundary window.
    // `computeSameValueUsdcBorrow(sameAssetBorrow)` then converts that exact fee-adjusted debt
    // notional into USDC, so only the liability mint changes.
    // Once the liability mint changes, the account loses the same-asset lift and falls back to the
    // plain 0.5 regular weight, so the equal-value USDC debt must be rejected.
    await depositDriftCollateral(user, marginfiAccount);

    const { accountedScaledBalance, accountedUnderlying } =
      await getDriftAccountedCollateralNative(marginfiAccount);
    const sameAssetBorrow = computeDriftSameAssetBorrow(accountedUnderlying);
    const differentMintSameValueBorrow =
      computeSameValueUsdcBorrow(sameAssetBorrow);
    assert.isTrue(
      accountedScaledBalance.sub(expectedScaledBalance).abs().lte(new BN(1)),
      `expected scaled balance ${accountedScaledBalance.toString()} to be within 1 unit of ${expectedScaledBalance.toString()}`
    );

    const sameAssetRemaining = getSameAssetRemainingGroups();
    await borrowFromRegularTokenA(
      user,
      marginfiAccount,
      sameAssetBorrow,
      sameAssetRemaining
    );

    let account = await pulseDriftSameAssetHealth(
      user,
      marginfiAccount,
      sameAssetRemaining
    );
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    const repayAllTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularTokenABank,
        tokenAccount: user.tokenAAccount,
        amount: new BN(0),
        repayAll: true,
        remaining: composeRemainingAccounts(sameAssetRemaining),
      })
    );
    await processBankrunTransaction(bankrunContext, repayAllTx, [user.wallet]);

    const unrelatedBorrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularUsdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(
          getSameAssetWithUsdcRemainingGroups()
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

  it("(admin) tightening same-asset leverage makes a Drift/P0 position liquidatable", async () => {
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
      ecosystem.tokenAMint.publicKey,
      liquidator.tokenAAccount,
      new BN(300 * 10 ** ecosystem.tokenADecimals)
    );

    await resetSameAssetLeverage();
    await depositRegularTokenACollateral(
      liquidator,
      liquidatorAccount,
      new BN(150 * 10 ** ecosystem.tokenADecimals)
    );
    await depositDriftCollateral(liquidatee, liquidateeAccount);

    const { accountedUnderlying: liquidateeUnderlying } =
      await getDriftAccountedCollateralNative(liquidateeAccount);
    const sameAssetBorrow = computeDriftSameAssetBorrow(liquidateeUnderlying);

    // The liquidatee deposit is 100 Token A at $10, so the nominal collateral is $1,000 before
    // confidence.
    // `computeDriftSameAssetBorrow(liquidateeUnderlying)` uses the live underlying-equivalent
    // collateral amount recovered from Drift, the oracle lower/upper confidence haircut, and the
    // 1% origination fee to place the fee-adjusted liability 25% of the way from the tightened
    // boundary back toward the healthy boundary
    await borrowFromRegularTokenA(
      liquidatee,
      liquidateeAccount,
      sameAssetBorrow
    );

    await tightenSameAssetLeverage();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          feePayer: liquidator.wallet.publicKey,
        }),
        await startLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          liquidationReceiver: liquidator.wallet.publicKey,
          remaining: startRemaining,
        }),
        await makeDriftWithdrawIx(
          liquidator.mrgnBankrunProgram,
          {
            marginfiAccount: liquidateeAccount,
            bank: driftTokenABank,
            destinationTokenAccount: liquidator.tokenAAccount,
            driftOracle: driftTokenAOracle,
          },
          {
            amount: RECEIVERSHIP_DRIFT_WITHDRAW,
            withdrawAll: false,
            remaining: composeRemainingAccounts(sameAssetRemaining),
          },
          driftBankrunProgram
        ),
        await repayIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: regularTokenABank,
          tokenAccount: liquidator.tokenAAccount,
          amount: RECEIVERSHIP_DRIFT_WITHDRAW,
          remaining: composeRemainingAccounts(sameAssetRemaining),
        }),
        await endLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          remaining: endRemaining,
        })
      ),
      [liquidator.wallet]
    );
  });

  it("(admin) same-asset deleverage can improve a tightened Drift/P0 position", async () => {
    const deleveragee = users[3];
    const deleverageeAccount = await initFreshAccount(deleveragee);
    const sameAssetRemaining = getSameAssetRemainingGroups();
    const startRemaining =
      composeRemainingAccountsWriteableMeta(sameAssetRemaining);
    const endRemaining =
      composeRemainingAccountsMetaBanksOnly(sameAssetRemaining);

    await mintToTokenAccount(
      ecosystem.tokenAMint.publicKey,
      riskAdmin.tokenAAccount,
      new BN(300 * 10 ** ecosystem.tokenADecimals)
    );

    await resetSameAssetLeverage({ newRiskAdmin: riskAdmin.wallet.publicKey });
    await depositDriftCollateral(deleveragee, deleverageeAccount);

    const { accountedUnderlying: deleverageeUnderlying } =
      await getDriftAccountedCollateralNative(deleverageeAccount);
    const sameAssetBorrow = computeDriftSameAssetBorrow(deleverageeUnderlying);

    // The deleveragee deposit is also 100 Token A at $10, so the nominal collateral is $1,000.
    // `computeDriftSameAssetBorrow(deleverageeUnderlying)` uses the live Drift underlying amount,
    // the oracle confidence haircut, and the 1% origination fee to place the fee-adjusted Token A
    // debt 25% of the way from the tightened 100x maint boundary back toward the healthy 101x
    // init boundary.
    await borrowFromRegularTokenA(
      deleveragee,
      deleverageeAccount,
      sameAssetBorrow
    );

    await tightenSameAssetLeverage();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          feePayer: riskAdmin.wallet.publicKey,
        }),
        await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          riskAdmin: riskAdmin.wallet.publicKey,
          remaining: startRemaining,
        }),
        await makeDriftWithdrawIx(
          riskAdmin.mrgnBankrunProgram,
          {
            marginfiAccount: deleverageeAccount,
            bank: driftTokenABank,
            destinationTokenAccount: riskAdmin.tokenAAccount,
            driftOracle: driftTokenAOracle,
          },
          {
            amount: RECEIVERSHIP_DRIFT_WITHDRAW,
            withdrawAll: false,
            remaining: composeRemainingAccounts(sameAssetRemaining),
          },
          driftBankrunProgram
        ),
        await repayIx(riskAdmin.mrgnBankrunProgram, {
          marginfiAccount: deleverageeAccount,
          bank: regularTokenABank,
          tokenAccount: riskAdmin.tokenAAccount,
          amount: RECEIVERSHIP_DRIFT_WITHDRAW,
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
