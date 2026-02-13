import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import {
  getMint,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  createSyncNativeInstruction,
  NATIVE_MINT,
} from "@solana/spl-token";
import { loadAltStagingEnv } from "./lib/env";
import { deriveLiquidityVaultAuthority, deriveBankWithSeed } from "./lib/pdas";
import {
  deriveJuplendCpiAccounts,
  findJuplendLendingAdminPda,
  JUPLEND_LENDING_PROGRAM_ID,
} from "./lib/juplend-pdas";
import { setupAdmin } from "./lib/setup";
import { simulateAndSend } from "./lib/tx";

/**
 * Initialize the JupLend position for a bank (seed deposit + activate).
 *
 * This script:
 * 1. Fetches the Bank account to get mint, juplend_lending, juplend_f_token_vault
 * 2. Derives all JupLend CPI accounts
 * 3. Auto-wraps SOL if the bank mint is WSOL
 * 4. Calls juplendInitPosition with a seed deposit
 *
 * Usage:
 *   npx ts-node ops/alt-staging/scripts/init-juplend-position.ts <bank-pubkey> [--amount <native>] [--send]
 *
 * Example:
 *   npx ts-node ops/alt-staging/scripts/init-juplend-position.ts 5xYz... --amount 1000 --send
 */

type Args = {
  bankPubkey: string;
  amount: number;
  send: boolean;
};

function parseArgs(argv: string[]): Args {
  const out: Args = {
    bankPubkey: "",
    amount: 10000, // Default seed deposit amount (in native units)
    send: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--send") {
      out.send = true;
    } else if (a === "--amount") {
      out.amount = Number(argv[++i]);
    } else if (!a.startsWith("--")) {
      out.bankPubkey = a;
    }
  }

  if (!out.bankPubkey) {
    throw new Error(
      "Usage: npx ts-node ops/alt-staging/scripts/init-juplend-position.ts <bank-pubkey> [--amount <native>] [--send]"
    );
  }

  return out;
}

// Bank account layout offsets (simplified)
const BANK_MINT_OFFSET = 8;
const BANK_JUPLEND_LENDING_OFFSET = 1736; // Bank.juplend_lending field
const BANK_JUPLEND_F_TOKEN_VAULT_OFFSET = 1768; // Bank.juplend_f_token_vault field
const BANK_LIQUIDITY_VAULT_OFFSET = 120; // Bank.liquidity_vault field

async function main() {
  const args = parseArgs(process.argv.slice(2));

  loadAltStagingEnv();

  const { connection, wallet, program, programId } = setupAdmin();

  const bank = new PublicKey(args.bankPubkey);

  // Fetch Bank account data
  const bankInfo = await connection.getAccountInfo(bank);
  if (!bankInfo) {
    throw new Error(`Bank not found: ${bank.toBase58()}`);
  }

  const bankData = bankInfo.data;

  // Extract mint
  const mintBytes = bankData.slice(BANK_MINT_OFFSET, BANK_MINT_OFFSET + 32);
  const mint = new PublicKey(mintBytes);

  // Extract liquidity_vault
  const liquidityVaultBytes = bankData.slice(
    BANK_LIQUIDITY_VAULT_OFFSET,
    BANK_LIQUIDITY_VAULT_OFFSET + 32
  );
  const liquidityVault = new PublicKey(liquidityVaultBytes);

  // Extract juplend_lending
  const juplendLendingBytes = bankData.slice(
    BANK_JUPLEND_LENDING_OFFSET,
    BANK_JUPLEND_LENDING_OFFSET + 32
  );
  const juplendLending = new PublicKey(juplendLendingBytes);

  // Extract juplend_f_token_vault
  const juplendFTokenVaultBytes = bankData.slice(
    BANK_JUPLEND_F_TOKEN_VAULT_OFFSET,
    BANK_JUPLEND_F_TOKEN_VAULT_OFFSET + 32
  );
  const juplendFTokenVault = new PublicKey(juplendFTokenVaultBytes);

  // Derive liquidity vault authority
  const liquidityVaultAuthority = deriveLiquidityVaultAuthority(programId, bank);

  // Token program auto-detect
  let tokenProgram = TOKEN_PROGRAM_ID;
  try {
    await getMint(connection, mint, "confirmed", TOKEN_2022_PROGRAM_ID);
    tokenProgram = TOKEN_2022_PROGRAM_ID;
  } catch {
    // ignore
  }

  // Derive JupLend CPI accounts
  const juplendAccounts = deriveJuplendCpiAccounts(mint, tokenProgram);
  const [lendingAdmin] = findJuplendLendingAdminPda(JUPLEND_LENDING_PROGRAM_ID);

  // Get fToken mint from juplend lending account
  const lendingInfo = await connection.getAccountInfo(juplendLending);
  if (!lendingInfo) {
    throw new Error(`JupLend Lending not found: ${juplendLending.toBase58()}`);
  }
  // fTokenMint is at offset 32 (after mint) in the Lending account
  const fTokenMintBytes = lendingInfo.data.slice(8 + 32, 8 + 32 + 32);
  const fTokenMint = new PublicKey(fTokenMintBytes);

  // Get token_reserves_liquidity and supply_position_on_liquidity from Lending account
  // Offsets from Lending struct: after discriminator(8), mint(32), f_token_mint(32), lending_id(2), decimals(1), rewards_rate_model(32), liquidity_exchange_price(8), token_exchange_price(8), last_update_timestamp(8)
  // token_reserves_liquidity is at offset 8 + 32 + 32 + 2 + 1 + 32 + 8 + 8 + 8 = 131
  const tokenReservesLiquidityBytes = lendingInfo.data.slice(131, 131 + 32);
  const supplyTokenReservesLiquidity = new PublicKey(tokenReservesLiquidityBytes);

  const supplyPositionBytes = lendingInfo.data.slice(131 + 32, 131 + 32 + 32);
  const lendingSupplyPositionOnLiquidity = new PublicKey(supplyPositionBytes);

  // Get user's token account (ATA for the underlying mint)
  const signerTokenAccount = getAssociatedTokenAddressSync(
    mint,
    wallet.publicKey,
    false,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const isWsol = mint.equals(NATIVE_MINT);
  const seedAmount = new BN(args.amount);

  console.log("=== init-juplend-position ===");
  console.log("Program:", programId.toBase58());
  console.log("Bank:", bank.toBase58());
  console.log("Mint:", mint.toBase58());
  console.log("Is WSOL:", isWsol);
  console.log("Seed amount:", seedAmount.toString());
  console.log("Fee payer:", wallet.publicKey.toBase58());
  console.log("Signer token account:", signerTokenAccount.toBase58());
  console.log("Liquidity vault:", liquidityVault.toBase58());
  console.log("Liquidity vault authority:", liquidityVaultAuthority.toBase58());
  console.log("JupLend Lending:", juplendLending.toBase58());
  console.log("fToken Mint:", fTokenMint.toBase58());
  console.log("fToken Vault:", juplendFTokenVault.toBase58());
  console.log("Lending Admin:", lendingAdmin.toBase58());
  console.log("Supply Token Reserves:", supplyTokenReservesLiquidity.toBase58());
  console.log("Supply Position:", lendingSupplyPositionOnLiquidity.toBase58());
  console.log("Rate Model:", juplendAccounts.rateModel.toBase58());
  console.log("Vault:", juplendAccounts.vault.toBase58());
  console.log("Liquidity:", juplendAccounts.liquidity.toBase58());
  console.log("Liquidity Program:", juplendAccounts.liquidityProgram.toBase58());
  console.log("Rewards Rate Model:", juplendAccounts.rewardsRateModel.toBase58());
  console.log();

  const tx = new Transaction();

  // If WSOL, we need to wrap SOL
  if (isWsol) {
    // Check if ATA exists
    const ataInfo = await connection.getAccountInfo(signerTokenAccount);
    if (!ataInfo) {
      console.log("Creating WSOL ATA...");
      tx.add(
        createAssociatedTokenAccountInstruction(
          wallet.publicKey,
          signerTokenAccount,
          wallet.publicKey,
          mint,
          tokenProgram,
          ASSOCIATED_TOKEN_PROGRAM_ID
        )
      );
    }

    // Transfer SOL to the ATA and sync
    console.log(`Wrapping ${seedAmount.toString()} lamports as WSOL...`);
    tx.add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: signerTokenAccount,
        lamports: seedAmount.toNumber(),
      })
    );
    tx.add(createSyncNativeInstruction(signerTokenAccount, tokenProgram));
  } else {
    // Check balance for non-WSOL
    const ataInfo = await connection.getAccountInfo(signerTokenAccount);
    if (!ataInfo) {
      throw new Error(
        `Signer token account not found: ${signerTokenAccount.toBase58()}. ` +
          `Please create an ATA and fund it with at least ${seedAmount.toString()} tokens.`
      );
    }
  }

  // Build init position instruction
  const initPositionIx = await (program.methods as any)
    .juplendInitPosition(seedAmount)
    .accounts({
      feePayer: wallet.publicKey,
      signerTokenAccount,
      bank,
      liquidityVaultAuthority,
      liquidityVault,
      mint,
      juplendLending,
      fTokenMint,
      juplendFTokenVault,
      lendingAdmin,
      supplyTokenReservesLiquidity,
      lendingSupplyPositionOnLiquidity,
      rateModel: juplendAccounts.rateModel,
      vault: juplendAccounts.vault,
      liquidity: juplendAccounts.liquidity,
      liquidityProgram: juplendAccounts.liquidityProgram,
      rewardsRateModel: juplendAccounts.rewardsRateModel,
      juplendProgram: JUPLEND_LENDING_PROGRAM_ID,
      tokenProgram,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .instruction();

  tx.add(initPositionIx);

  await simulateAndSend({
    connection,
    transaction: tx,
    feePayer: wallet.publicKey,
    signers: [wallet.payer],
    send: args.send,
    label: "init-juplend-position",
  });

  console.log("\n Bank activated:", bank.toBase58());
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
