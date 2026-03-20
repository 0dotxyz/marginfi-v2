import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { Farms } from "../fixtures/kamino_farms";
import farmsIdl from "../../idls/kamino_farms.json";
import {
  A_FARM_STATE,
  A_FARM_VAULTS_AUTHORITY,
  bankrunContext,
  bankRunProvider,
  banksClient,
  DRIFT_TOKEN_A_PULL_FEED,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  DRIFT_USDC_SPOT_MARKET,
  driftAccounts,
  driftBankrunProgram,
  ecosystem,
  farmAccounts,
  FARMS_PROGRAM_ID,
  globalProgramAdmin,
  GLOBAL_CONFIG,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
} from "../rootHooks";
import { createBankrunPythOracleAccount } from "./bankrun-oracles";
import {
  defaultSpotMarketConfig,
  DriftOracleSourceValues,
  quoteAssetSpotMarketConfig,
  refreshDriftOracles,
  TOKEN_A_MARKET_INDEX,
  USDC_MARKET_INDEX,
} from "./drift-utils";
import { makeInitializeDriftIx, makeInitializeSpotMarketIx } from "./drift-sdk";
import { LENDING_MARKET_SIZE } from "./kamino-utils";
import {
  deriveDriftStatePDA,
  deriveLendingMarketAuthority,
  deriveSpotMarketPDA,
} from "./pdas";
import { processBankrunTransaction } from "./tools";
import { DRIFT_ORACLE_RECEIVER_PROGRAM_ID } from "./types";
import { createReserve } from "./kamino-instructions";

const FARMS_GLOBAL_CONFIG_SIZE = 2136;
const FARMS_STATE_SIZE = 8336;

let setupPromise: Promise<void> | null = null;

const hasAccount = async (pubkey: PublicKey | null | undefined) => {
  if (!pubkey) {
    return false;
  }
  const account = await banksClient.getAccount(pubkey);
  return account !== null;
};

const createKaminoMarket = async (): Promise<PublicKey> => {
  const usdcString = "USDC";
  const quoteCurrency = Array.from(usdcString.padEnd(32, "\0")).map((c) =>
    c.charCodeAt(0),
  );

  const lendingMarket = Keypair.generate();
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    klendBankrunProgram.programId,
    lendingMarket.publicKey,
  );

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: lendingMarket.publicKey,
      space: LENDING_MARKET_SIZE + 8,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          LENDING_MARKET_SIZE + 8,
        ),
      programId: klendBankrunProgram.programId,
    }),
    await klendBankrunProgram.methods
      .initLendingMarket(quoteCurrency)
      .accounts({
        lendingMarketOwner: groupAdmin.wallet.publicKey,
        lendingMarket: lendingMarket.publicKey,
        lendingMarketAuthority,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .instruction(),
  );

  await processBankrunTransaction(bankrunContext, tx, [
    groupAdmin.wallet,
    lendingMarket,
  ]);

  return lendingMarket.publicKey;
};

/**
 * Machine generated copy of generic Drift ecosystem setup. When running e.g. m* tests that rely on
 * d* tests, this will bootstrap the drift setup so those tests can run independently.
 */
const ensureDriftSetup = async () => {
  const [driftStatePk] = deriveDriftStatePDA(driftBankrunProgram.programId);
  if (!(await hasAccount(driftStatePk))) {
    const initIx = await makeInitializeDriftIx(driftBankrunProgram, {
      admin: groupAdmin.wallet.publicKey,
      usdcMint: ecosystem.usdcMint.publicKey,
    });
    const tx = new Transaction().add(initIx);
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  }

  const [usdcSpotMarketPk] = deriveSpotMarketPDA(
    driftBankrunProgram.programId,
    USDC_MARKET_INDEX,
  );
  if (!(await hasAccount(usdcSpotMarketPk))) {
    const config = quoteAssetSpotMarketConfig();
    const initUsdcIx = await makeInitializeSpotMarketIx(
      driftBankrunProgram,
      {
        admin: groupAdmin.wallet.publicKey,
        spotMarketMint: ecosystem.usdcMint.publicKey,
        oracle: PublicKey.default,
      },
      {
        optimalUtilization: config.optimalUtilization,
        optimalRate: config.optimalRate,
        maxRate: config.maxRate,
        oracleSource: DriftOracleSourceValues.quoteAsset,
        initialAssetWeight: config.initialAssetWeight,
        maintenanceAssetWeight: config.maintenanceAssetWeight,
        initialLiabilityWeight: config.initialLiabilityWeight,
        maintenanceLiabilityWeight: config.maintenanceLiabilityWeight,
        marketIndex: USDC_MARKET_INDEX,
      },
    );

    const tx = new Transaction().add(initUsdcIx);
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  }
  driftAccounts.set(DRIFT_USDC_SPOT_MARKET, usdcSpotMarketPk);

  const [tokenASpotMarketPk] = deriveSpotMarketPDA(
    driftBankrunProgram.programId,
    TOKEN_A_MARKET_INDEX,
  );

  if (!(await hasAccount(tokenASpotMarketPk))) {
    let tokenAOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);
    if (!(await hasAccount(tokenAOracle))) {
      const tokenAOracleKp = Keypair.generate();
      await createBankrunPythOracleAccount(
        bankrunContext,
        banksClient,
        tokenAOracleKp,
        DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
      );
      tokenAOracle = tokenAOracleKp.publicKey;
      driftAccounts.set(DRIFT_TOKEN_A_PULL_ORACLE, tokenAOracle);
    }

    if (!driftAccounts.get(DRIFT_TOKEN_A_PULL_FEED)) {
      driftAccounts.set(DRIFT_TOKEN_A_PULL_FEED, Keypair.generate().publicKey);
    }

    await refreshDriftOracles(
      oracles,
      driftAccounts,
      bankrunContext,
      banksClient,
    );

    const config = defaultSpotMarketConfig();
    const initTokenAIx = await makeInitializeSpotMarketIx(
      driftBankrunProgram,
      {
        admin: groupAdmin.wallet.publicKey,
        spotMarketMint: ecosystem.tokenAMint.publicKey,
        oracle: tokenAOracle,
      },
      {
        optimalUtilization: config.optimalUtilization,
        optimalRate: config.optimalRate,
        maxRate: config.maxRate,
        oracleSource: DriftOracleSourceValues.pythPull,
        initialAssetWeight: config.initialAssetWeight,
        maintenanceAssetWeight: config.maintenanceAssetWeight,
        initialLiabilityWeight: config.initialLiabilityWeight,
        maintenanceLiabilityWeight: config.maintenanceLiabilityWeight,
        marketIndex: TOKEN_A_MARKET_INDEX,
      },
    );

    const tx = new Transaction().add(initTokenAIx);
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  } else {
    const tokenAMarket = await driftBankrunProgram.account.spotMarket.fetch(
      tokenASpotMarketPk,
    );
    if (!tokenAMarket.oracle.equals(PublicKey.default)) {
      driftAccounts.set(DRIFT_TOKEN_A_PULL_ORACLE, tokenAMarket.oracle);
    }
    if (!driftAccounts.get(DRIFT_TOKEN_A_PULL_FEED)) {
      driftAccounts.set(DRIFT_TOKEN_A_PULL_FEED, Keypair.generate().publicKey);
    }
  }

  driftAccounts.set(DRIFT_TOKEN_A_SPOT_MARKET, tokenASpotMarketPk);
  await refreshDriftOracles(
    oracles,
    driftAccounts,
    bankrunContext,
    banksClient,
  );
};

/**
 * Machine generated copy of generic Kamino ecosystem setup. When running e.g. m* tests that rely on
 * k* tests, this will bootstrap the kamino setup so those tests can run independently.
 */
const ensureKaminoSetup = async () => {
  let market = kaminoAccounts.get(MARKET);
  if (!(await hasAccount(market))) {
    market = await createKaminoMarket();
    kaminoAccounts.set(MARKET, market);
  }

  let tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  if (!(await hasAccount(tokenAReserve))) {
    const mintTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.tokenADecimals,
      ),
    );
    await processBankrunTransaction(bankrunContext, mintTx, [
      globalProgramAdmin.wallet,
    ]);

    const reserveKeypair = Keypair.generate();
    const mint = ecosystem.tokenAMint.publicKey;

    await createReserve(
      reserveKeypair,
      kaminoAccounts.get(MARKET),
      mint,
      TOKEN_A_RESERVE,
      ecosystem.tokenADecimals,
      oracles.tokenAOracle.publicKey,
      groupAdmin.tokenAAccount,
    );

    kaminoAccounts.set(TOKEN_A_RESERVE, reserveKeypair.publicKey);
  }

  tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  const reserveAcc = await klendBankrunProgram.account.reserve.fetch(
    tokenAReserve,
  );

  if (!reserveAcc.farmCollateral.equals(PublicKey.default)) {
    farmAccounts.set(A_FARM_STATE, reserveAcc.farmCollateral);
    return;
  }

  const farmsProgram = new Program<Farms>(farmsIdl as Farms, bankRunProvider);

  let globalConfig = farmAccounts.get(GLOBAL_CONFIG);
  if (!(await hasAccount(globalConfig))) {
    const globalConfigKp = Keypair.generate();
    const [treasuryVaultsAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("authority"), globalConfigKp.publicKey.toBuffer()],
      FARMS_PROGRAM_ID,
    );

    const initGlobalConfigTx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: groupAdmin.wallet.publicKey,
        newAccountPubkey: globalConfigKp.publicKey,
        space: 8 + FARMS_GLOBAL_CONFIG_SIZE,
        lamports:
          await bankRunProvider.connection.getMinimumBalanceForRentExemption(
            8 + FARMS_GLOBAL_CONFIG_SIZE,
          ),
        programId: FARMS_PROGRAM_ID,
      }),
      await farmsProgram.methods
        .initializeGlobalConfig()
        .accounts({
          globalAdmin: groupAdmin.wallet.publicKey,
          globalConfig: globalConfigKp.publicKey,
          treasuryVaultsAuthority,
          systemProgram: SystemProgram.programId,
        })
        .instruction(),
    );

    await processBankrunTransaction(bankrunContext, initGlobalConfigTx, [
      groupAdmin.wallet,
      globalConfigKp,
    ]);

    globalConfig = globalConfigKp.publicKey;
    farmAccounts.set(GLOBAL_CONFIG, globalConfig);
  }

  const farmState = Keypair.generate();
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    klendBankrunProgram.programId,
    market,
  );
  const [farmVaultsAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("authority"), farmState.publicKey.toBuffer()],
    FARMS_PROGRAM_ID,
  );

  const initFarmStateTx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: farmState.publicKey,
      space: 8 + FARMS_STATE_SIZE,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          8 + FARMS_STATE_SIZE,
        ),
      programId: FARMS_PROGRAM_ID,
    }),
    await klendBankrunProgram.methods
      .initFarmsForReserve(0)
      .accounts({
        lendingMarketOwner: groupAdmin.wallet.publicKey,
        lendingMarket: market,
        lendingMarketAuthority,
        reserve: tokenAReserve,
        farmsProgram: FARMS_PROGRAM_ID,
        farmsGlobalConfig: globalConfig,
        farmState: farmState.publicKey,
        farmsVaultAuthority: farmVaultsAuthority,
        rent: SYSVAR_RENT_PUBKEY,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
  );

  await processBankrunTransaction(bankrunContext, initFarmStateTx, [
    groupAdmin.wallet,
    farmState,
  ]);

  farmAccounts.set(A_FARM_STATE, farmState.publicKey);
  farmAccounts.set(A_FARM_VAULTS_AUTHORITY, farmVaultsAuthority);
};

const runSetup = async () => {
  await ensureDriftSetup();
  await ensureKaminoSetup();
};

/**
 * Run inside a test suite to bootstrap k* and d* setup so the suite can use Kamino/Drift.
 */
export const ensureMultiSuiteIntegrationsSetup = async () => {
  if (!setupPromise) {
    setupPromise = runSetup();
  }

  try {
    await setupPromise;
  } catch (error) {
    setupPromise = null;
    throw error;
  }
};
