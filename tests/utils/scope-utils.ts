import { PublicKey } from "@solana/web3.js";
import { ProgramTestContext } from "./litesvm";

export const SCOPE_PROGRAM = new PublicKey(
  "HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ"
);
/** sha256("account:OraclePrices")[..8] */
export const ORACLE_PRICES_DISCRIMINATOR = Buffer.from([
  89, 128, 118, 221, 6, 72, 180, 146,
]);
export const PRICES_OFFSET = 40;
export const DATED_PRICE_SIZE = 56;
export const MAX_ENTRIES = 512;
export const ORACLE_PRICES_SIZE =
  PRICES_OFFSET + MAX_ENTRIES * DATED_PRICE_SIZE;

export type ScopeEntry = {
  index: number;
  value: bigint;
  exp: bigint;
  timestamp: number;
};

/**
 * Builds a scope `OraclePrices` buffer: discriminator, the oracle_mappings pubkey, then 512
 * `DatedPrice { value: u64, exp: u64, last_updated_slot: u64, unix_timestamp: u64, _pad: [u8;24] }`.
 * Unlisted entries stay all-zero, i.e. "never refreshed".
 */
export const makeScopePrices = (entries: ScopeEntry[]): Buffer => {
  const data = Buffer.alloc(ORACLE_PRICES_SIZE);
  ORACLE_PRICES_DISCRIMINATOR.copy(data, 0);
  for (const e of entries) {
    const off = PRICES_OFFSET + e.index * DATED_PRICE_SIZE;
    data.writeBigUInt64LE(e.value, off);
    data.writeBigUInt64LE(e.exp, off + 8);
    data.writeBigUInt64LE(BigInt(e.timestamp), off + 16); // last_updated_slot (unused)
    data.writeBigUInt64LE(BigInt(e.timestamp), off + 24);
  }
  return data;
};

/** Writes `data` into the test context as a feed account owned by `owner` (Scope by default). */
export const setScopeFeed = (
  ctx: ProgramTestContext,
  pubkey: PublicKey,
  data: Buffer,
  owner: PublicKey = SCOPE_PROGRAM
) =>
  ctx.setAccount(pubkey, {
    executable: false,
    owner,
    lamports: 1_000_000_000,
    data,
    rentEpoch: 0,
  });
