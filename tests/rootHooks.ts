import * as anchor from "@coral-xyz/anchor";
import { Program, Wallet, AnchorProvider } from "@coral-xyz/anchor";
import {
  echoEcosystemInfo,
  Ecosystem,
  getGenericEcosystem,
  MockUser as MockUser,
  Oracles,
  Validator,
  createMintBankrun,
  setupTestUserBankrun,
  SetupTestUserBankrunOptions,
} from "./utils/mocks";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  SYSVAR_STAKE_HISTORY_PUBKEY,
  Transaction,
  VoteInit,
  VoteProgram,
} from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { patchBankrunConnection } from "./utils/bankrunConnection";

// ---------------------------------------------------------------------------
// Kamino farms (liquidity-incentive) program
// ---------------------------------------------------------------------------
export const FARMS_PROGRAM_ID = new PublicKey(
  "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr",
);

import { BankrunProvider, startAnchor } from "anchor-bankrun";
import { BanksClient, ProgramTestContext } from "solana-bankrun";
import type { AddedAccount, AddedProgram } from "solana-bankrun";
import {
  SINGLE_POOL_PROGRAM_ID,
  KLEND_PROGRAM_ID,
  DRIFT_PROGRAM_ID,
} from "./utils/types";

/** Marginfi program ID (from Anchor.toml) */
const MARGINFI_PROGRAM_ID = new PublicKey(
  "2jGhuVUuy3umdzByFx8sNWUAaf5vaeuDm78RDPEnhrMr",
);
/** Mocks program ID (from Anchor.toml) */
const MOCKS_PROGRAM_ID = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
);
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { initGlobalFeeState } from "./utils/group-instructions";
import { deriveGlobalFeeState } from "./utils/pdas";
import { KaminoLending } from "./fixtures/kamino_lending";
import klendIdl from "./fixtures/kamino_lending.json";
import { Drift } from "./fixtures/drift_v2";
import driftIdl from "./fixtures/drift_v2.json";
import { Marginfi } from "../target/types/marginfi";
import { Mocks } from "../target/types/mocks";
import marginfiIdl from "../target/idl/marginfi.json";
import mocksIdl from "../target/idl/mocks.json";
import { setupPythOraclesBankrun } from "./utils/bankrun-oracles";
import { processBankrunTransaction } from "./utils/tools";

import {
  findPoolAddress,
  findPoolMintAddress,
  findPoolStakeAddress,
  findPoolStakeAuthorityAddress,
  SinglePoolProgram,
} from "@solana/spl-single-pool-classic";

export const ecosystem: Ecosystem = getGenericEcosystem();
export let oracles: Oracles = undefined;
/** Show various information about accounts and tests */
export const verbose = true;
/** Show the raw buffer printout of various structs */
export const printBuffers = false;
/** The program owner is also the provider wallet */
export let globalProgramAdmin: MockUser = undefined;
/** Administers the mrgnlend group and/or stake holder accounts */
export let groupAdmin: MockUser = undefined;
/** Administers the emode group configuration */
export let emodeAdmin: MockUser = undefined;
/** Administers validator votes and withdraws */
export let validatorAdmin: MockUser = undefined;
/** Administers bankruptcy and deleveraging */
export let riskAdmin: MockUser = undefined;
export const users: MockUser[] = [];
export const numUsers = 4;

export const validators: Validator[] = [];
export const numValidators = 2;
export let globalFeeWallet: PublicKey = undefined;

/** Lamports charged when creating any pool */
export const INIT_POOL_ORIGINATION_FEE = 1000;
/** Lamports charged for receivership liquidation events */
export const LIQUIDATION_FLAT_FEE = 500;

export const PROGRAM_FEE_FIXED = 0.01;
export const PROGRAM_FEE_RATE = 0.02;
/** The most a liquidator can earn in profit from receivership liquidation events */
export const LIQUIDATION_MAX_FEE = 0.5;

// All groups and banks below need to be deterministic to ensure the same ordering of balances in
// lending accounts
/** Group used for most regular e2e tests */
const MARGINFI_GROUP_SEED = Buffer.from("MARGINFI_GROUP_SEED_000000000000");
export const marginfiGroup = Keypair.fromSeed(MARGINFI_GROUP_SEED);
/** Group used for e-mode tests */
const EMODE_GROUP_SEED = Buffer.from("EMODE_GROUP_SEED_000000000000000");
export const emodeGroup = Keypair.fromSeed(EMODE_GROUP_SEED);
/** Group used for kamino tests */
const KAMINO_GROUP_SEED = Buffer.from("KAMINO_GROUP_SEED_00000000000000");
export const kaminoGroup = Keypair.fromSeed(KAMINO_GROUP_SEED);
/** Group used for drift tests */
const DRIFT_GROUP_SEED = Buffer.from("DRIFT_GROUP_SEED_000000000000000");
export const driftGroup = Keypair.fromSeed(DRIFT_GROUP_SEED);
/** Group used for solend tests */
const SOLEND_GROUP_SEED = Buffer.from("SOLEND_GROUP_SEED_00000000000000");
export const solendGroup = Keypair.fromSeed(SOLEND_GROUP_SEED);

/** Bank for USDC */
const USDC_SEED = Buffer.from("USDC_BANK_SEED_00000000000000000");
export const bankKeypairUsdc = Keypair.fromSeed(USDC_SEED);
/** Bank for token A */
const TOKEN_A_SEED = Buffer.from("TOKEN_A_BANK_SEED_00000000000000");
export const bankKeypairA = Keypair.fromSeed(TOKEN_A_SEED);
/** Bank for "WSOL", which is treated the same as SOL */
const SOL_SEED = Buffer.from("SOL_BANK_SEED_000000000000000000");
export const bankKeypairSol = Keypair.fromSeed(SOL_SEED);

/** Group used for staked collateral tests (separate from marginfiGroup to avoid collision) */
const STAKED_GROUP_SEED = Buffer.from("STAKED_GROUP_SEED_00000000000000");
export const stakedMarginfiGroup = Keypair.fromSeed(STAKED_GROUP_SEED);
/** Bank for USDC in staked tests */
const STAKED_USDC_SEED = Buffer.from("STAKED_USDC_BANK_SEED_0000000000");
export const stakedBankKeypairUsdc = Keypair.fromSeed(STAKED_USDC_SEED);
/** Bank for SOL in staked tests */
const STAKED_SOL_SEED = Buffer.from("STAKED_SOL_BANK_SEED_00000000000");
export const stakedBankKeypairSol = Keypair.fromSeed(STAKED_SOL_SEED);

/** Multibank group created for liquidation test k10 that's recycled for time saving purposes where
 * applicable. */
export const THROWAWAY_GROUP_SEED_K10 = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000010",
);

/** Multibank group created for drift liquidation test d09 */
export const THROWAWAY_GROUP_SEED_D09 = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000019",
);

export let bankrunContext: ProgramTestContext;
export let bankRunProvider: BankrunProvider;
export let bankrunProgram: Program<Marginfi>;
export let mocksBankrunProgram: Program<Mocks>;
export let klendBankrunProgram: Program<KaminoLending>;
export let driftBankrunProgram: Program<Drift>;
export let banksClient: BanksClient;
/** A mainnet Pyth pull feed (Jup's Sol feed) */
export const PYTH_ORACLE_FEED_SAMPLE = new PublicKey(
  "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE",
);
/** A mainnet Pyth pull oracle (Jup's Sol feed) */
export const PYTH_ORACLE_SAMPLE = new PublicKey(
  "H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG",
);
/** An account with gaps */
export const GAPPY3_SAMPLE = new PublicKey(
  "7qoe1Xmd3WUfPFHQaMYMGwSJT2mU55t3d4C4ZXZ1GJmn",
);
/** An account with gaps */
export const GAPPY4_SAMPLE = new PublicKey(
  "6pbRghQuRw9AsPJqhrGLFRVYDcvfXeGh4zNdYMt8mods",
);
/** The production BONK bank, with owner artificially swapped for the localnet program. */
export const LEGACY_BANK_SAMPLE = new PublicKey(
  "DeyH7QxWvnbbaVB4zFrf4hoq7Q8z1ZT14co42BGwGtfM",
);
/** The production group (LEGACY_BANK_SAMPLE's group) */
export const MAINNET_GROUP = new PublicKey(
  "4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8",
);

/** Banks in the emode test suite use this seed */
export const EMODE_SEED = 44;
// Adjusted to avoid hitting exact leverage limits due to floating point precision
// SOL_TO_LST: init ~9.5x, maint ~19x (vs limits of 15x/20x)
export const EMODE_INIT_RATE_SOL_TO_LST = 0.8947; // ~9.5x leverage
export const EMODE_MAINT_RATE_SOL_TO_LST = 0.9474; // ~19x leverage
// LST_TO_LST: init ~5x, maint ~6.7x (well under limits)
export const EMODE_INIT_RATE_LST_TO_LST = 0.8;
export const EMODE_MAINT_RATE_LST_TO_LST = 0.85;

export let kaminoAccounts: Map<string, PublicKey>;
/** Kamino Market */
export const MARKET = "market";
/** Kamino USDC Reserve */
export const USDC_RESERVE = "usdc_reserve";
/** Kamino Token A Reserve */
export const TOKEN_A_RESERVE = "token_a_reserve";
/** mrgn USDC bank trading on `USDC_RESERVE` (the reserve for ecosystem.usdcMint) */
export const KAMINO_USDC_BANK = "kamino_usdc_bank";
/** mrgn Token A bank trading on `TOKEN_A_RESERVE` (the reserve for ecosystem.tokenAMint) */
export const KAMINO_TOKEN_A_BANK = "kamino_tokenA_bank";

// TODO: This should really be an object with defined fields not a dict
export let driftAccounts: Map<string, PublicKey>;
/** Drift USDC Spot Market */
export const DRIFT_USDC_SPOT_MARKET = "drift_usdc_spot_market";
/** Drift Token A Spot Market */
export const DRIFT_TOKEN_A_SPOT_MARKET = "drift_tokenA_spot_market";
/** mrgn USDC bank trading on `DRIFT_USDC_SPOT_MARKET` */
export const DRIFT_USDC_BANK = "drift_usdc_bank";
/** mrgn Token A bank trading on `DRIFT_TOKEN_A_SPOT_MARKET` */
export const DRIFT_TOKEN_A_BANK = "drift_tokenA_bank";
/** Drift Token A Pyth Pull Oracle (with mainnet owner) */
export const DRIFT_TOKEN_A_PULL_ORACLE = "drift_tokenA_pull_oracle";
/** Drift Token A Pyth Pull Feed */
export const DRIFT_TOKEN_A_PULL_FEED = "drift_tokenA_pull_feed";
export const DRIFT_TOKEN_B_SPOT_MARKET = "drift_token_b_spot_market";
export const DRIFT_TOKEN_B_PULL_ORACLE = "drift_token_b_pull_oracle";
export const DRIFT_TOKEN_B_PULL_FEED = "drift_token_b_pull_feed";

// Solend related accounts
export let solendAccounts: Map<string, PublicKey>;
/** Solend Lending Market */
export const SOLEND_MARKET = "solend_market";
/** Solend Market Authority (PDA) */
export const SOLEND_MARKET_AUTHORITY = "solend_market_authority";

// Reserve accounts
/** Solend USDC Reserve */
export const SOLEND_USDC_RESERVE = "solend_usdc_reserve";
/** Solend Token A Reserve */
export const SOLEND_TOKEN_A_RESERVE = "solend_tokena_reserve";

// Reserve component accounts (critical for operations)
/** USDC Reserve liquidity supply */
export const SOLEND_USDC_LIQUIDITY_SUPPLY = "solend_usdc_liquidity_supply";
/** USDC Reserve collateral mint */
export const SOLEND_USDC_COLLATERAL_MINT = "solend_usdc_collateral_mint";
/** USDC Reserve collateral supply */
export const SOLEND_USDC_COLLATERAL_SUPPLY = "solend_usdc_collateral_supply";
/** USDC Reserve fee receiver */
export const SOLEND_USDC_FEE_RECEIVER = "solend_usdc_fee_receiver";

/** Token A Reserve liquidity supply */
export const SOLEND_TOKEN_A_LIQUIDITY_SUPPLY = "solend_tokena_liquidity_supply";
/** Token A Reserve collateral mint */
export const SOLEND_TOKEN_A_COLLATERAL_MINT = "solend_tokena_collateral_mint";
/** Token A Reserve collateral supply */
export const SOLEND_TOKENA_COLLATERAL_SUPPLY =
  "solend_tokena_collateral_supply";
/** Token A Reserve fee receiver */
export const SOLEND_TOKEN_A_FEE_RECEIVER = "solend_tokena_fee_receiver";

// Bank accounts (for MarginFi integration)
/** mrgn USDC bank for Solend integration */
export const SOLEND_USDC_BANK = "solend_usdc_bank";
/** mrgn Token A bank for Solend integration */
export const SOLEND_TOKEN_A_BANK = "solend_tokena_bank";

// Kamino farms related accounts
export const farmAccounts = new Map<string, PublicKey>();
export const GLOBAL_CONFIG = "GLOBAL_CONFIG";
export const A_FARM_STATE = "FARM_STATE_TOKEN_A";
export const A_OBLIGATION_USER_STATE = "USER_STATE_TOKEN_A";
export const A_REWARD_MINT = "REWARD_MINT";
export const A_REWARD_VAULT = "REWARD_VAULT";
export const A_REWARD_TREASURY_VAULT = "REWARD_TREASURY_VAULT";
export const A_FARM_VAULTS_AUTHORITY = "FARM_VAULTS_AUTHORITY";
export const A_TREASURY_VAULTS_AUTHORITY = "TREASURY_VAULTS_AUTHORITY";

// ---------------------------------------------------------------------------
// Staked collateral helpers (Vote accounts + SPL single pools)
// ---------------------------------------------------------------------------

/**
 * Create a vote account for a validator inside bankrun.
 *
 * This is only required for the staked-collateral test suite (s01-s10).
 */
async function createValidatorBankrun(index: number): Promise<Validator> {
  const voteAccount = Keypair.generate();
  const node = Keypair.generate();
  const authorized = validatorAdmin.wallet.publicKey;

  const rentForVote =
    await bankRunProvider.connection.getMinimumBalanceForRentExemption(
      VoteProgram.space,
    );

  const voteInit = new VoteInit(node.publicKey, authorized, authorized, 0);
  // VoteProgram.initializeAccount returns a TransactionInstruction directly (not a Transaction)
  const initIx = VoteProgram.initializeAccount({
    votePubkey: voteAccount.publicKey,
    nodePubkey: node.publicKey,
    voteInit,
  });

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: authorized,
      newAccountPubkey: voteAccount.publicKey,
      lamports: rentForVote,
      space: VoteProgram.space,
      programId: VoteProgram.programId,
    }),
    initIx,
  );

  await processBankrunTransaction(bankrunContext, tx, [
    validatorAdmin.wallet,
    voteAccount,
    node,
  ]);

  if (verbose) {
    console.log(
      `*init validator ${index}: vote=${voteAccount.publicKey.toBase58()}`,
    );
  }

  return {
    node: node.publicKey,
    authorizedVoter: authorized,
    authorizedWithdrawer: authorized,
    voteAccount: voteAccount.publicKey,
    // Filled by createSplStakePoolBankrun
    splPool: PublicKey.default,
    splMint: PublicKey.default,
    splAuthority: PublicKey.default,
    splSolPool: PublicKey.default,
    // Filled by staked tests after permissionless add-bank
    bank: PublicKey.default,
  };
}

/**
 * Initialize a SPL single pool for a given validator vote account.
 */
async function createSplStakePoolBankrun(
  validator: Validator,
): Promise<Validator> {
  // SinglePoolProgram.initialize returns a ready-to-send Transaction.
  const payer = users[0].wallet;
  const initTx = await SinglePoolProgram.initialize(
    bankRunProvider.connection,
    validator.voteAccount,
    payer.publicKey,
    true,
  );
  await processBankrunTransaction(bankrunContext, initTx, [payer]);

  // Derive pool PDA keys (these return PublicKey directly, not [PublicKey, bump])
  const poolKey = await findPoolAddress(
    SINGLE_POOL_PROGRAM_ID,
    validator.voteAccount,
  );
  const poolMintKey = await findPoolMintAddress(
    SINGLE_POOL_PROGRAM_ID,
    poolKey,
  );
  const poolAuthority = await findPoolStakeAuthorityAddress(
    SINGLE_POOL_PROGRAM_ID,
    poolKey,
  );
  const poolStake = await findPoolStakeAddress(SINGLE_POOL_PROGRAM_ID, poolKey);

  if (verbose) {
    console.log(
      `*init single-pool: pool=${poolKey.toBase58()} mint=${poolMintKey.toBase58()}`,
    );
  }

  return {
    ...validator,
    splPool: poolKey,
    splMint: poolMintKey,
    splAuthority: poolAuthority,
    splSolPool: poolStake,
  };
}

/**
 * Load a JSON fixture file as an AddedAccount for startAnchor genesis.
 */
function loadJsonFixture(filepath: string): AddedAccount {
  const fullPath = path.resolve(__dirname, "..", filepath);
  const json = JSON.parse(fs.readFileSync(fullPath, "utf8"));
  return {
    address: new PublicKey(json.pubkey),
    info: {
      lamports: Number(json.account.lamports),
      owner: new PublicKey(json.account.owner),
      executable: json.account.executable,
      rentEpoch: Number(json.account.rentEpoch ?? 0),
      data: Buffer.from(json.account.data[0], json.account.data[1]),
    },
  };
}

/**
 * Extra programs to load in bankrun (external .so files)
 */
const extraPrograms: AddedProgram[] = [
  { name: "mocks", programId: MOCKS_PROGRAM_ID },
  {
    name: "kamino_lending",
    programId: new PublicKey("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"),
  },
  {
    name: "kamino_farms",
    programId: new PublicKey("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"),
  },
  {
    name: "spl_single_pool",
    programId: new PublicKey("SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE"),
  },
  {
    name: "drift_v2",
    programId: new PublicKey("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"),
  },
  {
    name: "solend",
    programId: new PublicKey("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"),
  },
];

/**
 * JSON fixtures to load as genesis accounts
 */
function getGenesisAccounts(): AddedAccount[] {
  return [
    loadJsonFixture("tests/fixtures/bonk_bank.json"),
    loadJsonFixture("tests/fixtures/cloud_bank.json"),
    loadJsonFixture("tests/fixtures/pyusd_bank.json"),
    loadJsonFixture("tests/fixtures/localnet_usdc.json"),
    loadJsonFixture("tests/fixtures/gappy_user3.json"),
    loadJsonFixture("tests/fixtures/gappy_user4.json"),
    loadJsonFixture("tests/fixtures/mainnet_group.json"),
    loadJsonFixture("tests/fixtures/sol_pyth_oracle.json"),
    loadJsonFixture("tests/fixtures/sol_pyth_price_feed.json"),
  ];
}

// ---------------------------------------------------------------------------
// Mocha Hooks - Pure Bankrun Setup
// ---------------------------------------------------------------------------

export const mochaHooks = {
  beforeAll: async () => {
    // If false, you are in the wrong environment to run this, update Node or try polyfill
    console.log("Environment supports crypto: ", !!global.crypto?.subtle);

    kaminoAccounts = new Map<string, PublicKey>();
    driftAccounts = new Map<string, PublicKey>();
    solendAccounts = new Map<string, PublicKey>();

    if (verbose) {
      console.log("Global Ecosystem Information ");
      echoEcosystemInfo(ecosystem, {
        skipA: false,
        skipB: false,
        skipUsdc: false,
        skipWsol: false,
      });
      console.log("");
    }

    // -------------------------------------------------------------------------
    // Step 1: Start bankrun FIRST with external programs and fixture accounts
    // -------------------------------------------------------------------------
    console.log("Starting bankrun with pure bankrun setup...");

    const genesisAccounts = getGenesisAccounts();

    bankrunContext = await startAnchor(
      path.resolve(__dirname, ".."),
      extraPrograms,
      genesisAccounts,
    );
    bankRunProvider = new BankrunProvider(bankrunContext);
    banksClient = bankrunContext.banksClient;

    // Patch missing connection methods that tests need
    patchBankrunConnection(bankRunProvider.connection, banksClient);

    // Set the global Anchor provider so getProvider() works
    // This is critical for tests that use anchor.getProvider() or program.provider
    const anchorProvider = new AnchorProvider(
      bankRunProvider.connection,
      new Wallet(bankrunContext.payer),
      {},
    );
    anchor.setProvider(anchorProvider);

    // Factory to create AnchorProvider for any wallet, reusing the patched connection
    const makeProvider = (keypair: Keypair) =>
      new AnchorProvider(bankRunProvider.connection, new Wallet(keypair), {});

    // Create bankrun programs using directly loaded IDLs with explicit program IDs
    // Set address in IDL since Anchor 0.31 requires it
    const marginfiIdlWithAddress = {
      ...marginfiIdl,
      address: MARGINFI_PROGRAM_ID.toBase58(),
    };
    const mocksIdlWithAddress = {
      ...mocksIdl,
      address: MOCKS_PROGRAM_ID.toBase58(),
    };
    const klendIdlWithAddress = {
      ...klendIdl,
      address: KLEND_PROGRAM_ID.toBase58(),
    };
    const driftIdlWithAddress = {
      ...driftIdl,
      address: DRIFT_PROGRAM_ID.toBase58(),
    };

    bankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      anchorProvider,
    );
    mocksBankrunProgram = new Program<Mocks>(
      mocksIdlWithAddress as Mocks,
      anchorProvider,
    );
    klendBankrunProgram = new Program<KaminoLending>(
      klendIdlWithAddress as KaminoLending,
      anchorProvider,
    );
    driftBankrunProgram = new Program<Drift>(
      driftIdlWithAddress as Drift,
      anchorProvider,
    );

    const payer = bankrunContext.payer;

    // -------------------------------------------------------------------------
    // Step 2: Create mints via bankrun transactions
    // -------------------------------------------------------------------------
    console.log("Creating mints in bankrun...");

    await createMintBankrun(
      bankrunContext,
      payer,
      ecosystem.wsolDecimals,
      ecosystem.wsolMint,
    );
    await createMintBankrun(
      bankrunContext,
      payer,
      ecosystem.usdcDecimals,
      ecosystem.usdcMint,
    );
    await createMintBankrun(
      bankrunContext,
      payer,
      ecosystem.tokenADecimals,
      ecosystem.tokenAMint,
    );
    await createMintBankrun(
      bankrunContext,
      payer,
      ecosystem.tokenBDecimals,
      ecosystem.tokenBMint,
    );
    await createMintBankrun(
      bankrunContext,
      payer,
      ecosystem.lstAlphaDecimals,
      ecosystem.lstAlphaMint,
    );

    // -------------------------------------------------------------------------
    // Step 3: Init global fee state via bankrun transaction
    // -------------------------------------------------------------------------
    console.log("Initializing global fee state...");

    const globalFeeKeypair = Keypair.generate();
    globalFeeWallet = globalFeeKeypair.publicKey;

    const miscSetupTx = new Transaction();
    miscSetupTx.feePayer = payer.publicKey;
    // Send some sol to the global fee wallet for rent
    miscSetupTx.add(
      SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: globalFeeWallet,
        lamports: 10 * LAMPORTS_PER_SOL,
      }),
    );
    // Init the global fee state
    miscSetupTx.add(
      await initGlobalFeeState(bankrunProgram, {
        payer: payer.publicKey,
        admin: payer.publicKey,
        wallet: globalFeeWallet,
        bankInitFlatSolFee: INIT_POOL_ORIGINATION_FEE,
        liquidationFlatSolFee: LIQUIDATION_FLAT_FEE,
        programFeeFixed: bigNumberToWrappedI80F48(PROGRAM_FEE_FIXED),
        programFeeRate: bigNumberToWrappedI80F48(PROGRAM_FEE_RATE),
        liquidationMaxFee: bigNumberToWrappedI80F48(LIQUIDATION_MAX_FEE),
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      miscSetupTx,
      [payer],
      false,
      true,
    );

    // -------------------------------------------------------------------------
    // Step 4: Create users via bankrun transactions
    // -------------------------------------------------------------------------
    console.log("Creating test users in bankrun...");

    const setupUserOptions: SetupTestUserBankrunOptions = {
      wsolMint: ecosystem.wsolMint.publicKey,
      tokenAMint: ecosystem.tokenAMint.publicKey,
      tokenBMint: ecosystem.tokenBMint.publicKey,
      usdcMint: ecosystem.usdcMint.publicKey,
      lstAlphaMint: ecosystem.lstAlphaMint.publicKey,
    };

    groupAdmin = await setupTestUserBankrun(
      bankrunContext,
      payer,
      setupUserOptions,
    );
    emodeAdmin = await setupTestUserBankrun(
      bankrunContext,
      payer,
      setupUserOptions,
    );
    validatorAdmin = await setupTestUserBankrun(
      bankrunContext,
      payer,
      setupUserOptions,
    );
    riskAdmin = await setupTestUserBankrun(
      bankrunContext,
      payer,
      setupUserOptions,
    );

    for (let i = 0; i < numUsers; i++) {
      const user = await setupTestUserBankrun(
        bankrunContext,
        payer,
        setupUserOptions,
      );
      users.push(user);
    }

    // Global admin uses the payer wallet...
    globalProgramAdmin = await setupTestUserBankrun(bankrunContext, payer, {
      ...setupUserOptions,
      forceWallet: payer,
    });

    // -------------------------------------------------------------------------
    // Step 5: Create oracles via bankrun transactions
    // -------------------------------------------------------------------------
    console.log("Creating oracles in bankrun...");

    oracles = await setupPythOraclesBankrun(
      bankrunContext,
      banksClient,
      ecosystem.wsolPrice,
      ecosystem.wsolDecimals,
      ecosystem.usdcPrice,
      ecosystem.usdcDecimals,
      ecosystem.tokenAPrice,
      ecosystem.tokenADecimals,
      ecosystem.tokenBPrice,
      ecosystem.tokenBDecimals,
      ecosystem.lstAlphaPrice,
      ecosystem.lstAlphaDecimals,
      verbose,
    );

    // ---------------------------------------------------------------------
    // Step 5b: Create validators + SPL single pools (staked collateral tests)
    // ---------------------------------------------------------------------
    console.log(
      "Setting up validators and SPL single pools for staked tests...",
    );
    for (let i = 0; i < numValidators; i++) {
      const v = await createValidatorBankrun(i);
      const vWithPool = await createSplStakePoolBankrun(v);
      validators.push(vWithPool);
    }

    // -------------------------------------------------------------------------
    // Step 6: Set up mrgnBankrunProgram for each user
    // Use AnchorProvider with the shared patched connection
    // -------------------------------------------------------------------------
    console.log("Setting up bankrun programs for users...");

    for (let i = 0; i < numUsers; i++) {
      const userProvider = makeProvider(users[i].wallet);
      users[i].mrgnBankrunProgram = new Program<Marginfi>(
        marginfiIdlWithAddress as Marginfi,
        userProvider,
      );
      users[i].mrgnProgram = users[i].mrgnBankrunProgram;
    }

    globalProgramAdmin.mrgnBankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      makeProvider(globalProgramAdmin.wallet),
    );
    globalProgramAdmin.mrgnProgram = globalProgramAdmin.mrgnBankrunProgram;

    groupAdmin.mrgnBankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      makeProvider(groupAdmin.wallet),
    );
    groupAdmin.mrgnProgram = groupAdmin.mrgnBankrunProgram;

    validatorAdmin.mrgnBankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      makeProvider(validatorAdmin.wallet),
    );
    validatorAdmin.mrgnProgram = validatorAdmin.mrgnBankrunProgram;

    emodeAdmin.mrgnBankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      makeProvider(emodeAdmin.wallet),
    );
    emodeAdmin.mrgnProgram = emodeAdmin.mrgnBankrunProgram;

    riskAdmin.mrgnBankrunProgram = new Program<Marginfi>(
      marginfiIdlWithAddress as Marginfi,
      makeProvider(riskAdmin.wallet),
    );
    riskAdmin.mrgnProgram = riskAdmin.mrgnBankrunProgram;

    if (verbose) {
      console.log("---End ecosystem setup (pure bankrun)---");
      console.log("");
    }
  },
};
