import {
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { assert } from "chai";
import {
  BANK_V1_ACCOUNT_LEN,
  bankrunContext,
  banksClient,
  groupAdmin,
  oracles,
} from "../rootHooks";
import { refreshPullOraclesBankrun } from "./bankrun-oracles";
import { pulseBankPrice } from "./user-instructions";

/** Match `BANK_RATE_READINGS`. `Bank.rate_readings` starts where the v1 layout ends. */
export const BANK_RATE_READINGS = 17;
/** One rate reading: asset index, debt index, timestamp. */
export const RATE_READING_LEN = 24;
/** Match `INTEREST_MIN_WINDOW_SECONDS` and `INTEREST_MAX_WINDOW_SECONDS`. */
export const INTEREST_MIN_WINDOW_SECONDS = 21_600;
export const INTEREST_MAX_WINDOW_SECONDS = 172_800;
const SECONDS_PER_YEAR = 31_536_000n;

const rewriteBank = async (bank: PublicKey, mutate: (data: Buffer) => void) => {
  const account = await banksClient.getAccount(bank);
  const data = Buffer.from(account.data);
  mutate(data);
  bankrunContext.setAccount(bank, { ...account, data });
};

/**
 * Start `bank`'s rate history over: its readings are cleared and a pulse, run after `cranks`,
 * records the one reading it then holds.
 */
export const restartRateHistory = async (
  bank: PublicKey,
  oracleAccounts: PublicKey[],
  cranks: TransactionInstruction[] = [],
) => {
  const end = BANK_V1_ACCOUNT_LEN + BANK_RATE_READINGS * RATE_READING_LEN;
  await rewriteBank(bank, (data) => data.fill(0, BANK_V1_ACCOUNT_LEN, end));
  await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  await groupAdmin.mrgnProgram.provider.sendAndConfirm(
    new Transaction().add(
      ...cranks,
      await pulseBankPrice(groupAdmin.mrgnProgram, {
        bank,
        remaining: oracleAccounts,
      }),
    ),
  );
};

/**
 * Give `bank` the rate history of having paid `aprBps` for a full max window: one reading that
 * old, at the asset index a pulse records now scaled back by that growth.
 */
export const seedRateHistory = async (
  bank: PublicKey,
  oracleAccounts: PublicKey[],
  aprBps: bigint,
  cranks: TransactionInstruction[] = [],
) => {
  await restartRateHistory(bank, oracleAccounts, cranks);
  const now = (await banksClient.getClock()).unixTimestamp;
  const window = BigInt(INTEREST_MAX_WINDOW_SECONDS);
  // The ring was empty, so the pulse's reading sits in the first slot.
  await rewriteBank(bank, (data) => {
    const indexNow = data.readBigUInt64LE(BANK_V1_ACCOUNT_LEN);
    const takenAt = data.readBigInt64LE(BANK_V1_ACCOUNT_LEN + 16);
    assert.equal(takenAt, now, "pulse recorded a reading");
    const year = 10_000n * SECONDS_PER_YEAR;
    data.writeBigUInt64LE(
      (indexNow * year) / (year + aprBps * window),
      BANK_V1_ACCOUNT_LEN,
    );
    data.writeBigInt64LE(takenAt - window, BANK_V1_ACCOUNT_LEN + 16);
  });
};
