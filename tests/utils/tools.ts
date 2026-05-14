import { BN, IdlAccounts } from "@coral-xyz/anchor";
import { inspect } from "util";
import {
  BanksTransactionMeta,
  BanksTransactionResultWithMeta,
} from "solana-bankrun";
import { ProgramTestContext } from "solana-bankrun";
import BigNumber from "bignumber.js";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  Transaction,
  TransactionInstruction,
  PublicKey,
  AccountInfo,
  VersionedTransaction,
} from "@solana/web3.js";
import { Keypair } from "@solana/web3.js";
import { MarginfiAccountRaw } from "@mrgnlabs/marginfi-client-v2";
import {
  TOKEN_PROGRAM_ID,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  ASSET_TAG_DRIFT,
  ASSET_TAG_JUPLEND,
  ASSET_TAG_KAMINO,
  ASSET_TAG_SOLEND,
  ASSET_TAG_STAKED,
  HEALTH_CACHE_HEALTHY,
} from "./types";
import { composeRemainingAccounts } from "./user-instructions";
import {
  globalProgramAdmin,
  bankrunContext,
  bankrunProgram,
  banksClient,
} from "../rootHooks";
import { getEpochAndSlot } from "./bankrunConnection";
import { createMintToInstruction } from "@solana/spl-token";
import { BankrunProvider } from "anchor-bankrun";
import { Marginfi } from "target/types/marginfi";

/**
 * Convert a human-readable amount to native token units based on decimals.
 * @param amount - The human-readable amount
 * @param decimals - The number of decimals for the token
 * @returns The amount in native units as a BN
 */
export const toNative = (amount: number, decimals: number): BN =>
  new BN(amount).mul(new BN(10).pow(new BN(decimals)));

/**
 * Process a signed transaction in a bankrun context and return the transaction result. This
 * internal helper is shared between legacy and v0 wrappers. In edge cases (particularly with v0)
 * bankrun can't pick up logs on failure.
 */
function processSignedBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction | VersionedTransaction,
  trySend: true,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionResultWithMeta>;
function processSignedBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction | VersionedTransaction,
  trySend?: false,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionMeta>;
function processSignedBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction | VersionedTransaction,
  trySend: boolean,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta>;
async function processSignedBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction | VersionedTransaction,
  trySend: boolean = false,
  dumpLogOnFail: boolean = false,
): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta> {
  const bankrunTx = tx;

  if (trySend) {
    const result = await bankrunContext.banksClient.tryProcessTransaction(
      bankrunTx,
    );
    if (dumpLogOnFail && result.result) {
      const dumped = dumpBankrunLogs(result);
      if (!dumped) {
        try {
          const simResult =
            await bankrunContext.banksClient.simulateTransaction(bankrunTx);
          dumpBankrunLogs(simResult);
        } catch (diagnosticError) {
          console.log(
            "[bankrun] trySend fallback simulateTransaction failed:",
            diagnosticError,
          );
        }
      }
    }
    return result;
  }

  try {
    return await bankrunContext.banksClient.processTransaction(bankrunTx);
  } catch (error) {
    // If processing throws, re-simulate for diagnostics without mutating state.
    if (dumpLogOnFail) {
      console.log(
        "[bankrun] processTransaction threw:",
        error instanceof Error ? error.message : error,
      );
      let dumped = false;
      try {
        const simulatedResult =
          await bankrunContext.banksClient.simulateTransaction(bankrunTx);
        dumped = dumpBankrunLogs(simulatedResult);
      } catch (diagnosticError) {
        console.log(
          "[bankrun] simulateTransaction for diagnostics failed:",
          diagnosticError,
        );
      }
      if (!dumped) {
        try {
          const tryResult =
            await bankrunContext.banksClient.tryProcessTransaction(bankrunTx);
          dumpBankrunLogs(tryResult);
        } catch (diagnosticError) {
          console.log(
            "[bankrun] tryProcessTransaction for diagnostics failed:",
            diagnosticError,
          );
        }
      }
    }
    throw error;
  }
}

/**
 * Process a legacy transaction in a bankrun context and return the transaction result.
 * @param bankrunContext - The bankrun context
 * @param tx - The legacy transaction to process
 * @param signers - The signers for the transaction
 * @param trySend - true to use tryProcess instead
 * @param dumpLogOnFail - true to print a tx log on fail
 * @returns The transaction result with metadata
 */
export function processBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction,
  signers: Keypair[],
  trySend: true,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionResultWithMeta>;
export function processBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction,
  signers: Keypair[],
  trySend?: false,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionMeta>;
export function processBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction,
  signers: Keypair[],
  trySend?: boolean,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionMeta>;
export async function processBankrunTransaction(
  bankrunContext: ProgramTestContext,
  tx: Transaction,
  signers: Keypair[],
  trySend: boolean = false,
  dumpLogOnFail: boolean = false,
): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta> {
  tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
  tx.sign(...signers);
  return processSignedBankrunTransaction(
    bankrunContext,
    tx,
    trySend,
    dumpLogOnFail,
  );
}

/**
 * Process a v0 transaction in a bankrun context and return the transaction result.
 * @param bankrunContext - The bankrun context
 * @param tx - The v0 transaction to process
 * @param signers - The signers for the transaction
 * @param trySend - true to use tryProcess instead
 * @param dumpLogOnFail - true to print a tx log on fail
 * @returns The transaction result with metadata
 */
export function processBankrunV0Transaction(
  bankrunContext: ProgramTestContext,
  tx: VersionedTransaction,
  signers: Keypair[],
  trySend: true,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionResultWithMeta>;
export function processBankrunV0Transaction(
  bankrunContext: ProgramTestContext,
  tx: VersionedTransaction,
  signers: Keypair[],
  trySend?: false,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionMeta>;
export function processBankrunV0Transaction(
  bankrunContext: ProgramTestContext,
  tx: VersionedTransaction,
  signers: Keypair[],
  trySend?: boolean,
  dumpLogOnFail?: boolean,
): Promise<BanksTransactionMeta>;
export async function processBankrunV0Transaction(
  bankrunContext: ProgramTestContext,
  tx: VersionedTransaction,
  signers: Keypair[],
  trySend: boolean = false,
  dumpLogOnFail: boolean = false,
): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta> {
  if (signers.length > 0) {
    tx.sign(signers);
  }
  return processSignedBankrunTransaction(
    bankrunContext,
    tx,
    trySend,
    dumpLogOnFail,
  );
}

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
  skipEmptyRows: boolean = true,
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

const readField = (obj: unknown, field: string): unknown => {
  if (!obj || typeof obj !== "object") return undefined;
  try {
    return (obj as Record<string, unknown>)[field];
  } catch {
    return undefined;
  }
};

const findFieldDeep = (obj: unknown, field: string, maxDepth = 5): unknown => {
  let current: unknown = obj;
  for (let depth = 0; depth <= maxDepth; depth += 1) {
    const value = readField(current, field);
    if (value !== undefined) return value;
    current = readField(current, "inner");
    if (!current) break;
  }
  return undefined;
};

export const dumpBankrunLogs = (result: any): boolean => {
  const logMessages =
    findFieldDeep(result?.meta, "logMessages") ??
    findFieldDeep(result, "logMessages");
  if (Array.isArray(logMessages) && logMessages.length > 0) {
    for (let i = 0; i < logMessages.length; i++) {
      console.log(i + " " + logMessages[i]);
    }
    return true;
  }

  const txResult = findFieldDeep(result, "result");
  const computeUnitsConsumed =
    findFieldDeep(result?.meta, "computeUnitsConsumed") ??
    findFieldDeep(result, "computeUnitsConsumed");
  const status = txResult === null || txResult === undefined ? "ok" : "failed";
  console.log(
    `[bankrun] no logMessages available (status=${status}, meta=${
      result?.meta === null ? "null" : typeof result?.meta
    })`,
  );
  if (txResult !== undefined) {
    console.log("[bankrun] tx result:", txResult);
  }
  if (computeUnitsConsumed !== undefined) {
    console.log("[bankrun] CU consumed:", computeUnitsConsumed.toString());
  }
  console.log(
    "[bankrun] raw result preview:",
    inspect(result, { depth: 4, colors: false, maxArrayLength: 50 }),
  );
  return false;
};

/**
 * Create and activate a lookup table for the provided instruction set.
 *
 * This is useful for tests that need deterministic v0 execution without relying
 * on a shared LUT that can drift as test state evolves.
 */
export const createLookupTableForInstructions = async (
  signer: Keypair,
  instructions: TransactionInstruction[],
): Promise<AddressLookupTableAccount> => {
  const addresses: PublicKey[] = [];
  const seen = new Set<string>();
  const push = (pk: PublicKey) => {
    const key = pk.toBase58();
    if (seen.has(key)) return;
    seen.add(key);
    addresses.push(pk);
  };

  for (const ix of instructions) {
    push(ix.programId);
    for (const meta of ix.keys) {
      push(meta.pubkey);
    }
  }

  return createLut(signer, addresses);
};

export const createLut = async (
  signer: Keypair,
  addresses: PublicKey[],
): Promise<AddressLookupTableAccount> => {
  const recentSlot = Number(await banksClient.getSlot());
  const [createLutIx, lutAddress] = AddressLookupTableProgram.createLookupTable(
    {
      authority: signer.publicKey,
      payer: signer.publicKey,
      recentSlot: Math.max(0, recentSlot - 1),
    },
  );

  const createLutTx = new Transaction().add(createLutIx);
  createLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
  createLutTx.sign(signer);
  await banksClient.processTransaction(createLutTx);

  const CHUNK_SIZE = 20;
  for (let i = 0; i < addresses.length; i += CHUNK_SIZE) {
    const extendTx = new Transaction().add(
      AddressLookupTableProgram.extendLookupTable({
        authority: signer.publicKey,
        payer: signer.publicKey,
        lookupTable: lutAddress,
        addresses: addresses.slice(i, i + CHUNK_SIZE),
      }),
    );
    extendTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    extendTx.sign(signer);
    await banksClient.processTransaction(extendTx);
  }

  // allow LUT to activate
  const { slot } = await getEpochAndSlot(banksClient);
  bankrunContext.warpToSlot(BigInt(slot + 25));

  const lutRaw = await banksClient.getAccount(lutAddress);
  const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
  return new AddressLookupTableAccount({
    key: lutAddress,
    state: lutState,
  });
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
  publicKey: PublicKey,
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
  exponent: number,
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
  bankValueMap = {},
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

type MarginfiAccount = IdlAccounts<Marginfi>["marginfiAccount"];
type MarginfiHealthCache = MarginfiAccount["healthCache"];

export const logHealthCache = (
  header: string,
  healthCache: MarginfiHealthCache,
) => {
  const av = wrappedI80F48toBigNumber(healthCache.assetValue);
  const lv = wrappedI80F48toBigNumber(healthCache.liabilityValue);
  const aValMaint = wrappedI80F48toBigNumber(healthCache.assetValueMaint);
  const lValMaint = wrappedI80F48toBigNumber(healthCache.liabilityValueMaint);
  const aValEqui = wrappedI80F48toBigNumber(healthCache.assetValueEquity);
  const lValEqui = wrappedI80F48toBigNumber(healthCache.liabilityValueEquity);
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
  console.log("asset value (equity): " + aValEqui.toString());
  console.log("liab value (equity): " + lValEqui.toString());
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

/** Shorthand to convert an I80F48 to BN (rounding off decimals) */
export const toBnFromI80 = (value: any): BN =>
  new BN(wrappedI80F48toBigNumber(value).toFixed(0));

/** Shorthand to cast BN/number/bigint as BN */
export const toBn = (value: BN | number | bigint) => {
  if (typeof value === "bigint") return new BN(value.toString());
  if (typeof value === "number") return new BN(value);
  return value;
};

/**
 * Returns the user's active asset shares for a given bank as raw BigNumber precision.
 * Returns zero when no active balance exists for that bank.
 */
export const getUserAssetShares = async (
  marginfiAccountPk: PublicKey,
  bankPk: PublicKey,
): Promise<BigNumber> => {
  const marginfiAccount = await bankrunProgram.account.marginfiAccount.fetch(
    marginfiAccountPk,
  );
  const userBalance = marginfiAccount.lendingAccount.balances.find(
    (b: any) => b.active && b.bankPk.equals(bankPk),
  );
  return userBalance
    ? wrappedI80F48toBigNumber(userBalance.assetShares)
    : new BigNumber(0);
};

/** Shorthand to mint some tokens to a destination, globalProgramAdmin always sign/sends */
export const mintToTokenAccount = async (
  mint: PublicKey,
  destination: PublicKey,
  amount: BN,
) => {
  const ix = createMintToInstruction(
    mint,
    destination,
    globalProgramAdmin.wallet.publicKey,
    BigInt(amount.toString()),
    [],
    TOKEN_PROGRAM_ID,
  );

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [globalProgramAdmin.wallet],
    false,
    true,
  );
};

/**
 * Useful when building risk accounts in tests where hand-construction is tedious or not neccessary
 * for the flow being demonstrated.
 * @param marginfiAccountPk
 * @param excludedBankPks
 * @param includedBankPks
 * @returns
 */
export const buildHealthRemainingAccounts = async (
  marginfiAccountPk: PublicKey,
  options: {
    excludedBankPks?: PublicKey[];
    includedBankPks?: PublicKey[];
  } = {},
): Promise<PublicKey[]> => {
  const excludedBankPks = options.excludedBankPks ?? [];
  const includedBankPks = options.includedBankPks ?? [];

  const marginfiAccount = await bankrunProgram.account.marginfiAccount.fetch(
    marginfiAccountPk,
  );
  const activeBankPks = marginfiAccount.lendingAccount.balances
    .filter((b: any) => b.active)
    .filter(
      (b: any) =>
        !excludedBankPks.some((excludedPk) => excludedPk.equals(b.bankPk)),
    )
    .map((b: any) => b.bankPk as PublicKey);

  const bankPks: PublicKey[] = [...activeBankPks];
  for (const bankPk of includedBankPks) {
    if (excludedBankPks.some((excludedPk) => excludedPk.equals(bankPk))) {
      continue;
    }
    if (!bankPks.some((existingBankPk) => existingBankPk.equals(bankPk))) {
      bankPks.push(bankPk);
    }
  }

  const banks = await Promise.all(
    bankPks.map((bankPk) => bankrunProgram.account.bank.fetch(bankPk)),
  );

  const groups: PublicKey[][] = [];
  for (let i = 0; i < bankPks.length; i++) {
    const bankPk = bankPks[i];
    const bank = banks[i];
    const assetTag = bank.config.assetTag;
    const group: PublicKey[] = [bankPk, bank.config.oracleKeys[0]];

    if (
      assetTag === ASSET_TAG_KAMINO ||
      assetTag === ASSET_TAG_DRIFT ||
      assetTag === ASSET_TAG_SOLEND ||
      assetTag === ASSET_TAG_JUPLEND
    ) {
      group.push(bank.config.oracleKeys[1]);
    }

    if (assetTag === ASSET_TAG_STAKED) {
      group.push(bank.config.oracleKeys[1], bank.config.oracleKeys[2]);
    }

    groups.push(group);
  }

  return composeRemainingAccounts(groups);
};

/**
 * Advance the bankrun clock by a specified number of seconds.
 *
 * @param ctx - The bankrun ProgramTestContext
 * @param seconds - Number of seconds to advance the clock
 * @returns The new unix timestamp after advancing
 */
export async function advanceBankrunClock(
  ctx: ProgramTestContext,
  seconds: number,
): Promise<number> {
  const { Clock } = await import("solana-bankrun");
  const clock = await ctx.banksClient.getClock();
  const newClock = new Clock(
    clock.slot + BigInt(1),
    clock.epochStartTimestamp,
    clock.epoch,
    clock.leaderScheduleEpoch,
    clock.unixTimestamp + BigInt(seconds),
  );
  ctx.setClock(newClock);
  return Number(newClock.unixTimestamp);
}

/**
 * Generally, use this instead of `bankrunContext.lastBlockhash` (which does not work if the test
 * has already run for some time and the blockhash has advanced)
 * @param bankrunContext
 * @returns
 */
export const getBankrunBlockhash = async (
  bankrunContext: ProgramTestContext,
) => {
  return (await bankrunContext.banksClient.getLatestBlockhash())[0];
};

const EMISSIONS_MINT_OFFSET = 864 + 8;

/**
 * Directly set an emissions mint on a bank account and return the previous mint
 */
export async function setEmissionsDirect(
  provider: BankrunProvider,
  bank: PublicKey,
  emissionsMint: PublicKey,
): Promise<PublicKey> {
  const existing = await provider.context.banksClient.getAccount(bank);
  if (!existing) throw new Error("Bank account not found in bankrun");

  const buf = Buffer.from(existing.data);
  const prevMint = new PublicKey(
    buf.subarray(EMISSIONS_MINT_OFFSET, EMISSIONS_MINT_OFFSET + 32),
  );

  const emissionsSlice = emissionsMint.toBuffer();
  emissionsSlice.copy(buf, EMISSIONS_MINT_OFFSET);

  provider.context.setAccount(bank, {
    ...existing,
    data: buf,
  });

  return prevMint;
}
