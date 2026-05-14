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
  STAKE_CONFIG_ID,
  StakeAuthorizationLayout,
  StakeProgram,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_STAKE_HISTORY_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import { SINGLE_POOL_PROGRAM_ID } from "./types";
import { ProgramTestContext } from "solana-bankrun";
import {
  deriveSVSPpool,
  deriveOnRampPool,
  deriveStakeAuthority,
  deriveStakePool,
} from "./pdas";

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
    buffer.subarray(offset, offset + 32),
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
  verbose: boolean = false,
) => {
  const splMint = await findPoolMintAddress(SINGLE_POOL_PROGRAM_ID, splPool);

  const splAuthority = await findPoolStakeAuthorityAddress(
    SINGLE_POOL_PROGRAM_ID,
    splPool,
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
        splMint,
      ),
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
    userWallet,
  );

  ixes.push(depositIx);

  return ixes;
};

/**
 * Spl Single Pool's CreateOnRamp instruction.
 *
 * Accounts (in order):
 *
 * * 0. `[]` Pool account
 * * 1. `[w]` Pool onramp account
 * * 2. `[]` Pool stake authority
 * * 3. `[]` Rent sysvar
 * * 4. `[]` System program
 * * 5. `[]` Stake program
 *
 * @param voteAccount - Validator's vote account
 *
 * @returns A TransactionInstruction
 */
export function createPoolOnramp(
  voteAccount: PublicKey,
): TransactionInstruction {
  const [poolAccount] = deriveSVSPpool(voteAccount);
  const [onRampAccount] = deriveOnRampPool(poolAccount);
  const [poolStakeAuthority] = deriveStakeAuthority(poolAccount);

  const keys = [
    { pubkey: poolAccount, isSigner: false, isWritable: false },
    { pubkey: onRampAccount, isSigner: false, isWritable: true },
    { pubkey: poolStakeAuthority, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
  ];

  // Note: Hard-coded, if foundation ever changes the ix index, then woops.
  const data = Buffer.from(Uint8Array.of(6));

  return new TransactionInstruction({
    keys,
    programId: SINGLE_POOL_PROGRAM_ID,
    data,
  });
}

/**
 * Spl Single Pool's CreateOnRamp instruction.
 *
 * Accounts (in order):
 *
 * * 0. `[]` Validator vote account
 * * 1. `[]` Pool account
 * * 2. `[w]` Pool stake account
 * * 3. `[w]` Pool onramp account
 * * 4. `[]` Pool stake authority
 * * 5. `[]` Clock sysvar
 * * 6. `[]` Stake history sysvar
 * * 7. `[]` Stake config sysvar
 * * 8. `[]` Stake program
 *
 * @param voteAccount - Validator's vote account
 *
 * @returns A TransactionInstruction
 */
export function replenishPool(voteAccount: PublicKey): TransactionInstruction {
  const [poolAccount] = deriveSVSPpool(voteAccount);
  const [stakePool] = deriveStakePool(poolAccount);
  const [onRampPool] = deriveOnRampPool(poolAccount);
  const [authority] = deriveStakeAuthority(poolAccount);

  const keys = [
    { pubkey: voteAccount, isSigner: false, isWritable: false },
    { pubkey: poolAccount, isSigner: false, isWritable: false },
    { pubkey: stakePool, isSigner: false, isWritable: true },
    { pubkey: onRampPool, isSigner: false, isWritable: true },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_STAKE_HISTORY_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: STAKE_CONFIG_ID, isSigner: false, isWritable: false },
    { pubkey: StakeProgram.programId, isSigner: false, isWritable: false },
  ];

  // Note: Hard-coded, if foundation ever changes the ix index, then woops.
  const data = Buffer.from(Uint8Array.of(1));

  return new TransactionInstruction({
    keys,
    programId: SINGLE_POOL_PROGRAM_ID,
    data,
  });
}
