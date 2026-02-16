import { BN, Program } from "@coral-xyz/anchor";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";

import { Marginfi } from "../../../target/types/marginfi";
import { deriveLiquidityVaultAuthority } from "../pdas";
import { findJuplendClaimAccountPda } from "./juplend-pdas";
import type { JuplendLendingIdl, JuplendPoolKeys } from "./types";
import {
  makeJuplendNativeUpdateRateIx,
  makeJuplendWithdrawIx,
} from "./user-instructions";

export type RefreshJupSimpleArgs = {
  pool: JuplendPoolKeys;
};

/**
 * Useful when one already has a `JuplendPoolKeys` and is too lazy too call
 * `makeJuplendNativeUpdateRateIx`
 * @param program
 * @param args
 * @returns
 */
export const refreshJupSimple = (
  program: Program<JuplendLendingIdl>,
  args: RefreshJupSimpleArgs,
): Promise<TransactionInstruction> => {
  return makeJuplendNativeUpdateRateIx(program, {
    lending: args.pool.lending,
    tokenReserve: args.pool.tokenReserve,
    rewardsRateModel: args.pool.lendingRewardsRateModel,
  });
};

export type JuplendWithdrawSimpleArgs = {
  marginfiAccount: PublicKey;
  destinationTokenAccount: PublicKey;
  bank: PublicKey;
  pool: JuplendPoolKeys;
  amount: BN;
  withdrawAll?: boolean;
  remainingAccounts?: PublicKey[];
  tokenProgram?: PublicKey;
};

/**
 * Small-call wrapper around `makeJuplendWithdrawIx`.
 * Derives the canonical Juplend claim PDA from `(liquidity_vault_authority, mint)`.
 */
export const makeJuplendWithdrawSimpleIx = (
  program: Program<Marginfi>,
  args: JuplendWithdrawSimpleArgs,
): Promise<TransactionInstruction> => {
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    args.bank,
  );
  const [claimAccount] = findJuplendClaimAccountPda(
    liquidityVaultAuthority,
    args.pool.mint,
    args.pool.liquidityProgram,
  );

  return makeJuplendWithdrawIx(program, {
    marginfiAccount: args.marginfiAccount,
    destinationTokenAccount: args.destinationTokenAccount,
    bank: args.bank,
    pool: args.pool,
    claimAccount,
    amount: args.amount,
    withdrawAll: args.withdrawAll,
    remainingAccounts: args.remainingAccounts,
    tokenProgram: args.tokenProgram,
  });
};
