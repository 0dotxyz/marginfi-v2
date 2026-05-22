import { Program } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createMintToInstruction, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import Decimal from "decimal.js";
import { Farms } from "../fixtures/kamino_farms";
import farmsIdl from "../../idls-complete/kamino_farms.json";
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
  USDC_RESERVE,
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
import {
  LENDING_MARKET_SIZE,
  RESERVE_SIZE,
  simpleRefreshReserve,
  toWeb3Ix,
} from "./kamino-utils";
import {
  deriveDriftStatePDA,
  deriveFeeReceiver,
  deriveLendingMarketAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
  deriveSpotMarketPDA,
} from "./pdas";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
} from "./tools";
import { DRIFT_ORACLE_RECEIVER_PROGRAM_ID } from "./types";
import { address } from "@solana/addresses";
import { createNoopSigner } from "@solana/kit";
import {
  AssetReserveConfig,
  BorrowRateCurve,
  BorrowRateCurveFields,
  CurvePoint,
  LendingMarket,
  MarketWithAddress,
  PriceFeed,
  Reserve,
  parseForChangesReserveConfigAndGetIxs,
} from "@kamino-finance/klend-sdk";

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

const createReserve = async (params: {
  market: PublicKey;
  mint: PublicKey;
  decimals: number;
  oracle: PublicKey;
  liquiditySource: PublicKey;
  reserveLabel: string;
}): Promise<PublicKey> => {
  const { market, mint, decimals, oracle, liquiditySource, reserveLabel } =
    params;
  const reserve = Keypair.generate();
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    klendBankrunProgram.programId,
    market,
  );
  const [feeReceiver] = deriveFeeReceiver(
    klendBankrunProgram.programId,
    reserve.publicKey,
  );
  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    klendBankrunProgram.programId,
    reserve.publicKey,
  );
  const [reserveCollateralMint] = deriveReserveCollateralMint(
    klendBankrunProgram.programId,
    reserve.publicKey,
  );
  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    klendBankrunProgram.programId,
    reserve.publicKey,
  );

  const initTx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: reserve.publicKey,
      space: RESERVE_SIZE + 8,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          RESERVE_SIZE + 8,
        ),
      programId: klendBankrunProgram.programId,
    }),
    await klendBankrunProgram.methods
      .initReserve()
      .accountsStrict({
        signer: groupAdmin.wallet.publicKey,
        lendingMarket: market,
        lendingMarketAuthority,
        reserve: reserve.publicKey,
        reserveLiquidityMint: mint,
        reserveLiquiditySupply,
        feeReceiver,
        reserveCollateralMint,
        reserveCollateralSupply,
        initialLiquiditySource: liquiditySource,
        rent: SYSVAR_RENT_PUBKEY,
        liquidityTokenProgram: TOKEN_PROGRAM_ID,
        collateralTokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
  );
  await processBankrunTransaction(bankrunContext, initTx, [
    groupAdmin.wallet,
    reserve,
  ]);

  const marketAcc: LendingMarket = LendingMarket.decode(
    (await bankRunProvider.connection.getAccountInfo(market))!.data,
  );
  const reserveAcc: Reserve = Reserve.decode(
    (await bankRunProvider.connection.getAccountInfo(reserve.publicKey))!.data,
  );

  const marketWithAddress: MarketWithAddress = {
    address: address(market.toString()),
    state: marketAcc,
  };
  const borrowRateCurve = new BorrowRateCurve({
    points: [
      new CurvePoint({ utilizationRateBps: 0, borrowRateBps: 50000 }),
      new CurvePoint({ utilizationRateBps: 5000, borrowRateBps: 100000 }),
      new CurvePoint({ utilizationRateBps: 8000, borrowRateBps: 500000 }),
      new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
      ...Array(7).fill(
        new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
      ),
    ],
  } as BorrowRateCurveFields);
  const priceFeed: PriceFeed = {
    pythPrice: address(oracle.toString()),
  };
  const assetReserveConfig = new AssetReserveConfig({
    mint: address(mint.toString()),
    mintTokenProgram: address(TOKEN_PROGRAM_ID.toString()),
    tokenName: reserveLabel,
    mintDecimals: decimals,
    priceFeed,
    loanToValuePct: 75,
    liquidationThresholdPct: 85,
    borrowRateCurve,
    depositLimit: new Decimal(1_000_000_000),
    borrowLimit: new Decimal(1_000_000_000),
  }).getReserveConfig();

  const signer = createNoopSigner(address(groupAdmin.wallet.publicKey.toString()));
  const instructions: TransactionInstruction[] = [
    ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
  ];
  const ixes = await parseForChangesReserveConfigAndGetIxs(
    marketWithAddress,
    reserveAcc,
    address(reserve.publicKey.toString()),
    assetReserveConfig,
    address(klendBankrunProgram.programId.toString()),
    signer,
  );
  for (const ix of ixes) {
    instructions.push(toWeb3Ix(ix.ix as any));
  }

  const lutAccount = await createLookupTableForInstructions(
    groupAdmin.wallet,
    instructions,
  );
  const messageV0 = new TransactionMessage({
    payerKey: groupAdmin.wallet.publicKey,
    recentBlockhash: await getBankrunBlockhash(bankrunContext),
    instructions,
  }).compileToV0Message([lutAccount]);
  const versionedTx = new VersionedTransaction(messageV0);
  versionedTx.sign([groupAdmin.wallet]);
  await banksClient.processTransaction(versionedTx);

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        reserve.publicKey,
        market,
        oracle,
      ),
    ),
    [groupAdmin.wallet],
  );

  return reserve.publicKey;
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

  let usdcReserve = kaminoAccounts.get(USDC_RESERVE);
  if (!(await hasAccount(usdcReserve))) {
    const mintUsdcTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.usdcDecimals,
      ),
    );
    await processBankrunTransaction(bankrunContext, mintUsdcTx, [
      globalProgramAdmin.wallet,
    ]);

    usdcReserve = await createReserve({
      market,
      mint: ecosystem.usdcMint.publicKey,
      decimals: ecosystem.usdcDecimals,
      oracle: oracles.usdcOracle.publicKey,
      liquiditySource: groupAdmin.usdcAccount,
      reserveLabel: USDC_RESERVE,
    });
    kaminoAccounts.set(USDC_RESERVE, usdcReserve);
  }

  let tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  if (!(await hasAccount(tokenAReserve))) {
    const mintTokenATx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.tokenADecimals,
      ),
    );
    await processBankrunTransaction(bankrunContext, mintTokenATx, [
      globalProgramAdmin.wallet,
    ]);

    tokenAReserve = await createReserve({
      market,
      mint: ecosystem.tokenAMint.publicKey,
      decimals: ecosystem.tokenADecimals,
      oracle: oracles.tokenAOracle.publicKey,
      liquiditySource: groupAdmin.tokenAAccount,
      reserveLabel: TOKEN_A_RESERVE,
    });
    kaminoAccounts.set(TOKEN_A_RESERVE, tokenAReserve);
  }

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
