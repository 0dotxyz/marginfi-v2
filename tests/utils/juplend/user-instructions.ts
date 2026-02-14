import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import { Marginfi } from "../../../target/types/marginfi";
import type {
  JuplendLendingIdl,
  JuplendLiquidityIdl,
  JuplendPoolKeys,
} from "./types";

export type JuplendDepositAccounts = {
  marginfiAccount: PublicKey;
  // authority: PublicKey;
  signerTokenAccount: PublicKey;
  bank: PublicKey;
  fTokenVault: PublicKey;
  pool: JuplendPoolKeys;
  amount: BN;
  tokenProgram?: PublicKey;
};

/**
 * Build `juplend_deposit(amount)`.
 *
 * Note: `fTokenMint` still needs to be passed via `accountsPartial` because
 * Anchor cannot infer it through external JupLend account relations.
 */
export const makeJuplendDepositIx = async (
  program: Program<Marginfi>,
  accounts: JuplendDepositAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .juplendDeposit(accounts.amount)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      // authority: accounts.authority,
      signerTokenAccount: accounts.signerTokenAccount,
      bank: accounts.bank,
      lendingAdmin: accounts.pool.lendingAdmin,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      // integrationAcc2: accounts.fTokenVault,
      tokenProgram:
        accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .accountsPartial({
      fTokenMint: accounts.pool.fTokenMint,
    })
    .instruction();
};

export type JuplendWithdrawAccounts = {
  marginfiAccount: PublicKey;
  destinationTokenAccount: PublicKey;
  bank: PublicKey;
  withdrawIntermediaryAta: PublicKey;
  pool: JuplendPoolKeys;
  claimAccount: PublicKey;
  amount: BN;
  withdrawAll?: boolean;
  remainingAccounts?: PublicKey[];
  tokenProgram?: PublicKey;
  associatedTokenProgram?: PublicKey;
  systemProgram?: PublicKey;
};

/**
 * Build `juplend_withdraw(amount, withdraw_all)`.
 *
 * Note: `fTokenMint` still needs to be passed via `accountsPartial` because
 * Anchor cannot infer it through external JupLend account relations.
 */
export const makeJuplendWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: JuplendWithdrawAccounts,
): Promise<TransactionInstruction> => {
  const remaining: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
    (pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    }),
  );

  return program.methods
    .juplendWithdraw(accounts.amount, accounts.withdrawAll ? true : null)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      destinationTokenAccount: accounts.destinationTokenAccount,
      bank: accounts.bank,
      integrationAcc3: accounts.withdrawIntermediaryAta,
      lendingAdmin: accounts.pool.lendingAdmin,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      claimAccount: accounts.claimAccount,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      tokenProgram:
        accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
      associatedTokenProgram:
        accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: accounts.systemProgram ?? SystemProgram.programId,
    })
    .accountsPartial({
      fTokenMint: accounts.pool.fTokenMint,
    })
    .remainingAccounts(remaining)
    .instruction();
};

export type JuplendNativeLendingDepositAccounts = {
  signer: PublicKey;
  depositorTokenAccount: PublicKey;
  recipientTokenAccount: PublicKey;
  pool: JuplendPoolKeys;
  assets: BN;
  tokenProgram?: PublicKey;
};

/**
 * Build native JupLend `deposit(assets)`.
 */
export const makeJuplendNativeLendingDepositIx = async (
  program: Program<JuplendLendingIdl>,
  accounts: JuplendNativeLendingDepositAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .deposit(accounts.assets)
    .accounts({
      signer: accounts.signer,
      depositorTokenAccount: accounts.depositorTokenAccount,
      recipientTokenAccount: accounts.recipientTokenAccount,
      lendingAdmin: accounts.pool.lendingAdmin,
      lending: accounts.pool.lending,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      tokenProgram:
        accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .accountsPartial({
      mint: accounts.pool.mint,
      fTokenMint: accounts.pool.fTokenMint,
    })
    .instruction();
};

export type JuplendNativePreOperateAccounts = {
  protocol: PublicKey;
  mint: PublicKey;
  pool: JuplendPoolKeys;
  userSupplyPosition: PublicKey;
  userBorrowPosition: PublicKey;
  tokenProgram?: PublicKey;
};

/**
 * Build native Liquidity `preOperate(mint)`.
 */
export const makeJuplendNativePreOperateIx = async (
  program: Program<JuplendLiquidityIdl>,
  accounts: JuplendNativePreOperateAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .preOperate(accounts.mint)
    .accounts({
      protocol: accounts.protocol,
      liquidity: accounts.pool.liquidity,
      userSupplyPosition: accounts.userSupplyPosition,
      userBorrowPosition: accounts.userBorrowPosition,
      vault: accounts.pool.vault,
      tokenReserve: accounts.pool.tokenReserve,
      tokenProgram:
        accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .instruction();
};

export type JuplendNativeBorrowAccounts = {
  protocol: PublicKey;
  mint: PublicKey;
  pool: JuplendPoolKeys;
  userSupplyPosition: PublicKey;
  userBorrowPosition: PublicKey;
  borrowTo: PublicKey;
  borrowToAccount: PublicKey;
  borrowAmount: BN;
  tokenProgram?: PublicKey;
};

/**
 * Build native Liquidity `operate` for direct borrow.
 */
export const makeJuplendNativeBorrowIx = async (
  program: Program<JuplendLiquidityIdl>,
  accounts: JuplendNativeBorrowAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .operate(
      new BN(0),
      accounts.borrowAmount,
      accounts.protocol,
      accounts.borrowTo,
      { direct: {} },
    )
    .accounts({
      protocol: accounts.protocol,
      liquidity: accounts.pool.liquidity,
      tokenReserve: accounts.pool.tokenReserve,
      mint: accounts.mint,
      vault: accounts.pool.vault,
      userSupplyPosition: accounts.userSupplyPosition,
      userBorrowPosition: accounts.userBorrowPosition,
      rateModel: accounts.pool.rateModel,
      borrowToAccount: accounts.borrowToAccount,
      borrowClaimAccount: null,
      withdrawClaimAccount: null,
      tokenProgram:
        accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .instruction();
};

export type JuplendNativeUpdateRateAccounts = {
  pool: JuplendPoolKeys;
};

/**
 * Build native JupLend `updateRate()`.
 *
 * Useful right before `healthPulse` so Jup lending state is fresh in the same tx.
 */
export const makeJuplendNativeUpdateRateIx = async (
  program: Program<JuplendLendingIdl>,
  accounts: JuplendNativeUpdateRateAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .updateRate()
    .accountsPartial({
      lending: accounts.pool.lending,
      mint: accounts.pool.mint,
      fTokenMint: accounts.pool.fTokenMint,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
    })
    .instruction();
};
