// import { BN, Idl, Program } from "@coral-xyz/anchor";
// import {
//   Keypair,
//   PublicKey,
//   SystemProgram,
//   SYSVAR_INSTRUCTIONS_PUBKEY,
//   SYSVAR_RENT_PUBKEY,
//   Transaction,
//   TransactionInstruction,
// } from "@solana/web3.js";
// import {
//   ASSOCIATED_TOKEN_PROGRAM_ID,
//   TOKEN_2022_PROGRAM_ID,
//   TOKEN_PROGRAM_ID,
//   createMintToInstruction,
//   createAssociatedTokenAccountIdempotentInstruction,
//   getAssociatedTokenAddressSync,
// } from "@solana/spl-token";

// import liquidityIdl from "../../../idls/liquidity.json";
// import lendingIdl from "../../../idls/juplend_earn.json";
// import rewardsIdl from "../../../idls/lending_reward_rate_model.json";

// import { bankRunProvider, bankrunContext } from "../../rootHooks";
// import { processBankrunTransaction } from "../tools";
// import {
//   deriveJuplendLendingPdas,
//   deriveJuplendLiquidityVaultAta,
//   findJuplendLiquidityPda,
//   findJuplendLiquidityRateModelPda,
//   findJuplendLiquiditySupplyPositionPda,
//   findJuplendLiquidityBorrowPositionPda,
//   findJuplendLiquidityTokenReservePda,
//   findJuplendRewardsRateModelPdaBestEffort,
//   findJuplendClaimAccountPda,
//   JUPLEND_EARN_REWARDS_PROGRAM_ID,
//   JUPLEND_LENDING_PROGRAM_ID,
//   JUPLEND_LIQUIDITY_PROGRAM_ID,
// } from "./juplend-pdas";

// /**
//  * NOTE: init_lending creates metadata for the fToken mint via Metaplex Token Metadata.
//  * If you want to use `ensureJuplendPoolForMint`, you MUST load the token metadata
//  * program (metaqbxx...) into your bankrun test genesis (Anchor.toml).
//  */
// export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
//   "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
// );

// // These values are intentionally very permissive for tests.
// const U64_MAX = new BN("18446744073709551615");
// const DEFAULT_PERCENT_PRECISION = new BN(100); // 1% = 100, 100% = 10_000
// const DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT = new BN(20).mul(DEFAULT_PERCENT_PRECISION); // 20%
// const DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS = new BN(2 * 24 * 60 * 60); // 2 days

// // Debt ceilings (matching Fluid test defaults)
// // Note: max_debt_ceiling cannot exceed 10x token total supply per program validation
// const LAMPORTS_PER_SOL = 1_000_000_000;
// const DEFAULT_BASE_DEBT_CEILING = new BN(1e4 * LAMPORTS_PER_SOL); // 10k SOL worth
// const DEFAULT_MAX_DEBT_CEILING = new BN(1e6 * LAMPORTS_PER_SOL);  // 1M SOL worth

// export type JuplendPrograms = {
//   liquidity: Program;
//   lending: Program;
//   rewards: Program;
// };

// export type JuplendPoolKeys = {
//   mint: PublicKey;
//   tokenProgram: PublicKey;

//   /**
//    * Program IDs (mainnet).
//    *
//    * We include these in the pool keys so the Marginfi CPI helpers can be
//    * completely program-first (no hidden globals).
//    */
//   liquidityProgram: PublicKey;
//   lendingProgram: PublicKey;

//   // Liquidity program
//   liquidity: PublicKey;
//   authList: PublicKey;
//   tokenReserve: PublicKey;
//   rateModel: PublicKey;
//   vault: PublicKey;

//   // Lending rewards rate model program
//   lendingRewardsAdmin: PublicKey;
//   lendingRewardsRateModel: PublicKey;

//   // Lending program
//   lendingAdmin: PublicKey;
//   lending: PublicKey;
//   fTokenMint: PublicKey;
//   fTokenMetadata: PublicKey;

//   // Liquidity positions owned by the lending PDA
//   /**
//    * JupLend calls these “positions”; Marginfi CPI accounts expect
//    * `lending_supply_position_on_liquidity` naming.
//    */
//   supplyPositionOnLiquidity: PublicKey;
//   borrowPositionOnLiquidity: PublicKey;

//   /** Alias for Marginfi CPI account name. */
//   lendingSupplyPositionOnLiquidity: PublicKey;
//   /** Alias for Marginfi CPI account name (not used in deposit/withdraw). */
//   lendingBorrowPositionOnLiquidity: PublicKey;
// };

// export function getJuplendPrograms(): JuplendPrograms {
//   return {
//     liquidity: new Program(liquidityIdl as unknown as Idl, bankRunProvider),
//     lending: new Program(lendingIdl as unknown as Idl, bankRunProvider),
//     rewards: new Program(rewardsIdl as unknown as Idl, bankRunProvider),
//   };
// }

// async function accountExists(address: PublicKey): Promise<boolean> {
//   const acc = await bankRunProvider.connection.getAccountInfo(address);
//   return acc !== null;
// }

// async function getTokenProgramForMint(mint: PublicKey): Promise<PublicKey> {
//   const info = await bankRunProvider.connection.getAccountInfo(mint);
//   if (!info) throw new Error(`Mint account missing: ${mint.toBase58()}`);

//   // Most mints are owned by the classic SPL Token program. Token-2022 is also possible.
//   if (info.owner.equals(TOKEN_PROGRAM_ID) || info.owner.equals(TOKEN_2022_PROGRAM_ID)) {
//     return info.owner;
//   }

//   // If we ever see a mint owned by something else, we'd rather error loudly.
//   throw new Error(
//     `Unsupported mint owner for ${mint.toBase58()}: ${info.owner.toBase58()}`
//   );
// }

// function deriveAuthListPda(liquidityProgramId: PublicKey): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync([Buffer.from("auth_list")], liquidityProgramId);
// }


// function deriveMetadataPda(mint: PublicKey): [PublicKey, number] {
//   return PublicKey.findProgramAddressSync(
//     [
//       Buffer.from("metadata"),
//       TOKEN_METADATA_PROGRAM_ID.toBuffer(),
//       mint.toBuffer(),
//     ],
//     TOKEN_METADATA_PROGRAM_ID
//   );
// }

// /**
//  * Creates (or reuses if already initialized) a complete Juplend pool for a given underlying mint.
//  *
//  * What this does:
//  * - Liquidity: init_liquidity (global) + init_token_reserve + set rate data + token config
//  * - Rewards: init_lending_rewards_admin (global) + init_lending_rewards_rate_model (per mint)
//  * - Lending: init_lending_admin (global) + init_lending (per mint) + set_rewards_rate_model
//  * - Liquidity: init_new_protocol (per mint/protocol) + user supply/borrow configs + set user class
//  *
//  * All transactions are sent through BANKRUN (banksClient), not the local validator RPC.
//  */
// export async function ensureJuplendPoolForMint(args: {
//   admin: Keypair;
//   mint: PublicKey;
//   symbol: string; // ex: "jlUSDC"
//   /** Optional: mint authority keypair to mint tokens (needed for borrow config validation) */
//   mintAuthority?: Keypair;
// }): Promise<JuplendPoolKeys> {
//   const { admin, mint, symbol, mintAuthority } = args;

//   const programs = getJuplendPrograms();

//   // Derive token program from mint owner. JupLend uses SPL Token / Token-2022 token interface.
//   const tokenProgram = await getTokenProgramForMint(mint);

//   // -----------------------------
//   // Liquidity global PDAs
//   // -----------------------------
//   const [liquidity] = findJuplendLiquidityPda(JUPLEND_LIQUIDITY_PROGRAM_ID);
//   const [authList] = deriveAuthListPda(JUPLEND_LIQUIDITY_PROGRAM_ID);

//   // -----------------------------
//   // Liquidity per-mint PDAs
//   // -----------------------------
//   const [tokenReserve] = findJuplendLiquidityTokenReservePda(mint, JUPLEND_LIQUIDITY_PROGRAM_ID);
//   const [rateModel] = findJuplendLiquidityRateModelPda(mint, JUPLEND_LIQUIDITY_PROGRAM_ID);
//   const vault = deriveJuplendLiquidityVaultAta(mint, liquidity, tokenProgram);

//   // -----------------------------
//   // Lending per-mint PDAs
//   // -----------------------------
//   const { fTokenMint, lending, lendingAdmin } = deriveJuplendLendingPdas(mint);
//   const [fTokenMetadata] = deriveMetadataPda(fTokenMint);

//   // -----------------------------
//   // Rewards PDAs
//   // -----------------------------
//   const [lendingRewardsAdmin] = PublicKey.findProgramAddressSync(
//     [Buffer.from("lending_rewards_admin")],
//     JUPLEND_EARN_REWARDS_PROGRAM_ID
//   );
//   const [lendingRewardsRateModel] = findJuplendRewardsRateModelPdaBestEffort(
//     mint,
//     JUPLEND_EARN_REWARDS_PROGRAM_ID
//   );

//   // -----------------------------
//   // Liquidity protocol positions
//   // -----------------------------
//   const [supplyPositionOnLiquidity] = findJuplendLiquiditySupplyPositionPda(
//     mint,
//     lending,
//     JUPLEND_LIQUIDITY_PROGRAM_ID
//   );
//   const [borrowPositionOnLiquidity] = findJuplendLiquidityBorrowPositionPda(
//     mint,
//     lending,
//     JUPLEND_LIQUIDITY_PROGRAM_ID
//   );

//   // -----------------------------
//   // 1) Liquidity global init (idempotent)
//   // -----------------------------
//   if (!(await accountExists(liquidity))) {
//     const ix = await programs.liquidity.methods
//       .initLiquidity(admin.publicKey, admin.publicKey)
//       .accounts({
//         signer: admin.publicKey,
//         liquidity,
//         authList,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 2) Rewards global init (idempotent)
//   // -----------------------------
//   if (!(await accountExists(lendingRewardsAdmin))) {
//     const ix = await programs.rewards.methods
//       .initLendingRewardsAdmin(admin.publicKey, JUPLEND_LENDING_PROGRAM_ID)
//       .accounts({
//         signer: admin.publicKey,
//         lendingRewardsAdmin,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 3) Lending global init (idempotent)
//   // -----------------------------
//   if (!(await accountExists(lendingAdmin))) {
//     const ix = await programs.lending.methods
//       .initLendingAdmin(JUPLEND_LIQUIDITY_PROGRAM_ID, admin.publicKey, admin.publicKey)
//       .accounts({
//         authority: admin.publicKey,
//         lendingAdmin,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 4) Liquidity mint reserve init (idempotent)
//   // -----------------------------
//   if (!(await accountExists(tokenReserve))) {
//     const ix = await programs.liquidity.methods
//       .initTokenReserve()
//       .accounts({
//         authority: admin.publicKey,
//         liquidity,
//         authList,
//         mint,
//         vault,
//         rateModel,
//         tokenReserve,
//         tokenProgram,
//         associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // Configure rate model + token config every time (safe overwrite).
//   {
//     const rateIx = await programs.liquidity.methods
//       .updateRateDataV1({
//         kink: new BN(80).mul(DEFAULT_PERCENT_PRECISION), // 80%
//         rateAtUtilizationZero: new BN(4).mul(DEFAULT_PERCENT_PRECISION), // 4%
//         rateAtUtilizationKink: new BN(10).mul(DEFAULT_PERCENT_PRECISION), // 10%
//         rateAtUtilizationMax: new BN(150).mul(DEFAULT_PERCENT_PRECISION), // 150%
//       })
//       .accounts({
//         authority: admin.publicKey,
//         authList,
//         rateModel,
//         mint,
//         tokenReserve,
//       })
//       .instruction();

//     const tokenIx = await programs.liquidity.methods
//       .updateTokenConfig({
//         token: mint,
//         fee: new BN(0),
//         maxUtilization: new BN(100).mul(DEFAULT_PERCENT_PRECISION), // 100%
//       })
//       .accounts({
//         authority: admin.publicKey,
//         authList,
//         rateModel,
//         mint,
//         tokenReserve,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(rateIx, tokenIx),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 5) Rewards mint init (idempotent)
//   // -----------------------------
//   if (!(await accountExists(lendingRewardsRateModel))) {
//     const ix = await programs.rewards.methods
//       .initLendingRewardsRateModel()
//       .accounts({
//         authority: admin.publicKey,
//         lendingRewardsAdmin,
//         mint,
//         lendingRewardsRateModel,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 6) Lending mint init (idempotent)
//   // -----------------------------
//   const lendingNeedsInit = !(await accountExists(lending));
//   if (lendingNeedsInit) {
//     const initIx = await programs.lending.methods
//       .initLending(symbol, JUPLEND_LIQUIDITY_PROGRAM_ID)
//       .accounts({
//         signer: admin.publicKey,
//         lendingAdmin,
//         mint,
//         fTokenMint,
//         metadataAccount: fTokenMetadata,
//         lending,
//         tokenReservesLiquidity: tokenReserve,
//         tokenProgram,
//         systemProgram: SystemProgram.programId,
//         sysvarInstruction: SYSVAR_INSTRUCTIONS_PUBKEY,
//         metadataProgram: TOKEN_METADATA_PROGRAM_ID,
//         rent: SYSVAR_RENT_PUBKEY,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(initIx),
//       [admin],
//       false,
//       true
//     );
//   }

//   // Set rewards rate model only if we just created the lending account.
//   // (If already created, it was already set.)
//   if (lendingNeedsInit) {
//     const ix = await programs.lending.methods
//       .setRewardsRateModel(mint)
//       .accounts({
//         signer: admin.publicKey,
//         lendingAdmin,
//         lending,
//         fTokenMint,
//         newRewardsRateModel: lendingRewardsRateModel,
//         supplyTokenReservesLiquidity: tokenReserve,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 7) Init liquidity positions for the lending PDA (idempotent)
//   // -----------------------------
//   if (!(await accountExists(supplyPositionOnLiquidity))) {
//     const ix = await programs.liquidity.methods
//       .initNewProtocol(mint, mint, lending)
//       .accounts({
//         authority: admin.publicKey,
//         authList,
//         userSupplyPosition: supplyPositionOnLiquidity,
//         userBorrowPosition: borrowPositionOnLiquidity,
//         systemProgram: SystemProgram.programId,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(ix),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 7.5) Mint tokens to admin's ATA (required for debt ceiling validation + seed deposit)
//   // The liquidity program validates: max_debt_ceiling <= 10 * total_supply
//   // The admin also needs tokens for the seed deposit in juplend_init_position.
//   // -----------------------------
//   if (mintAuthority) {
//     // We need total_supply >= max_debt_ceiling / 10
//     // So we mint 10x the max debt ceiling to be safe
//     const mintAmount = DEFAULT_MAX_DEBT_CEILING.mul(new BN(10));

//     // Create an ATA for the admin to receive tokens (admin needs them for seed deposit)
//     const adminAta = getAssociatedTokenAddressSync(
//       mint,
//       admin.publicKey,
//       false,
//       tokenProgram,
//       ASSOCIATED_TOKEN_PROGRAM_ID
//     );

//     const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
//       admin.publicKey,    // payer
//       adminAta,           // ata
//       admin.publicKey,    // owner
//       mint,               // mint
//       tokenProgram,       // tokenProgram
//       ASSOCIATED_TOKEN_PROGRAM_ID
//     );

//     // Mint tokens using the actual mint authority, but to admin's ATA
//     const mintIx = createMintToInstruction(
//       mint,
//       adminAta,
//       mintAuthority.publicKey, // mint authority (signer)
//       BigInt(mintAmount.toString()),
//       [],
//       tokenProgram
//     );

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(createAtaIx, mintIx),
//       [admin, mintAuthority],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // 8) Allowances + class (safe overwrite)
//   // -----------------------------
//   {
//     const supplyConfigIx = await programs.liquidity.methods
//       .updateUserSupplyConfig({
//         mode: 1, // with interest
//         expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
//         expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
//         baseWithdrawalLimit: U64_MAX,
//       })
//       .accounts({
//         authority: admin.publicKey,
//         protocol: lending,
//         authList,
//         rateModel,
//         mint,
//         tokenReserve,
//         userSupplyPosition: supplyPositionOnLiquidity,
//       })
//       .instruction();

//     const borrowConfigIx = await programs.liquidity.methods
//       .updateUserBorrowConfig({
//         mode: 1, // with interest
//         expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
//         expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
//         baseDebtCeiling: DEFAULT_BASE_DEBT_CEILING,
//         maxDebtCeiling: DEFAULT_MAX_DEBT_CEILING,
//       })
//       .accounts({
//         authority: admin.publicKey,
//         protocol: lending,
//         authList,
//         rateModel,
//         mint,
//         tokenReserve,
//         userBorrowPosition: borrowPositionOnLiquidity,
//       })
//       .instruction();

//     const userClassIx = await programs.liquidity.methods
//       .updateUserClass([{ addr: lending, value: 1 }]) // 1 = can’t be paused (matches upstream tests)
//       .accounts({
//         authority: admin.publicKey,
//         authList,
//       })
//       .instruction();

//     await processBankrunTransaction(
//       bankrunContext,
//       new Transaction().add(supplyConfigIx, borrowConfigIx, userClassIx),
//       [admin],
//       false,
//       true
//     );
//   }

//   // -----------------------------
//   // Done
//   // -----------------------------
//   return {
//     mint,
//     tokenProgram,

//     // Program IDs
//     liquidityProgram: JUPLEND_LIQUIDITY_PROGRAM_ID,
//     lendingProgram: JUPLEND_LENDING_PROGRAM_ID,
//     liquidity,
//     authList,
//     tokenReserve,
//     rateModel,
//     vault,
//     lendingRewardsAdmin,
//     lendingRewardsRateModel,
//     lendingAdmin,
//     lending,
//     fTokenMint,
//     fTokenMetadata,
//     supplyPositionOnLiquidity,
//     borrowPositionOnLiquidity,

//     // CPI-friendly aliases
//     lendingSupplyPositionOnLiquidity: supplyPositionOnLiquidity,
//     lendingBorrowPositionOnLiquidity: borrowPositionOnLiquidity,
//   };
// }

// /**
//  * Convenience helper: build a Liquidity `update_exchange_price` instruction.
//  *
//  * This is useful for interest tests:
//  * - warp slots/time forward
//  * - call updateExchangePrice
//  * - then call Lending.update_rate (or marginfi deposit/withdraw which does it internally)
//  *
//  * NOTE: This instruction is permissionless - no authority/authList needed.
//  * IDL signature: accounts=[token_reserve, rate_model], args=[_mint]
//  */
// export async function makeJuplendUpdateExchangePriceIx(args: {
//   mint: PublicKey;
//   rateModel: PublicKey;
//   tokenReserve: PublicKey;
// }): Promise<TransactionInstruction> {
//   const programs = getJuplendPrograms();

//   return programs.liquidity.methods
//     .updateExchangePrice(args.mint)
//     .accounts({
//       tokenReserve: args.tokenReserve,
//       rateModel: args.rateModel,
//     })
//     .instruction();
// }

// /**
//  * Create a JupLend claim account for a user/mint pair.
//  *
//  * The claim account is required for withdraw operations despite being marked
//  * as optional in the IDL. This is a permissionless operation - anyone can
//  * create a claim account for any user/mint combination.
//  *
//  * Seeds: ["user_claim", user, mint] on Liquidity program.
//  */
// export async function ensureJuplendClaimAccount(args: {
//   payer: Keypair;
//   user: PublicKey;
//   mint: PublicKey;
// }): Promise<PublicKey> {
//   const { payer, user, mint } = args;

//   const [claimAccount] = findJuplendClaimAccountPda(user, mint);

//   // Check if already exists
//   if (await accountExists(claimAccount)) {
//     return claimAccount;
//   }

//   const programs = getJuplendPrograms();

//   const ix = await programs.liquidity.methods
//     .initClaimAccount(mint, user)
//     .accounts({
//       signer: payer.publicKey,
//       claimAccount,
//       systemProgram: SystemProgram.programId,
//     })
//     .instruction();

//   await processBankrunTransaction(
//     bankrunContext,
//     new Transaction().add(ix),
//     [payer],
//     false,
//     true
//   );

//   return claimAccount;
// }
