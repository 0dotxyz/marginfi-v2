import { AnchorProvider, Idl, Program, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { readFileSync } from "fs";
import { homedir } from "os";
import { resolve } from "path";
import { loadAltStagingEnv, requireEnv } from "./env";

export type SetupResult = {
  connection: Connection;
  wallet: Wallet;
  program: Program;
  programId: PublicKey;
};

export function setupAdmin(programIdOverride?: string): SetupResult {
  loadAltStagingEnv();
  const walletPath =
    process.env.ALT_STAGING_ADMIN_KEYPAIR ||
    process.env.ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR ||
    process.env.ALT_STAGING_DEPLOY_PAYER_KEYPAIR;

  if (!walletPath) {
    throw new Error(
      "ALT_STAGING_ADMIN_KEYPAIR not set (and no fallback to ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR)"
    );
  }

  return setupWithKeypairPath(walletPath, programIdOverride);
}

export function setupUser(programIdOverride?: string): SetupResult {
  loadAltStagingEnv();
  const walletPath = process.env.ALT_STAGING_USER_KEYPAIR;
  if (!walletPath) {
    throw new Error("ALT_STAGING_USER_KEYPAIR not set");
  }
  return setupWithKeypairPath(walletPath, programIdOverride);
}

export function setupWithKeypairPath(
  walletKeypairPath: string,
  programIdOverride?: string
): SetupResult {
  loadAltStagingEnv();

  const rpcUrl = requireEnv("ALT_STAGING_RPC_URL");
  const programIdStr = programIdOverride || requireEnv("ALT_STAGING_PROGRAM_ID");
  const programId = new PublicKey(programIdStr);

  const idlPathRaw = process.env.ALT_STAGING_IDL_PATH || "target/idl/marginfi.json";
  const idlPath = resolvePath(idlPathRaw);

  const wallet = new Wallet(loadKeypairFromFile(resolvePath(walletKeypairPath)));
  const connection = new Connection(rpcUrl, "confirmed");

  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });

  const idlJson = JSON.parse(readFileSync(idlPath, "utf-8")) as Idl;
  const idlWithProgramId: Idl = {
    ...(idlJson as any),
    address: programId.toBase58(),
  };

  const program = new Program(idlWithProgramId as any, provider);

  return { connection, wallet, program, programId };
}

export function loadKeypairFromFile(filepath: string): Keypair {
  const raw = JSON.parse(readFileSync(filepath, "utf-8"));
  const secret = new Uint8Array(raw);
  return Keypair.fromSecretKey(secret);
}

function resolvePath(p: string): string {
  // Expand ~/
  if (p.startsWith("~/")) {
    return resolve(homedir(), p.slice(2));
  }

  // If already absolute, keep it
  if (p.startsWith("/")) {
    return p;
  }

  // Otherwise resolve relative to process.cwd() (assumes you run from repo root)
  return resolve(process.cwd(), p);
}
