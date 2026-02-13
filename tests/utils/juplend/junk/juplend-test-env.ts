// import { BN, Program } from "@coral-xyz/anchor";
// import {
//   AccountMeta,
//   PublicKey,
//   SystemProgram,
//   TransactionInstruction,
// } from "@solana/web3.js";
// import {
//   ASSOCIATED_TOKEN_PROGRAM_ID,
// } from "@solana/spl-token";

// import { Marginfi } from "../../../target/types/marginfi";

// import type { JuplendPoolKeys } from "./types";

// // Re-export for convenience
// export { findJuplendClaimAccountPda } from "./juplend-pdas";
// export {
//   deriveJuplendMrgnAddresses,
//   type DeriveJuplendMrgnAddressesArgs,
//   type JuplendMrgnAddresses,
// } from "./pdas";
// export {
//   addJuplendBankIx as makeAddJuplendBankIx,
//   makeJuplendInitPositionIx,
//   type AddJuplendBankAccounts,
//   type JuplendInitPositionAccounts,
// } from "./group-instructions";

// /**
//  * For a JupLend bank, health checks require:
//  *   [bank, pyth_oracle_price_update, lending_state]
//  * where the lending_state is used both for owner checks and for the fToken price conversion.
//  */
// export function juplendHealthRemainingAccounts(
//   bank: PublicKey,
//   pythPriceUpdateV2: PublicKey,
//   integrationAcc1: PublicKey
// ): PublicKey[] {
//   return [bank, pythPriceUpdateV2, integrationAcc1];
// }

// // ---------------------------------------------------------------------------
// // Marginfi instruction helpers
// // ---------------------------------------------------------------------------

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

// /**
//  * Build `juplend_deposit(amount)`.
//  */
// export const makeJuplendDepositIx = async (
//   program: Program<Marginfi>,
//   accounts: JuplendDepositAccounts
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
//   /** (Optional) used only for readability when constructing remaining accounts */
//   underlyingOracle?: PublicKey;
//   pool: JuplendPoolKeys;

//   amount: BN;
//   /** If true, ignore `amount` and withdraw the entire position (burn all shares). */
//   withdrawAll?: boolean;
//   /** Remaining accounts for risk engine (bank/oracles) */
//   remainingAccounts?: PublicKey[];

//   /**
//    * JupLend claim account for liquidity_vault_authority.
//    * TEMPORARY: Mainnet currently requires this (passing None causes ConstraintMut errors),
//    * but an upcoming upgrade is expected to make it truly optional.
//    */
//   claimAccount: PublicKey;

//   tokenProgram?: PublicKey;
//   associatedTokenProgram?: PublicKey;
//   systemProgram?: PublicKey;
// };

// /** Build `juplend_withdraw(amount, withdraw_all)` */
// export const makeJuplendWithdrawIx = async (
//   program: Program<Marginfi>,
//   accounts: JuplendWithdrawAccounts
// ): Promise<TransactionInstruction> => {
//   const remaining: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
//     (pubkey) => ({
//       pubkey,
//       isSigner: false,
//       isWritable: false,
//     })
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
