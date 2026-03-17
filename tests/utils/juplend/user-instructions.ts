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
import {
  assertProtocolAccountCount,
  INTEGRATION_PROTOCOL_ACCOUNT_COUNTS,
} from "../integration-account-layouts";

export type JuplendDepositAccounts = {
  marginfiAccount: PublicKey;
  signerTokenAccount: PublicKey;
  bank: PublicKey;
  pool: JuplendPoolKeys;
  amount: BN;
  tokenProgram?: PublicKey;
};

/**
 * Build `integration_deposit(amount)` for JupLend.
 */
export const makeJuplendDepositIx = async (
  program: Program<Marginfi>,
  accounts: JuplendDepositAccounts,
): Promise<TransactionInstruction> => {
  // Fetch bank to get integration accounts and mint
  const bank = await program.account.bank.fetch(accounts.bank);

  // Build protocol-specific remaining accounts (JupLend deposit layout)
  const protocolAccounts: AccountMeta[] = [
    { pubkey: bank.integrationAcc1, isSigner: false, isWritable: true },          // [0] lending
    { pubkey: accounts.pool.fTokenMint, isSigner: false, isWritable: true },      // [1] f_token_mint
    { pubkey: bank.integrationAcc2, isSigner: false, isWritable: true },          // [2] fToken vault
    { pubkey: accounts.pool.lendingAdmin, isSigner: false, isWritable: false },   // [3] lending_admin
    { pubkey: accounts.pool.tokenReserve, isSigner: false, isWritable: true },    // [4] supply_token_reserves_liquidity
    { pubkey: accounts.pool.supplyPositionOnLiquidity, isSigner: false, isWritable: true }, // [5] lending_supply_position_on_liquidity
    { pubkey: accounts.pool.rateModel, isSigner: false, isWritable: false },      // [6] rate_model
    { pubkey: accounts.pool.vault, isSigner: false, isWritable: true },           // [7] vault
    { pubkey: accounts.pool.liquidity, isSigner: false, isWritable: true },       // [8] liquidity
    { pubkey: JUPLEND_LIQUIDITY_PROGRAM_ID, isSigner: false, isWritable: false }, // [9] liquidity_program
    { pubkey: accounts.pool.lendingRewardsRateModel, isSigner: false, isWritable: false }, // [10] rewards_rate_model
    { pubkey: JUPLEND_LENDING_PROGRAM_ID, isSigner: false, isWritable: false },   // [11] juplend_program
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false }, // [12] associated_token_program
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },      // [13] system_program
  ];
  assertProtocolAccountCount(
    "juplend",
    "deposit",
    protocolAccounts.length,
    INTEGRATION_PROTOCOL_ACCOUNT_COUNTS.juplend.deposit,
  );

  return program.methods
    .integrationDeposit(accounts.amount)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      signerTokenAccount: accounts.signerTokenAccount,
      mint: accounts.pool.mint,
      tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(protocolAccounts)
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
 * Build `integration_withdraw(amount, withdraw_all)` for JupLend.
 */
export const makeJuplendWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: JuplendWithdrawAccounts,
): Promise<TransactionInstruction> => {
  // Fetch bank to get integration accounts and mint
  const bank = await program.account.bank.fetch(accounts.bank);

  // Build protocol-specific remaining accounts (JupLend withdraw layout)
  const protocolAccounts: AccountMeta[] = [
    { pubkey: bank.integrationAcc1, isSigner: false, isWritable: true },          // [0] lending
    { pubkey: accounts.pool.fTokenMint, isSigner: false, isWritable: true },      // [1] f_token_mint
    { pubkey: bank.integrationAcc2, isSigner: false, isWritable: true },          // [2] fToken vault
    { pubkey: bank.integrationAcc3, isSigner: false, isWritable: true },          // [3] withdraw intermediary ATA
    { pubkey: accounts.pool.lendingAdmin, isSigner: false, isWritable: false },   // [4] lending_admin
    { pubkey: accounts.pool.tokenReserve, isSigner: false, isWritable: true },    // [5] supply_token_reserves_liquidity
    { pubkey: accounts.pool.supplyPositionOnLiquidity, isSigner: false, isWritable: true }, // [6] lending_supply_position_on_liquidity
    { pubkey: accounts.pool.rateModel, isSigner: false, isWritable: false },      // [7] rate_model
    { pubkey: accounts.pool.vault, isSigner: false, isWritable: true },           // [8] vault
    { pubkey: accounts.claimAccount, isSigner: false, isWritable: true },         // [9] claim_account
    { pubkey: accounts.pool.liquidity, isSigner: false, isWritable: true },       // [10] liquidity
    { pubkey: JUPLEND_LIQUIDITY_PROGRAM_ID, isSigner: false, isWritable: false }, // [11] liquidity_program
    { pubkey: accounts.pool.lendingRewardsRateModel, isSigner: false, isWritable: false }, // [12] rewards_rate_model
    { pubkey: JUPLEND_LENDING_PROGRAM_ID, isSigner: false, isWritable: false },   // [13] juplend_program
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false }, // [14] associated_token_program
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },      // [15] system_program
  ];
  assertProtocolAccountCount(
    "juplend",
    "withdraw",
    protocolAccounts.length,
    INTEGRATION_PROTOCOL_ACCOUNT_COUNTS.juplend.withdraw,
  );

  // Oracle/health remaining accounts come after protocol accounts
  const oracleAccounts: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
    (pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    }),
  );

  return program.methods
    .integrationWithdraw(accounts.amount, accounts.withdrawAll ? true : null)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      destinationTokenAccount: accounts.destinationTokenAccount,
      mint: accounts.pool.mint,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .remainingAccounts([...protocolAccounts, ...oracleAccounts])
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
