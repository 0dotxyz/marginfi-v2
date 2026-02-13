// import { PublicKey } from "@solana/web3.js";
// import {
//   ASSOCIATED_TOKEN_PROGRAM_ID,
//   TOKEN_PROGRAM_ID,
//   getAssociatedTokenAddressSync,
// } from "@solana/spl-token";

// /**
//  * Jupiter Lend (JupLend / Fluid) program IDs (mainnet).
//  *
//  * NOTE: In local tests you typically load the mainnet .so under the same address.
//  */
// export const JUPLEND_LENDING_PROGRAM_ID = new PublicKey(
//   "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
// );

// export const JUPLEND_LIQUIDITY_PROGRAM_ID = new PublicKey(
//   "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC"
// );

// export const JUPLEND_EARN_REWARDS_PROGRAM_ID = new PublicKey(
//   "jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar"
// );

// export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
//   "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
// );

// export const JUPLEND_LIQUIDITY_AUTH_LIST_SEED = "auth_list";
// export const JUPLEND_LENDING_REWARDS_ADMIN_SEED = "lending_rewards_admin";
// export const TOKEN_METADATA_SEED = "metadata";

// /**
//  * Where the test harness expects the dumped mainnet .so to live (convention).
//  *
//  * Command:
//  *   solana program dump ${JUPLEND_LENDING_PROGRAM_ID.toBase58()} tests/fixtures/juplend_lending.so --url https://api.mainnet-beta.solana.com
//  */
// export const DEFAULT_JUPLEND_LENDING_SO_PATH =
//   "tests/fixtures/juplend_lending.so";

// /**
//  * LendingAdmin PDA
//  * Seeds: ["lending_admin"]
//  */
// export function findJuplendLendingAdminPda(
//   lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("lending_admin")],
//     lendingProgramId
//   );
// }

// /**
//  * fToken Mint PDA
//  * Seeds: ["f_token_mint", underlyingMint]
//  */
// export function findJuplendFTokenMintPda(
//   underlyingMint: PublicKey,
//   lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("f_token_mint"), underlyingMint.toBuffer()],
//     lendingProgramId
//   );
// }

// /**
//  * Lending pool PDA
//  * Seeds: ["lending", underlyingMint, fTokenMint]
//  */
// export function findJuplendLendingPda(
//   underlyingMint: PublicKey,
//   fTokenMint: PublicKey,
//   lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("lending"), underlyingMint.toBuffer(), fTokenMint.toBuffer()],
//     lendingProgramId
//   );
// }

// export type JuplendLendingPdas = {
//   lendingAdmin: PublicKey;
//   lendingAdminBump: number;
//   fTokenMint: PublicKey;
//   fTokenMintBump: number;
//   lending: PublicKey;
//   lendingBump: number;
// };

// /**
//  * Convenience: derive all Lending-program PDAs from the underlying mint.
//  */
// export function deriveJuplendLendingPdas(
//   underlyingMint: PublicKey,
//   lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
// ): JuplendLendingPdas {
//   const [lendingAdmin, lendingAdminBump] =
//     findJuplendLendingAdminPda(lendingProgramId);
//   const [fTokenMint, fTokenMintBump] = findJuplendFTokenMintPda(
//     underlyingMint,
//     lendingProgramId
//   );
//   const [lending, lendingBump] = findJuplendLendingPda(
//     underlyingMint,
//     fTokenMint,
//     lendingProgramId
//   );

//   return {
//     lendingAdmin,
//     lendingAdminBump,
//     fTokenMint,
//     fTokenMintBump,
//     lending,
//     lendingBump,
//   };
// }

// /**
//  * Liquidity global PDA
//  * Seeds: ["liquidity"]
//  */
// export function findJuplendLiquidityPda(
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("liquidity")],
//     liquidityProgramId
//   );
// }

// /**
//  * Liquidity authorization list PDA
//  * Seeds: ["auth_list"]
//  */
// export function findJuplendLiquidityAuthListPda(
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from(JUPLEND_LIQUIDITY_AUTH_LIST_SEED)],
//     liquidityProgramId
//   );
// }

// /**
//  * Liquidity TokenReserve PDA
//  * Seeds: ["reserve", underlyingMint]
//  */
// export function findJuplendLiquidityTokenReservePda(
//   underlyingMint: PublicKey,
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("reserve"), underlyingMint.toBuffer()],
//     liquidityProgramId
//   );
// }

// /**
//  * Liquidity RateModel PDA
//  * Seeds: ["rate_model", underlyingMint]
//  */
// export function findJuplendLiquidityRateModelPda(
//   underlyingMint: PublicKey,
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("rate_model"), underlyingMint.toBuffer()],
//     liquidityProgramId
//   );
// }

// /**
//  * Liquidity protocol supply position PDA (protocol = lending PDA)
//  * Seeds: ["user_supply_position", underlyingMint, lendingPda]
//  */
// export function findJuplendLiquiditySupplyPositionPda(
//   underlyingMint: PublicKey,
//   lendingPda: PublicKey,
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [
//       Buffer.from("user_supply_position"),
//       underlyingMint.toBuffer(),
//       lendingPda.toBuffer(),
//     ],
//     liquidityProgramId
//   );
// }



// /**
//  * Liquidity protocol borrow position PDA (protocol = lending PDA)
//  * Seeds: ["user_borrow_position", underlyingMint, lendingPda]
//  */
// export function findJuplendLiquidityBorrowPositionPda(
//   underlyingMint: PublicKey,
//   lendingPda: PublicKey,
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [
//       Buffer.from("user_borrow_position"),
//       underlyingMint.toBuffer(),
//       lendingPda.toBuffer(),
//     ],
//     liquidityProgramId
//   );
// }

// /**
//  * Liquidity vault token account (ATA(liquidity_pda, underlyingMint))
//  */
// export function deriveJuplendLiquidityVaultAta(
//   underlyingMint: PublicKey,
//   liquidityPda: PublicKey,
//   tokenProgramId: PublicKey = TOKEN_PROGRAM_ID
// ): PublicKey {
//   return getAssociatedTokenAddressSync(
//     underlyingMint,
//     liquidityPda,
//     true, // allowOwnerOffCurve
//     tokenProgramId,              // Token program (SPL Token or Token-2022)
//     ASSOCIATED_TOKEN_PROGRAM_ID  // Associated token program
//   );
// }

// /**
//  * Token metadata PDA
//  * Seeds: ["metadata", token_metadata_program_id, mint]
//  */
// export function findJuplendTokenMetadataPda(
//   mint: PublicKey
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [
//       Buffer.from(TOKEN_METADATA_SEED),
//       TOKEN_METADATA_PROGRAM_ID.toBuffer(),
//       mint.toBuffer(),
//     ],
//     TOKEN_METADATA_PROGRAM_ID
//   );
// }

// /**
//  * LendingRewardsAdmin PDA
//  * Seeds: ["lending_rewards_admin"]
//  */
// export function findJuplendLendingRewardsAdminPda(
//   rewardsProgramId: PublicKey = JUPLEND_EARN_REWARDS_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from(JUPLEND_LENDING_REWARDS_ADMIN_SEED)],
//     rewardsProgramId
//   );
// }

// /**
//  * Rewards Rate Model PDA (best-effort helper).
//  *
//  * NOTE: Prefer reading `rewards_rate_model` directly from the on-chain Lending state when possible,
//  * because this seed/program-id pair is not guaranteed by the Lending IDL alone.
//  */
// export function findJuplendRewardsRateModelPdaBestEffort(
//   underlyingMint: PublicKey,
//   rewardsProgramId: PublicKey = JUPLEND_EARN_REWARDS_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("lending_rewards_rate_model"), underlyingMint.toBuffer()],
//     rewardsProgramId
//   );
// }

// /**
//  * Liquidity UserClaim PDA
//  * Seeds: ["user_claim", user, mint]
//  *
//  * This account tracks reward claims for a user on a specific mint.
//  * It's required (despite IDL marking it optional) for withdraw operations.
//  */
// export function findJuplendClaimAccountPda(
//   user: PublicKey,
//   mint: PublicKey,
//   liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
// ): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [Buffer.from("user_claim"), user.toBuffer(), mint.toBuffer()],
//     liquidityProgramId
//   );
// }
