import {
  Connection,
  PublicKey,
  Transaction,
  VersionedTransaction,
  Commitment,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { BanksClient } from "solana-bankrun";
import { Instruction, utils } from "@coral-xyz/anchor";

/**
 * Patches a bankrun connection to add missing methods that tests need.
 * BankrunConnectionProxy only provides: getAccountInfo, getAccountInfoAndContext, getMinimumBalanceForRentExemption
 * This adds: getBalance, getLatestBlockhash, sendRawTransaction, confirmTransaction, getStakeMinimumDelegation
 */
export function patchBankrunConnection(
  connection: Connection,
  banksClient: BanksClient
): void {
  type PatchedConnection = Connection & {
    getBalance: (publicKey: PublicKey) => Promise<number>;
    getLatestBlockhash: () => Promise<{
      blockhash: string;
      lastValidBlockHeight: number;
    }>;
    sendRawTransaction: (
      rawTransaction: Buffer | Uint8Array
    ) => Promise<string>;
  };

  const conn = connection as PatchedConnection;

  // BankrunConnectionProxy throws "Could not find" for unknown accounts.
  // Real RPC connections return `null` for missing accounts.
  // Normalize bankrun behavior to match real RPC to avoid test-only try/catch patterns.
  const originalGetAccountInfo = connection.getAccountInfo.bind(connection);
  conn.getAccountInfo = async (
    publicKey: PublicKey,
    commitment?: Commitment
  ) => {
    try {
      return await originalGetAccountInfo(publicKey, commitment);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.startsWith("Could not find")) return null;
      throw e;
    }
  };

  // Also patch getAccountInfoAndContext (used by Anchor's AccountClient.fetch)
  const originalGetAccountInfoAndContext =
    connection.getAccountInfoAndContext.bind(connection);
  conn.getAccountInfoAndContext = async (
    publicKey: PublicKey,
    commitment?: Commitment
  ) => {
    try {
      return await originalGetAccountInfoAndContext(publicKey, commitment);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.startsWith("Could not find"))
        return { context: { slot: 0 }, value: null };
      throw e;
    }
  };

  conn.getBalance = async (publicKey: PublicKey) => {
    const balance = await banksClient.getBalance(publicKey);
    return Number(balance);
  };

  conn.getLatestBlockhash = async () => {
    const [blockhash, lastValidBlockHeight] =
      await banksClient.getLatestBlockhash();
    return { blockhash, lastValidBlockHeight: Number(lastValidBlockHeight) };
  };

  conn.sendRawTransaction = async (rawTransaction: Buffer | Uint8Array) => {
    const raw = Buffer.isBuffer(rawTransaction)
      ? rawTransaction
      : Buffer.from(rawTransaction);

    // Support both legacy and v0 transactions (LUT / Address Lookup Tables)
    // Versioned txs have the high bit set on the first byte
    const isVersioned = (raw[0] & 0x80) !== 0;
    const tx = isVersioned
      ? VersionedTransaction.deserialize(raw)
      : Transaction.from(raw);

    const result = await banksClient.tryProcessTransaction(tx);
    if (result.result) {
      const logs = result.meta?.logMessages || [];
      const error = new Error(result.result) as Error & { logs: string[] };
      error.logs = logs;
      throw error;
    }

    // Return real base58-encoded signature for better debug output
    const signature = isVersioned
      ? (tx as VersionedTransaction).signatures[0]
      : (tx as Transaction).signature;
    return signature ? utils.bytes.bs58.encode(signature) : "unsigned-tx";
  };

  conn.confirmTransaction = async (_strategy, _commitment) => {
    // Bankrun transactions are confirmed immediately (errors thrown above in sendRawTransaction)
    return { context: { slot: 0 }, value: { err: null } };
  };

  // Shim for SPL single pool staked tests
  conn.getStakeMinimumDelegation = async (_config) => {
    // Minimum stake delegation on mainnet is 1 SOL
    return { context: { slot: 0 }, value: 1_000_000_000 };
  };
}

/**
 * A random dummy ix to trick bankrun.
 * @param from - must sign
 * @param to
 * @returns
 */
export function dummyTx(
  from: PublicKey,
  to: PublicKey
): TransactionInstruction {
  let ix = SystemProgram.transfer({
    fromPubkey: from,
    toPubkey: to,
    lamports: Math.round(Math.random() * 123456),
  });
  return ix;
}
