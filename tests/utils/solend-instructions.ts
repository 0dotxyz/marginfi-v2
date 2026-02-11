import {
  PublicKey,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import { BN, Program } from "@coral-xyz/anchor";
import {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { Marginfi } from "../../target/types/marginfi";
import {
  SolendConfigCompact,
  deriveLendingMarketAuthority,
  SOLEND_NULL_PUBKEY,
  parseSolendReserve,
} from "./solend-utils";
import { deriveBankWithSeed, deriveLiquidityVaultAuthority } from "./pdas";
import { SOLEND_PROGRAM_ID } from "./types";

export interface AddSolendBankAccounts {
  group: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  integrationAcc1: PublicKey;
  solendMarket: PublicKey;
  oracle: PublicKey;
  tokenProgram?: PublicKey;
}

export interface AddSolendBankArgs {
  seed: BN;
  config: SolendConfigCompact;
}

/**
 * Add a Solend bank to the marginfi lending pool
 *
 * This instruction creates a marginfi bank that integrates with a Solend reserve.
 * It requires:
 * - group: The marginfi group to add the bank to (must be admin)
 * - feePayer: Account that pays for the transaction
 * - bankMint: The token mint that matches the Solend reserve mint
 * - integrationAcc1: The Solend reserve account
 * - solendMarket: The Solend lending market account
 * - oracle: The oracle account for price feeds
 *
 * @param program The marginfi program
 * @param accounts The required accounts
 * @param args The configuration and seed for the bank
 * @returns The instruction to add a Solend bank
 */
export const makeAddSolendBankIx = async (
  program: Program<Marginfi>,
  accounts: AddSolendBankAccounts,
  args: AddSolendBankArgs
): Promise<TransactionInstruction> => {
  // Derive the bank PDA
  const [bank] = deriveBankWithSeed(
    program.programId,
    accounts.group,
    accounts.bankMint,
    args.seed
  );

  // Derive the liquidity vault authority for the bank
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bank
  );

  const ix = await program.methods
    .lendingPoolAddBankSolend(args.config, args.seed)
    .accounts({
      group: accounts.group,
      feePayer: accounts.feePayer,
      bankMint: accounts.bankMint,
      integrationAcc1: accounts.integrationAcc1,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .remainingAccounts([
      {
        pubkey: accounts.oracle,
        isSigner: false,
        isWritable: false,
      },
      {
        pubkey: accounts.integrationAcc1,
        isSigner: false,
        isWritable: false,
      },
    ])
    .instruction();

  return ix;
};

export interface SolendInitObligationAccounts {
  feePayer: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  lendingMarket: PublicKey; // The Solend lending market
  pythPrice: PublicKey; // Oracle account (pass the actual oracle from the bank config)
  tokenProgram?: PublicKey;
}

export interface SolendInitObligationArgs {
  amount: BN;
}

/**
 * Initialize a Solend obligation for a marginfi bank
 *
 * This instruction initializes the Solend obligation that allows the marginfi bank
 * to interact with Solend protocol. It requires a minimum deposit to ensure the
 * obligation remains active. Must be called after creating a Solend bank.
 *
 * @param program The marginfi program
 * @param accounts The required accounts including signer's token account
 * @param args The amount to deposit (minimum 10 units in native decimals)
 * @returns The instruction to initialize Solend obligation
 */
export const makeSolendInitObligationIx = async (
  program: Program<Marginfi>,
  accounts: SolendInitObligationAccounts,
  args: SolendInitObligationArgs
): Promise<TransactionInstruction> => {
  // Load the bank to get the solend reserve
  const bank = await program.account.bank.fetch(accounts.bank);

  // Load the Solend reserve to get liquidity supply and collateral mint
  const reserveData = await program.provider.connection.getAccountInfo(
    bank.integrationAcc1
  );
  if (!reserveData) {
    throw new Error("Solend reserve not found");
  }

  // Parse the reserve using the SDK
  const reserve = parseSolendReserve(bank.integrationAcc1, reserveData);
  const liquiditySupplyPubkey = reserve.info.liquidity.supplyPubkey;
  const collateralMintPubkey = reserve.info.collateral.mintPubkey;
  const collateralSupplyPubkey = reserve.info.collateral.supplyPubkey;

  // Derive the lending market authority PDA
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  );

  // Derive liquidity vault authority
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank
  );

  // Derive user collateral ATA for the cTokens
  const userCollateral = getAssociatedTokenAddressSync(
    collateralMintPubkey,
    liquidityVaultAuthority,
    true // allowOwnerOffCurve since it's a PDA
  );

  const ix = await program.methods
    .solendInitObligation(args.amount)
    .accounts({
      feePayer: accounts.feePayer,
      bank: accounts.bank,
      signerTokenAccount: accounts.signerTokenAccount,
      lendingMarket: accounts.lendingMarket,
      lendingMarketAuthority,
      reserveLiquiditySupply: liquiditySupplyPubkey,
      reserveCollateralMint: collateralMintPubkey,
      reserveCollateralSupply: collateralSupplyPubkey,
      userCollateral,
      pythPrice: accounts.pythPrice,
      switchboardFeed: SOLEND_NULL_PUBKEY, // Default to null oracle
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export interface SolendDepositAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  pythPrice: PublicKey; // Oracle account - use the oracle from bank config
  tokenProgram?: PublicKey;
}

export interface SolendDepositArgs {
  amount: BN;
}

/**
 * Deposit tokens into a Solend reserve through marginfi
 *
 * This instruction deposits tokens from the user's account into a Solend reserve
 * via a marginfi bank that's configured for Solend integration.
 *
 * @param program The marginfi program
 * @param accounts The required accounts
 * @param args The amount to deposit (in token base units)
 * @returns The instruction to deposit to Solend
 */
export const makeSolendDepositIx = async (
  program: Program<Marginfi>,
  accounts: SolendDepositAccounts,
  args: SolendDepositArgs
): Promise<TransactionInstruction> => {
  // Load the bank to get the solend reserve
  const bank = await program.account.bank.fetch(accounts.bank);

  // Load the Solend reserve to get liquidity supply and collateral mint
  const reserveData = await program.provider.connection.getAccountInfo(
    bank.integrationAcc1
  );
  if (!reserveData) {
    throw new Error("Solend reserve not found");
  }

  // Parse the reserve using the SDK
  const reserve = parseSolendReserve(bank.integrationAcc1, reserveData);
  const liquiditySupplyPubkey = reserve.info.liquidity.supplyPubkey;
  const collateralMintPubkey = reserve.info.collateral.mintPubkey;
  const collateralSupplyPubkey = reserve.info.collateral.supplyPubkey;

  // Derive the lending market authority PDA
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  );

  // Derive liquidity vault authority
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank
  );

  // Derive user collateral ATA for the cTokens
  const userCollateral = getAssociatedTokenAddressSync(
    collateralMintPubkey,
    liquidityVaultAuthority,
    true
  );

  const ix = await program.methods
    .solendDeposit(args.amount)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      signerTokenAccount: accounts.signerTokenAccount,
      lendingMarket: accounts.lendingMarket,
      lendingMarketAuthority,
      reserveLiquiditySupply: liquiditySupplyPubkey,
      reserveCollateralMint: collateralMintPubkey,
      reserveCollateralSupply: collateralSupplyPubkey,
      userCollateral,
      pythPrice: accounts.pythPrice,
      switchboardFeed: SOLEND_NULL_PUBKEY,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export interface SolendWithdrawAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  destinationTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  pythPrice: PublicKey; // Oracle account - use the oracle from bank config
  tokenProgram?: PublicKey;
}

export interface SolendWithdrawArgs {
  amount: BN;
  withdrawAll: boolean;
  /** Oracle and other remaining accounts needed for health checks */
  remaining: PublicKey[];
}

/**
 * Withdraw tokens from a Solend reserve through marginfi
 *
 * This instruction withdraws tokens from a Solend reserve via a marginfi bank
 * that's configured for Solend integration. The tokens are transferred to the
 * user's destination token account.
 *
 * @param program The marginfi program
 * @param accounts The required accounts including destination for withdrawn tokens
 * @param args The withdrawal parameters including amount and withdrawAll flag
 * @returns The instruction to withdraw from Solend
 */
export const makeSolendWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: SolendWithdrawAccounts,
  args: SolendWithdrawArgs
): Promise<TransactionInstruction> => {
  // Load the bank to get the solend reserve
  const bank = await program.account.bank.fetch(accounts.bank);

  // Load the Solend reserve to get liquidity supply and collateral mint
  const reserveData = await program.provider.connection.getAccountInfo(
    bank.integrationAcc1
  );
  if (!reserveData) {
    throw new Error("Solend reserve not found");
  }

  // Parse the reserve using the SDK
  const reserve = parseSolendReserve(bank.integrationAcc1, reserveData);
  const liquiditySupplyPubkey = reserve.info.liquidity.supplyPubkey;
  const collateralMintPubkey = reserve.info.collateral.mintPubkey;
  const collateralSupplyPubkey = reserve.info.collateral.supplyPubkey;

  // Derive the lending market authority PDA
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  );

  // Derive liquidity vault authority
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank
  );

  // Derive user collateral ATA for the cTokens
  const userCollateral = getAssociatedTokenAddressSync(
    collateralMintPubkey,
    liquidityVaultAuthority,
    true
  );

  const ix = await program.methods
    .solendWithdraw(args.amount, args.withdrawAll ? true : null)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      destinationTokenAccount: accounts.destinationTokenAccount,
      lendingMarket: accounts.lendingMarket,
      lendingMarketAuthority,
      reserveLiquiditySupply: liquiditySupplyPubkey,
      reserveCollateralMint: collateralMintPubkey,
      reserveCollateralSupply: collateralSupplyPubkey,
      userCollateral,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(
      args.remaining.map((pubkey) => ({
        pubkey,
        isSigner: false,
        isWritable: false,
      }))
    )
    .instruction();

  return ix;
};
