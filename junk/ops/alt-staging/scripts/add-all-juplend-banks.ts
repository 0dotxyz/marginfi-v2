import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import {
  getMint,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  createSyncNativeInstruction,
  NATIVE_MINT,
} from "@solana/spl-token";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve } from "path";
import { loadAltStagingEnv } from "./lib/env";
import { i80f48FromString } from "./lib/i80f48";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVaultAuthority,
} from "./lib/pdas";
import {
  deriveJuplendCpiAccounts,
  findJuplendLendingAdminPda,
  JUPLEND_LENDING_PROGRAM_ID,
} from "./lib/juplend-pdas";
import { setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Orchestrator script to add all JupLend banks to the alt-staging group.
 *
 * This script:
 * 1. Reads JupLend asset configuration
 * 2. Fetches/uses cached oracle addresses from mainnet marginfi banks
 * 3. For each asset, creates the bank and activates it
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/add-all-juplend-banks.ts [--send] [--skip-oracles] [--seed-amount <amount>]
 *
 * Options:
 *   --send           Actually send transactions (default: simulate only)
 *   --skip-oracles   Skip fetching oracles, use cached oracles.json
 *   --seed-amount    Seed deposit amount in native units (default: 10000)
 *   --asset <symbol> Only process a specific asset (e.g., --asset WSOL)
 */

// Oracle setup types (numeric values for reference)
const ORACLE_SETUP_VALUES = {
  None: 0,
  PythLegacy: 1,
  SwitchboardV2: 2,
  PythPushOracle: 3,
  SwitchboardPull: 4,
  JuplendPythPull: 14,
  JuplendSwitchboardPull: 15,
};

// Oracle setup as Anchor enum objects
const ORACLE_SETUP_ENUM: Record<number, any> = {
  0: { none: {} },
  1: { pythLegacy: {} },
  2: { switchboardV2: {} },
  3: { pythPushOracle: {} },
  4: { switchboardPull: {} },
  5: { stakedWithPythPush: {} },
  6: { kaminoPythPush: {} },
  7: { kaminoSwitchboardPull: {} },
  8: { fixed: {} },
  9: { driftPythPull: {} },
  10: { driftSwitchboardPull: {} },
  11: { solendPythPull: {} },
  12: { solendSwitchboardPull: {} },
  14: { juplendPythPull: {} },
  15: { juplendSwitchboardPull: {} },
};

// Mainnet marginfi program ID
const MARGINFI_MAINNET_PROGRAM_ID = new PublicKey(
  "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA"
);

// Bank account discriminator
const BANK_DISCRIMINATOR = Buffer.from([
  142, 49, 166, 242, 50, 66, 97, 188,
]);

// Bank layout offsets
const BANK_MINT_OFFSET = 8;
const BANK_CONFIG_OFFSET = 568;
const CONFIG_ORACLE_SETUP_OFFSET = 192;
const CONFIG_ORACLE_KEYS_OFFSET = 193;

// JupLend Bank layout offsets
const JUPLEND_BANK_MINT_OFFSET = 8;
const JUPLEND_BANK_JUPLEND_LENDING_OFFSET = 1736;
const JUPLEND_BANK_JUPLEND_F_TOKEN_VAULT_OFFSET = 1768;
const JUPLEND_BANK_LIQUIDITY_VAULT_OFFSET = 120;

type Args = {
  send: boolean;
  skipOracles: boolean;
  seedAmount: number;
  specificAsset?: string;
};

function parseArgs(argv: string[]): Args {
  const out: Args = {
    send: false,
    skipOracles: false,
    seedAmount: 10000,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--send") {
      out.send = true;
    } else if (a === "--skip-oracles") {
      out.skipOracles = true;
    } else if (a === "--seed-amount") {
      out.seedAmount = Number(argv[++i]);
    } else if (a === "--asset") {
      out.specificAsset = argv[++i];
    }
  }

  return out;
}

type AssetConfig = {
  symbol: string;
  lending: string;
  fTokenMint: string;
  mint?: string;
};

type AssetsFile = {
  juplendProgram: string;
  lendingAdmin: string;
  bankSeed: number;
  assets: AssetConfig[];
};

type OracleInfo = {
  symbol: string;
  mint: string;
  oracle: string;
  oracleSetup: number | string;
  oracleSetupValue?: number;
  juplendOracleSetup: string;
  juplendOracleSetupValue: number;
};

type OraclesFile = {
  fetchedAt: string;
  marginfiProgram: string;
  oracles: OracleInfo[];
};

// Known mainnet mints
const KNOWN_MINTS: Record<string, string> = {
  WSOL: "So11111111111111111111111111111111111111112",
  USDC: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  USDT: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
  EURC: "HzwqbKZw8HxMN6bF2yFZNrht3c2iXXzpKcFu7uBEDKtr",
  USDG: "8zTH3VWGgCTzRtPWULTkgzXdNDcJnmGaLBuRJCYFBVGC",
  USDS: "USDSwr9ApdHk5bvJKMjzff41FfuX8bSxdKcR81vTwcA",
  USDV: "2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo",
  EURCV: "8s1gMCVEcGJTr8LShHiZW3rbFqnmNZ96z2GCyYX5vZrC",
  JUPUSD: "PzuaVAUH2tfxGZcbBR6kMxeJsBngnsPLFotGJNCtcsd",
};

function mapToJuplendOracleSetup(setup: number): number {
  switch (setup) {
    case ORACLE_SETUP.PythPushOracle:
    case ORACLE_SETUP.PythLegacy:
      return ORACLE_SETUP.JuplendPythPull;
    case ORACLE_SETUP.SwitchboardPull:
    case ORACLE_SETUP.SwitchboardV2:
      return ORACLE_SETUP.JuplendSwitchboardPull;
    default:
      console.warn(`Unknown oracle setup: ${setup}, defaulting to JuplendPythPull`);
      return ORACLE_SETUP.JuplendPythPull;
  }
}

function riskTierFromString(v?: string): any {
  switch ((v || "collateral").toLowerCase()) {
    case "collateral":
      return { collateral: {} };
    case "isolated":
      return { isolated: {} };
    default:
      throw new Error(`Unknown riskTier: ${v}`);
  }
}

async function fetchOracles(
  rpcUrl: string,
  assets: AssetConfig[]
): Promise<Map<string, OracleInfo>> {
  const { Connection } = await import("@solana/web3.js");
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("\n=== Fetching Oracles from Mainnet Marginfi ===");
  console.log("RPC:", rpcUrl);
  console.log("Marginfi Program:", MARGINFI_MAINNET_PROGRAM_ID.toBase58());

  // Build mint to symbol map
  const mintSymbolMap = new Map<string, string>();
  for (const asset of assets) {
    const mint = asset.mint || KNOWN_MINTS[asset.symbol];
    if (mint) {
      mintSymbolMap.set(mint, asset.symbol);
    }
  }

  // Fetch all Bank accounts
  console.log("\nFetching marginfi Bank accounts...");
  const bankAccounts = await connection.getProgramAccounts(
    MARGINFI_MAINNET_PROGRAM_ID,
    {
      filters: [
        {
          memcmp: {
            offset: 0,
            bytes: BANK_DISCRIMINATOR.toString("base64"),
            encoding: "base64",
          },
        },
      ],
    }
  );

  console.log(`Found ${bankAccounts.length} Bank accounts`);

  const oracleMap = new Map<string, OracleInfo>();

  for (const { pubkey, account } of bankAccounts) {
    const data = account.data;

    // Extract mint
    const mintBytes = data.slice(BANK_MINT_OFFSET, BANK_MINT_OFFSET + 32);
    const mint = new PublicKey(mintBytes).toBase58();

    // Check if this mint is one we care about
    const symbol = mintSymbolMap.get(mint);
    if (!symbol) continue;

    // Already have this symbol
    if (oracleMap.has(symbol)) continue;

    // Extract oracle setup
    const oracleSetupOffset = BANK_CONFIG_OFFSET + CONFIG_ORACLE_SETUP_OFFSET;
    const oracleSetup = data[oracleSetupOffset];

    // Extract first oracle key
    const oracleKeyOffset = BANK_CONFIG_OFFSET + CONFIG_ORACLE_KEYS_OFFSET;
    const oracleKeyBytes = data.slice(oracleKeyOffset, oracleKeyOffset + 32);
    const oracleKey = new PublicKey(oracleKeyBytes).toBase58();

    const juplendOracleSetupValue = mapToJuplendOracleSetup(oracleSetup);
    const juplendOracleSetupName = juplendOracleSetupValue === 14 ? "juplendPythPull" : "juplendSwitchboardPull";

    console.log(`Found ${symbol}: oracle=${oracleKey.slice(0, 8)}... setup=${oracleSetup} -> ${juplendOracleSetupValue}`);

    oracleMap.set(symbol, {
      symbol,
      mint,
      oracle: oracleKey,
      oracleSetup,
      juplendOracleSetup: juplendOracleSetupName,
      juplendOracleSetupValue,
    });
  }

  return oracleMap;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const programIdStr = process.env.ALT_STAGING_PROGRAM_ID;
  if (!programIdStr) {
    throw new Error("Missing ALT_STAGING_PROGRAM_ID in environment");
  }

  const groupStr = process.env.ALT_STAGING_GROUP;
  if (!groupStr) {
    throw new Error("Missing ALT_STAGING_GROUP in environment");
  }

  const { connection, wallet, program, programId } = setupAdmin(programIdStr);

  const group = new PublicKey(groupStr);

  // Load asset configuration
  const assetsPath = resolve(__dirname, "..", "configs", "juplend-assets.json");
  const assetsFile: AssetsFile = JSON.parse(readFileSync(assetsPath, "utf-8"));
  const seed = new BN(assetsFile.bankSeed);

  console.log("=== Add All JupLend Banks ===");
  console.log("Program:", programId.toBase58());
  console.log("Group:", group.toBase58());
  console.log("Admin:", wallet.publicKey.toBase58());
  console.log("Bank seed:", seed.toString());
  console.log("Seed deposit amount:", args.seedAmount);
  console.log("Mode:", args.send ? "SEND" : "SIMULATE");
  console.log();

  // Filter assets if specific one requested
  let assets = assetsFile.assets;
  if (args.specificAsset) {
    assets = assets.filter((a) => a.symbol === args.specificAsset);
    if (assets.length === 0) {
      throw new Error(`Asset not found: ${args.specificAsset}`);
    }
    console.log(`Processing only: ${args.specificAsset}`);
  }

  // Get or fetch oracles
  const oraclesPath = resolve(__dirname, "..", "configs", "oracles.json");
  let oracleMap: Map<string, OracleInfo>;

  if (args.skipOracles && existsSync(oraclesPath)) {
    console.log("Using cached oracles from:", oraclesPath);
    const oraclesFile: OraclesFile = JSON.parse(readFileSync(oraclesPath, "utf-8"));
    oracleMap = new Map(oraclesFile.oracles.map((o) => [o.symbol, o]));
  } else {
    const mainnetRpc =
      process.env.MAINNET_RPC_URL ||
      process.env.ALT_STAGING_RPC_URL ||
      "https://api.mainnet-beta.solana.com";
    oracleMap = await fetchOracles(mainnetRpc, assets);

    // Save oracles
    const oraclesOutput = {
      fetchedAt: new Date().toISOString(),
      marginfiProgram: MARGINFI_MAINNET_PROGRAM_ID.toBase58(),
      oracles: Array.from(oracleMap.values()),
    };
    writeFileSync(oraclesPath, JSON.stringify(oraclesOutput, null, 2));
    console.log(`\nSaved oracles to: ${oraclesPath}`);
  }

  // Results tracking
  const results: { symbol: string; bank: string; status: string }[] = [];

  // Comprehensive bank details for frontend
  type CreatedBankDetails = {
    symbol: string;
    mint: string;
    mintDecimals?: number;
    seed: number;
    assetTag: number; // 6 = JUPLEND

    // Core accounts
    bank: string;
    group: string;

    // Vault accounts
    liquidityVault: string;
    liquidityVaultAuthority: string;
    insuranceVault: string;
    insuranceVaultAuthority: string;
    feeVault: string;
    feeVaultAuthority: string;

    // JupLend-specific accounts
    juplendLending: string;
    fTokenMint: string;
    juplendFTokenVault: string;

    // Oracle
    oracle: string;
    oracleSetup: number;
    oracleSetupName: string;

    // Token program
    tokenProgram: string;

    // JupLend CPI accounts (for deposits/withdraws)
    juplendCpi: {
      lendingAdmin: string;
      liquidity: string;
      liquidityProgram: string;
      rateModel: string;
      vault: string;
      supplyTokenReservesLiquidity: string;
      lendingSupplyPositionOnLiquidity: string;
      rewardsRateModel: string;
    };
  };

  const createdBanks: CreatedBankDetails[] = [];

  // Process each asset
  for (const asset of assets) {
    console.log(`\n${"=".repeat(60)}`);
    console.log(`Processing ${asset.symbol}`);
    console.log("=".repeat(60));

    const bankMint = new PublicKey(asset.mint || KNOWN_MINTS[asset.symbol]);
    if (!bankMint) {
      console.log(`Skipping ${asset.symbol}: no mint address found`);
      results.push({ symbol: asset.symbol, bank: "-", status: "SKIPPED (no mint)" });
      continue;
    }

    const oracleInfo = oracleMap.get(asset.symbol);
    if (!oracleInfo) {
      console.log(`Skipping ${asset.symbol}: no oracle found`);
      results.push({ symbol: asset.symbol, bank: "-", status: "SKIPPED (no oracle)" });
      continue;
    }

    const juplendLending = new PublicKey(asset.lending);
    const fTokenMint = new PublicKey(asset.fTokenMint);
    const oracle = new PublicKey(oracleInfo.oracle);

    const bank = deriveBankWithSeed(programId, group, bankMint, seed);

    console.log("Bank mint:", bankMint.toBase58());
    console.log("Derived bank:", bank.toBase58());
    console.log("JupLend Lending:", juplendLending.toBase58());
    console.log("fToken Mint:", fTokenMint.toBase58());
    console.log("Oracle:", oracle.toBase58());
    console.log("Oracle setup:", oracleInfo.juplendOracleSetup, `(${oracleInfo.juplendOracleSetupValue})`);

    // Check if bank already exists
    const bankInfo = await connection.getAccountInfo(bank);
    const bankExists = !!bankInfo;

    if (bankExists) {
      console.log("\nBank already exists, checking if activated...");

      // Check operational state - if not activated, do init position
      // For now, we'll try init position anyway and let it fail if already done
    }

    // Token program auto-detect
    let tokenProgram = TOKEN_PROGRAM_ID;
    try {
      await getMint(connection, bankMint, "confirmed", TOKEN_2022_PROGRAM_ID);
      tokenProgram = TOKEN_2022_PROGRAM_ID;
    } catch {
      // ignore
    }

    // Derive vault PDAs
    const liquidityVaultAuthority = deriveLiquidityVaultAuthority(programId, bank);
    const insuranceVaultAuthority = deriveInsuranceVaultAuthority(programId, bank);
    const insuranceVault = deriveInsuranceVault(programId, bank);
    const feeVaultAuthority = deriveFeeVaultAuthority(programId, bank);
    const feeVault = deriveFeeVault(programId, bank);

    // For JupLend banks, liquidity_vault is an ATA
    const liquidityVault = getAssociatedTokenAddressSync(
      bankMint,
      liquidityVaultAuthority,
      true,
      tokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    // fToken vault is also an ATA - detect the fToken mint's token program
    let fTokenProgram = TOKEN_PROGRAM_ID;
    try {
      await getMint(connection, fTokenMint, "confirmed", TOKEN_2022_PROGRAM_ID);
      fTokenProgram = TOKEN_2022_PROGRAM_ID;
      console.log("fToken mint uses Token-2022");
    } catch {
      console.log("fToken mint uses SPL Token");
    }

    const juplendFTokenVault = getAssociatedTokenAddressSync(
      fTokenMint,
      liquidityVaultAuthority,
      true,
      fTokenProgram,
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    try {
      // Step 1: Add bank if it doesn't exist
      if (!bankExists) {
        console.log("\n--- Step 1: Add Bank ---");

        const bankConfig = {
          oracle,
          assetWeightInit: i80f48FromString("0.8"),
          assetWeightMaint: i80f48FromString("0.9"),
          depositLimit: new BN("1000000000000"),
          oracleSetup: ORACLE_SETUP_ENUM[oracleInfo.juplendOracleSetupValue],
          riskTier: riskTierFromString("collateral"),
          configFlags: 1,
          totalAssetValueInitLimit: new BN("1000000000"),
          oracleMaxAge: 60,
          oracleMaxConfidence: 0,
        };

        const remainingAccounts = [
          { pubkey: oracle, isSigner: false, isWritable: false },
          { pubkey: juplendLending, isSigner: false, isWritable: false },
        ];

        const addBankTx = new Transaction();
        console.log("  bankConfig:", JSON.stringify(bankConfig, (k, v) => typeof v === 'bigint' ? v.toString() : v, 2));

        console.log("  Building method call...");
        const methodCall = (program.methods as any).lendingPoolAddBankJuplend(bankConfig, seed);
        console.log("  Method call created, adding accounts...");

        const withAccounts = methodCall.accounts({
          group,
          admin: wallet.publicKey,
          feePayer: wallet.publicKey,
          bankMint,
          bank,
          juplendLending,
          liquidityVaultAuthority,
          liquidityVault,
          insuranceVaultAuthority,
          insuranceVault,
          feeVaultAuthority,
          feeVault,
          fTokenMint,
          juplendFTokenVault,
          tokenProgram,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        });
        console.log("  Accounts added, adding remaining accounts...");

        const withRemaining = withAccounts.remainingAccounts(remainingAccounts);
        console.log("  Remaining accounts added, building instruction...");

        const addBankIx = await withRemaining.instruction();
        console.log("  Instruction built successfully");

        addBankTx.add(addBankIx);
        console.log("  Added to transaction, calling simulateAndSend...");

        try {
          await simulateAndSend({
            connection,
            transaction: addBankTx,
            feePayer: wallet.publicKey,
            signers: [wallet.payer],
            send: args.send,
            label: `add-bank-${asset.symbol}`,
          });
          console.log(`Bank created: ${bank.toBase58()}`);
        } catch (simErr: any) {
          console.error("  SimulateAndSend error:", simErr.message);
          console.error("  Full simulation error:", simErr);
          throw simErr;
        }
      } else {
        console.log("\n--- Step 1: Bank already exists, skipping add ---");
      }

      // Step 2: Initialize position (activate bank)
      console.log("\n--- Step 2: Init Position ---");

      // In simulation mode without actual send, we can't do init position since bank doesn't exist yet
      if (!args.send && !bankExists) {
        console.log("  Skipping init position in simulation mode (bank not created yet)");
        console.log("  Run with --send to create bank and init position");
        results.push({ symbol: asset.symbol, bank: bank.toBase58(), status: "SIMULATED" });
        continue;
      }

      // Fetch fresh bank data for init position
      const freshBankInfo = await connection.getAccountInfo(bank);
      if (!freshBankInfo) {
        throw new Error(`Bank not found after creation: ${bank.toBase58()}`);
      }

      const bankData = freshBankInfo.data;

      // Extract data from bank for init position
      const lendingInfo = await connection.getAccountInfo(juplendLending);
      if (!lendingInfo) {
        throw new Error(`JupLend Lending not found: ${juplendLending.toBase58()}`);
      }

      // fTokenMint is at offset 32 (after discriminator + mint) in the Lending account
      const fetchedFTokenMintBytes = lendingInfo.data.slice(8 + 32, 8 + 32 + 32);
      const fetchedFTokenMint = new PublicKey(fetchedFTokenMintBytes);

      // token_reserves_liquidity at offset 131, supply_position at 163
      const tokenReservesLiquidityBytes = lendingInfo.data.slice(131, 131 + 32);
      const supplyTokenReservesLiquidity = new PublicKey(tokenReservesLiquidityBytes);

      const supplyPositionBytes = lendingInfo.data.slice(131 + 32, 131 + 32 + 32);
      const lendingSupplyPositionOnLiquidity = new PublicKey(supplyPositionBytes);

      // Derive JupLend CPI accounts
      const juplendAccounts = deriveJuplendCpiAccounts(bankMint, tokenProgram);
      const [lendingAdmin] = findJuplendLendingAdminPda(JUPLEND_LENDING_PROGRAM_ID);

      // Get user's token account
      const signerTokenAccount = getAssociatedTokenAddressSync(
        bankMint,
        wallet.publicKey,
        false,
        tokenProgram,
        ASSOCIATED_TOKEN_PROGRAM_ID
      );

      const isWsol = bankMint.equals(NATIVE_MINT);
      const seedAmount = new BN(args.seedAmount);

      const initTx = new Transaction();

      // If WSOL, wrap SOL
      if (isWsol) {
        const ataInfo = await connection.getAccountInfo(signerTokenAccount);
        if (!ataInfo) {
          console.log("Creating WSOL ATA...");
          initTx.add(
            createAssociatedTokenAccountInstruction(
              wallet.publicKey,
              signerTokenAccount,
              wallet.publicKey,
              bankMint,
              tokenProgram,
              ASSOCIATED_TOKEN_PROGRAM_ID
            )
          );
        }

        console.log(`Wrapping ${seedAmount.toString()} lamports as WSOL...`);
        initTx.add(
          SystemProgram.transfer({
            fromPubkey: wallet.publicKey,
            toPubkey: signerTokenAccount,
            lamports: seedAmount.toNumber(),
          })
        );
        initTx.add(createSyncNativeInstruction(signerTokenAccount, tokenProgram));
      } else {
        // Check balance for non-WSOL
        const ataInfo = await connection.getAccountInfo(signerTokenAccount);
        if (!ataInfo) {
          throw new Error(
            `Signer token account not found: ${signerTokenAccount.toBase58()}. ` +
              `Please create an ATA and fund it with at least ${seedAmount.toString()} tokens.`
          );
        }
      }

      // Build init position instruction
      const initPositionIx = await (program.methods as any)
        .juplendInitPosition(seedAmount)
        .accounts({
          feePayer: wallet.publicKey,
          signerTokenAccount,
          bank,
          liquidityVaultAuthority,
          liquidityVault,
          mint: bankMint,
          juplendLending,
          fTokenMint: fetchedFTokenMint,
          juplendFTokenVault,
          lendingAdmin,
          supplyTokenReservesLiquidity,
          lendingSupplyPositionOnLiquidity,
          rateModel: juplendAccounts.rateModel,
          vault: juplendAccounts.vault,
          liquidity: juplendAccounts.liquidity,
          liquidityProgram: juplendAccounts.liquidityProgram,
          rewardsRateModel: juplendAccounts.rewardsRateModel,
          juplendProgram: JUPLEND_LENDING_PROGRAM_ID,
          tokenProgram,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .instruction();

      initTx.add(initPositionIx);

      await simulateAndSend({
        connection,
        transaction: initTx,
        feePayer: wallet.publicKey,
        signers: [wallet.payer],
        send: args.send,
        label: `init-position-${asset.symbol}`,
      });

      console.log(`Position initialized for bank: ${bank.toBase58()}`);
      results.push({ symbol: asset.symbol, bank: bank.toBase58(), status: "SUCCESS" });

      // Get mint decimals
      let mintDecimals: number | undefined;
      try {
        const mintInfo = await getMint(connection, bankMint, "confirmed", tokenProgram);
        mintDecimals = mintInfo.decimals;
      } catch {
        // ignore
      }

      // Build comprehensive bank details
      const oracleSetupNames: Record<number, string> = {
        14: "juplendPythPull",
        15: "juplendSwitchboardPull",
      };

      const bankDetails: CreatedBankDetails = {
        symbol: asset.symbol,
        mint: bankMint.toBase58(),
        mintDecimals,
        seed: seed.toNumber(),
        assetTag: 6, // JUPLEND

        // Core accounts
        bank: bank.toBase58(),
        group: group.toBase58(),

        // Vault accounts
        liquidityVault: liquidityVault.toBase58(),
        liquidityVaultAuthority: liquidityVaultAuthority.toBase58(),
        insuranceVault: insuranceVault.toBase58(),
        insuranceVaultAuthority: insuranceVaultAuthority.toBase58(),
        feeVault: feeVault.toBase58(),
        feeVaultAuthority: feeVaultAuthority.toBase58(),

        // JupLend-specific accounts
        juplendLending: juplendLending.toBase58(),
        fTokenMint: fTokenMint.toBase58(),
        juplendFTokenVault: juplendFTokenVault.toBase58(),

        // Oracle
        oracle: oracle.toBase58(),
        oracleSetup: oracleInfo.juplendOracleSetupValue,
        oracleSetupName: oracleSetupNames[oracleInfo.juplendOracleSetupValue] || "unknown",

        // Token program
        tokenProgram: tokenProgram.toBase58(),

        // JupLend CPI accounts
        juplendCpi: {
          lendingAdmin: lendingAdmin.toBase58(),
          liquidity: juplendAccounts.liquidity.toBase58(),
          liquidityProgram: juplendAccounts.liquidityProgram.toBase58(),
          rateModel: juplendAccounts.rateModel.toBase58(),
          vault: juplendAccounts.vault.toBase58(),
          supplyTokenReservesLiquidity: supplyTokenReservesLiquidity.toBase58(),
          lendingSupplyPositionOnLiquidity: lendingSupplyPositionOnLiquidity.toBase58(),
          rewardsRateModel: juplendAccounts.rewardsRateModel.toBase58(),
        },
      };

      createdBanks.push(bankDetails);

    } catch (err: any) {
      console.error(`Error processing ${asset.symbol}:`, err.message);
      results.push({ symbol: asset.symbol, bank: bank.toBase58(), status: `ERROR: ${err.message.slice(0, 50)}` });
    }
  }

  // Print summary
  console.log("\n" + "=".repeat(60));
  console.log("SUMMARY");
  console.log("=".repeat(60));
  console.log("\nMode:", args.send ? "TRANSACTIONS SENT" : "SIMULATED ONLY");
  console.log();
  console.log("Asset".padEnd(10) + "Bank".padEnd(50) + "Status");
  console.log("-".repeat(80));
  for (const r of results) {
    console.log(r.symbol.padEnd(10) + r.bank.padEnd(50) + r.status);
  }

  // Count successes
  const successes = results.filter((r) => r.status === "SUCCESS").length;
  const failures = results.filter((r) => r.status.startsWith("ERROR")).length;
  const skipped = results.filter((r) => r.status.startsWith("SKIPPED")).length;

  console.log();
  console.log(`Total: ${results.length} | Success: ${successes} | Failed: ${failures} | Skipped: ${skipped}`);

  // Write created banks JSON for frontend
  if (createdBanks.length > 0) {
    const createdBanksPath = resolve(__dirname, "..", "configs", "created-banks.json");
    const createdBanksOutput = {
      createdAt: new Date().toISOString(),
      program: programId.toBase58(),
      group: group.toBase58(),
      bankSeed: seed.toNumber(),
      assetTag: 6,
      assetTagName: "JUPLEND",
      juplendLendingProgram: JUPLEND_LENDING_PROGRAM_ID.toBase58(),
      banks: createdBanks,
    };
    writeFileSync(createdBanksPath, JSON.stringify(createdBanksOutput, null, 2));
    console.log(`\nSaved ${createdBanks.length} bank details to: ${createdBanksPath}`);
  }

  if (!args.send) {
    console.log("\nTo actually send transactions, run with --send flag");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
