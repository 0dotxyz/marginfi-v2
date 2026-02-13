import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import {
  getMint,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { readFileSync } from "fs";
import { resolve } from "path";
import { loadAltStagingEnv } from "./lib/env";
import { i80f48FromString, WrappedI80F48 } from "./lib/i80f48";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVaultAuthority,
} from "./lib/pdas";
import { setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Add a JupLend bank to a marginfi group on alt staging.
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/add-bank-juplend.ts <config.json> [--send]
 */

// Oracle setup types
const ORACLE_SETUP_JUPLEND_PYTH_PULL = 14;
const ORACLE_SETUP_JUPLEND_SWITCHBOARD_PULL = 15;

type JuplendBankConfigFile = {
  programId?: string;
  group?: string;

  // JupLend-specific
  juplendLending: string;
  fTokenMint: string;
  bankMint: string;
  seed?: string | number;

  // Oracle
  oracle: string;
  oracleSetup: "juplendPythPull" | "juplendSwitchboardPull" | number;

  // Weights
  assetWeightInit?: string;
  assetWeightMaint?: string;

  // Limits
  depositLimit?: string | number;
  totalAssetValueInitLimit?: string | number;

  // Risk
  riskTier?: "collateral" | "isolated";
  configFlags?: number;
  oracleMaxAge?: number;
  oracleMaxConfidence?: number;
};

function parseArgs(argv: string[]) {
  const configFile = argv.find((a) => !a.startsWith("--"));
  const send = argv.includes("--send");
  if (!configFile) {
    throw new Error(
      "Usage: npx ts-node ops/alt-staging/scripts/add-bank-juplend.ts <config.json> [--send]"
    );
  }
  return { configFile, send };
}

function bn(value: string | number | undefined, fallback: string): BN {
  const v = value === undefined ? fallback : value.toString();
  return new BN(v);
}

function parseOracleSetup(v: JuplendBankConfigFile["oracleSetup"]): number {
  if (typeof v === "number") return v;
  if (v === "juplendPythPull") return ORACLE_SETUP_JUPLEND_PYTH_PULL;
  if (v === "juplendSwitchboardPull") return ORACLE_SETUP_JUPLEND_SWITCHBOARD_PULL;
  throw new Error(`Unknown oracleSetup: ${v}`);
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

async function main() {
  const { configFile, send } = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const configPath = configFile.startsWith("/")
    ? configFile
    : resolve(process.cwd(), configFile);

  const config = JSON.parse(readFileSync(configPath, "utf-8")) as JuplendBankConfigFile;

  const programIdStr = config.programId || process.env.ALT_STAGING_PROGRAM_ID;
  if (!programIdStr) {
    throw new Error("Missing program id (set ALT_STAGING_PROGRAM_ID or config.programId)");
  }

  const groupStr = config.group || process.env.ALT_STAGING_GROUP;
  if (!groupStr) {
    throw new Error("Missing group pubkey (set ALT_STAGING_GROUP or config.group)");
  }

  const { connection, wallet, program, programId } = setupAdmin(programIdStr);

  const group = new PublicKey(groupStr);
  const bankMint = new PublicKey(config.bankMint);
  const seed = bn(config.seed, "600");
  const juplendLending = new PublicKey(config.juplendLending);
  const fTokenMint = new PublicKey(config.fTokenMint);
  const oracle = new PublicKey(config.oracle);

  const bank = deriveBankWithSeed(programId, group, bankMint, seed);

  // Derive vault PDAs
  const liquidityVaultAuthority = deriveLiquidityVaultAuthority(programId, bank);
  const insuranceVaultAuthority = deriveInsuranceVaultAuthority(programId, bank);
  const insuranceVault = deriveInsuranceVault(programId, bank);
  const feeVaultAuthority = deriveFeeVaultAuthority(programId, bank);
  const feeVault = deriveFeeVault(programId, bank);

  // Token program auto-detect (SPL vs Token-2022)
  let tokenProgram = TOKEN_PROGRAM_ID;
  try {
    await getMint(connection, bankMint, "confirmed", TOKEN_2022_PROGRAM_ID);
    tokenProgram = TOKEN_2022_PROGRAM_ID;
  } catch {
    // ignore
  }

  // For JupLend banks, liquidity_vault is an ATA
  const liquidityVault = getAssociatedTokenAddressSync(
    bankMint,
    liquidityVaultAuthority,
    true, // allowOwnerOffCurve
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  // fToken vault is also an ATA
  const juplendFTokenVault = getAssociatedTokenAddressSync(
    fTokenMint,
    liquidityVaultAuthority,
    true, // allowOwnerOffCurve
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const oracleSetup = parseOracleSetup(config.oracleSetup);

  // Build JuplendConfigCompact
  const bankConfig = {
    oracle,
    assetWeightInit: i80f48FromString(config.assetWeightInit || "0.8"),
    assetWeightMaint: i80f48FromString(config.assetWeightMaint || "0.9"),
    depositLimit: bn(config.depositLimit, "1000000000000"),
    oracleSetup: oracleSetup,
    riskTier: riskTierFromString(config.riskTier),
    configFlags: config.configFlags ?? 1,
    totalAssetValueInitLimit: bn(config.totalAssetValueInitLimit, "1000000000"),
    oracleMaxAge: config.oracleMaxAge ?? 60,
    oracleMaxConfidence: config.oracleMaxConfidence ?? 0,
  };

  console.log("=== add-bank-juplend ===");
  console.log("Config:", configPath);
  console.log("Program:", programId.toBase58());
  console.log("Group:", group.toBase58());
  console.log("Admin:", wallet.publicKey.toBase58());
  console.log("Mint:", bankMint.toBase58());
  console.log("Seed:", seed.toString());
  console.log("Derived bank:", bank.toBase58());
  console.log("Token program:", tokenProgram.toBase58());
  console.log("Oracle:", oracle.toBase58());
  console.log("Oracle setup:", oracleSetup);
  console.log("JupLend Lending:", juplendLending.toBase58());
  console.log("fToken Mint:", fTokenMint.toBase58());
  console.log("fToken Vault:", juplendFTokenVault.toBase58());
  console.log();

  const tx = new Transaction();

  const bankInfo = await connection.getAccountInfo(bank);
  if (bankInfo) {
    console.log("Bank already exists; skipping add-bank instruction");
  } else {
    // Remaining accounts for oracle validation:
    // 0. oracle feed
    // 1. JupLend Lending state (already validated via constraint)
    const remainingAccounts = [
      { pubkey: oracle, isSigner: false, isWritable: false },
      { pubkey: juplendLending, isSigner: false, isWritable: false },
    ];

    const addBankIx = await (program.methods as any)
      .lendingPoolAddBankJuplend(bankConfig, seed)
      .accounts({
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
      })
      .remainingAccounts(remainingAccounts)
      .instruction();

    tx.add(addBankIx);
  }

  if (tx.instructions.length === 0) {
    console.log("No instructions to send (bank already exists).");
    return;
  }

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer],
    send,
    label: "add-bank-juplend",
  });

  console.log("\n Bank:", bank.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
