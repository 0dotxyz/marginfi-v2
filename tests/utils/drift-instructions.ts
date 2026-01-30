import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import BN from "bn.js";
import { Marginfi } from "../../target/types/marginfi";
import { Program } from "@coral-xyz/anchor";
import {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { DRIFT_PROGRAM_ID } from "./types";
import { DriftConfigCompact } from "./drift-utils";
import { Drift } from "../fixtures/drift_v2";
import {
  deriveDriftStatePDA,
  deriveSpotMarketVaultPDA,
} from "./pdas";
import { globalFeeWallet } from "../rootHooks";

export interface AddDriftBankAccounts {
  group: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  integrationAcc1: PublicKey;
  oracle: PublicKey;
  tokenProgram?: PublicKey;
}

export interface AddDriftBankArgs {
  seed: BN;
  config: DriftConfigCompact;
}

export interface InitDriftUserAccounts {
  feePayer: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  driftOracle?: PublicKey; // Oracle account for the asset (not needed if using oracle type QuoteAsset)
  tokenProgram?: PublicKey;
}

export interface InitDriftUserArgs {
  amount: BN;
}

/**
 * Add a Drift bank to the marginfi lending pool
 *
 * This instruction creates a marginfi bank that integrates with a Drift spot market.
 * It requires:
 * - group: The marginfi group to add the bank to (must be admin)
 * - feePayer: Account that pays for the transaction
 * - bankMint: The token mint that matches the Drift spot market mint
 * - integrationAcc1: The Drift spot market account
 *
 * Note: The oracle is specified in the config.oracle field, not as an account
 *
 * @param program The marginfi program
 * @param accounts The required accounts
 * @param args The configuration and seed for the bank
 * @returns The instruction to add a Drift bank
 */
export const makeAddDriftBankIx = (
  program: Program<Marginfi>,
  accounts: AddDriftBankAccounts,
  args: AddDriftBankArgs
): Promise<TransactionInstruction> => {
  const oracleMeta: AccountMeta = {
    pubkey: accounts.oracle,
    isSigner: false,
    isWritable: false,
  };
  const spotMarketMeta: AccountMeta = {
    pubkey: accounts.integrationAcc1,
    isSigner: false,
    isWritable: false,
  };

  const ix = program.methods
    .lendingPoolAddBankDrift(args.config, args.seed)
    .accounts({
      integrationAcc1: accounts.integrationAcc1,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
      ...accounts,
    })
    .remainingAccounts([oracleMeta, spotMarketMeta])
    .instruction();

  return ix;
};

/**
 * Initialize Drift user accounts for a marginfi Drift bank
 *
 * This instruction creates the Drift user and user stats accounts that are required
 * for the marginfi bank to interact with Drift protocol. Must be called after
 * creating a Drift bank. Requires a minimum deposit to ensure the account remains active.
 *
 * @param program The marginfi program
 * @param accounts The required accounts including signer's token account
 * @param args The amount to deposit (minimum 10 units)
 * @returns The instruction to initialize Drift user accounts
 */
export const makeInitDriftUserIx = async (
  program: Program<Marginfi>,
  accounts: InitDriftUserAccounts,
  args: InitDriftUserArgs,
  marketIndex: number
): Promise<TransactionInstruction> => {
  // Derive the drift state PDA using helper function
  const [driftState] = deriveDriftStatePDA(DRIFT_PROGRAM_ID);

  // Derive the spot market vault PDA using the market index
  const [driftSpotMarketVault] = deriveSpotMarketVaultPDA(
    DRIFT_PROGRAM_ID,
    marketIndex
  );

  const ix = program.methods
    .driftInitUser(args.amount)
    .accounts({
      feePayer: accounts.feePayer,
      signerTokenAccount: accounts.signerTokenAccount,
      bank: accounts.bank,
      driftState,
      driftSpotMarketVault,
      driftOracle: accounts.driftOracle || null,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export interface DriftDepositAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  driftOracle?: PublicKey; // Oracle account for the asset (not needed if using oracle type QuoteAsset)
  tokenProgram?: PublicKey;
}

/**
 * Deposit tokens into a Drift spot market through marginfi
 *
 * This instruction deposits tokens from the user's account into a Drift spot market
 * via a marginfi bank that's configured for Drift integration.
 *
 * Required remaining accounts for Drift (in order):
 * 1. Oracle account (optional - only if not using oracle type QuoteAsset)
 * 2. Spot market account (always required)
 * 3. Token mint (optional - only for Token-2022)
 *
 * @param program The marginfi program
 * @param accounts The required accounts
 * @param amount The amount to deposit (in token base units)
 * @param marketIndex The drift spot market index
 * @returns The instruction to deposit to Drift
 */
export const makeDriftDepositIx = async (
  program: Program<Marginfi>,
  accounts: DriftDepositAccounts,
  amount: BN,
  marketIndex: number
): Promise<TransactionInstruction> => {
  // Derive PDAs using helper functions
  const [driftState] = deriveDriftStatePDA(DRIFT_PROGRAM_ID);
  const [driftSpotMarketVault] = deriveSpotMarketVaultPDA(
    DRIFT_PROGRAM_ID,
    marketIndex
  );

  const ix = await program.methods
    .driftDeposit(amount)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      signerTokenAccount: accounts.signerTokenAccount,
      driftState,
      driftSpotMarketVault,
      driftOracle: accounts.driftOracle || null,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export interface DriftWithdrawAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  destinationTokenAccount: PublicKey;
  driftOracle?: PublicKey; // not needed if using oracle type QuoteAsset
  driftRewardOracle?: PublicKey; // only needed if rewards exist
  driftRewardSpotMarket?: PublicKey; // only needed if rewards exist
  driftRewardMint?: PublicKey; // only needed if rewards exist
  driftRewardOracle2?: PublicKey; // backup in case multiple rewards
  driftRewardSpotMarket2?: PublicKey; // backup in case multiple rewards
  driftRewardMint2?: PublicKey; // backup in case multiple rewards
  tokenProgram?: PublicKey;
}

export interface DriftWithdrawArgs {
  amount: BN;
  withdraw_all: boolean;
  remaining: PublicKey[];
}

/**
 * Withdraw tokens from a Drift spot market through marginfi
 *
 * This instruction withdraws tokens from a Drift spot market via a marginfi bank
 * that's configured for Drift integration. The tokens are transferred to the
 * user's destination token account.
 *
 * Optional reward accounts are required when Drift admin has deposited reward assets:
 * - driftRewardOracle + driftRewardSpotMarket: Required if 2+ active deposits exist
 * - driftRewardOracle2 + driftRewardSpotMarket2: Required if exactly 3 active deposits exist
 *
 * Required remaining accounts for Drift (in order):
 * 1. Oracle account (optional - only if not using oracle type QuoteAsset)
 * 2. Spot market account (always required)
 * 3. Token mint (optional - only for Token-2022)
 *
 * @param program The marginfi program
 * @param accounts The required accounts including destination for withdrawn tokens
 * @param args The withdrawal parameters including amount and withdraw_all flag
 * @param driftProgram The Drift program to fetch spot market data
 * @returns The instruction to withdraw from Drift
 */
export const makeDriftWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: DriftWithdrawAccounts,
  args: DriftWithdrawArgs,
  driftProgram: Program<Drift>
): Promise<TransactionInstruction> => {
  // Get the bank to find the drift spot market
  const bank = await program.account.bank.fetch(accounts.bank);

  // Load the drift spot market to get the market index
  const driftSpotMarket = await driftProgram.account.spotMarket.fetch(
    bank.integrationAcc1
  );
  const marketIndex = driftSpotMarket.marketIndex;

  // Derive PDAs using helper functions - similar to deposit
  const [driftState] = deriveDriftStatePDA(DRIFT_PROGRAM_ID);
  const [driftSpotMarketVault] = deriveSpotMarketVaultPDA(
    DRIFT_PROGRAM_ID,
    marketIndex
  );
  const [driftSigner] = PublicKey.findProgramAddressSync(
    [Buffer.from("drift_signer")],
    DRIFT_PROGRAM_ID
  );

  const ix = await program.methods
    .driftWithdraw(args.amount, args.withdraw_all ? true : null)
    .accounts({
      marginfiAccount: accounts.marginfiAccount,
      bank: accounts.bank,
      destinationTokenAccount: accounts.destinationTokenAccount,
      driftState,
      driftSpotMarketVault,
      driftSigner,
      driftOracle: accounts.driftOracle || null,
      driftRewardOracle: accounts.driftRewardOracle || null,
      driftRewardSpotMarket: accounts.driftRewardSpotMarket || null,
      driftRewardMint: accounts.driftRewardMint || null,
      driftRewardOracle2: accounts.driftRewardOracle2 || null,
      driftRewardSpotMarket2: accounts.driftRewardSpotMarket2 || null,
      driftRewardMint2: accounts.driftRewardMint2 || null,
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

export interface DriftHarvestRewardAccounts {
  bank: PublicKey;
  harvestDriftSpotMarket: PublicKey;
  tokenProgram?: PublicKey;
}

/**
 * Harvest rewards from admin deposits in Drift positions 2+
 *
 * This instruction allows withdrawing from Drift positions that were created
 * by admin deposits (positions 2-7). The tokens are sent to the global fee wallet.
 *
 * The instruction will automatically derive all necessary PDAs including:
 * - Fee state, liquidity vault authority, and associated token accounts
 * - Drift state, user, user stats, and spot market vault
 * - Reward mint (fetched from harvest spot market)
 *
 * Remaining accounts should be passed in the order required by Drift's withdraw instruction:
 * 1. Oracle accounts (optional)
 * 2. Spot market accounts (always required)
 * 3. Token mint (optional for Token-2022)
 *
 * @param program The marginfi program
 * @param driftProgram The Drift program for fetching spot market data
 * @param accounts The required accounts
 * @param remainingAccounts Additional accounts required by Drift (oracles, markets, etc)
 * @returns The instruction to harvest rewards
 */
export const makeDriftHarvestRewardIx = async (
  program: Program<Marginfi>,
  driftProgram: Program<Drift>,
  accounts: DriftHarvestRewardAccounts,
  remainingAccounts: AccountMeta[] = []
): Promise<TransactionInstruction> => {
  const [driftState] = deriveDriftStatePDA(DRIFT_PROGRAM_ID);

  const [driftSigner] = PublicKey.findProgramAddressSync(
    [Buffer.from("drift_signer")],
    DRIFT_PROGRAM_ID
  );

  const harvestSpotMarket = await driftProgram.account.spotMarket.fetch(
    accounts.harvestDriftSpotMarket
  );

  const rewardMint = harvestSpotMarket.mint;

  const [harvestDriftSpotMarketVault] = deriveSpotMarketVaultPDA(
    DRIFT_PROGRAM_ID,
    harvestSpotMarket.marketIndex
  );

  const expectedDestinationTokenAccount = getAssociatedTokenAddressSync(
    rewardMint,
    globalFeeWallet,
    false // allowOwnerOffCurve
  );

  // 6. Build instruction
  return (
    program.methods
      .driftHarvestReward()
      .accounts({
        bank: accounts.bank,
        // feeState is auto-derived via seeds constraint
        driftState,
        // integration_acc_2 and integration_acc_3 are auto-included via has_one constraint
        harvestDriftSpotMarket: accounts.harvestDriftSpotMarket,
        harvestDriftSpotMarketVault,
        driftSigner,
        rewardMint,
        tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
      })
      // Explicit ATA required: authority is fee_state.global_fee_wallet (on-chain data),
      // which Anchor TS cannot use for auto account resolution.
      .accountsPartial({
        destinationTokenAccount: expectedDestinationTokenAccount,
      })
      .remainingAccounts(remainingAccounts)
      .instruction()
  );
};

export interface DepositIntoSpotMarketVaultAccounts {
  spotMarket: PublicKey;
  admin: PublicKey;
  sourceVault: PublicKey;
  spotMarketVault: PublicKey;
  tokenProgram?: PublicKey;
}

export interface DepositIntoSpotMarketVaultArgs {
  amount: BN;
  remainingAccounts?: PublicKey[];
}

/**
 * Deposit tokens into a Drift spot market vault (admin-only).
 *
 * This instruction increases cumulative deposit interest by topping up the
 * spot market vault, improving depositor value without touching user accounts.
 *
 * @param program The Drift program
 * @param accounts The required accounts
 * @param args The amount and optional remaining accounts
 * @returns The instruction to deposit into the spot market vault
 */
export const makeDepositIntoSpotMarketVaultIx = async (
  program: Program<Drift>,
  accounts: DepositIntoSpotMarketVaultAccounts,
  args: DepositIntoSpotMarketVaultArgs
): Promise<TransactionInstruction> => {
  const [driftState] = deriveDriftStatePDA(program.programId);

  return program.methods
    .depositIntoSpotMarketVault(args.amount)
    .accounts({
      state: driftState,
      spotMarket: accounts.spotMarket,
      admin: accounts.admin,
      sourceVault: accounts.sourceVault,
      spotMarketVault: accounts.spotMarketVault,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(
      (args.remainingAccounts || []).map((pubkey) => ({
        pubkey,
        isSigner: false,
        isWritable: false,
      }))
    )
    .instruction();
};
