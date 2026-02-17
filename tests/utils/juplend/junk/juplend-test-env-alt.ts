// import { BN, Program } from "@coral-xyz/anchor";
// import {
//   AccountMeta,
//   PublicKey,
//   SystemProgram,
//   TransactionInstruction,
// } from "@solana/web3.js";
// import { ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token";

// import { Marginfi } from "../../../target/types/marginfi";
// import type { JuplendPoolKeys } from "./types";

// export function juplendHealthRemainingAccounts(
//   bank: PublicKey,
//   pythPriceUpdateV2: PublicKey,
//   integrationAcc1: PublicKey,
// ): PublicKey[] {
//   return [bank, pythPriceUpdateV2, integrationAcc1];
// }

// export type JuplendDepositAccounts = {
//   group: PublicKey;
//   marginfiAccount: PublicKey;
//   authority: PublicKey;
//   signerTokenAccount: PublicKey;
//   bank: PublicKey;
//   liquidityVaultAuthority: PublicKey;
//   liquidityVault: PublicKey;
//   fTokenVault: PublicKey;
//   mint: PublicKey;
//   pool: JuplendPoolKeys;
//   amount: BN;
//   tokenProgram?: PublicKey;
//   associatedTokenProgram?: PublicKey;
//   systemProgram?: PublicKey;
// };

// export const makeJuplendDepositIx = async (
//   program: Program<Marginfi>,
//   accounts: JuplendDepositAccounts,
// ): Promise<TransactionInstruction> => {
//   return program.methods
//     .juplendDeposit(accounts.amount)
//     .accounts({
//       group: accounts.group,
//       marginfiAccount: accounts.marginfiAccount,
//       authority: accounts.authority,
//       signerTokenAccount: accounts.signerTokenAccount,
//       bank: accounts.bank,
//       liquidityVaultAuthority: accounts.liquidityVaultAuthority,
//       liquidityVault: accounts.liquidityVault,
//       integrationAcc2: accounts.fTokenVault,
//       mint: accounts.mint,
//       lendingAdmin: accounts.pool.lendingAdmin,
//       integrationAcc1: accounts.pool.lending,
//       fTokenMint: accounts.pool.fTokenMint,
//       supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
//       lendingSupplyPositionOnLiquidity:
//         accounts.pool.lendingSupplyPositionOnLiquidity,
//       rateModel: accounts.pool.rateModel,
//       vault: accounts.pool.vault,
//       liquidity: accounts.pool.liquidity,
//       liquidityProgram: accounts.pool.liquidityProgram,
//       rewardsRateModel: accounts.pool.lendingRewardsRateModel,
//       juplendProgram: accounts.pool.lendingProgram,
//       tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram,
//       associatedTokenProgram:
//         accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
//       systemProgram: accounts.systemProgram ?? SystemProgram.programId,
//     })
//     .instruction();
// };

// export type JuplendWithdrawAccounts = {
//   group: PublicKey;
//   marginfiAccount: PublicKey;
//   authority: PublicKey;
//   destinationTokenAccount: PublicKey;
//   bank: PublicKey;
//   liquidityVaultAuthority: PublicKey;
//   liquidityVault: PublicKey;
//   fTokenVault: PublicKey;
//   mint: PublicKey;
//   underlyingOracle?: PublicKey;
//   pool: JuplendPoolKeys;
//   amount: BN;
//   withdrawAll?: boolean;
//   remainingAccounts?: PublicKey[];
//   claimAccount: PublicKey;
//   tokenProgram?: PublicKey;
//   associatedTokenProgram?: PublicKey;
//   systemProgram?: PublicKey;
// };

// export const makeJuplendWithdrawIx = async (
//   program: Program<Marginfi>,
//   accounts: JuplendWithdrawAccounts,
// ): Promise<TransactionInstruction> => {
//   const remaining: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
//     (pubkey) => ({
//       pubkey,
//       isSigner: false,
//       isWritable: false,
//     }),
//   );

//   return program.methods
//     .juplendWithdraw(accounts.amount, accounts.withdrawAll ? true : null)
//     .accounts({
//       group: accounts.group,
//       marginfiAccount: accounts.marginfiAccount,
//       authority: accounts.authority,
//       destinationTokenAccount: accounts.destinationTokenAccount,
//       bank: accounts.bank,
//       liquidityVaultAuthority: accounts.liquidityVaultAuthority,
//       liquidityVault: accounts.liquidityVault,
//       integrationAcc2: accounts.fTokenVault,
//       claimAccount: accounts.claimAccount,
//       mint: accounts.mint,
//       lendingAdmin: accounts.pool.lendingAdmin,
//       integrationAcc1: accounts.pool.lending,
//       fTokenMint: accounts.pool.fTokenMint,
//       supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
//       lendingSupplyPositionOnLiquidity:
//         accounts.pool.lendingSupplyPositionOnLiquidity,
//       rateModel: accounts.pool.rateModel,
//       vault: accounts.pool.vault,
//       liquidity: accounts.pool.liquidity,
//       liquidityProgram: accounts.pool.liquidityProgram,
//       rewardsRateModel: accounts.pool.lendingRewardsRateModel,
//       juplendProgram: accounts.pool.lendingProgram,
//       tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram,
//       associatedTokenProgram:
//         accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
//       systemProgram: accounts.systemProgram ?? SystemProgram.programId,
//     })
//     .remainingAccounts(remaining)
//     .instruction();
// };
