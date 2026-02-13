import {
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { loadAltStagingEnv } from "./lib/env";
import { deriveMarginfiAccountPda } from "./lib/pdas";
import { setupUser } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Initializes a user marginfi account PDA for quick testing.
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/init-user.ts [--send] [--group <pubkey>] [--index <u16>] [--third-party-id <u16>] 
 */

type Args = {
  send: boolean;
  group?: string;
  index: number;
  thirdPartyId?: number;
};

function parseArgs(argv: string[]): Args {
  const out: Args = {
    send: false,
    index: 0,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--send") out.send = true;
    else if (a === "--group") out.group = argv[++i];
    else if (a === "--index") out.index = Number.parseInt(argv[++i], 10);
    else if (a === "--third-party-id") out.thirdPartyId = Number.parseInt(argv[++i], 10);
  }

  return out;
}

function assertU16(n: number, label: string): void {
  if (!Number.isInteger(n) || n < 0 || n > 65535) {
    throw new Error(`${label} must be u16 (0..65535), got: ${n}`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const groupStr = args.group || process.env.ALT_STAGING_GROUP;
  if (!groupStr) {
    throw new Error("Missing group pubkey. Pass --group or set ALT_STAGING_GROUP in ops/alt-staging/.env");
  }

  assertU16(args.index, "index");
  if (args.thirdPartyId !== undefined) assertU16(args.thirdPartyId, "thirdPartyId");

  const { connection, wallet, program, programId } = setupUser();

  const group = new PublicKey(groupStr);
  const thirdPartyIdForSeeds = args.thirdPartyId ?? 0;

  const marginfiAccount = deriveMarginfiAccountPda(
    programId,
    group,
    wallet.publicKey,
    args.index,
    thirdPartyIdForSeeds
  );

  const existing = await connection.getAccountInfo(marginfiAccount);
  if (existing) {
    console.log("Marginfi account PDA already exists:", marginfiAccount.toBase58());
    return;
  }

  console.log("=== init-user (marginfiAccountInitializePda) ===");
  console.log("Program:", programId.toBase58());
  console.log("Group:", group.toBase58());
  console.log("User:", wallet.publicKey.toBase58());
  console.log("Account index:", args.index);
  console.log("Third party id:", args.thirdPartyId ?? null);
  console.log("Derived PDA:", marginfiAccount.toBase58());
  console.log();

  const thirdPartyIdArg = args.thirdPartyId === undefined ? null : args.thirdPartyId;

  const ix = await (program.methods as any)
    .marginfiAccountInitializePda(args.index, thirdPartyIdArg)
    .accounts({
      marginfiGroup: group,
      marginfiAccount,
      authority: wallet.publicKey,
      feePayer: wallet.publicKey,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    })
    .instruction();

  const tx = new Transaction().add(ix);

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer],
    send: args.send,
    label: "init-user",
  });

  console.log("\n✓ Marginfi account PDA:", marginfiAccount.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
