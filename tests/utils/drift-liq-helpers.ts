import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";
import { BanksTransactionResultWithMeta } from "solana-bankrun";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  driftAccounts,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "../rootHooks";
import { MockUser } from "./mocks";
import {
  groupInitialize,
  addBankWithSeed,
  configureBank,
  accrueInterest,
} from "./group-instructions";
import {
  accountInit,
  depositIx,
  borrowIx,
  healthPulse,
  composeRemainingAccounts,
  liquidateIx,
} from "./user-instructions";
import { deriveBankWithSeed } from "./pdas";
import { getBankrunBlockhash } from "./spl-staking-utils";
import { processBankrunTransaction as processBankrunTx } from "./tools";
import {
  makeAddDriftBankIx,
  makeInitDriftUserIx,
  makeDriftDepositIx,
} from "./drift-instructions";
import {
  defaultDriftBankConfig,
  TOKEN_A_MARKET_INDEX,
  tokenAmountToScaledBalance,
  getSpotMarketAccount,
} from "./drift-utils";
import {
  defaultBankConfig,
  I80F48_ZERO,
  makeRatePoints,
  ORACLE_SETUP_PYTH_PUSH,
  blankBankConfigOptRaw,
} from "./types";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";

// Group seed for d14 tests
export const THROWAWAY_GROUP_SEED_D14 = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000014"
);

const USER_ACCOUNT_D14 = "d14_account";
const STARTING_SEED = 100;

// Max drift banks that should pass (9th will fail due to active bank limit)
export const PASSING_DRIFT_COUNT = 8;
// Regular banks needed: 15 - 1 = 14 minimum (when driftCount=1, regularCount=14)
const REGULAR_BANK_COUNT = 15;

export interface DriftLiqEnv {
  // Core context
  group: PublicKey;
  throwawayGroup: Keypair;
  driftSpotMarket: PublicKey;
  driftOracle: PublicKey;
  tokenMint: PublicKey;

  // Banks (PASSING_DRIFT_COUNT + 1 drift, REGULAR_BANK_COUNT regular, 1 liab)
  driftBanks: PublicKey[];
  regularBanks: PublicKey[];
  liabBank: PublicKey;

  // Liquidator (reused across scenarios)
  liquidatorUser: MockUser;
  liquidatorAccount: PublicKey;

  // LUT for V0 transactions
  lutAddress: PublicKey;

  // Context
  ctx: ProgramTestContext;
}

export interface ScenarioResult {
  driftCount: number;
  regularCount: number;
  success: boolean;
  errorType?: "OOM" | "CU_EXCEEDED" | "TOO_SEVERE" | "OTHER";
  errorMessage?: string;
  errorCode?: number;
  cuUsed?: number;
}

export interface ScenarioOpts {
  driftCount: number;
  regularCount: number;
  scenarioIndex: number;
}

/**
 * Sets up environment with PASSING_DRIFT_COUNT+1 drift banks + REGULAR_BANK_COUNT regular banks + 1 liability bank + LUT.
 * Banks are created once and reused across all scenarios.
 */
export async function setupDriftLiqEnv(): Promise<DriftLiqEnv> {
  const ctx = bankrunContext;
  const mrgnID = bankrunProgram.programId;
  const throwawayGroup = Keypair.fromSeed(THROWAWAY_GROUP_SEED_D14);
  const driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
  const driftOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);

  const driftBanks: PublicKey[] = [];
  const regularBanks: PublicKey[] = [];

  // 1. Initialize group
  {
    const tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(ctx);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);
  }

  // 2. Create drift banks (one extra for the failing test)
  for (let i = 0; i < PASSING_DRIFT_COUNT + 1; i++) {
    const seed = new BN(STARTING_SEED + i);
    const [driftBank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );

    const defaultConfig = defaultDriftBankConfig(
      oracles.tokenAOracle.publicKey
    );
    const tx = new Transaction().add(
      await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: driftSpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        { config: defaultConfig, seed }
      )
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);

    driftBanks.push(driftBank);
  }

  // 3. Initialize drift user for each drift bank
  for (let i = 0; i < PASSING_DRIFT_COUNT + 1; i++) {
    const driftBank = driftBanks[i];
    const initUserAmount = new BN(100 + i);

    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        initUserAmount.toNumber()
      )
    );
    await processBankrunTx(ctx, fundTx, [globalProgramAdmin.wallet]);

    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank: driftBank,
          signerTokenAccount: groupAdmin.tokenAAccount,
          driftOracle: driftOracle,
        },
        { amount: initUserAmount },
        TOKEN_A_MARKET_INDEX
      )
    );
    await processBankrunTx(ctx, initUserTx, [groupAdmin.wallet]);
  }

  // 4. Create regular banks using Token A (same as drift banks)
  for (let i = 0; i < REGULAR_BANK_COUNT; i++) {
    const seed = new BN(STARTING_SEED + 50 + i);
    await addGenericRegularBank(throwawayGroup, seed, i, undefined, true);

    const [regularBank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );
    regularBanks.push(regularBank);
  }

  // 5. Create 1 liability bank (seed 199) using LST
  const liabSeed = new BN(STARTING_SEED + 99);
  await addGenericRegularBank(throwawayGroup, liabSeed, -1, "liab");
  const [liabBank] = deriveBankWithSeed(
    mrgnID,
    throwawayGroup.publicKey,
    ecosystem.lstAlphaMint.publicKey,
    liabSeed
  );

  // 6. Seed liquidity in liab bank (admin deposits)
  {
    const wallet = globalProgramAdmin.wallet;
    const seedAmount = new BN(1_000 * 10 ** ecosystem.lstAlphaDecimals);

    // Fund admin with LST
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        groupAdmin.lstAlphaAccount,
        wallet.publicKey,
        seedAmount.toNumber()
      )
    );
    await processBankrunTx(ctx, fundTx, [wallet]);

    // Create admin marginfi account
    if (!groupAdmin.accounts.has(USER_ACCOUNT_D14)) {
      const adminKp = Keypair.generate();
      groupAdmin.accounts.set(USER_ACCOUNT_D14, adminKp.publicKey);

      const initTx = new Transaction().add(
        await accountInit(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: adminKp.publicKey,
          authority: groupAdmin.wallet.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
        })
      );
      await processBankrunTx(ctx, initTx, [groupAdmin.wallet, adminKp]);
    }

    const adminAccount = groupAdmin.accounts.get(USER_ACCOUNT_D14);
    const depositTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: liabBank,
        tokenAccount: groupAdmin.lstAlphaAccount,
        amount: seedAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, depositTx, [groupAdmin.wallet]);
  }

  // 7. Setup liquidator (reused across scenarios)
  const liquidatorUser = users[1];
  let liquidatorAccount: PublicKey;

  if (!liquidatorUser.accounts.has(USER_ACCOUNT_D14)) {
    const kp = Keypair.generate();
    liquidatorUser.accounts.set(USER_ACCOUNT_D14, kp.publicKey);

    const tx = new Transaction().add(
      await accountInit(liquidatorUser.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        marginfiAccount: kp.publicKey,
        authority: liquidatorUser.wallet.publicKey,
        feePayer: liquidatorUser.wallet.publicKey,
      })
    );
    await processBankrunTx(ctx, tx, [liquidatorUser.wallet, kp]);
  }
  liquidatorAccount = liquidatorUser.accounts.get(USER_ACCOUNT_D14);

  // Fund liquidator with LST
  {
    const lstAmount = new BN(10_000 * 10 ** ecosystem.lstAlphaDecimals);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        liquidatorUser.lstAlphaAccount,
        globalProgramAdmin.wallet.publicKey,
        lstAmount.toNumber()
      )
    );
    await processBankrunTx(ctx, fundTx, [globalProgramAdmin.wallet]);

    // Deposit into liab bank for liquidator
    const depositTx = new Transaction().add(
      await depositIx(liquidatorUser.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: liabBank,
        tokenAccount: liquidatorUser.lstAlphaAccount,
        amount: lstAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, depositTx, [liquidatorUser.wallet]);
  }

  // 8. Create and extend LUT
  const lutAddress = await createAndExtendLUT(
    ctx,
    liquidatorUser,
    throwawayGroup.publicKey,
    liquidatorAccount,
    driftBanks,
    regularBanks,
    liabBank,
    driftSpotMarket
  );

  return {
    group: throwawayGroup.publicKey,
    throwawayGroup,
    driftSpotMarket,
    driftOracle,
    tokenMint: ecosystem.tokenAMint.publicKey,
    driftBanks,
    regularBanks,
    liabBank,
    liquidatorUser,
    liquidatorAccount,
    lutAddress,
    ctx,
  };
}

/**
 * Runs a single liquidation scenario with fresh borrower account.
 */
export async function runLiquidationScenario(
  env: DriftLiqEnv,
  opts: ScenarioOpts
): Promise<ScenarioResult> {
  const { driftCount, regularCount, scenarioIndex } = opts;
  const {
    ctx,
    driftBanks,
    regularBanks,
    liabBank,
    driftSpotMarket,
    driftOracle,
  } = env;

  // Use a different user for each scenario to get fresh account
  const borrowerUser = users[0];
  const accountName = `d14_scenario_${scenarioIndex}`;

  // 1. Create fresh marginfi account for this scenario
  const borrowerKp = Keypair.generate();
  borrowerUser.accounts.set(accountName, borrowerKp.publicKey);

  const initTx = new Transaction().add(
    await accountInit(borrowerUser.mrgnBankrunProgram, {
      marginfiGroup: env.group,
      marginfiAccount: borrowerKp.publicKey,
      authority: borrowerUser.wallet.publicKey,
      feePayer: borrowerUser.wallet.publicKey,
    })
  );
  await processBankrunTx(ctx, initTx, [borrowerUser.wallet, borrowerKp]);

  const borrowerAccount = borrowerKp.publicKey;

  // 2. Fund borrower with Token A and LST
  const tokenAAmount = new BN(1000 * 10 ** ecosystem.tokenADecimals);
  const lstAmount = new BN(1000 * 10 ** ecosystem.lstAlphaDecimals);

  const fundTokenATx = new Transaction().add(
    createMintToInstruction(
      ecosystem.tokenAMint.publicKey,
      borrowerUser.tokenAAccount,
      globalProgramAdmin.wallet.publicKey,
      tokenAAmount.toNumber()
    )
  );
  await processBankrunTx(ctx, fundTokenATx, [globalProgramAdmin.wallet]);

  const fundLstTx = new Transaction().add(
    createMintToInstruction(
      ecosystem.lstAlphaMint.publicKey,
      borrowerUser.lstAlphaAccount,
      globalProgramAdmin.wallet.publicKey,
      lstAmount.toNumber()
    )
  );
  await processBankrunTx(ctx, fundLstTx, [globalProgramAdmin.wallet]);

  // 3. Open drift positions
  const driftDepositAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);
  for (let i = 0; i < driftCount; i++) {
    const depositTx = new Transaction().add(
      await makeDriftDepositIx(
        borrowerUser.mrgnBankrunProgram,
        {
          marginfiAccount: borrowerAccount,
          bank: driftBanks[i],
          signerTokenAccount: borrowerUser.tokenAAccount,
          driftOracle,
        },
        driftDepositAmount,
        TOKEN_A_MARKET_INDEX
      )
    );
    await processBankrunTx(ctx, depositTx, [borrowerUser.wallet]);
  }

  // 4. Open regular positions (same Token A as drift banks)
  const regularDepositAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);
  for (let i = 0; i < regularCount; i++) {
    const depositTx = new Transaction().add(
      await depositIx(borrowerUser.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        bank: regularBanks[i],
        tokenAccount: borrowerUser.tokenAAccount,
        amount: regularDepositAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, depositTx, [borrowerUser.wallet]);
  }

  // 5. Build remaining accounts for all positions
  const allPositions: PublicKey[][] = [];

  // Drift positions: [bank, oracle, spotMarket]
  for (let i = 0; i < driftCount; i++) {
    allPositions.push([
      driftBanks[i],
      oracles.tokenAOracle.publicKey,
      driftSpotMarket,
    ]);
  }

  // Regular positions: [bank, oracle] - using Token A oracle (same as drift)
  for (let i = 0; i < regularCount; i++) {
    allPositions.push([regularBanks[i], oracles.tokenAOracle.publicKey]);
  }

  // Liab position: [bank, oracle]
  allPositions.push([liabBank, oracles.pythPullLst.publicKey]);

  const remainingForBorrow = composeRemainingAccounts(allPositions);

  // 6. Borrow from liab bank
  // Borrow enough to make account unhealthy when liability weight is increased
  // With 150 Token A at $10 (0.5 weight) = $750 effective collateral
  // Can borrow max ~4 LST at $175 = $700 initially
  // After 5.5x liability weight: 4 * 175 * 5.5 = $3,850 vs $750 = unhealthy
  const borrowAmount = new BN(4 * 10 ** ecosystem.lstAlphaDecimals);
  const borrowTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
    await borrowIx(borrowerUser.mrgnBankrunProgram, {
      marginfiAccount: borrowerAccount,
      bank: liabBank,
      tokenAccount: borrowerUser.lstAlphaAccount,
      remaining: remainingForBorrow,
      amount: borrowAmount,
    })
  );
  await processBankrunTx(ctx, borrowTx, [borrowerUser.wallet], false, true);

  // 7. Make borrower unhealthy by raising liability weights
  const config = blankBankConfigOptRaw();
  config.liabilityWeightInit = bigNumberToWrappedI80F48(6.0);
  config.liabilityWeightMaint = bigNumberToWrappedI80F48(5.5);

  const configTx = new Transaction().add(
    await configureBank(groupAdmin.mrgnBankrunProgram, {
      bank: liabBank,
      bankConfigOpt: config,
    })
  );
  await processBankrunTx(ctx, configTx, [groupAdmin.wallet]);

  // Health pulse to update cache
  const healthTx = new Transaction().add(
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
    await healthPulse(borrowerUser.mrgnBankrunProgram, {
      marginfiAccount: borrowerAccount,
      remaining: remainingForBorrow,
    })
  );
  await processBankrunTx(ctx, healthTx, [borrowerUser.wallet], false, true);

  // 8. Attempt liquidation
  const result = await attemptLiquidation(
    env,
    borrowerAccount,
    driftCount,
    regularCount
  );

  // 9. Restore liab bank config
  const restoreConfig = blankBankConfigOptRaw();
  restoreConfig.liabilityWeightInit = bigNumberToWrappedI80F48(1.0);
  restoreConfig.liabilityWeightMaint = bigNumberToWrappedI80F48(1.0);

  const restoreTx = new Transaction().add(
    await configureBank(groupAdmin.mrgnBankrunProgram, {
      bank: liabBank,
      bankConfigOpt: restoreConfig,
    })
  );
  await processBankrunTx(ctx, restoreTx, [groupAdmin.wallet]);

  return {
    driftCount,
    regularCount,
    ...result,
  };
}

async function attemptLiquidation(
  env: DriftLiqEnv,
  borrowerAccount: PublicKey,
  driftCount: number,
  regularCount: number
): Promise<Omit<ScenarioResult, "driftCount" | "regularCount">> {
  const {
    ctx,
    driftBanks,
    regularBanks,
    liabBank,
    driftSpotMarket,
    liquidatorUser,
    liquidatorAccount,
  } = env;

  // Build remaining accounts for liquidation
  // Header: asset oracle, asset spot market, liab oracle
  const remaining: PublicKey[] = [
    oracles.tokenAOracle.publicKey, // asset oracle
    driftSpotMarket, // asset spot market
    oracles.pythPullLst.publicKey, // liab oracle
  ];

  // Liquidator positions (will receive drift asset, pay from liab)
  const liquidatorPositions: PublicKey[][] = [
    [liabBank, oracles.pythPullLst.publicKey],
    [driftBanks[0], oracles.tokenAOracle.publicKey, driftSpotMarket],
  ];
  remaining.push(...composeRemainingAccounts(liquidatorPositions));

  // Liquidatee positions (all 16)
  const liquidateePositions: PublicKey[][] = [];

  // Drift positions
  for (let i = 0; i < driftCount; i++) {
    liquidateePositions.push([
      driftBanks[i],
      oracles.tokenAOracle.publicKey,
      driftSpotMarket,
    ]);
  }

  // Regular positions (using Token A oracle, same as drift)
  for (let i = 0; i < regularCount; i++) {
    liquidateePositions.push([regularBanks[i], oracles.tokenAOracle.publicKey]);
  }

  // Liab position
  liquidateePositions.push([liabBank, oracles.pythPullLst.publicKey]);

  remaining.push(...composeRemainingAccounts(liquidateePositions));

  // Get spot market for amount scaling
  const spotMarket = await getSpotMarketAccount(
    (
      await import("../rootHooks")
    ).driftBankrunProgram,
    TOKEN_A_MARKET_INDEX
  );

  const smallAmount = new BN(0.01 * 10 ** ecosystem.tokenADecimals);
  const liquidateAmount = tokenAmountToScaledBalance(smallAmount, spotMarket);

  // Build liquidation instruction
  // Liquidator: 2 positions = 5 accounts (liab: 2, drift: 3)
  // Liquidatee: driftCount*3 + regularCount*2 + 2 (liab) accounts
  const liquidatorAccountCount = 5;
  const liquidateeAccountCount = driftCount * 3 + regularCount * 2 + 2;

  const liquidateInstruction = await liquidateIx(
    liquidatorUser.mrgnBankrunProgram,
    {
      assetBankKey: driftBanks[0],
      liabilityBankKey: liabBank,
      liquidatorMarginfiAccount: liquidatorAccount,
      liquidateeMarginfiAccount: borrowerAccount,
      remaining,
      amount: liquidateAmount,
      liquidateeAccounts: liquidateeAccountCount,
      liquidatorAccounts: liquidatorAccountCount,
    }
  );

  const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
    units: 1_400_000,
  });

  // Accrue interest on the asset bank before liquidation
  const accrueIx = await accrueInterest(liquidatorUser.mrgnBankrunProgram, {
    bank: driftBanks[0],
  });

  const tx = new Transaction().add(
    computeBudgetIx,
    accrueIx,
    liquidateInstruction
  );

  // Process with dumpLogOnFail=true - will print logs and throw on failure
  const result = (await processBankrunTx(
    ctx,
    tx,
    [liquidatorUser.wallet],
    false, // trySend=false so it simulates first for logs
    true // dumpLogOnFail
  )) as BanksTransactionResultWithMeta;

  // Transaction succeeded
  const cuUsed = result.meta?.computeUnitsConsumed
    ? Number(result.meta.computeUnitsConsumed)
    : undefined;

  return { success: true, cuUsed };
}

async function addGenericRegularBank(
  throwawayGroup: Keypair,
  seed: BN,
  index: number,
  label?: string,
  useTokenA: boolean = false
) {
  const config = defaultBankConfig();
  config.assetWeightInit = bigNumberToWrappedI80F48(0.5);
  config.assetWeightMaint = bigNumberToWrappedI80F48(0.6);
  config.depositLimit = new BN(100_000_000_000_000);
  config.borrowLimit = new BN(100_000_000_000_000);
  config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
  config.interestRateConfig.points = makeRatePoints([0.8], [0.2]);

  const bankMint = useTokenA
    ? ecosystem.tokenAMint.publicKey
    : ecosystem.lstAlphaMint.publicKey;
  const oracleKey = useTokenA
    ? oracles.tokenAOracle.publicKey
    : oracles.pythPullLst.publicKey;

  const [bankKey] = deriveBankWithSeed(
    bankrunProgram.programId,
    throwawayGroup.publicKey,
    bankMint,
    seed
  );

  const oracleMeta = {
    pubkey: oracleKey,
    isSigner: false,
    isWritable: false,
  };

  const config_ix = await groupAdmin.mrgnProgram.methods
    .lendingPoolConfigureBankOracle(ORACLE_SETUP_PYTH_PUSH, oracleKey)
    .accountsPartial({
      group: throwawayGroup.publicKey,
      bank: bankKey,
      admin: groupAdmin.wallet.publicKey,
    })
    .remainingAccounts([oracleMeta])
    .instruction();

  const addBankIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
    marginfiGroup: throwawayGroup.publicKey,
    feePayer: groupAdmin.wallet.publicKey,
    bankMint: bankMint,
    config,
    seed,
  });

  const tx = new Transaction().add(addBankIx, config_ix);
  await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);
}

async function createAndExtendLUT(
  ctx: ProgramTestContext,
  user: MockUser,
  group: PublicKey,
  liquidatorAccount: PublicKey,
  driftBanks: PublicKey[],
  regularBanks: PublicKey[],
  liabBank: PublicKey,
  driftSpotMarket: PublicKey
): Promise<PublicKey> {
  const recentSlot = Number(await banksClient.getSlot());
  const [createLutIx, lutAddress] = AddressLookupTableProgram.createLookupTable(
    {
      authority: user.wallet.publicKey,
      payer: user.wallet.publicKey,
      recentSlot: recentSlot - 1,
    }
  );

  const createLutTx = new Transaction().add(createLutIx);
  await processBankrunTx(ctx, createLutTx, [user.wallet]);

  const { TOKEN_PROGRAM_ID } = await import("@solana/spl-token");

  const allAddresses = [
    group,
    bankrunProgram.programId,
    liquidatorAccount,
    TOKEN_PROGRAM_ID,
    ...driftBanks,
    ...regularBanks,
    liabBank,
    driftSpotMarket,
    oracles.tokenAOracle.publicKey,
    oracles.pythPullLst.publicKey,
  ];

  const chunkSize = 20;
  for (let i = 0; i < allAddresses.length; i += chunkSize) {
    const chunk = allAddresses.slice(i, i + chunkSize);
    const extendLutTx = new Transaction().add(
      AddressLookupTableProgram.extendLookupTable({
        authority: user.wallet.publicKey,
        payer: user.wallet.publicKey,
        lookupTable: lutAddress,
        addresses: chunk,
      })
    );
    await processBankrunTx(ctx, extendLutTx, [user.wallet]);
  }

  return lutAddress;
}
