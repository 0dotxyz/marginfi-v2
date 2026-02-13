import {
  Connection,
  PublicKey,
  Signer,
  Transaction,
  VersionedTransaction,
  TransactionMessage,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

export type SimulateAndSendOpts = {
  connection: Connection;
  transaction: Transaction;
  feePayer: PublicKey;
  signers?: Signer[];

  /** If false, only simulate (and optionally print a base58 transaction) */
  send: boolean;

  /** If true, print base58-encoded tx when send=false */
  printBase58?: boolean;

  /** Optional label for logs */
  label?: string;
};

export async function simulateAndSend(opts: SimulateAndSendOpts): Promise<string | null> {
  const {
    connection,
    transaction,
    feePayer,
    signers = [],
    send,
    printBase58 = false,
    label,
  } = opts;

  transaction.feePayer = feePayer;
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
  transaction.recentBlockhash = blockhash;

  // If we're printing a transaction for offline signing, we still want any ephemeral signers
  // (e.g. newly generated group keypair) to sign so the tx is valid.
  if (!send && printBase58 && signers.length > 0) {
    transaction.partialSign(...signers);
  }

  // Convert to VersionedTransaction for simulation (more compatible with modern RPCs)
  const messageV0 = new TransactionMessage({
    payerKey: feePayer,
    recentBlockhash: blockhash,
    instructions: transaction.instructions,
  }).compileToLegacyMessage();

  const versionedTx = new VersionedTransaction(messageV0);

  const sim = await connection.simulateTransaction(versionedTx, {
    sigVerify: false,
    commitment: "confirmed",
    replaceRecentBlockhash: true,
  });

  const prefix = label ? `[${label}] ` : "";

  if (sim.value.logs?.length) {
    console.log(`\n${prefix}Program logs:`);
    for (const l of sim.value.logs) console.log("  " + l);
  }

  if (sim.value.err) {
    console.error(`\n${prefix}Simulation failed:`);
    console.error(JSON.stringify(sim.value.err, null, 2));
    return null;
  }

  console.log(`\n${prefix}Simulation OK. CU:`, sim.value.unitsConsumed);

  if (!send) {
    if (printBase58) {
      const serialized = transaction.serialize({
        requireAllSignatures: false,
        verifySignatures: false,
      });
      console.log(`\n${prefix}Base58 transaction:`);
      console.log(bs58.encode(serialized));
    }
    return null;
  }

  const sig = await sendAndConfirmTransaction(connection, transaction, signers);
  console.log(`\n${prefix}Sent. Signature:`, sig);
  return sig;
}
