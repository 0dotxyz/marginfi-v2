import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { getMint, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { readFileSync } from "fs";
import { resolve } from "path";
import { loadAltStagingEnv } from "./lib/env";
import { i80f48FromString } from "./lib/i80f48";
import {
  deriveBankWithSeed,
  deriveFeeState,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
} from "./lib/pdas";
import { setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Add a bank (asset) to a marginfi group on alt staging.
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/add-bank.ts <config.json> [--send]
 */

const ORACLE_TYPE_PYTH = 3;
const ORACLE_TYPE_SWITCHBOARD = 4;

type BankConfigFile = {
  programId?: string;
  group?: string;
  bankMint: string;
  seed?: string | number;

  oracle: string;
  oracleType: "pyth" | "switchboard" | number;

  depositLimit?: string | number;
  borrowLimit?: string | number;

  assetWeightInit?: string;
  assetWeightMaint?: string;
  liabilityWeightInit?: string;
  liabilityWeightMaint?: string;

  totalAssetValueInitLimit?: string | number;
  assetTag?: number;
  riskTier?: "collateral" | "isolated";
  operationalState?: "operational" | "paused" | "reduceOnly";
  oracleMaxAge?: number;
  oracleMaxConfidence?: number;

  interestRateConfig?: {
    optimalUtilizationRate?: string;
    plateauInterestRate?: string;
    maxInterestRate?: string;
    insuranceFeeFixedApr?: string;
    insuranceIrFee?: string;
    protocolFixedFeeApr?: string;
    protocolIrFee?: string;
    protocolOriginationFee?: string;
  };
};

function parseArgs(argv: string[]) {
  const configFile = argv.find((a) => !a.startsWith("--"));
  const send = argv.includes("--send");
  if (!configFile) {
    throw new Error(
      "Usage: npx ts-node ops/alt-staging/scripts/add-bank.ts <config.json> [--send]"
    );
  }
  return { configFile, send };
}

function bn(value: string | number | undefined, fallback: string): BN {
  const v = value === undefined ? fallback : value.toString();
  return new BN(v);
}

function parseOracleType(v: BankConfigFile["oracleType"]): number {
  if (typeof v === "number") return v;
  if (v === "pyth") return ORACLE_TYPE_PYTH;
  if (v === "switchboard") return ORACLE_TYPE_SWITCHBOARD;
  throw new Error(`Unknown oracleType: ${v}`);
}

function parseU16(label: string, v: number, fallback: number): number {
  const n = v ?? fallback;
  if (!Number.isInteger(n) || n < 0 || n > 65535) {
    throw new Error(`${label} must be u16 (0..65535), got: ${n}`);
  }
  return n;
}

function parseU8(label: string, v: number | undefined, fallback: number): number {
  const n = v ?? fallback;
  if (!Number.isInteger(n) || n < 0 || n > 255) {
    throw new Error(`${label} must be u8 (0..255), got: ${n}`);
  }
  return n;
}

function operationalStateFromString(v?: string): any {
  switch ((v || "operational").toLowerCase()) {
    case "operational":
      return { operational: {} };
    case "paused":
      return { paused: {} };
    case "reduceonly":
    case "reduce_only":
    case "reduce-only":
      return { reduceOnly: {} };
    default:
      throw new Error(`Unknown operationalState: ${v}`);
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

async function main() {
  const { configFile, send } = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const configPath = configFile.startsWith("/")
    ? configFile
    : resolve(process.cwd(), configFile);

  const config = JSON.parse(readFileSync(configPath, "utf-8")) as BankConfigFile;

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
  const seed = bn(config.seed, "0");

  const bank = deriveBankWithSeed(programId, group, bankMint, seed);

  // Derive vault PDAs explicitly so we don't depend on Anchor's account resolver.
  const liquidityVaultAuthority = deriveLiquidityVaultAuthority(programId, bank);
  const liquidityVault = deriveLiquidityVault(programId, bank);
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

  const feeState = deriveFeeState(programId);
  const feeStateInfo = await connection.getAccountInfo(feeState);
  if (!feeStateInfo) {
    throw new Error(
      `FeeState PDA does not exist (${feeState.toBase58()}). Run init-global-fee-state.ts first.`
    );
  }

  // Resolve globalFeeWallet
  let globalFeeWallet: PublicKey | null = null;
  if (process.env.ALT_STAGING_GLOBAL_FEE_WALLET) {
    globalFeeWallet = new PublicKey(process.env.ALT_STAGING_GLOBAL_FEE_WALLET);
  } else {
    try {
      const fsAcct: any = await (program.account as any).feeState.fetch(feeState);
      globalFeeWallet = fsAcct.globalFeeWallet ?? fsAcct.global_fee_wallet;
    } catch {
      // ignore; handled below
    }
  }

  if (!globalFeeWallet) {
    throw new Error(
      "Could not resolve global fee wallet. Set ALT_STAGING_GLOBAL_FEE_WALLET in ops/alt-staging/.env"
    );
  }

  const oracle = new PublicKey(config.oracle);
  const oracleType = parseOracleType(config.oracleType);

  const bankConfig = {
    assetWeightInit: i80f48FromString(config.assetWeightInit || "1"),
    assetWeightMaint: i80f48FromString(config.assetWeightMaint || "1"),
    liabilityWeightInit: i80f48FromString(config.liabilityWeightInit || "1"),
    liabilityWeightMaint: i80f48FromString(config.liabilityWeightMaint || "1"),
    depositLimit: bn(config.depositLimit, "18446744073709551615"),
    interestRateConfig: {
      optimalUtilizationRate: i80f48FromString(
        config.interestRateConfig?.optimalUtilizationRate || "0.8"
      ),
      plateauInterestRate: i80f48FromString(
        config.interestRateConfig?.plateauInterestRate || "0.1"
      ),
      maxInterestRate: i80f48FromString(
        config.interestRateConfig?.maxInterestRate || "0.5"
      ),
      insuranceFeeFixedApr: i80f48FromString(
        config.interestRateConfig?.insuranceFeeFixedApr || "0"
      ),
      insuranceIrFee: i80f48FromString(
        config.interestRateConfig?.insuranceIrFee || "0"
      ),
      protocolFixedFeeApr: i80f48FromString(
        config.interestRateConfig?.protocolFixedFeeApr || "0"
      ),
      protocolIrFee: i80f48FromString(
        config.interestRateConfig?.protocolIrFee || "0"
      ),
      protocolOriginationFee: i80f48FromString(
        config.interestRateConfig?.protocolOriginationFee || "0"
      ),
    },
    operationalState: operationalStateFromString(config.operationalState),
    borrowLimit: bn(config.borrowLimit, "0"),
    riskTier: riskTierFromString(config.riskTier),
    assetTag: parseU8("assetTag", config.assetTag, 0),
    configFlags: 1,
    pad0: [0, 0, 0, 0, 0],
    totalAssetValueInitLimit: bn(config.totalAssetValueInitLimit, "1000000000"),
    oracleMaxAge: parseU16("oracleMaxAge", config.oracleMaxAge ?? 30, 30),
    oracleMaxConfidence: config.oracleMaxConfidence ?? 0,
  };

  console.log("=== add-bank ===");
  console.log("Config:", configPath);
  console.log("Program:", programId.toBase58());
  console.log("Group:", group.toBase58());
  console.log("Admin:", wallet.publicKey.toBase58());
  console.log("Mint:", bankMint.toBase58());
  console.log("Seed:", seed.toString());
  console.log("Derived bank:", bank.toBase58());
  console.log("Token program:", tokenProgram.toBase58());
  console.log("Oracle:", oracle.toBase58());
  console.log("Oracle type:", oracleType);
  console.log("Global fee wallet:", globalFeeWallet.toBase58());
  console.log();

  const tx = new Transaction();

  const bankInfo = await connection.getAccountInfo(bank);
  if (bankInfo) {
    console.log("Bank already exists; skipping add-bank instruction");
  } else {
    const addBankIx = await (program.methods as any)
      .lendingPoolAddBankWithSeed(bankConfig, seed)
      .accounts({
        marginfiGroup: group,
        admin: wallet.publicKey,
        feePayer: wallet.publicKey,
        feeState,
        globalFeeWallet,
        bankMint,
        bank,
        liquidityVaultAuthority,
        liquidityVault,
        insuranceVaultAuthority,
        insuranceVault,
        feeVaultAuthority,
        feeVault,
        tokenProgram,
      })
      .instruction();

    tx.add(addBankIx);
  }

  const oracleMeta = {
    pubkey: oracle,
    isSigner: false,
    isWritable: false,
  };

  const configureOracleIx = await (program.methods as any)
    .lendingPoolConfigureBankOracle(oracleType, oracle)
    .accounts({
      bank,
      group,
      admin: wallet.publicKey,
    })
    .remainingAccounts([oracleMeta])
    .instruction();

  tx.add(configureOracleIx);

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer],
    send,
    label: "add-bank",
  });

  console.log("\n✓ Bank:", bank.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
