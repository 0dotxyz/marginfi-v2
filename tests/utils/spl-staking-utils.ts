import {
  findPoolMintAddress,
  findPoolStakeAuthorityAddress,
  SinglePoolInstruction,
} from "@solana/spl-single-pool-classic";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  Connection,
  PublicKey,
  StakeAuthorizationLayout,
  StakeProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { SINGLE_POOL_PROGRAM_ID } from "./types";
import { ProgramTestContext } from "solana-bankrun";

export enum SinglePoolAccountType {
  Uninitialized = 0,
  Pool = 1,
}

export type SinglePool = {
  accountType: SinglePoolAccountType;
  voteAccountAddress: PublicKey;
};

const decodeSinglePoolAccountType = (buffer: Buffer, offset: number) => {
  const accountType = buffer.readUInt8(offset);
  if (accountType === 0) {
    return SinglePoolAccountType.Uninitialized;
  } else if (accountType === 1) {
    return SinglePoolAccountType.Pool;
  } else {
    throw new Error("Unknown SinglePoolAccountType");
  }
};

/**
 * Decode an spl single pool from buffer.
 *
 * Get the data buffer with `const data = (await provider.connection.getAccountInfo(poolKey)).data;`
 * and note that there is no discriminator (i.e. pass data directly without additional slicing)
 */
export const decodeSinglePool = (buffer: Buffer) => {
  let offset = 0;

  const accountType = decodeSinglePoolAccountType(buffer, offset);
  offset += 1;

  const voteAccountAddress = new PublicKey(
    buffer.subarray(offset, offset + 32)
  );
  offset += 32;

  return {
    accountType,
    voteAccountAddress,
  };
};

// See `https://www.npmjs.com/package/@solana/spl-single-pool` transactions.ts for the original

/**
 * Builds ixes to create the LST ata as-needed, pass stake authority to the spl pool, and deposit to
 * the stake pool
 * @param connection
 * @param userWallet
 * @param splPool
 * @param userStakeAccount
 * @param verbose
 * @returns
 */
export const depositToSinglePoolIxes = async (
  connection: Connection,
  userWallet: PublicKey,
  splPool: PublicKey,
  userStakeAccount: PublicKey,
  verbose: boolean = false
) => {
  const splMint = await findPoolMintAddress(SINGLE_POOL_PROGRAM_ID, splPool);

  const splAuthority = await findPoolStakeAuthorityAddress(
    SINGLE_POOL_PROGRAM_ID,
    splPool
  );

  const ixes: TransactionInstruction[] = [];
  const lstAta = getAssociatedTokenAddressSync(splMint, userWallet);
  const ataInfo = await connection.getAccountInfo(lstAta);
  if (ataInfo !== null) {
    if (verbose) {
      console.log("Existing LST ata at: " + lstAta);
    }
  } else {
    if (verbose) {
      console.log("Failed to find ata, creating: " + lstAta);
    }
    ixes.push(
      createAssociatedTokenAccountInstruction(
        userWallet,
        lstAta,
        userWallet,
        splMint
      )
    );
  }

  const authorizeStakerIxes = StakeProgram.authorize({
    stakePubkey: userStakeAccount,
    authorizedPubkey: userWallet,
    newAuthorizedPubkey: splAuthority,
    stakeAuthorizationType: StakeAuthorizationLayout.Staker,
  }).instructions;

  ixes.push(...authorizeStakerIxes);

  const authorizeWithdrawIxes = StakeProgram.authorize({
    stakePubkey: userStakeAccount,
    authorizedPubkey: userWallet,
    newAuthorizedPubkey: splAuthority,
    stakeAuthorizationType: StakeAuthorizationLayout.Withdrawer,
  }).instructions;

  ixes.push(...authorizeWithdrawIxes);

  const depositIx = await SinglePoolInstruction.depositStake(
    splPool,
    userStakeAccount,
    lstAta,
    userWallet
  );

  ixes.push(depositIx);

  return ixes;
};

/**
 * Generally, use this instead of `bankrunContext.lastBlockhash` (which does not work if the test
 * has already run for some time and the blockhash has advanced)
 * @param bankrunContext
 * @returns
 */
export const getBankrunBlockhash = async (
  bankrunContext: ProgramTestContext
) => {
  return (await bankrunContext.banksClient.getLatestBlockhash())[0];
};
