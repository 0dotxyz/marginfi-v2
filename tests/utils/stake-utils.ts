import {
  Keypair,
  Transaction,
  SystemProgram,
  StakeProgram,
  PublicKey,
  Connection,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { MockUser } from "./mocks";
import { BanksClient } from "solana-bankrun";
import { BN } from "@coral-xyz/anchor";

/**
 * Create a stake account for some user
 * @param user
 * @param amount - in SOL (lamports), in native decimals
 * @returns
 */
export const createStakeAccount = (user: MockUser, amount: number) => {
  const stakeAccount = Keypair.generate();
  const userPublicKey = user.wallet.publicKey;

  // Create a stake account and fund it with the specified amount of SOL
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: userPublicKey,
      newAccountPubkey: stakeAccount.publicKey,
      lamports: amount,
      space: StakeProgram.space, // Space required for a stake account
      programId: StakeProgram.programId,
    }),
    StakeProgram.initialize({
      stakePubkey: stakeAccount.publicKey,
      authorized: {
        staker: userPublicKey,
        withdrawer: userPublicKey,
      },
    })
  );

  return { createTx: tx, stakeAccountKeypair: stakeAccount };
};

/**
 * Delegate a stake account to a validator.
 * @param user - wallet signs
 * @param stakeAccount
 * @param validatorVoteAccount
 */
export const delegateStake = (
  user: MockUser,
  stakeAccount: PublicKey,
  validatorVoteAccount: PublicKey
) => {
  return StakeProgram.delegate({
    stakePubkey: stakeAccount,
    authorizedPubkey: user.wallet.publicKey,
    votePubkey: validatorVoteAccount,
  });
};

/**
 * Delegation information for a StakeAccount
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/stake.ts
 * */
export type Delegation = {
  voterPubkey: PublicKey;
  stake: bigint;
  activationEpoch: bigint;
  deactivationEpoch: bigint;
};

/**
 * Parsed content of an on-chain StakeAccount
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/stake.ts
 * */
export type StakeAccount = {
  discriminant: bigint;
  meta: {
    rentExemptReserve: bigint;
    authorized: {
      staker: PublicKey;
      withdrawer: PublicKey;
    };
    lockup: {
      unixTimestamp: bigint;
      epoch: bigint;
      custodian: PublicKey;
    };
  };
  stake: {
    delegation: {
      voterPubkey: PublicKey;
      stake: bigint;
      activationEpoch: bigint;
      deactivationEpoch: bigint;
    };
    creditsObserved: bigint;
  };
};

/**
 * Decode a StakeAccount from parsed account data.
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/stake.ts
 * */
export const getStakeAccount = function (data: Buffer): StakeAccount {
  let offset = 0;

  // Discriminant (4 bytes)
  const discriminant = data.readBigUInt64LE(offset);
  offset += 4;

  // Meta
  const rentExemptReserve = data.readBigUInt64LE(offset);
  offset += 8;

  // Authorized staker and withdrawer (2 public keys)
  const staker = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  const withdrawer = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;

  // Lockup: unixTimestamp, epoch, custodian
  const unixTimestamp = data.readBigUInt64LE(offset);
  offset += 8;
  const epoch = data.readBigUInt64LE(offset);
  offset += 8;
  const custodian = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;

  // Stake: Delegation
  const voterPubkey = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  const stake = data.readBigUInt64LE(offset);
  offset += 8;
  const activationEpoch = data.readBigUInt64LE(offset);
  offset += 8;
  const deactivationEpoch = data.readBigUInt64LE(offset);
  offset += 8;

  // Credits observed
  const creditsObserved = data.readBigUInt64LE(offset);

  // Return the parsed StakeAccount object
  return {
    discriminant,
    meta: {
      rentExemptReserve,
      authorized: {
        staker,
        withdrawer,
      },
      lockup: {
        unixTimestamp,
        epoch,
        custodian,
      },
    },
    stake: {
      delegation: {
        voterPubkey,
        stake,
        activationEpoch,
        deactivationEpoch,
      },
      creditsObserved,
    },
  };
};

/**
 * Parsed content of an on-chain Stake History Entry
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/stake.ts
 * */
export type StakeHistoryEntry = {
  epoch: bigint;
  effective: bigint;
  activating: bigint;
  deactivating: bigint;
};

/**
 * Decode a StakeHistoryEntry from parsed account data.
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/stake.ts
 * and modified to directly read from buffer
 * */
export const getStakeHistory = function (data: Buffer): StakeHistoryEntry[] {
  // Note: Is just `Vec<(Epoch, StakeHistoryEntry)>` internally
  const stakeHistory: StakeHistoryEntry[] = [];
  const entrySize = 32; // Each entry is 32 bytes (4 x 8-byte u64 fields)

  for (
    // skip the first 8 bytes for the Vec overhead
    let offset = 8;
    offset + entrySize < data.length;
    offset += entrySize
  ) {
    const epoch = data.readBigUInt64LE(offset); // Note `epoch` is just a u64 renamed
    const effective = data.readBigUInt64LE(offset + 8); // u64 effective
    const activating = data.readBigUInt64LE(offset + 16); // u64 activating
    const deactivating = data.readBigUInt64LE(offset + 24); // u64 deactivating

    // if (epoch < 10 && offset < 300) {
    //   console.log("epoch " + epoch);
    //   console.log("e " + effective);
    //   console.log("a " + activating);
    //   console.log("d " + deactivating);
    // }

    stakeHistory.push({
      epoch,
      effective,
      activating,
      deactivating,
    });
  }

  return stakeHistory;
};

/**
 * Representation of on-chain stake
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/delegation.ts
 */
export interface StakeActivatingAndDeactivating {
  effective: bigint;
  activating: bigint;
  deactivating: bigint;
}

/**
 * Representation of on-chain stake excluding deactivating stake
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/delegation.ts
 */
export interface EffectiveAndActivating {
  effective: bigint;
  activating: bigint;
}

/**
 * Get stake histories for a given epoch
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/delegation.ts
 */
function getStakeHistoryEntry(
  epoch: bigint,
  stakeHistory: StakeHistoryEntry[]
): StakeHistoryEntry | null {
  for (const entry of stakeHistory) {
    if (entry.epoch === epoch) {
      return entry;
    }
  }
  return null;
}

const WARMUP_COOLDOWN_RATE = 0.09;

/**
 * Get on-chain status of activating stake
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/delegation.ts
 */
export function getStakeAndActivating(
  delegation: Delegation,
  targetEpoch: bigint,
  stakeHistory: StakeHistoryEntry[]
): EffectiveAndActivating {
  if (delegation.activationEpoch === delegation.deactivationEpoch) {
    // activated but instantly deactivated; no stake at all regardless of target_epoch
    return {
      effective: BigInt(0),
      activating: BigInt(0),
    };
  } else if (targetEpoch === delegation.activationEpoch) {
    // all is activating
    return {
      effective: BigInt(0),
      activating: delegation.stake,
    };
  } else if (targetEpoch < delegation.activationEpoch) {
    // not yet enabled
    return {
      effective: BigInt(0),
      activating: BigInt(0),
    };
  }

  let currentEpoch = delegation.activationEpoch;
  let entry = getStakeHistoryEntry(currentEpoch, stakeHistory);
  if (entry !== null) {
    // target_epoch > self.activation_epoch

    // loop from my activation epoch until the target epoch summing up my entitlement
    // current effective stake is updated using its previous epoch's cluster stake
    let currentEffectiveStake = BigInt(0);
    while (entry !== null) {
      currentEpoch++;
      const remaining = delegation.stake - currentEffectiveStake;
      const weight = Number(remaining) / Number(entry.activating);
      const newlyEffectiveClusterStake =
        Number(entry.effective) * WARMUP_COOLDOWN_RATE;
      const newlyEffectiveStake = BigInt(
        Math.max(1, Math.round(weight * newlyEffectiveClusterStake))
      );

      currentEffectiveStake += newlyEffectiveStake;
      if (currentEffectiveStake >= delegation.stake) {
        currentEffectiveStake = delegation.stake;
        break;
      }

      if (
        currentEpoch >= targetEpoch ||
        currentEpoch >= delegation.deactivationEpoch
      ) {
        break;
      }
      entry = getStakeHistoryEntry(currentEpoch, stakeHistory);
    }
    return {
      effective: currentEffectiveStake,
      activating: delegation.stake - currentEffectiveStake,
    };
  } else {
    // no history or I've dropped out of history, so assume fully effective
    return {
      effective: delegation.stake,
      activating: BigInt(0),
    };
  }
}

/**
 * Get on-chain status of activating and deactivating stake
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/delegation.ts
 */
export function getStakeActivatingAndDeactivating(
  delegation: Delegation,
  targetEpoch: bigint,
  stakeHistory: StakeHistoryEntry[]
): StakeActivatingAndDeactivating {
  const { effective, activating } = getStakeAndActivating(
    delegation,
    targetEpoch,
    stakeHistory
  );

  // then de-activate some portion if necessary
  if (targetEpoch < delegation.deactivationEpoch) {
    return {
      effective,
      activating,
      deactivating: BigInt(0),
    };
  } else if (targetEpoch == delegation.deactivationEpoch) {
    // can only deactivate what's activated
    return {
      effective,
      activating: BigInt(0),
      deactivating: effective,
    };
  }
  let currentEpoch = delegation.deactivationEpoch;
  let entry = getStakeHistoryEntry(currentEpoch, stakeHistory);
  if (entry !== null) {
    // target_epoch > self.activation_epoch
    // loop from my deactivation epoch until the target epoch
    // current effective stake is updated using its previous epoch's cluster stake
    let currentEffectiveStake = effective;
    while (entry !== null) {
      currentEpoch++;
      // if there is no deactivating stake at prev epoch, we should have been
      // fully undelegated at this moment
      if (entry.deactivating === BigInt(0)) {
        break;
      }

      // I'm trying to get to zero, how much of the deactivation in stake
      //   this account is entitled to take
      const weight = Number(currentEffectiveStake) / Number(entry.deactivating);

      // portion of newly not-effective cluster stake I'm entitled to at current epoch
      const newlyNotEffectiveClusterStake =
        Number(entry.effective) * WARMUP_COOLDOWN_RATE;
      const newlyNotEffectiveStake = BigInt(
        Math.max(1, Math.round(weight * newlyNotEffectiveClusterStake))
      );

      currentEffectiveStake -= newlyNotEffectiveStake;
      if (currentEffectiveStake <= 0) {
        currentEffectiveStake = BigInt(0);
        break;
      }

      if (currentEpoch >= targetEpoch) {
        break;
      }
      entry = getStakeHistoryEntry(currentEpoch, stakeHistory);
    }

    // deactivating stake should equal to all of currently remaining effective stake
    return {
      effective: currentEffectiveStake,
      deactivating: currentEffectiveStake,
      activating: BigInt(0),
    };
  } else {
    return {
      effective: BigInt(0),
      activating: BigInt(0),
      deactivating: BigInt(0),
    };
  }
}

/**
 * Representation of on-chain stake
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/rpc.ts
 */
export interface StakeActivation {
  status: string;
  active: bigint;
  inactive: bigint;
}

/**
 * Get on-chain stake status of a stake account (activating, inactive, etc)
 *
 * Copied from https://github.com/solana-developers/solana-rpc-get-stake-activation/blob/main/web3js-1.0/src/rpc.ts
 */
export async function getStakeActivation(
  connection: Connection,
  stakeAddress: PublicKey,
  epoch: number | undefined = undefined // Added to bypass connection.getEpochInfo() when using a bankrun provider.
): Promise<StakeActivation> {
  const SYSVAR_STAKE_HISTORY_ADDRESS = new PublicKey(
    "SysvarStakeHistory1111111111111111111111111"
  );
  const epochInfoPromise =
    epoch !== undefined
      ? Promise.resolve({ epoch })
      : connection.getEpochInfo();
  const [epochInfo, { stakeAccount, stakeAccountLamports }, stakeHistory] =
    await Promise.all([
      epochInfoPromise,
      (async () => {
        const stakeAccountInfo = await connection.getAccountInfo(stakeAddress);
        if (stakeAccountInfo === null) {
          throw new Error("Account not found");
        }
        const stakeAccount = getStakeAccount(stakeAccountInfo.data);
        const stakeAccountLamports = stakeAccountInfo.lamports;
        return { stakeAccount, stakeAccountLamports };
      })(),
      (async () => {
        const stakeHistoryInfo = await connection.getAccountInfo(
          SYSVAR_STAKE_HISTORY_ADDRESS
        );
        if (stakeHistoryInfo === null) {
          throw new Error("StakeHistory not found");
        }
        return getStakeHistory(stakeHistoryInfo.data);
      })(),
    ]);

  const targetEpoch = epoch ? epoch : epochInfo.epoch;
  const { effective, activating, deactivating } =
    getStakeActivatingAndDeactivating(
      stakeAccount.stake.delegation,
      BigInt(targetEpoch),
      stakeHistory
    );

  let status;
  if (deactivating > 0) {
    status = "deactivating";
  } else if (activating > 0) {
    status = "activating";
  } else if (effective > 0) {
    status = "active";
  } else {
    status = "inactive";
  }
  const inactive =
    BigInt(stakeAccountLamports) -
    effective -
    stakeAccount.meta.rentExemptReserve;

  return {
    status,
    active: effective,
    inactive,
  };
}
