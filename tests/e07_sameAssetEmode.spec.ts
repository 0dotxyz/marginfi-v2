import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { groupConfigure } from "./utils/group-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  EMODE_SEED,
  emodeGroup,
  groupAdmin,
  oracles,
  riskAdmin,
  users,
  verbose,
} from "./rootHooks";
import { assertBankrunTxFailed } from "./utils/genericTests";
import {
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
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endDeleverageIx,
  healthPulse,
  initLiquidationRecordIx,
  liquidateIx,
  repayIx,
  startDeleverageIx,
  withdrawIx,
} from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  getBankrunBlockhash,
  mintToTokenAccount,
} from "./utils/tools";
import { deriveLiquidationRecord } from "./utils/pdas";
import {
  computeSameAssetBoundaryBorrowNative,
  computeSameValueBorrowNative,
} from "./utils/same-asset-emode";

// Reuse banks from e01 (emode group) - these share the same USDC mint
const seed = new BN(EMODE_SEED);
let usdcBankA: PublicKey; // seed = EMODE_SEED
let usdcBankB: PublicKey; // seed = EMODE_SEED + 1
let solBank: PublicKey; // seed = EMODE_SEED

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

/** in mockUser.accounts, key used to get/set the user's account for same-asset emode tests */
const USER_ACCOUNT_SA: string = "sa_acc";

const SAME_ASSET_DEPOSIT = new BN(100 * 10 ** ecosystem.usdcDecimals);
const SAME_ASSET_TIGHTENED_INIT_LEVERAGE = 99;
const SAME_ASSET_TIGHTENED_MAINT_LEVERAGE = 100;
const SAME_ASSET_PARTIAL_LIQUIDATION = new BN(50_000);
const SAME_ASSET_LIQUIDATION_INIT_LEVERAGE = 20;
const SAME_ASSET_LIQUIDATION_MAINT_LEVERAGE = 21;
const SAME_ASSET_LIQUIDATION_TIGHTENED_INIT_LEVERAGE = 18;
const SAME_ASSET_LIQUIDATION_TIGHTENED_MAINT_LEVERAGE = 19;
const SAME_ASSET_BORROW_ORIGINATION_FEE_RATE = 0;

/**
 * Same-asset automatic emode: when two banks share the same underlying mint (e.g. two USDC banks),
 * higher leverage is automatically applied without requiring emode tag configuration.
 *
 * Formula: w_asset = w_liab * (1 - 1/L) where L = leverage
 * e.g. L=100, w_liab=1.0 → w_asset = 0.99 → allows ~100x leverage
 */
describe("Same-asset automatic emode", () => {
  const getSameAssetRemaining = () =>
    [
      [usdcBankA, oracles.usdcOracle.publicKey],
      [usdcBankB, oracles.usdcOracle.publicKey],
    ] as PublicKey[][];
  const getCollateralAndSolRemaining = () =>
    [
      [usdcBankA, oracles.usdcOracle.publicKey],
      [solBank, oracles.wsolOracle.publicKey],
    ] as PublicKey[][];

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
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet, accountKeypair);
    await banksClient.processTransaction(tx);
    return accountKeypair.publicKey;
  };

  const setupSameAssetScenario = async () => {
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_MAINT_LEVERAGE
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    for (let i = 0; i < users.length; i++) {
      const userAccKeypair = Keypair.generate();
      const userAccount = userAccKeypair.publicKey;
      users[i].accounts.set(USER_ACCOUNT_SA, userAccount);

      if (verbose) {
        console.log("same-asset user [" + i + "]: " + userAccount);
      }

      let userinitTx = new Transaction().add(
        await accountInit(users[i].mrgnBankrunProgram, {
          marginfiGroup: emodeGroup.publicKey,
          marginfiAccount: userAccount,
          authority: users[i].wallet.publicKey,
          feePayer: users[i].wallet.publicKey,
        })
      );
      userinitTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      userinitTx.sign(users[i].wallet, userAccKeypair);
      await banksClient.processTransaction(userinitTx);
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
    const seedUserAccount = seedUser.accounts.get(USER_ACCOUNT_SA);

    tx = new Transaction().add(
      await depositIx(seedUser.mrgnBankrunProgram, {
        marginfiAccount: seedUserAccount,
        bank: usdcBankB,
        tokenAccount: seedUser.usdcAccount,
        amount: new BN(1000 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(seedUser.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await depositIx(seedUser.mrgnBankrunProgram, {
        marginfiAccount: seedUserAccount,
        bank: solBank,
        tokenAccount: seedUser.wsolAccount,
        amount: new BN(10 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(seedUser.wallet);
    await banksClient.processTransaction(tx);
  };

  // Leverage values for configuration
  const SAME_ASSET_INIT_LEVERAGE = 101;
  const SAME_ASSET_MAINT_LEVERAGE = 102;
  const SAME_ASSET_BORROW = computeSameAssetBoundaryBorrowNative({
    collateralNative: SAME_ASSET_DEPOSIT,
    collateralDecimals: ecosystem.usdcDecimals,
    collateralPrice: ecosystem.usdcPrice,
    liabilityDecimals: ecosystem.usdcDecimals,
    liabilityPrice: ecosystem.usdcPrice,
    healthyInitLeverage: SAME_ASSET_INIT_LEVERAGE,
    tightenedRequirementLeverage: SAME_ASSET_TIGHTENED_MAINT_LEVERAGE,
    liabilityOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });
  const DIFFERENT_MINT_SAME_VALUE_BORROW = computeSameValueBorrowNative({
    sourceBorrowNative: SAME_ASSET_BORROW,
    sourceDecimals: ecosystem.usdcDecimals,
    sourcePrice: ecosystem.usdcPrice,
    targetDecimals: ecosystem.wsolDecimals,
    targetPrice: ecosystem.wsolPrice,
    sourceOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
    targetOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });
  const SAME_ASSET_LIQUIDATION_BORROW = computeSameAssetBoundaryBorrowNative({
    collateralNative: SAME_ASSET_DEPOSIT,
    collateralDecimals: ecosystem.usdcDecimals,
    collateralPrice: ecosystem.usdcPrice,
    liabilityDecimals: ecosystem.usdcDecimals,
    liabilityPrice: ecosystem.usdcPrice,
    healthyInitLeverage: SAME_ASSET_LIQUIDATION_INIT_LEVERAGE,
    tightenedRequirementLeverage:
      SAME_ASSET_LIQUIDATION_TIGHTENED_MAINT_LEVERAGE,
    liabilityOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });

  before(async () => {
    // Derive bank addresses (created in e01_initGroup)
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
    await setupSameAssetScenario();
  });

  // -----------------------------------------------------------------------
  // Configuration tests
  // -----------------------------------------------------------------------

  it("(admin) Configure same-asset emode with init >= maint - fails", async () => {
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(200),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(100),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // BadEmodeConfig (6075 = 0x17bb)
    assertBankrunTxFailed(result, "0x17bb");
  });

  it("(admin) Configure same-asset emode with leverage < 1 - fails", async () => {
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(0),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(200),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // BadEmodeConfig (6075 = 0x17bb)
    assertBankrunTxFailed(result, "0x17bb");
  });

  it("(admin) Configure same-asset emode with leverage > 1000 - fails", async () => {
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(1001),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(1002),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // BadEmodeConfig (6075 = 0x17bb)
    assertBankrunTxFailed(result, "0x17bb");
  });

  // -----------------------------------------------------------------------
  // Same-mint borrow tests
  // -----------------------------------------------------------------------

  it("(user 0) same-mint borrowing is healthy only because same-asset emode lifts the weight", async () => {
    const user = users[0];
    const userAccount = await initFreshAccount(user);

    // Deposit = 100 USDC.
    // The helper that produced `SAME_ASSET_BORROW` applies the same oracle-confidence haircut used
    // by the risk engine: collateral is valued at the lower bound and liabilities at the upper
    // bound.
    // With 101x init leverage, the healthy same-asset weight is 100 / 101 ~= 0.990099, so the
    // healthy init liability boundary is about 100 * lower_oracle / upper_oracle * 100 / 101.
    // After tightening to 99x / 100x, the relevant tightened requirement is the 100x maint weight
    // = 99 / 100 = 0.99, so the tightened boundary is about
    // 100 * lower_oracle / upper_oracle * 99 / 100.
    // `SAME_ASSET_BORROW` is computed 25% of the way from the tightened boundary back toward the
    // healthy boundary, so the position is accepted before the tighten and flips unhealthy after the
    // 101x/102x -> 99x/100x change.
    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        amount: SAME_ASSET_DEPOSIT,
        depositUpToLimit: false,
      }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
        amount: SAME_ASSET_BORROW,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    let account = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    let health = getNetHealth(account.healthCache);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);
    assert.isTrue(health.init.isGreaterThan(0));
    assert.isTrue(health.maint.isGreaterThan(0));

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_TIGHTENED_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_TIGHTENED_MAINT_LEVERAGE
        ),
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    account = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    health = getNetHealth(account.healthCache);
    assert.equal(account.healthCache.flags & HEALTH_CACHE_HEALTHY, 0);
    assert.isTrue(health.init.isLessThan(0));
    assert.isTrue(health.maint.isLessThan(0));

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_MAINT_LEVERAGE
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(user 1) repaying the same-mint borrow and switching to an equal-value SOL liability removes the lift", async () => {
    const user = users[1];
    const userAccount = await initFreshAccount(user);

    // Deposit = 100 USDC.
    // `SAME_ASSET_BORROW` uses the same boundary window as the test above:
    // - healthy same-asset init boundary at 101x
    // - tightened same-asset maint boundary at 100x
    // with zero origination fee, a 25%-into-the-gap position, and the oracle lower/upper
    // confidence haircut.
    // `DIFFERENT_MINT_SAME_VALUE_BORROW` then converts that exact same debt notional into SOL at
    // $10/SOL, so only the liability mint changes.
    // Once the USDC debt is repaid, borrowing SOL removes the same-asset lift and the collateral
    // falls back to the plain 0.5 weight. That means the account can support only about half of
    // the $100 collateral value, so the equal-value SOL borrow must be rejected.
    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBankA,
        tokenAccount: user.usdcAccount,
        amount: SAME_ASSET_DEPOSIT,
        depositUpToLimit: false,
      }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
        amount: SAME_ASSET_BORROW,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    let account = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBankB,
        tokenAccount: user.usdcAccount,
        amount: new BN(0),
        repayAll: true,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts(getCollateralAndSolRemaining()),
        amount: DIFFERENT_MINT_SAME_VALUE_BORROW,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    assertBankrunTxFailed(result, "0x1779");
  });

  it("(admin) tightening same-asset leverage makes a high-leverage P0/P0 position liquidatable", async () => {
    const liquidatee = users[1];
    const liquidator = users[2];
    const liquidateeAccount = await initFreshAccount(liquidatee);
    const liquidatorAccount = await initFreshAccount(liquidator);

    // This liquidation-specific path uses 20x / 21x -> 18x / 19x.
    // On a 100 USDC deposit with zero origination fee, the helper places
    // `SAME_ASSET_LIQUIDATION_BORROW` between:
    // - the healthy init boundary at 20x, using weight 19 / 20 = 0.95
    // - the tightened maint boundary at 19x, using weight 18 / 19 ~= 0.947368
    // after use the same oracle lower/upper confidence haircut used by the risk engine, at a 25%
    // position up from the tightened boundary toward the healthy one.
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_LIQUIDATION_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_LIQUIDATION_MAINT_LEVERAGE
        ),
      }),
      await depositIx(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: usdcBankA,
        tokenAccount: liquidatee.usdcAccount,
        amount: SAME_ASSET_DEPOSIT,
        depositUpToLimit: false,
      }),
      await borrowIx(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: usdcBankB,
        tokenAccount: liquidatee.usdcAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
        amount: SAME_ASSET_LIQUIDATION_BORROW,
      }),
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: usdcBankB,
        tokenAccount: liquidator.usdcAccount,
        amount: new BN(150 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, liquidatee.wallet, liquidator.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_LIQUIDATION_TIGHTENED_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_LIQUIDATION_TIGHTENED_MAINT_LEVERAGE
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    const liquidatorRemaining = await buildHealthRemainingAccounts(
      liquidatorAccount,
      {
        includedBankPks: [usdcBankA, usdcBankB],
      }
    );
    const liquidateeRemaining = await buildHealthRemainingAccounts(
      liquidateeAccount
    );

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: usdcBankA,
        liabilityBankKey: usdcBankB,
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.usdcOracle.publicKey,
          oracles.usdcOracle.publicKey,
          ...liquidatorRemaining,
          ...liquidateeRemaining,
        ],
        amount: SAME_ASSET_PARTIAL_LIQUIDATION,
        liquidatorAccounts: liquidatorRemaining.length,
        liquidateeAccounts: liquidateeRemaining.length,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidator.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(admin) same-asset deleverage can improve a tightened P0/P0 position", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = await initFreshAccount(deleveragee);
    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      deleverageeAccount
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      riskAdmin.usdcAccount,
      new BN(100 * 10 ** ecosystem.usdcDecimals)
    );

    // The position is opened with `SAME_ASSET_BORROW`, which the helper computes from the 100 USDC
    // deposit, the oracle lower/upper confidence haircut, zero origination fee, and the boundary
    // gap between:
    // - the healthy 101x init weight = 100 / 101 ~= 0.990099
    // - the tightened 100x maint weight = 99 / 100 = 0.99
    // `SAME_ASSET_BORROW` sits 25% of the way up from the tightened boundary toward the healthy
    // boundary.
    let tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        newRiskAdmin: riskAdmin.wallet.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_MAINT_LEVERAGE
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await depositIx(deleveragee.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: usdcBankA,
        tokenAccount: deleveragee.usdcAccount,
        amount: SAME_ASSET_DEPOSIT,
        depositUpToLimit: false,
      }),
      await borrowIx(deleveragee.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: usdcBankB,
        tokenAccount: deleveragee.usdcAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
        amount: SAME_ASSET_BORROW,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(deleveragee.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await healthPulse(deleveragee.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(deleveragee.wallet);
    await banksClient.processTransaction(tx);

    let account = await bankrunProgram.account.marginfiAccount.fetch(
      deleverageeAccount
    );
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_TIGHTENED_INIT_LEVERAGE
        ),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(
          SAME_ASSET_TIGHTENED_MAINT_LEVERAGE
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 700_000 }),
      await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        feePayer: riskAdmin.wallet.publicKey,
      }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(
          getSameAssetRemaining()
        ),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: usdcBankA,
        tokenAccount: riskAdmin.usdcAccount,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
        amount: SAME_ASSET_PARTIAL_LIQUIDATION,
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: usdcBankB,
        tokenAccount: riskAdmin.usdcAccount,
        amount: SAME_ASSET_PARTIAL_LIQUIDATION,
        remaining: composeRemainingAccounts(getSameAssetRemaining()),
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccountsMetaBanksOnly(
          getSameAssetRemaining()
        ),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(riskAdmin.wallet);
    await banksClient.processTransaction(tx);
  });
});
