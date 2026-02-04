import {
  BanksTransactionMeta,
  BanksTransactionResultWithMeta,
} from "solana-bankrun";
import { ProgramTestContext } from "solana-bankrun";
import { Transaction, PublicKey, AccountInfo } from "@solana/web3.js";
import { Keypair } from "@solana/web3.js";
import { AccountLayout } from "@solana/spl-token";
import { getBankrunBlockhash } from "./spl-staking-utils";
import { MarginfiAccountRaw } from "@mrgnlabs/marginfi-client-v2";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { HEALTH_CACHE_HEALTHY } from "./types";

// TODO: Add these as options instead of args
interface ProcessBankrunTransactionOptions {
  trySend?: boolean;
  dumpLogOnFail?: boolean;
}

/**
 * Process a transaction in a bankrun context and return the transaction result
 * @param bankrunContext - The bankrun context
 * @param tx - The transaction to process
 * @param signers - The signers for the transaction
 * @param trySend - true to use tryProcess instead
 * @param dumpLogOnFail - true to print a tx log on fail
 * @returns The transaction result with metadata
 */
export const processBankrunTransaction = async (
  bankrunContext: ProgramTestContext,
  tx: Transaction,
  signers: Keypair[],
  trySend: boolean = false,
  dumpLogOnFail: boolean = false
  //   options: ProcessBankrunTransactionOptions = {}
): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta> => {
  // const { trySend = false, dumpLogOnFail = false } = options;
  tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
  tx.sign(...signers);

  if (trySend) {
    let result = await bankrunContext.banksClient.tryProcessTransaction(tx);
    if (dumpLogOnFail) {
      dumpBankrunLogs(result);
    }
    return result;
  } else {
    // TODO throw on error?
    // If we want to dump logs on fail, simulate first
    if (dumpLogOnFail) {
      const simulationResult = await bankrunContext.banksClient.simulateTransaction(tx);
      if (simulationResult.result) {
        dumpBankrunLogs(simulationResult);
      }
    }
    return await bankrunContext.banksClient.processTransaction(tx);
  }

};

/**
 * Function to print bytes from a Buffer in groups with column labels and color highlighting for non-zero values
 * @param buffer - The Buffer to process
 * @param groupLength - The number of bytes in each group, usually 8 or 16
 * @param totalLength - The total number of bytes to process
 * @param skipEmptyRows - If a row is all-zero, it will not print
 */
export const printBufferGroups = (
  buffer: Buffer,
  groupLength: number,
  totalLength: number,
  skipEmptyRows: boolean = true
) => {
  // Print the column headers
  let columnHeader = "        |";
  for (let col = 0; col < groupLength; col++) {
    if (col < groupLength - 1) {
      columnHeader += col.toString().padStart(3, " ").padEnd(6, " ");
    } else {
      // No end padding for the last column
      columnHeader += col.toString().padStart(3, " ");
    }
  }
  console.log(columnHeader);

  // Function to calculate RGB color based on row index
  const calculateGradientColor = (startIndex) => {
    const maxIndex = 255 * 3;
    const normalizedIndex = startIndex % maxIndex;

    let r = 0,
      g = 0,
      b = 0;

    if (normalizedIndex < 255) {
      b = 255;
      g = normalizedIndex;
    } else if (normalizedIndex < 510) {
      g = 255;
      b = 510 - normalizedIndex;
    } else {
      g = 765 - normalizedIndex;
      r = normalizedIndex - 510;
    }

    return `\x1b[38;2;${r};${g};${b}m`;
  };

  // Print the buffer content
  for (let i = 0; i < totalLength; i += groupLength) {
    let group = [];
    let allZero = true;

    for (let j = 0; j < groupLength; j++) {
      let value = buffer[i + j];
      let valueStr =
        value !== undefined ? value.toString().padStart(3, " ") : "   ";
      if (value !== 0) {
        allZero = false;
      }
      if (value !== 0 && value !== undefined) {
        // Apply red color to non-zero values
        group.push(`\x1b[31m${valueStr}\x1b[0m`);
      } else {
        group.push(valueStr);
      }
    }

    // Skip printing if the entire group is zero
    if (!allZero || !skipEmptyRows) {
      const color = calculateGradientColor(i);
      const label = `${i.toString().padStart(3, " ")}-${(i + groupLength - 1)
        .toString()
        .padStart(3, " ")}`;
      console.log(`${color}${label}\x1b[0m | ${group.join(" | ")}`);
    }
  }
};

export const dumpBankrunLogs = (result: BanksTransactionResultWithMeta) => {
  for (let i = 0; i < result.meta.logMessages.length; i++) {
    console.log(i + " " + result.meta.logMessages[i]);
  }
};

/**
 * Decode an f64 from 8 bytes (little‑endian).
 * @param bytes - either a Uint8Array or number[] of length 8
 * @returns the decoded number
 */
export function bytesToF64(bytes: Uint8Array | number[]): number {
  // Normalize to a Uint8Array
  const u8: Uint8Array =
    bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);

  if (u8.length !== 8) {
    throw new Error(`Invalid length ${u8.length}, expected exactly 8 bytes`);
  }

  // Create a DataView on the buffer (little‑endian)
  const dv = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
  return dv.getFloat64(0, /* littleEndian */ true);
}

/**
 * Safely get account info from BankrunConnectionProxy, handling "Could not find" errors
 *
 * BankrunConnectionProxy throws an error when an account doesn't exist, unlike a real RPC
 * which returns null. This helper catches the specific error and returns null instead.
 *
 * @param connection - The connection to use
 * @param publicKey - The public key to look up
 * @returns The account info or null if it doesn't exist
 */
export const safeGetAccountInfo = async (
  connection: any,
  publicKey: PublicKey
): Promise<AccountInfo<Buffer> | null> => {
  try {
    return await connection.getAccountInfo(publicKey);
  } catch (e: any) {
    // Bank-Run throws on completely unknown accounts
    if (e?.message?.startsWith("Could not find")) return null;
    throw e; // genuine error – re-throw
  }
};

/**
 * Format a price value with the correct decimal places
 * @param price - The raw price value from the oracle
 * @param exponent - The exponent value from the oracle
 * @returns The formatted price as a string
 */
export function formatPriceWithDecimals(
  price: bigint,
  exponent: number
): string {
  const powerFactor = Math.pow(10, Math.abs(exponent));
  const priceNumber = Number(price);

  if (exponent < 0) {
    // Negative exponent means we divide (e.g., price * 10^-6)
    return (priceNumber / powerFactor).toFixed(Math.abs(exponent));
  } else {
    // Positive exponent means we multiply (e.g., price * 10^3)
    return (priceNumber * powerFactor).toString();
  }
}

/**
 * Print account balances in a pretty table. If you're getting a type error here, due to a different
 * client version. feel free to ts-ignore it.
 */
export function dumpAccBalances(
  account: MarginfiAccountRaw,
  bankValueMap = {}
) {
  const balances = account.lendingAccount.balances;
  const activeBalances = [];

  function fmt(num) {
    const s = parseFloat(num).toFixed(4);
    return s === "0.0000" ? "-" : s;
  }

  for (let b of balances) {
    if (b.active == 0) {

      activeBalances.push({
        "Bank PK": "empty",
        // Tag: "-",
        "Liab Shares": "-",
        "Liab Value": "-",
        "Asset Shares": "-",
        "Asset Value": "-",
      });
      continue;
    }

    const pk = b.bankPk.toString();
    const liabS = wrappedI80F48toBigNumber(b.liabilityShares).toNumber();
    const assetS = wrappedI80F48toBigNumber(b.assetShares).toNumber();

    // lookup per-share values; default to zero if omitted
    const { liability: perLiab = 0, asset: perAsset = 0 } =
      bankValueMap[pk] || {};

    activeBalances.push({
      "Bank PK": pk,
      "Liab Shares": fmt(liabS),
      "Liab Value": fmt(liabS * perLiab),
      "Asset Shares": fmt(assetS),
      "Asset Value": fmt(assetS * perAsset),
    });
  }

  console.table(activeBalances);
}

export const logHealthCache = (header: string, healthCache: any) => {
  const av = wrappedI80F48toBigNumber(healthCache.assetValue);
  const lv = wrappedI80F48toBigNumber(healthCache.liabilityValue);
  const aValMaint = wrappedI80F48toBigNumber(healthCache.assetValueMaint);
  const lValMaint = wrappedI80F48toBigNumber(healthCache.liabilityValueMaint);
  console.log(`---${header}---`);
  if (healthCache.flags & HEALTH_CACHE_HEALTHY) {
    console.log("**HEALTHY**");
  } else {
    console.log("**UNHEALTHY OR INVALID**");
  }
  console.log("asset value: " + av.toString());
  console.log("liab value: " + lv.toString());
  console.log("asset value (maint): " + aValMaint.toString());
  console.log("liab value (maint): " + lValMaint.toString());
  console.log("prices: ");
  healthCache.prices.forEach((priceWrapped: any, i: number) => {
    const price = bytesToF64(priceWrapped);
    if (price !== 0) {
      console.log(` [${i}] ${price}`);
    }
  });
  if (healthCache.mrgnErr != 0 || healthCache.internalErr != 0) {
    console.log("err: " + healthCache.mrgnErr);
    console.log("internal err: " + healthCache.internalErr);
    console.log("internal liq err: " + healthCache.internalLiqErr);
    console.log("err index: " + healthCache.errIndex);
  }
  console.log("");
};

/**
 * Exclude any fields containing the word "padding" from an object. Useful when printing a struct with JSON.stringify.
 * @param obj
 * @returns
 */
export function omitPadding(obj: any) {
  if (Array.isArray(obj)) {
    return obj.map(omitPadding);
  } else if (obj && typeof obj === "object") {
    return Object.entries(obj).reduce((clean, [key, val]) => {
      if (key.toLowerCase().includes("padding")) return clean;
      clean[key] = omitPadding(val);
      return clean;
    }, {});
  }
  return obj;
}

/**
 * Get the current bankrun blockchain time in seconds.
 * Use this instead of Date.now()/1000 to avoid clock contamination issues
 * when tests advance the clock using setClock/warpToSlot.
 * @param ctx - The bankrun ProgramTestContext
 * @returns The current unix timestamp from the bankrun clock
 */
export async function getBankrunTime(ctx: ProgramTestContext): Promise<number> {
  const clock = await ctx.banksClient.getClock();
  return Number(clock.unixTimestamp);
}

/**
 * Advance the bankrun clock by a specified number of seconds.
 * This is useful for testing time-dependent features like rate limiting,
 * emissions, and interest accrual.
 *
 * @param ctx - The bankrun ProgramTestContext
 * @param seconds - Number of seconds to advance the clock
 * @returns The new unix timestamp after advancing
 */
export async function advanceBankrunClock(
  ctx: ProgramTestContext,
  seconds: number
): Promise<number> {
  const { Clock } = await import("solana-bankrun");
  const clock = await ctx.banksClient.getClock();
  const newClock = new Clock(
    clock.slot + BigInt(1),
    clock.epochStartTimestamp,
    clock.epoch,
    clock.leaderScheduleEpoch,
    clock.unixTimestamp + BigInt(seconds)
  );
  ctx.setClock(newClock);
  return Number(newClock.unixTimestamp);
}
