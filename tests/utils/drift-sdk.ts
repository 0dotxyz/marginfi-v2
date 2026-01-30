import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  AccountMeta,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import BN from "bn.js";
import { Drift } from "tests/fixtures/drift_v2";
import {
  DriftOracleSource,
  DriftAssetTier,
  DriftAssetTierValues,
} from "./drift-utils";
import {
  deriveDriftStatePDA,
  deriveDriftSignerPDA,
  deriveSpotMarketPDA,
  deriveSpotMarketVaultPDA,
  deriveInsuranceFundVaultPDA,
  deriveUserPDA,
  deriveUserStatsPDA,
} from "./pdas";

// Drift instruction helper functions

interface InitializeDriftAccounts {
  admin: PublicKey;
  usdcMint: PublicKey;
}

export const makeInitializeDriftIx = async (
  program: Program<Drift>,
  accounts: InitializeDriftAccounts,
): Promise<TransactionInstruction> => {
  const [statePublicKey] = deriveDriftStatePDA(program.programId);
  const [driftSigner] = deriveDriftSignerPDA(program.programId);

  return program.methods
    .initialize()
    .accounts({
      admin: accounts.admin,
      state: statePublicKey,
      quoteAssetMint: accounts.usdcMint,
      driftSigner: driftSigner,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();
};

interface InitializeSpotMarketAccounts {
  admin: PublicKey;
  spotMarketMint: PublicKey;
  oracle: PublicKey;
}

interface InitializeSpotMarketArgs {
  optimalUtilization: number;
  optimalRate: number;
  maxRate: number;
  oracleSource: DriftOracleSource;
  // assetTier: DriftAssetTier;
  initialAssetWeight: number;
  maintenanceAssetWeight: number;
  initialLiabilityWeight: number;
  maintenanceLiabilityWeight: number;
  marketIndex: number;
  activeStatus?: boolean; // Optional, defaults to true
}

export const makeInitializeSpotMarketIx = async (
  program: Program<Drift>,
  accounts: InitializeSpotMarketAccounts,
  args: InitializeSpotMarketArgs,
): Promise<TransactionInstruction> => {
  const [statePublicKey] = deriveDriftStatePDA(program.programId);
  const [spotMarketPublicKey] = deriveSpotMarketPDA(
    program.programId,
    args.marketIndex,
  );
  const [spotMarketVault] = deriveSpotMarketVaultPDA(
    program.programId,
    args.marketIndex,
  );
  const [insuranceFundVault] = deriveInsuranceFundVaultPDA(
    program.programId,
    args.marketIndex,
  );
  const [driftSigner] = deriveDriftSignerPDA(program.programId);

  return program.methods
    .initializeSpotMarket(
      args.optimalUtilization,
      args.optimalRate,
      args.maxRate,
      args.oracleSource,
      args.initialAssetWeight,
      args.maintenanceAssetWeight,
      args.initialLiabilityWeight,
      args.maintenanceLiabilityWeight,
      0, // imfFactor
      0, // liquidatorFee
      0, // ifLiquidationFee
      args.activeStatus ?? true, // activeStatus - defaults to true if not provided
      // args.assetTier,
      DriftAssetTierValues.collateral,
      new BN(0), // scaleInitialAssetWeightStart
      new BN(10_000_000_000_000), // withdrawGuardThreshold
      new BN(1), // orderTickSize
      new BN(1), // orderStepSize
      0, // ifTotalFactor
      Array(32).fill(0), // name
    )
    .accounts({
      spotMarket: spotMarketPublicKey,
      spotMarketMint: accounts.spotMarketMint,
      spotMarketVault: spotMarketVault,
      insuranceFundVault: insuranceFundVault,
      driftSigner: driftSigner,
      state: statePublicKey,
      oracle: accounts.oracle,
      admin: accounts.admin,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();
};

interface InitializeUserStatsAccounts {
  authority: PublicKey;
  payer: PublicKey;
}

export const makeInitializeUserStatsIx = async (
  program: Program<Drift>,
  accounts: InitializeUserStatsAccounts,
): Promise<TransactionInstruction> => {
  const [userStatsPublicKey] = deriveUserStatsPDA(
    program.programId,
    accounts.authority,
  );
  const [statePublicKey] = deriveDriftStatePDA(program.programId);

  return program.methods
    .initializeUserStats()
    .accounts({
      userStats: userStatsPublicKey,
      state: statePublicKey,
      authority: accounts.authority,
      payer: accounts.payer,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
};

interface InitializeUserAccounts {
  authority: PublicKey;
  payer: PublicKey;
  user: PublicKey;
}

interface InitializeUserArgs {
  subAccountId: number;
  name: number[]; // 32-byte array
}

export const makeInitializeUserIx = async (
  program: Program<Drift>,
  accounts: InitializeUserAccounts,
  args: InitializeUserArgs,
): Promise<TransactionInstruction> => {
  const [userStatsPublicKey] = deriveUserStatsPDA(
    program.programId,
    accounts.authority,
  );
  const [statePublicKey] = deriveDriftStatePDA(program.programId);

  return program.methods
    .initializeUser(args.subAccountId, args.name)
    .accounts({
      user: accounts.user,
      userStats: userStatsPublicKey,
      state: statePublicKey,
      authority: accounts.authority,
      payer: accounts.payer,
      rent: SYSVAR_RENT_PUBKEY,
      systemProgram: SystemProgram.programId,
    })
    .instruction();
};

// CRITICAL: Account loading order for Drift deposits:
// 1. Oracle accounts (loaded until non-oracle found)
// 2. Spot market accounts (loaded until non-spot-market found)
// 3. Perp market accounts (loaded until non-perp-market found)
// 4. Token mint (optional, consumed by get_token_mint())

interface DepositAccounts {
  authority: PublicKey;
  userTokenAccount: PublicKey;
}

interface DepositArgs {
  marketIndex: number;
  amount: BN;
  subAccountId: number;
  reduceOnly: boolean;
  /** Oracle accounts (loaded until non-oracle account found) */
  remainingOracles?: PublicKey[];
  /** Spot market accounts (loaded until non-spot-market account found) */
  remainingMarkets: PublicKey[];
  /** Perp market accounts (loaded until non-perp-market account found) */
  remainingPerpMarkets?: PublicKey[];
  /** Token mint (optional, consumed by get_token_mint()) */
  tokenMint?: PublicKey;
}

export const makeDepositIx = async (
  program: Program<Drift>,
  accounts: DepositAccounts,
  args: DepositArgs,
): Promise<TransactionInstruction> => {
  const [statePublicKey] = deriveDriftStatePDA(program.programId);
  const [userPublicKey] = deriveUserPDA(
    program.programId,
    accounts.authority,
    args.subAccountId,
  );
  const [userStatsPublicKey] = deriveUserStatsPDA(
    program.programId,
    accounts.authority,
  );
  const [spotMarketVault] = deriveSpotMarketVaultPDA(
    program.programId,
    args.marketIndex,
  );

  // CRITICAL: Build remaining accounts in exact loading order:
  // 1. Oracle accounts (loaded until non-oracle found)
  // 2. Spot market accounts (loaded until non-spot-market found)
  // 3. Perp market accounts (loaded until non-perp-market found)
  // 4. Token mint (optional, consumed by get_token_mint())
  const remainingAccounts: AccountMeta[] = [
    // 1. Oracle accounts first
    ...(args.remainingOracles || []).map((pubkey) => ({
      pubkey,
      isWritable: false,
      isSigner: false,
    })),
    // 2. Spot market accounts second
    ...args.remainingMarkets.map((pubkey) => ({
      pubkey,
      isWritable: true,
      isSigner: false,
    })),
    // 3. Perp market accounts third
    ...(args.remainingPerpMarkets || []).map((pubkey) => ({
      pubkey,
      isWritable: true,
      isSigner: false,
    })),
    // 4. Token mint last (if provided)
    ...(args.tokenMint
      ? [{ pubkey: args.tokenMint, isWritable: false, isSigner: false }]
      : []),
  ];

  return program.methods
    .deposit(args.marketIndex, args.amount, args.reduceOnly)
    .accounts({
      state: statePublicKey,
      user: userPublicKey,
      userStats: userStatsPublicKey,
      authority: accounts.authority,
      spotMarketVault: spotMarketVault,
      userTokenAccount: accounts.userTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();
};

interface AdminDepositAccounts {
  admin: PublicKey;
  integrationAcc2: PublicKey; // The drift user account to deposit into
  adminTokenAccount: PublicKey; // Admin's token account (source of funds)
}

interface AdminDepositArgs {
  marketIndex: number;
  amount: BN;
  /** Remaining accounts: oracle, spot market, and optionally token mint */
  remainingAccounts?: PublicKey[];
}

/**
 * Manually construct admin_deposit instruction since it's not in our IDL
 * The admin_deposit function allows admins to deposit tokens into user accounts
 *
 * Remaining accounts required (in order):
 * 1. Oracle accounts for the spot market (optional)
 * 2. The spot market account itself (required)
 * 3. The token mint account (optional for Token-2022)
 */
export const makeAdminDepositIx = async (
  program: Program<Drift>,
  accounts: AdminDepositAccounts,
  args: AdminDepositArgs,
): Promise<TransactionInstruction> => {
  // Calculate discriminator for "admin_deposit"
  // Anchor uses first 8 bytes of sha256("global:admin_deposit")
  const { createHash } = await import("crypto");
  const hash = createHash("sha256").update("global:admin_deposit").digest();
  const discriminator = hash.subarray(0, 8);

  // Encode instruction data: discriminator + market_index (u16) + amount (u64)
  const marketIndexBuffer = Buffer.alloc(2);
  marketIndexBuffer.writeUInt16LE(args.marketIndex, 0);

  const data = Buffer.concat([
    discriminator,
    marketIndexBuffer,
    args.amount.toArrayLike(Buffer, "le", 8),
  ]);

  // Derive required PDAs
  const [driftState] = deriveDriftStatePDA(program.programId);
  const [spotMarketVault] = deriveSpotMarketVaultPDA(
    program.programId,
    args.marketIndex,
  );

  // Build accounts array in the order expected by AdminDeposit context
  // Based on the guide:
  // 1. state
  // 2. user (writable)
  // 3. admin (signer)
  // 4. spot_market_vault (writable)
  // 5. admin_token_account (writable)
  // 6. token_program
  const keys = [
    { pubkey: driftState, isSigner: false, isWritable: false }, // state
    { pubkey: accounts.integrationAcc2, isSigner: false, isWritable: true }, // user
    { pubkey: accounts.admin, isSigner: true, isWritable: false }, // admin (signer)
    { pubkey: spotMarketVault, isSigner: false, isWritable: true }, // spot_market_vault
    { pubkey: accounts.adminTokenAccount, isSigner: false, isWritable: true }, // admin_token_account
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false }, // token_program
  ];

  // Add remaining accounts if provided
  if (args.remainingAccounts) {
    for (let i = 0; i < args.remainingAccounts.length; i++) {
      const account = args.remainingAccounts[i];
      // Spot market account needs to be writable
      const isWritable = i === args.remainingAccounts.length - 1; // Last account is spot market
      keys.push({ pubkey: account, isSigner: false, isWritable });
    }
  }

  return new TransactionInstruction({
    keys,
    programId: program.programId,
    data,
  });
};

interface WithdrawAccounts {
  authority: PublicKey;
  userTokenAccount: PublicKey;
}

interface WithdrawArgs {
  marketIndex: number;
  amount: BN;
  subAccountId: number;
  reduceOnly: boolean;
  /** Oracle accounts (loaded until non-oracle account found) */
  remainingOracles?: PublicKey[];
  /** Spot market accounts (loaded until non-spot-market account found) */
  remainingMarkets: PublicKey[];
  /** Perp market accounts (loaded until non-perp-market account found) */
  remainingPerpMarkets?: PublicKey[];
  /** Token mint (optional, consumed by get_token_mint()) */
  tokenMint?: PublicKey;
}

export const makeWithdrawIx = async (
  program: Program<Drift>,
  accounts: WithdrawAccounts,
  args: WithdrawArgs,
): Promise<TransactionInstruction> => {
  const [statePublicKey] = deriveDriftStatePDA(program.programId);
  const [userPublicKey] = deriveUserPDA(
    program.programId,
    accounts.authority,
    args.subAccountId,
  );
  const [userStatsPublicKey] = deriveUserStatsPDA(
    program.programId,
    accounts.authority,
  );
  const [spotMarketVault] = deriveSpotMarketVaultPDA(
    program.programId,
    args.marketIndex,
  );
  const [driftSigner] = deriveDriftSignerPDA(program.programId);

  // CRITICAL: Build remaining accounts in exact loading order:
  // 1. Oracle accounts (loaded until non-oracle found)
  // 2. Spot market accounts (loaded until non-spot-market found)
  // 3. Perp market accounts (loaded until non-perp-market found)
  // 4. Token mint (optional, consumed by get_token_mint())
  const remainingAccounts: AccountMeta[] = [
    // 1. Oracle accounts first
    ...(args.remainingOracles || []).map((pubkey) => ({
      pubkey,
      isWritable: false,
      isSigner: false,
    })),
    // 2. Spot market accounts second
    ...args.remainingMarkets.map((pubkey) => ({
      pubkey,
      isWritable: true,
      isSigner: false,
    })),
    // 3. Perp market accounts third
    ...(args.remainingPerpMarkets || []).map((pubkey) => ({
      pubkey,
      isWritable: true,
      isSigner: false,
    })),
    // 4. Token mint last (if provided)
    ...(args.tokenMint
      ? [{ pubkey: args.tokenMint, isWritable: false, isSigner: false }]
      : []),
  ];

  return program.methods
    .withdraw(args.marketIndex, args.amount, args.reduceOnly)
    .accounts({
      state: statePublicKey,
      user: userPublicKey,
      userStats: userStatsPublicKey,
      authority: accounts.authority,
      spotMarketVault: spotMarketVault,
      driftSigner: driftSigner,
      userTokenAccount: accounts.userTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();
};

// Update spot market cumulative interest
export interface UpdateSpotMarketCumulativeInterestAccounts {
  oracle?: PublicKey; // Optional oracle for the market (not needed for USDC/market 0)
}

export const makeUpdateSpotMarketCumulativeInterestIx = async (
  program: Program<Drift>,
  accounts: UpdateSpotMarketCumulativeInterestAccounts,
  marketIndex: number,
): Promise<TransactionInstruction> => {
  const [statePublicKey] = deriveDriftStatePDA(program.programId);
  const [spotMarketPublicKey] = deriveSpotMarketPDA(
    program.programId,
    marketIndex,
  );
  const [spotMarketVaultPublicKey] = deriveSpotMarketVaultPDA(
    program.programId,
    marketIndex,
  );

  // Build instruction - oracle can be system program if not provided
  const ix = await program.methods
    .updateSpotMarketCumulativeInterest()
    .accounts({
      state: statePublicKey,
      spotMarket: spotMarketPublicKey,
      oracle: accounts.oracle || SystemProgram.programId,
      spotMarketVault: spotMarketVaultPublicKey,
    })
    .instruction();

  return ix;
};
