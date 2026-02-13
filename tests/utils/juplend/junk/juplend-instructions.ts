// import {
//   AccountMeta,
//   PublicKey,
//   SystemProgram,
//   TransactionInstruction,
// } from "@solana/web3.js";
// import {
//   ASSOCIATED_TOKEN_PROGRAM_ID,
//   TOKEN_PROGRAM_ID,
// } from "@solana/spl-token";
// import { JUPLEND_LENDING_PROGRAM_ID } from "./juplend-pdas";

// /**
//  * Discriminators are copied from `idls/lending.json`.
//  */
// export const JUPLEND_IX = {
//   UPDATE_RATE: Buffer.from([24, 225, 53, 189, 72, 212, 225, 178]),
//   DEPOSIT: Buffer.from([242, 35, 198, 137, 82, 225, 242, 182]),
//   WITHDRAW: Buffer.from([183, 18, 70, 156, 148, 109, 161, 34]),
// } as const;

// function u64Le(value: bigint): Buffer {
//   if (value < 0n) throw new Error("u64Le: negative");
//   const out = Buffer.alloc(8);
//   let v = value;
//   for (let i = 0; i < 8; i++) {
//     out[i] = Number(v & 0xffn);
//     v >>= 8n;
//   }
//   return out;
// }

// function ixData(discriminator: Buffer, args?: Buffer): Buffer {
//   return args ? Buffer.concat([discriminator, args]) : discriminator;
// }

// export type JuplendUpdateRateAccounts = {
//   lending: PublicKey;
//   mint: PublicKey;
//   fTokenMint: PublicKey;
//   supplyTokenReservesLiquidity: PublicKey;
//   rewardsRateModel: PublicKey;
// };

// export function juplendUpdateRateIx(
//   accounts: JuplendUpdateRateAccounts,
//   programId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): TransactionInstruction {
//   const keys: AccountMeta[] = [
//     { pubkey: accounts.lending, isSigner: false, isWritable: true },
//     { pubkey: accounts.mint, isSigner: false, isWritable: false },
//     { pubkey: accounts.fTokenMint, isSigner: false, isWritable: false },
//     {
//       pubkey: accounts.supplyTokenReservesLiquidity,
//       isSigner: false,
//       isWritable: false,
//     },
//     { pubkey: accounts.rewardsRateModel, isSigner: false, isWritable: false },
//   ];

//   return new TransactionInstruction({
//     programId,
//     keys,
//     data: ixData(JUPLEND_IX.UPDATE_RATE),
//   });
// }

// export type JuplendDepositAccounts = {
//   signer: PublicKey;
//   depositorTokenAccount: PublicKey;
//   recipientTokenAccount: PublicKey;
//   mint: PublicKey;
//   lendingAdmin: PublicKey;
//   lending: PublicKey;
//   fTokenMint: PublicKey;
//   supplyTokenReservesLiquidity: PublicKey;
//   lendingSupplyPositionOnLiquidity: PublicKey;
//   rateModel: PublicKey;
//   vault: PublicKey;
//   liquidity: PublicKey;
//   liquidityProgram: PublicKey;
//   rewardsRateModel: PublicKey;
//   tokenProgram?: PublicKey;
//   associatedTokenProgram?: PublicKey;
//   systemProgram?: PublicKey;
// };

// /**
//  * Builds a direct JupLend Lending `deposit(assets)` instruction.
//  *
//  * NOTE: marginfi will typically CPI into deposit, but in protocol-level tests it's helpful to call it directly.
//  */
// export function juplendDepositIx(
//   accounts: JuplendDepositAccounts,
//   assets: bigint,
//   programId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): TransactionInstruction {
//   const tokenProgram = accounts.tokenProgram ?? TOKEN_PROGRAM_ID;
//   const associatedTokenProgram =
//     accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID;
//   const systemProgram = accounts.systemProgram ?? SystemProgram.programId;

//   const keys: AccountMeta[] = [
//     { pubkey: accounts.signer, isSigner: true, isWritable: true },
//     { pubkey: accounts.depositorTokenAccount, isSigner: false, isWritable: true },
//     { pubkey: accounts.recipientTokenAccount, isSigner: false, isWritable: true },
//     { pubkey: accounts.mint, isSigner: false, isWritable: false },
//     { pubkey: accounts.lendingAdmin, isSigner: false, isWritable: false },
//     { pubkey: accounts.lending, isSigner: false, isWritable: true },
//     { pubkey: accounts.fTokenMint, isSigner: false, isWritable: true },
//     {
//       pubkey: accounts.supplyTokenReservesLiquidity,
//       isSigner: false,
//       isWritable: true,
//     },
//     {
//       pubkey: accounts.lendingSupplyPositionOnLiquidity,
//       isSigner: false,
//       isWritable: true,
//     },
//     { pubkey: accounts.rateModel, isSigner: false, isWritable: false },
//     { pubkey: accounts.vault, isSigner: false, isWritable: true },
//     { pubkey: accounts.liquidity, isSigner: false, isWritable: true },
//     // Always pass programs as read-only
//     { pubkey: accounts.liquidityProgram, isSigner: false, isWritable: false },
//     { pubkey: accounts.rewardsRateModel, isSigner: false, isWritable: false },
//     { pubkey: tokenProgram, isSigner: false, isWritable: false },
//     { pubkey: associatedTokenProgram, isSigner: false, isWritable: false },
//     { pubkey: systemProgram, isSigner: false, isWritable: false },
//   ];

//   return new TransactionInstruction({
//     programId,
//     keys,
//     data: ixData(JUPLEND_IX.DEPOSIT, u64Le(assets)),
//   });
// }

// export type JuplendWithdrawAccounts = {
//   signer: PublicKey;
//   ownerTokenAccount: PublicKey;
//   recipientTokenAccount: PublicKey;
//   lendingAdmin: PublicKey;
//   lending: PublicKey;
//   mint: PublicKey;
//   fTokenMint: PublicKey;
//   supplyTokenReservesLiquidity: PublicKey;
//   lendingSupplyPositionOnLiquidity: PublicKey;
//   rateModel: PublicKey;
//   vault: PublicKey;
//   liquidity: PublicKey;
//   liquidityProgram: PublicKey;
//   rewardsRateModel: PublicKey;
//   /**
//    * Optional claim account. If omitted, the program may interpret this as `None`
//    * (depending on account owner/type checks in the on-chain Accounts parser).
//    */
//   claimAccount?: PublicKey;
//   tokenProgram?: PublicKey;
//   associatedTokenProgram?: PublicKey;
//   systemProgram?: PublicKey;
// };

// /**
//  * Builds a direct JupLend Lending `withdraw(amount)` instruction.
//  */
// export function juplendWithdrawIx(
//   accounts: JuplendWithdrawAccounts,
//   amount: bigint,
//   programId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): TransactionInstruction {
//   const tokenProgram = accounts.tokenProgram ?? TOKEN_PROGRAM_ID;
//   const associatedTokenProgram =
//     accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID;
//   const systemProgram = accounts.systemProgram ?? SystemProgram.programId;

//   const keys: AccountMeta[] = [
//     { pubkey: accounts.signer, isSigner: true, isWritable: true },
//     { pubkey: accounts.ownerTokenAccount, isSigner: false, isWritable: true },
//     { pubkey: accounts.recipientTokenAccount, isSigner: false, isWritable: true },
//     { pubkey: accounts.lendingAdmin, isSigner: false, isWritable: false },
//     { pubkey: accounts.lending, isSigner: false, isWritable: true },
//     { pubkey: accounts.mint, isSigner: false, isWritable: false },
//     { pubkey: accounts.fTokenMint, isSigner: false, isWritable: true },
//     {
//       pubkey: accounts.supplyTokenReservesLiquidity,
//       isSigner: false,
//       isWritable: true,
//     },
//     {
//       pubkey: accounts.lendingSupplyPositionOnLiquidity,
//       isSigner: false,
//       isWritable: true,
//     },
//     { pubkey: accounts.rateModel, isSigner: false, isWritable: false },
//     { pubkey: accounts.vault, isSigner: false, isWritable: true },
//   ];

//   if (accounts.claimAccount) {
//     keys.push({ pubkey: accounts.claimAccount, isSigner: false, isWritable: true });
//   }

//   keys.push(
//     { pubkey: accounts.liquidity, isSigner: false, isWritable: true },
//     // Always pass programs as read-only
//     { pubkey: accounts.liquidityProgram, isSigner: false, isWritable: false },
//     { pubkey: accounts.rewardsRateModel, isSigner: false, isWritable: false },
//     { pubkey: tokenProgram, isSigner: false, isWritable: false },
//     { pubkey: associatedTokenProgram, isSigner: false, isWritable: false },
//     { pubkey: systemProgram, isSigner: false, isWritable: false }
//   );

//   return new TransactionInstruction({
//     programId,
//     keys,
//     data: ixData(JUPLEND_IX.WITHDRAW, u64Le(amount)),
//   });
// }
