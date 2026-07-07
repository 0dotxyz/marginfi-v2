import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";

import { Marginfi } from "../../../target/types/marginfi";
import type {
  JuplendLendingIdl,
  JuplendLiquidityIdl,
  JuplendPoolKeys,
} from "./types";
import { JUPLEND_LENDING_PROGRAM_ID, JUPLEND_LIQUIDITY_PROGRAM_ID } from "./juplend-pdas";
import { deriveLiquidityVaultAuthority } from "../pdas";

export type JuplendDepositAccounts = {
  marginfiAccount: PublicKey;
  signerTokenAccount: PublicKey;
  bank: PublicKey;
  pool: JuplendPoolKeys;
  amount: BN;
  tokenProgram?: PublicKey;
};

const resolveJuplendWrapperAccounts = async (
  program: Program<Marginfi>,
  marginfiAccountPk: PublicKey,
  bankPk: PublicKey,
) => {
  const [marginfiAccount, bank] = await Promise.all([
    program.account.marginfiAccount.fetch(marginfiAccountPk),
    program.account.bank.fetch(bankPk),
  ]);
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bankPk,
  );

  return {
    group: marginfiAccount.group,
    liquidityVaultAuthority,
    integrationAcc1: bank.integrationAcc1,
    integrationAcc2: bank.integrationAcc2,
    integrationAcc3: bank.integrationAcc3,
    liquidityVault: bank.liquidityVault,
    mint: bank.mint,
  };
};

/**
 * Build `juplendDeposit(amount)` for JupLend.
 */
export const makeJuplendDepositIx = async (
  program: Program<Marginfi>,
  accounts: JuplendDepositAccounts,
): Promise<TransactionInstruction> => {
  const common = await resolveJuplendWrapperAccounts(
    program,
    accounts.marginfiAccount,
    accounts.bank,
  );
  const authority = program.provider.publicKey as PublicKey;

  return program.methods
    .juplendDeposit(accounts.amount)
    .accountsStrict({
      group: common.group,
      marginfiAccount: accounts.marginfiAccount,
      authority,
      bank: accounts.bank,
      signerTokenAccount: accounts.signerTokenAccount,
      liquidityVaultAuthority: common.liquidityVaultAuthority,
      liquidityVault: common.liquidityVault,
      mint: common.mint,
      integrationAcc1: common.integrationAcc1,
      fTokenMint: accounts.pool.fTokenMint,
      integrationAcc2: common.integrationAcc2,
      lendingAdmin: accounts.pool.lendingAdmin,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity: accounts.pool.supplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: JUPLEND_LIQUIDITY_PROGRAM_ID,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      juplendProgram: JUPLEND_LENDING_PROGRAM_ID,
      tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
};

export type JuplendWithdrawAccounts = {
  marginfiAccount: PublicKey;
  destinationTokenAccount: PublicKey;
  bank: PublicKey;
  pool: JuplendPoolKeys;
  claimAccount: PublicKey;
  amount: BN;
  withdrawAll?: boolean;
  remainingAccounts?: PublicKey[];
  tokenProgram?: PublicKey;
};

/**
 * Build `juplendWithdraw(amount, withdraw_all)` for JupLend.
 */
export const makeJuplendWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: JuplendWithdrawAccounts,
): Promise<TransactionInstruction> => {
  const common = await resolveJuplendWrapperAccounts(
    program,
    accounts.marginfiAccount,
    accounts.bank,
  );
  const authority = program.provider.publicKey as PublicKey;
  const remaining: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
    (pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    }),
  );

  return program.methods
    .juplendWithdraw(accounts.amount, accounts.withdrawAll ? true : null)
    .accountsStrict({
      group: common.group,
      marginfiAccount: accounts.marginfiAccount,
      authority,
      bank: accounts.bank,
      destinationTokenAccount: accounts.destinationTokenAccount,
      liquidityVaultAuthority: common.liquidityVaultAuthority,
      mint: common.mint,
      integrationAcc1: common.integrationAcc1,
      fTokenMint: accounts.pool.fTokenMint,
      integrationAcc2: common.integrationAcc2,
      integrationAcc3: common.integrationAcc3,
      lendingAdmin: accounts.pool.lendingAdmin,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity: accounts.pool.supplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      claimAccount: accounts.claimAccount,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: JUPLEND_LIQUIDITY_PROGRAM_ID,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      juplendProgram: JUPLEND_LENDING_PROGRAM_ID,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
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
      lendingSupplyPositionOnLiquidity: accounts.pool.supplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      // liquidityProgram is fixed for JupLend and inferred via constant in other builders.
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
      // protocol: accounts.protocol,
      liquidity: accounts.pool.liquidity,
      userSupplyPosition: accounts.userSupplyPosition,
      userBorrowPosition: accounts.userBorrowPosition,
      // vault: accounts.pool.vault,
      tokenReserve: accounts.pool.tokenReserve,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .accountsPartial({
      // Same reason as `operate`: this account is relation-derived in IDL and
      // Anchor TS resolution can fail with max-depth recursion.
      protocol: accounts.protocol,
    })
    .instruction();
};

export type JuplendNativeBorrowAccounts = {
  protocol: PublicKey;
  pool: JuplendPoolKeys;
  userSupplyPosition: PublicKey;
  userBorrowPosition: PublicKey;
  borrowTo: PublicKey;
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
      liquidity: accounts.pool.liquidity,
      tokenReserve: accounts.pool.tokenReserve,
      userSupplyPosition: accounts.userSupplyPosition,
      userBorrowPosition: accounts.userBorrowPosition,
      rateModel: accounts.pool.rateModel,
      borrowClaimAccount: null,
      withdrawClaimAccount: null,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .accountsPartial({
      // `protocol` in the Liquidity IDL is relation-only (derived from user positions),
      // not seed-derived from args. Anchor's TS resolver often fails relation-only
      // resolution here with recursive dependency/depth errors, so we pass it explicitly.
      protocol: accounts.protocol,
    })
    .instruction();
};

export type JuplendNativeUpdateRateAccounts = {
  lending: PublicKey;
  tokenReserve: PublicKey;
  rewardsRateModel: PublicKey;
};

/**
 * Build native JupLend `updateRate()`.
 *
 * Use before any risk check so Jup lending state is fresh in the same tx.
 */
export const makeJuplendNativeUpdateRateIx = async (
  program: Program<JuplendLendingIdl>,
  accounts: JuplendNativeUpdateRateAccounts,
): Promise<TransactionInstruction> => {
  return program.methods
    .updateRate()
    .accounts({
      lending: accounts.lending,
      supplyTokenReservesLiquidity: accounts.tokenReserve,
      rewardsRateModel: accounts.rewardsRateModel,
    })
    .instruction();
};
