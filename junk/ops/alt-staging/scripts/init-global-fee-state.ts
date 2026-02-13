import { PublicKey, Transaction } from "@solana/web3.js";
import { i80f48FromString } from "./lib/i80f48";
import { deriveFeeState } from "./lib/pdas";
import { loadAltStagingEnv } from "./lib/env";
import { setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Initializes the FeeState PDA (runs once per program deployment).
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/init-global-fee-state.ts [--send]
 */

function parseArgs(argv: string[]) {
  return {
    send: argv.includes("--send"),
  };
}

function parseU32(name: string, fallback: string): number {
  const raw = process.env[name] ?? fallback;
  const v = Number.parseInt(raw, 10);
  if (!Number.isFinite(v) || v < 0 || v > 2 ** 32 - 1) {
    throw new Error(`${name} must be a u32 (0..2^32-1), got: ${raw}`);
  }
  return v;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const { connection, wallet, program, programId } = setupAdmin();

  const feeState = deriveFeeState(programId);
  const existing = await connection.getAccountInfo(feeState);
  if (existing) {
    console.log("FeeState already exists:", feeState.toBase58());
    return;
  }

  const globalFeeAdmin = new PublicKey(
    process.env.ALT_STAGING_GLOBAL_FEE_ADMIN || wallet.publicKey.toBase58()
  );

  const globalFeeWallet = new PublicKey(
    process.env.ALT_STAGING_GLOBAL_FEE_WALLET || wallet.publicKey.toBase58()
  );

  const bankInitFlatSolFeeLamports = parseU32(
    "ALT_STAGING_BANK_INIT_FLAT_SOL_FEE_LAMPORTS",
    "0"
  );

  const liquidationFlatSolFeeLamports = parseU32(
    "ALT_STAGING_LIQUIDATION_FLAT_SOL_FEE_LAMPORTS",
    "0"
  );

  const programFeeFixed = i80f48FromString(
    process.env.ALT_STAGING_PROGRAM_FEE_FIXED || "0"
  );

  const programFeeRate = i80f48FromString(
    process.env.ALT_STAGING_PROGRAM_FEE_RATE || "0"
  );

  const liquidationMaxFee = i80f48FromString(
    process.env.ALT_STAGING_LIQUIDATION_MAX_FEE || "0"
  );

  console.log("=== init-global-fee-state ===");
  console.log("Program:", programId.toBase58());
  console.log("FeeState PDA:", feeState.toBase58());
  console.log("Global fee admin:", globalFeeAdmin.toBase58());
  console.log("Global fee wallet:", globalFeeWallet.toBase58());
  console.log("Bank init flat SOL fee (lamports):", bankInitFlatSolFeeLamports);
  console.log(
    "Liquidation flat SOL fee (lamports):",
    liquidationFlatSolFeeLamports
  );
  console.log("Program fee fixed:", process.env.ALT_STAGING_PROGRAM_FEE_FIXED || "0");
  console.log("Program fee rate:", process.env.ALT_STAGING_PROGRAM_FEE_RATE || "0");
  console.log(
    "Liquidation max fee:",
    process.env.ALT_STAGING_LIQUIDATION_MAX_FEE || "0"
  );
  console.log();

  const ix = await (program.methods as any)
    .initGlobalFeeState(
      globalFeeAdmin,
      globalFeeWallet,
      bankInitFlatSolFeeLamports,
      liquidationFlatSolFeeLamports,
      programFeeFixed,
      programFeeRate,
      liquidationMaxFee
    )
    .accounts({
      payer: wallet.publicKey,
      feeState,
    })
    .instruction();

  const tx = new Transaction().add(ix);

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer],
    send: args.send,
    label: "init-global-fee-state",
  });

  console.log("\n✓ FeeState PDA:", feeState.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
