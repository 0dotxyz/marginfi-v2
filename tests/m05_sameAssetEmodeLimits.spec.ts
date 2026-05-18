import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { assert } from "chai";
import {
  groupAdmin,
  bankrunContext,
  banksClient,
  bankrunProgram,
  ecosystem,
  oracles,
  users,
  verbose,
  riskAdmin,
} from "./rootHooks";
import {
  addBankWithSeed,
  configureBankOracle,
  groupConfigure,
  groupInitialize,
  handleBankruptcy,
} from "./utils/group-instructions";
import {
  defaultBankConfig,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  I80F48_ZERO,
  makeRatePoints,
  MAX_BALANCES,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
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
import { deriveBankWithSeed } from "./utils/pdas";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  buildHealthRemainingAccounts,
  createLut,
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  assertSameAssetBadDebtSurvivability,
  computeSameAssetBoundaryBorrowNative,
  setAssetShareValueHaircut,
  warpToNextBankrunSlot,
} from "./utils/same-asset-emode";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assertBankrunTxFailed } from "./utils/genericTests";

const startingSeed = 399;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_1234000000M5");

// Half of MAX_BALANCES are collateral (deposits) and half are liabilities (borrows),
// exercising the min-cost max-flow exact engine at its maximum fan-in/fan-out.
const NUM_COLLATERAL_BANKS = MAX_BALANCES / 2; // 8
const NUM_LIABILITY_BANKS = MAX_BALANCES / 2; // 8

// Leverage range for the liquidation and deleverage tests:
//   healthy at 20x/21x, maintenance-underwater after tightening to 18x/19x.
const INIT_LEVERAGE = 20;
const MAINT_LEVERAGE = 21;
const TIGHTENED_INIT_LEVERAGE = 18;
const TIGHTENED_MAINT_LEVERAGE = 19;

// 100 USDC per collateral bank, 800 USDC total collateral across all 8 banks.
const DEPOSIT_PER_BANK = new BN(100 * 10 ** ecosystem.usdcDecimals);
const TOTAL_COLLATERAL = new BN(
  NUM_COLLATERAL_BANKS * 100 * 10 ** ecosystem.usdcDecimals
);

// Partial deleverage/liquidation sizes (USDC native; small enough to fit in one tx).
const PARTIAL_DELEVERAGE_AMOUNT = new BN(1_000_000); // 1 USDC
const PARTIAL_LIQUIDATE_AMOUNT = new BN(50_000);

// Key names for user accounts in the shared mockUser.accounts map.
const USER_LIQUIDATEE = "m05_liquidatee";
const USER_LIQUIDATOR = "m05_liquidator";
const USER_DELEVERAGEE = "m05_deleveragee";
const USER_BAD_DEBT = "m05_bad_debt";

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;
// All bank+oracle pairs ordered by bank pubkey (descending), matching composeRemainingAccounts.
let allBankPairs: PublicKey[][] = [];
let lutAccount: AddressLookupTableAccount;

/**
 * Total borrow sized between the healthy 20x init boundary and the tightened 19x maint boundary.
 * This borrow is used for the liquidation and deleverage tests.
 * The position is healthy under 20x/21x and flips maintenance-underwater after tightening to 18x/19x.
 */
const LIQUIDATION_BORROW_TOTAL = computeSameAssetBoundaryBorrowNative({
  collateralNative: TOTAL_COLLATERAL,
  collateralDecimals: ecosystem.usdcDecimals,
  collateralPrice: ecosystem.usdcPrice,
  liabilityDecimals: ecosystem.usdcDecimals,
  liabilityPrice: ecosystem.usdcPrice,
  healthyInitLeverage: INIT_LEVERAGE,
  tightenedRequirementLeverage: TIGHTENED_MAINT_LEVERAGE,
  liabilityOriginationFeeRate: 0,
});

/**
 * Total borrow sized between the healthy 20x init boundary and the post-haircut 21x maint boundary.
 * The haircut (199/200) is applied to each of the 8 collateral banks, pushing the account
 * maintenance-underwater while leaving it equity-solvent.
 */
const BAD_DEBT_BORROW_TOTAL = computeSameAssetBoundaryBorrowNative({
  collateralNative: TOTAL_COLLATERAL,
  collateralDecimals: ecosystem.usdcDecimals,
  collateralPrice: ecosystem.usdcPrice,
  liabilityDecimals: ecosystem.usdcDecimals,
  liabilityPrice: ecosystem.usdcPrice,
  healthyInitLeverage: INIT_LEVERAGE,
  tightenedRequirementLeverage: MAINT_LEVERAGE,
  haircut: { numerator: 199, denominator: 200 },
  liabilityOriginationFeeRate: 0,
});

/**
 * Split a total BN borrow evenly across `count` banks.
 * All banks receive the floor-divided amount; the last bank absorbs any remainder
 * so that the sum exactly equals `total`.
 */
function splitBorrowAcrossBanks(total: BN, count: number): BN[] {
  const perBank = total.divn(count);
  const parts: BN[] = [];
  for (let i = 0; i < count - 1; i++) {
    parts.push(perBank);
  }
  // Last bank absorbs the rounding remainder.
  parts.push(total.sub(perBank.muln(count - 1)));
  return parts;
}

async function buildVersionedTx(
  signer: Keypair,
  instructions: TransactionInstruction[],
  lut: AddressLookupTableAccount
): Promise<VersionedTransaction> {
  const blockhash = await getBankrunBlockhash(bankrunContext);
  const messageV0 = new TransactionMessage({
    payerKey: signer.publicKey,
    recentBlockhash: blockhash,
    instructions,
  }).compileToV0Message([lut]);
  const versionedTx = new VersionedTransaction(messageV0);
  versionedTx.sign([signer]);
  return versionedTx;
}

/**
 * Deposit DEPOSIT_PER_BANK USDC into each of the NUM_COLLATERAL_BANKS collateral banks, then borrow
 * from each liability bank using the given split amounts.
 */
async function openMaxPositions(
  user: (typeof users)[number],
  accountPk: PublicKey,
  borrowAmounts: BN[]
): Promise<void> {
  const DEPOSITS_PER_TX = 4;
  for (let i = 0; i < NUM_COLLATERAL_BANKS; i += DEPOSITS_PER_TX) {
    const chunk = banks.slice(i, Math.min(i + DEPOSITS_PER_TX, NUM_COLLATERAL_BANKS));
    const tx = new Transaction();
    for (const bank of chunk) {
      tx.add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: accountPk,
          bank,
          tokenAccount: user.usdcAccount,
          amount: DEPOSIT_PER_BANK,
          depositUpToLimit: false,
        })
      );
    }
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  }

  // Build remaining-accounts for the collaterals.
  const activeGroups: PublicKey[][] = [];
  for (let i = 0; i < NUM_COLLATERAL_BANKS; i++) {
    activeGroups.push([banks[i], oracles.usdcOracle.publicKey]);
  }

  for (let j = 0; j < NUM_LIABILITY_BANKS; j++) {
    const liabBank = banks[NUM_COLLATERAL_BANKS + j];
    activeGroups.push([liabBank, oracles.usdcOracle.publicKey]);
    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: accountPk,
        bank: liabBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(activeGroups),
        amount: borrowAmounts[j],
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  }
}

describe("m05: Same-asset emode limits (MAX_BALANCES positions)", () => {
  // -------------------------------------------------------------------------
  // Setup: new group, 16 USDC banks, same-asset emode, user accounts, LUT
  // -------------------------------------------------------------------------

  it("(setup) Init group, add 16 USDC banks, configure same-asset emode, seed liquidity", async () => {
    throwawayGroup = Keypair.fromSeed(groupBuff);

    // Init group with groupAdmin as admin.
    let tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        newRiskAdmin: riskAdmin.wallet.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    // Add NUM_COLLATERAL_BANKS + NUM_LIABILITY_BANKS = 16 USDC banks.
    const bankConfig = defaultBankConfig();
    bankConfig.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    bankConfig.assetWeightMaint = bigNumberToWrappedI80F48(0.6);
    bankConfig.depositLimit = new BN(100_000_000_000_000);
    bankConfig.borrowLimit = new BN(100_000_000_000_000);
    bankConfig.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
    bankConfig.interestRateConfig.points = makeRatePoints([0.8], [0.2]);

    // Add all 16 banks.
    for (let i = 0; i < MAX_BALANCES; i++) {
      const seed = new BN(startingSeed + i);
      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        throwawayGroup.publicKey,
        ecosystem.usdcMint.publicKey,
        seed
      );
      const addTx = new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          config: bankConfig,
          seed,
        })
      );
      await processBankrunTransaction(bankrunContext, addTx, [groupAdmin.wallet]);
      banks.push(bankPk);
      allBankPairs.push([bankPk, oracles.usdcOracle.publicKey]);
      if (verbose) console.log(`*init USDC bank #${i}: ${bankPk}`);
    }

    // Configure oracles.
    for (let i = 0; i < MAX_BALANCES; i++) {
      const oracleTx = new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
          bank: banks[i],
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: oracles.usdcOracle.publicKey,
        })
      );
      await processBankrunTransaction(bankrunContext, oracleTx, [groupAdmin.wallet]);
    }

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Init user accounts in the throwawayGroup.
    const userKeys = [USER_LIQUIDATEE, USER_LIQUIDATOR, USER_DELEVERAGEE, USER_BAD_DEBT];
    for (let i = 0; i < users.length; i++) { // users.length = 4
      const accountKeypair = Keypair.generate();
      users[i].accounts.set(userKeys[i], accountKeypair.publicKey);
      const initTx = new Transaction().add(
        await accountInit(users[i].mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: accountKeypair.publicKey,
          authority: users[i].wallet.publicKey,
          feePayer: users[i].wallet.publicKey,
        })
      );
      await processBankrunTransaction(bankrunContext, initTx, [
        users[i].wallet,
        accountKeypair,
      ]);
    }

    // Mint enough USDC for each user (2,000 USDC; 800 for deposits, headroom for borrows/repays).
    const mintAmount = new BN(2_000 * 10 ** ecosystem.usdcDecimals);
    for (const user of users) {
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        mintAmount
      );
    }

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      riskAdmin.usdcAccount,
      mintAmount
    );

    // Seed liability banks.
    const seederKeypair = Keypair.generate();
    const seederInitTx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        marginfiAccount: seederKeypair.publicKey,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, seederInitTx, [
      groupAdmin.wallet,
      seederKeypair,
    ]);

    // 500 USDC per liability bank; 4 users each borrow ~92 USDC/bank = ~368 USDC total demand per bank.
    const seedAmount = new BN(500 * 10 ** ecosystem.usdcDecimals);
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      groupAdmin.usdcAccount,
      new BN(NUM_LIABILITY_BANKS * 500 * 10 ** ecosystem.usdcDecimals)
    );
    const SEED_PER_TX = 4;
    const liabilityBanks = banks.slice(NUM_COLLATERAL_BANKS);
    for (let i = 0; i < liabilityBanks.length; i += SEED_PER_TX) {
      const chunk = liabilityBanks.slice(i, Math.min(i + SEED_PER_TX, liabilityBanks.length));
      const seedTx = new Transaction();
      for (const bank of chunk) {
        seedTx.add(
          await depositIx(groupAdmin.mrgnBankrunProgram, {
            marginfiAccount: seederKeypair.publicKey,
            bank,
            tokenAccount: groupAdmin.usdcAccount,
            amount: seedAmount,
            depositUpToLimit: false,
          })
        );
      }
      await processBankrunTransaction(bankrunContext, seedTx, [groupAdmin.wallet]);
    }

    // Build the shared LUT for all 16 bank+oracle pairs.
    const allAddresses = allBankPairs.flat();
    lutAccount = await createLut(groupAdmin.wallet, allAddresses);

    if (verbose) {
      console.log("throwawayGroup:", throwawayGroup.publicKey.toBase58());
      console.log("LUT:", lutAccount.key.toBase58());
    }
  });

  it("(admin) tightening same-asset leverage makes a MAX_BALANCES P0/P0 position liquidatable", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];
    const liquidateeAccount = liquidatee.accounts.get(USER_LIQUIDATEE);
    const liquidatorAccount = liquidator.accounts.get(USER_LIQUIDATOR);

    // The total borrow is split evenly across 8 liability banks; the last bank absorbs
    // the integer-division remainder so the sum is exact.
    const borrowAmounts = splitBorrowAcrossBanks(LIQUIDATION_BORROW_TOTAL, NUM_LIABILITY_BANKS);

    // Liquidatee opens 8 deposits + 8 borrows (MAX_BALANCES positions total).
    // The final borrow call exercises the risk engine across all 16 active balances with
    // same-asset emode active, implicitly testing OOM safety on the same-asset-emode paths.
    await openMaxPositions(liquidatee, liquidateeAccount, borrowAmounts);

    // Liquidator holds a stake in banks[8] (the first liability bank) so the liquidation
    // can transfer debt-repayment shares from the liquidator to the liquidatee.
    let tx = new Transaction().add(
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: banks[NUM_COLLATERAL_BANKS],
        tokenAccount: liquidator.usdcAccount,
        amount: new BN(200 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [liquidator.wallet]);

    // Confirm the position is healthy before tightening.
    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts(allBankPairs),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [liquidatee.wallet]);
    let account = await bankrunProgram.account.marginfiAccount.fetch(liquidateeAccount);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0, "should be healthy before tightening");
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);

    // Tighten leverage from 20x/21x to 18x/19x. The borrow was sized 25% of the way between the
    // tightened 19x maint boundary and the healthy 20x init boundary, so the account flips
    // maintenance-underwater once the maint weight drops from sameAsset(21)=20/21 to sameAsset(19)=18/19.
    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(TIGHTENED_INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(TIGHTENED_MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    await warpToNextBankrunSlot(bankrunContext); // This is to help with blockhash errors.

    // Liquidate banks[0] (asset) against banks[8].
    const liquidatorRemaining = await buildHealthRemainingAccounts(liquidatorAccount, {
      includedBankPks: [banks[0], banks[NUM_COLLATERAL_BANKS]],
    });
    const liquidateeRemaining = await buildHealthRemainingAccounts(liquidateeAccount);

    const liquidateTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: banks[0],
        liabilityBankKey: banks[NUM_COLLATERAL_BANKS],
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.usdcOracle.publicKey, // asset oracle
          oracles.usdcOracle.publicKey, // liability oracle
          ...liquidatorRemaining,
          ...liquidateeRemaining,
        ],
        amount: PARTIAL_LIQUIDATE_AMOUNT,
        liquidatorAccounts: liquidatorRemaining.length,
        liquidateeAccounts: liquidateeRemaining.length,
      })
    );
    const versionedLiquidateTx = await buildVersionedTx(
      liquidator.wallet,
      liquidateTx.instructions,
      lutAccount
    );
    await banksClient.processTransaction(versionedLiquidateTx);
    
  });

  it("(admin) same-asset deleverage can improve a tightened MAX_BALANCES P0/P0 position", async () => {
    const deleveragee = users[2];
    const deleverageeAccount = deleveragee.accounts.get(USER_DELEVERAGEE);

    // Restore leverage so subsequent tests start from a clean state.
    const resetTx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, resetTx, [groupAdmin.wallet]);

    const borrowAmounts = splitBorrowAcrossBanks(LIQUIDATION_BORROW_TOTAL, NUM_LIABILITY_BANKS);

    await openMaxPositions(deleveragee, deleverageeAccount, borrowAmounts);

    // Confirm healthy before tightening.
    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await healthPulse(deleveragee.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccounts(allBankPairs),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [deleveragee.wallet]);
    let account = await bankrunProgram.account.marginfiAccount.fetch(deleverageeAccount);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0, "should be healthy before tightening");

    // Tighten to 18x/19x to make the position maintenance-underwater.
    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(TIGHTENED_INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(TIGHTENED_MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    await warpToNextBankrunSlot(bankrunContext); // This is to help with blockhash errors.

    // Partially deleverage: withdraw 1 USDC from banks[0] and repay 1 USDC to banks[8].
    // All 16 remaining accounts are passed through the LUT-backed versioned tx.
    const delevTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        feePayer: riskAdmin.wallet.publicKey,
      }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(allBankPairs),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[0],
        tokenAccount: riskAdmin.usdcAccount,
        remaining: composeRemainingAccounts(allBankPairs),
        amount: PARTIAL_DELEVERAGE_AMOUNT,
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[NUM_COLLATERAL_BANKS],
        tokenAccount: riskAdmin.usdcAccount,
        remaining: composeRemainingAccounts(allBankPairs),
        amount: PARTIAL_DELEVERAGE_AMOUNT,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccountsMetaBanksOnly(allBankPairs),
      })
    );
    const versionedDelevTx = await buildVersionedTx(
      riskAdmin.wallet,
      delevTx.instructions,
      lutAccount
    );
    await banksClient.processTransaction(versionedDelevTx);

    // Restore leverage for subsequent tests.
    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(admin) same-asset bad-debt haircut across all collateral banks preserves equity buffer and enables deleverage", async () => {
    const badDebtUser = users[3];
    const badDebtAccount = badDebtUser.accounts.get(USER_BAD_DEBT);

    // Restore leverage so subsequent tests start from a clean state.
    const resetTx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(INIT_LEVERAGE),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(MAINT_LEVERAGE),
      })
    );
    await processBankrunTransaction(bankrunContext, resetTx, [groupAdmin.wallet]);    

    // The borrow is sized between the pre-haircut 20x init boundary and the post-haircut 21x maint
    // boundary (haircut 199/200 on each collateral bank). The position is accepted before the haircut
    // and becomes maintenance-underwater only after all 8 collateral banks are haircutted, while
    // remaining equity-solvent throughout.
    const borrowAmounts = splitBorrowAcrossBanks(BAD_DEBT_BORROW_TOTAL, NUM_LIABILITY_BANKS);
    await openMaxPositions(badDebtUser, badDebtAccount, borrowAmounts);

    // Pulse health and record the pre-haircut equity asset value for the survivability assertion.
    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await healthPulse(badDebtUser.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        remaining: composeRemainingAccounts(allBankPairs),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [badDebtUser.wallet]);
    let account = await bankrunProgram.account.marginfiAccount.fetch(badDebtAccount);
    const originalAssetValueEquity = wrappedI80F48toBigNumber(
      account.healthCache.assetValueEquity
    );
    // Assert the position is healthy before any haircut is applied.
    assertSameAssetBadDebtSurvivability({
      healthCache: account.healthCache,
      originalAssetValueEquity,
      label: "pre-haircut (MAX_BALANCES)",
      requireMaintenanceUnderwater: false,
    });

    // Apply a 199/200 (50bps) asset-share-value haircut to every collateral bank (banks[0..7]).
    for (let i = 0; i < NUM_COLLATERAL_BANKS; i++) {
      await setAssetShareValueHaircut(
        bankrunProgram,
        banksClient,
        bankrunContext,
        banks[i],
        199,
        200
      );
    }

    await warpToNextBankrunSlot(bankrunContext); // This is to help with blockhash errors.

    // Re-pulse health after all 8 haircuts are applied.
    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await healthPulse(badDebtUser.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        remaining: composeRemainingAccounts(allBankPairs),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [badDebtUser.wallet]);
    account = await bankrunProgram.account.marginfiAccount.fetch(badDebtAccount);

    // The account must be maintenance-underwater (eligible for deleverage) yet remain
    // equity-solvent (bankruptcy is not possible) and the equity-to-maint buffer must be
    // at least 50bps of the original equity asset value.
    assertSameAssetBadDebtSurvivability({
      healthCache: account.healthCache,
      originalAssetValueEquity,
      label: "post-haircut (MAX_BALANCES, all 8 collateral banks)",
    });

    // Confirm that the bankruptcy instruction is rejected because the account is equity-solvent.
    // Error 6013 = NotBankrupt.
    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await handleBankruptcy(groupAdmin.mrgnBankrunProgram, {
        signer: groupAdmin.wallet.publicKey,
        marginfiAccount: badDebtAccount,
        bank: banks[NUM_COLLATERAL_BANKS],
        remaining: composeRemainingAccounts(allBankPairs),
      })
    );
    const bankruptcyResult = await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      true,
      true
    );
    assertBankrunTxFailed(bankruptcyResult, 6013);

    // Risk admin partially deleverages by withdrawing from banks[0] and repaying to banks[8].
    const badDebtDelevTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        feePayer: riskAdmin.wallet.publicKey,
      }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(allBankPairs),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        bank: banks[0],
        tokenAccount: riskAdmin.usdcAccount,
        remaining: composeRemainingAccounts(allBankPairs),
        amount: PARTIAL_DELEVERAGE_AMOUNT,
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        bank: banks[NUM_COLLATERAL_BANKS],
        tokenAccount: riskAdmin.usdcAccount,
        remaining: composeRemainingAccounts(allBankPairs),
        amount: PARTIAL_DELEVERAGE_AMOUNT,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: badDebtAccount,
        remaining: composeRemainingAccountsMetaBanksOnly(allBankPairs),
      })
    );
    const versionedBadDebtDelevTx = await buildVersionedTx(
      riskAdmin.wallet,
      badDebtDelevTx.instructions,
      lutAccount
    );
    await banksClient.processTransaction(versionedBadDebtDelevTx);
  });
});