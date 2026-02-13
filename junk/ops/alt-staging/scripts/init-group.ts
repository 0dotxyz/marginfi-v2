import { Keypair, Transaction } from "@solana/web3.js";
import { existsSync, mkdirSync, writeFileSync } from "fs";
import { resolve } from "path";
import { loadAltStagingEnv } from "./lib/env";
import { deriveFeeState } from "./lib/pdas";
import { loadKeypairFromFile, setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Initialize a new marginfi group ("pool").
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/init-group.ts [--send] [--arena] [--group-keypair <path>] [--write-keypair <path>]
 */

type Args = {
  send: boolean;
  arena: boolean;
  groupKeypairPath?: string;
  writeKeypairPath?: string;
};

function parseArgs(argv: string[]): Args {
  const args: Args = {
    send: false,
    arena: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--send") args.send = true;
    else if (a === "--arena") args.arena = true;
    else if (a === "--group-keypair") args.groupKeypairPath = argv[++i];
    else if (a === "--write-keypair") args.writeKeypairPath = argv[++i];
  }

  return args;
}

function writeKeypairJson(path: string, kp: Keypair): void {
  const abs = path.startsWith("/") ? path : resolve(process.cwd(), path);
  const dir = resolve(abs, "..");
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });

  // Solana keypair json is an array of u8
  writeFileSync(abs, JSON.stringify(Array.from(kp.secretKey)), {
    encoding: "utf-8",
  });
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const { connection, wallet, program, programId } = setupAdmin();

  const feeState = deriveFeeState(programId);
  const feeStateInfo = await connection.getAccountInfo(feeState);
  if (!feeStateInfo) {
    throw new Error(
      `FeeState PDA does not exist (${feeState.toBase58()}). Run init-global-fee-state.ts first.`
    );
  }

  const groupKeypair = args.groupKeypairPath
    ? loadKeypairFromFile(resolve(process.cwd(), args.groupKeypairPath))
    : Keypair.generate();

  if (!args.groupKeypairPath && args.writeKeypairPath) {
    writeKeypairJson(args.writeKeypairPath, groupKeypair);
    console.log("Wrote group keypair:", args.writeKeypairPath);
  }

  console.log("=== init-group ===");
  console.log("Program:", programId.toBase58());
  console.log("FeeState:", feeState.toBase58());
  console.log("Admin:", wallet.publicKey.toBase58());
  console.log("Group (new):", groupKeypair.publicKey.toBase58());
  console.log("isArenaGroup:", args.arena);
  console.log();

  const ix = await (program.methods as any)
    .marginfiGroupInitialize(args.arena)
    .accounts({
      marginfiGroup: groupKeypair.publicKey,
      admin: wallet.publicKey,
      feeState,
    })
    .instruction();

  const tx = new Transaction().add(ix);

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer, groupKeypair],
    send: args.send,
    label: "init-group",
  });

  console.log("\n✓ Group:", groupKeypair.publicKey.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
