import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { BanksClient, Clock, ProgramTestContext } from "./litesvm";
import { BN } from "@coral-xyz/anchor";

/**
 * A random dummy ix to trick bankrun.
 * @param from - must sign
 * @param to
 * @returns
 */
let dummyIxNonce = 1;

export function dummyIx(
  from: PublicKey,
  to: PublicKey,
): TransactionInstruction {
  const lamports = dummyIxNonce;
  dummyIxNonce += 1;
  if (dummyIxNonce >= Number.MAX_SAFE_INTEGER) {
    dummyIxNonce = 1;
  }

  let ix = SystemProgram.transfer({
    fromPubkey: from,
    toPubkey: to,
    lamports,
  });
  return ix;
}

const ONE_HOUR_IN_SECONDS = 60 * 60;
/**
 * Great for generating nominal interest
 * @param banksClient
 * @param bankrunContext
 */
export const advanceOneHour = async (
  banksClient: BanksClient,
  bankrunContext: ProgramTestContext,
) => {
  const before = await banksClient.getClock();
  bankrunContext.setClock(
    new Clock(
      before.slot,
      before.epochStartTimestamp,
      before.epoch,
      before.leaderScheduleEpoch,
      before.unixTimestamp + BigInt(ONE_HOUR_IN_SECONDS),
    ),
  );
};

const FIVE_MINUTES_IN_SECONDS = 5 * 60;
/**
 * Great for forcing oracles to be stale
 * @param banksClient
 * @param bankrunContext
 */
export const advanceFiveMinutes = async (
  banksClient: BanksClient,
  bankrunContext: ProgramTestContext,
) => {
  const before = await banksClient.getClock();
  bankrunContext.setClock(
    new Clock(
      before.slot,
      before.epochStartTimestamp,
      before.epoch,
      before.leaderScheduleEpoch,
      before.unixTimestamp + BigInt(FIVE_MINUTES_IN_SECONDS),
    ),
  );
};

export const getEpochAndSlot = async (banksClient: BanksClient) => {
  let clock = await banksClient.getAccount(SYSVAR_CLOCK_PUBKEY);

  // Slot is bytes 0-8
  let slot = new BN(clock.data.slice(0, 8), 10, "le").toNumber();

  // Epoch is bytes 16-24
  let epoch = new BN(clock.data.slice(16, 24), 10, "le").toNumber();

  return { epoch, slot };
};
