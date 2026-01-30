import {
  PublicKey,
  Keypair,
  AccountMeta,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import BN from "bn.js";
import { Program, IdlAccounts, IdlTypes } from "@coral-xyz/anchor";
import { Drift } from "tests/fixtures/drift_v2";
import { WrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
  I80F48_ONE,
  ORACLE_CONF_INTERVAL,
} from "../utils/types";
import { BanksClient, ProgramTestContext } from "solana-bankrun";
import {
  createBankrunPythOracleAccount,
  setPythPullOraclePrice,
} from "./bankrun-oracles";
import { Oracles, MockUser } from "./mocks";
import {
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_PULL_FEED,
  ecosystem,
  bankrunContext,
  banksClient,
  bankrunProgram,
  driftBankrunProgram,
  driftAccounts,
  globalFeeWallet,
  globalProgramAdmin,
  groupAdmin,
  DRIFT_TOKEN_B_PULL_ORACLE,
  DRIFT_TOKEN_B_PULL_FEED,
} from "../rootHooks";
import { makeInitializeSpotMarketIx, makeAdminDepositIx } from "./drift-sdk";
import { deriveSpotMarketPDA } from "./pdas";
import { accountInit } from "./user-instructions";
import { processBankrunTransaction } from "./tools";

// Import Drift account types using IdlAccounts - the clean Anchor-native way
export type DriftState = IdlAccounts<Drift>["state"];
export type DriftUser = IdlAccounts<Drift>["user"];
export type DriftUserStats = IdlAccounts<Drift>["userStats"];
export type DriftSpotMarket = IdlAccounts<Drift>["spotMarket"];
export type DriftSpotPosition = IdlTypes<Drift>["spotPosition"];

/**
 * Determines if a Drift spot position represents a borrow position
 * @param position - The Drift spot position to check
 * @returns True if the position is a borrow (negative cumulative deposits)
 */
export const isDriftPositionBorrow = (position: DriftSpotPosition): boolean => {
  // In Drift, negative cumulativeDeposits indicates a borrow position
  return new BN(position.cumulativeDeposits).lt(new BN(0));
};

/**
 * Determines if a Drift spot position represents a deposit position
 * @param position - The Drift spot position to check
 * @returns True if the position is a deposit (positive cumulative deposits)
 */
export const isDriftPositionDeposit = (
  position: DriftSpotPosition,
): boolean => {
  // In Drift, positive cumulativeDeposits indicates a deposit position
  return new BN(position.cumulativeDeposits).gt(new BN(0));
};

// Note: Could import these types from IdlTypes<Drift> if needed
// export type DriftOracleSource = IdlTypes<Drift>["oracleSource"];
// export type DriftAssetTier = IdlTypes<Drift>["assetTier"];

// Manually defined enum types based on Drift IDL
export type DriftOracleSource =
  | { pyth: {} }
  | { switchboard: {} }
  | { quoteAsset: {} }
  | { pyth1K: {} }
  | { pyth1M: {} }
  | { pythStableCoin: {} }
  | { prelaunch: {} }
  | { pythPull: {} }
  | { pyth1KPull: {} }
  | { pyth1MPull: {} }
  | { pythStableCoinPull: {} }
  | { switchboardOnDemand: {} }
  | { pythLazer: {} }
  | { pythLazer1K: {} }
  | { pythLazer1M: {} }
  | { pythLazerStableCoin: {} };

export type DriftAssetTier =
  | { collateral: {} }
  | { protected: {} }
  | { cross: {} }
  | { isolated: {} }
  | { unlisted: {} };

// Value objects for enum types - use these in your code
export const DriftOracleSourceValues = {
  pyth: { pyth: {} } as DriftOracleSource,
  switchboard: { switchboard: {} } as DriftOracleSource,
  quoteAsset: { quoteAsset: {} } as DriftOracleSource,
  pyth1K: { pyth1K: {} } as DriftOracleSource,
  pyth1M: { pyth1M: {} } as DriftOracleSource,
  pythStableCoin: { pythStableCoin: {} } as DriftOracleSource,
  prelaunch: { prelaunch: {} } as DriftOracleSource,
  pythPull: { pythPull: {} } as DriftOracleSource,
  pyth1KPull: { pyth1KPull: {} } as DriftOracleSource,
  pyth1MPull: { pyth1MPull: {} } as DriftOracleSource,
  pythStableCoinPull: { pythStableCoinPull: {} } as DriftOracleSource,
  switchboardOnDemand: { switchboardOnDemand: {} } as DriftOracleSource,
  pythLazer: { pythLazer: {} } as DriftOracleSource,
  pythLazer1K: { pythLazer1K: {} } as DriftOracleSource,
  pythLazer1M: { pythLazer1M: {} } as DriftOracleSource,
  pythLazerStableCoin: { pythLazerStableCoin: {} } as DriftOracleSource,
} as const;

export const DriftAssetTierValues = {
  collateral: { collateral: {} } as DriftAssetTier,
  protected: { protected: {} } as DriftAssetTier,
  cross: { cross: {} } as DriftAssetTier,
  isolated: { isolated: {} } as DriftAssetTier,
  unlisted: { unlisted: {} } as DriftAssetTier,
} as const;

// Drift-specific utility functions

// Create deterministic keypairs for Drift entities
export const DRIFT_STATE_SEED = Buffer.from("DRIFT_STATE_SEED_000000000000000");
export const DRIFT_USDC_MARKET_SEED = Buffer.from(
  "DRIFT_USDC_MARKET_SEED_000000000",
);
export const DRIFT_TOKEN_A_MARKET_SEED = Buffer.from(
  "DRIFT_TOKEN_A_MARKET_SEED_000000",
);

export const driftStateKeypair = Keypair.fromSeed(DRIFT_STATE_SEED);
export const driftUsdcMarketKeypair = Keypair.fromSeed(DRIFT_USDC_MARKET_SEED);
export const driftTokenAMarketKeypair = Keypair.fromSeed(
  DRIFT_TOKEN_A_MARKET_SEED,
);

// Drift market index constants
export const USDC_MARKET_INDEX = 0;
export const TOKEN_A_MARKET_INDEX = 1;
export const USDC_POOL2_MARKET_INDEX = 2;
export const TOKEN_A_POOL3_MARKET_INDEX = 3;

// Drift pool ID constants
export const POOL2_ID = 2;
export const POOL3_ID = 3;

// The initial deposits for respective Drift banks. Used during Drift Users initialization.
export const USDC_INIT_DEPOSIT_AMOUNT = new BN(100); // 100 smallest units (0.0001 USDC)
export const TOKEN_A_INIT_DEPOSIT_AMOUNT = new BN(200); // 200 smallest units (0.000002 Token A)

/// Drift utilization calculation uses 6 decimal precision (1000000 = 100%)
export const DRIFT_UTILIZATION_PRECISION = new BN(1000000);
export const DRIFT_PRECISION_EXP = 19;
export const ZERO = new BN(0);
export const ONE = new BN(1);
export const TEN = new BN(10);
export const ONE_YEAR = new BN(31536000);

// Default spot market configuration
export interface SpotMarketConfig {
  optimalUtilization: number;
  optimalRate: number;
  maxRate: number;
  initialAssetWeight: number;
  maintenanceAssetWeight: number;
  initialLiabilityWeight: number;
  maintenanceLiabilityWeight: number;
  imfFactor?: number;
  liquidatorFee?: number;
  ifLiquidationFee?: number;
  activeStatus?: boolean;
  assetTier?: number;
}

export const defaultSpotMarketConfig = (): SpotMarketConfig => {
  return {
    optimalUtilization: 500_000, // 50%
    optimalRate: 20_000_000, // 2,000%
    maxRate: 50_000_000, // 5,000%
    initialAssetWeight: 8_000, // 80%
    maintenanceAssetWeight: 9_000, // 90%
    initialLiabilityWeight: 10_000, // 100%
    maintenanceLiabilityWeight: 10_000, // 100%
    imfFactor: 0,
    liquidatorFee: 0,
    ifLiquidationFee: 0,
    activeStatus: true,
    assetTier: 0, // Normal
  };
};

export const quoteAssetSpotMarketConfig = (): SpotMarketConfig => {
  return {
    optimalUtilization: 500_000, // 50%
    optimalRate: 50_000, // 5%
    maxRate: 1_000_000, // 100%
    initialAssetWeight: 10_000, // 100%
    maintenanceAssetWeight: 10_000, // 100%
    initialLiabilityWeight: 10_000, // 100%
    maintenanceLiabilityWeight: 10_000, // 100%
    imfFactor: 0,
    liquidatorFee: 0,
    ifLiquidationFee: 0,
    activeStatus: true,
    assetTier: 0, // Normal
  };
};

/**
 * Fetches a Drift spot market account by market index
 * @param program - The Drift program instance
 * @param marketIndex - The market index to fetch (0 for USDC, 1 for Token A, etc.)
 * @returns The Drift spot market account data
 */
export const getSpotMarketAccount = async (
  program: Program<Drift>,
  marketIndex: number,
): Promise<DriftSpotMarket> => {
  const [spotMarketPDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("spot_market"),
      new BN(marketIndex).toArrayLike(Buffer, "le", 2),
    ],
    program.programId,
  );

  return await program.account.spotMarket.fetch(spotMarketPDA);
};

/**
 * Fetches a Drift user account by authority and sub-account ID
 * @param program - The Drift program instance
 * @param authority - The wallet authority that owns the user account
 * @param subAccountId - The sub-account ID (usually 0 for main account)
 * @returns The Drift user account data
 */
export const getUserAccount = async (
  program: Program<Drift>,
  authority: PublicKey,
  subAccountId: number,
): Promise<DriftUser> => {
  const [userPDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("user"),
      authority.toBuffer(),
      new BN(subAccountId).toArrayLike(Buffer, "le", 2),
    ],
    program.programId,
  );

  return await program.account.user.fetch(userPDA);
};

/**
 * Fetches a Drift user stats account by authority
 * @param program - The Drift program instance
 * @param authority - The wallet authority that owns the user stats
 * @returns The Drift user stats account data
 */
export const getUserStatsAccount = async (
  program: Program<Drift>,
  authority: PublicKey,
): Promise<DriftUserStats> => {
  const [userStatsPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("user_stats"), authority.toBuffer()],
    program.programId,
  );

  return await program.account.userStats.fetch(userStatsPDA);
};

/**
 * Fetches the global Drift state account
 * @param program - The Drift program instance
 * @returns The Drift state account containing global protocol data
 */
export const getDriftStateAccount = async (
  program: Program<Drift>,
): Promise<DriftState> => {
  const [statePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("drift_state")],
    program.programId,
  );

  return await program.account.state.fetch(statePDA);
};

// copied from Drift: https://github.com/drift-labs/protocol-v2/blob/1fed025269eed8ea5159dc56e2143fb904dbf14e/sdk/src/math/spotBalance.ts#L65
/**
 * Calculates the spot token amount including any accumulated interest.
 *
 * @param {BN} balanceAmount - The balance amount, typically from `SpotPosition.scaledBalance`
 * @param {SpotMarketAccount} spotMarket - The spot market account details
 * @param {boolean} isDeposit - The balance type to be used for calculation: deposit or borrow
 * @returns {BN} The calculated token amount, scaled by `SpotMarketConfig.precision`
 */
export function getTokenAmount(
  balanceAmount: BN,
  spotMarket: DriftSpotMarket,
  isDeposit: boolean,
): BN {
  const precisionDecrease = TEN.pow(
    new BN(DRIFT_PRECISION_EXP - spotMarket.decimals),
  );

  if (isDeposit) {
    return balanceAmount
      .mul(spotMarket.cumulativeDepositInterest)
      .div(precisionDecrease);
  } else {
    return divCeil(
      balanceAmount.mul(spotMarket.cumulativeBorrowInterest),
      precisionDecrease,
    );
  }
}

export const divCeil = (a: BN, b: BN): BN => {
  const quotient = a.div(b);

  const remainder = a.mod(b);

  if (remainder.gt(ZERO)) {
    return quotient.add(ONE);
  } else {
    return quotient;
  }
};

// copied from Drift: https://github.com/drift-labs/protocol-v2/blob/1fed025269eed8ea5159dc56e2143fb904dbf14e/sdk/src/math/spotBalance.ts#L266
export const calculateUtilization = (
  spotMarket: DriftSpotMarket,
  delta = ZERO,
): BN => {
  let tokenDepositAmount = getTokenAmount(
    spotMarket.depositBalance,
    spotMarket,
    true,
  );
  let tokenBorrowAmount = getTokenAmount(
    spotMarket.borrowBalance,
    spotMarket,
    false,
  );

  if (delta.gt(ZERO)) {
    tokenDepositAmount = tokenDepositAmount.add(delta);
  } else if (delta.lt(ZERO)) {
    tokenBorrowAmount = tokenBorrowAmount.add(delta.abs());
  }

  let utilization: BN;
  if (tokenBorrowAmount.eq(ZERO) && tokenDepositAmount.eq(ZERO)) {
    utilization = ZERO;
  } else if (tokenDepositAmount.eq(ZERO)) {
    utilization = DRIFT_UTILIZATION_PRECISION;
  } else {
    utilization = tokenBorrowAmount
      .mul(DRIFT_UTILIZATION_PRECISION)
      .div(tokenDepositAmount);
  }

  return utilization;
};

// copied from Drift: https://github.com/drift-labs/protocol-v2/blob/1fed025269eed8ea5159dc56e2143fb904dbf14e/sdk/src/math/spotBalance.ts#L381
export const calculateInterestRate = (
  spotMarket: DriftSpotMarket,
  delta = ZERO,
  currentUtilization: BN = null,
): BN => {
  const utilization =
    currentUtilization || calculateUtilization(spotMarket, delta);

  const optimalUtil = new BN(spotMarket.optimalUtilization);
  const optimalRate = new BN(spotMarket.optimalBorrowRate);
  const maxRate = new BN(spotMarket.maxBorrowRate);
  const minRate = new BN(spotMarket.minBorrowRate).mul(
    DRIFT_UTILIZATION_PRECISION.divn(200),
  );

  const weightsDivisor = new BN(1000);
  const segments: [BN, BN][] = [
    [new BN(850_000), new BN(50)],
    [new BN(900_000), new BN(100)],
    [new BN(950_000), new BN(150)],
    [new BN(990_000), new BN(200)],
    [new BN(995_000), new BN(250)],
    [DRIFT_UTILIZATION_PRECISION, new BN(250)],
  ];

  let rate: BN;
  if (utilization.lte(optimalUtil)) {
    // below optimal: linear ramp from 0 to optimalRate
    const slope = optimalRate.mul(DRIFT_UTILIZATION_PRECISION).div(optimalUtil);
    rate = utilization.mul(slope).div(DRIFT_UTILIZATION_PRECISION);
  } else {
    // above optimal: piecewise segments
    const totalExtraRate = maxRate.sub(optimalRate);

    rate = optimalRate.clone();
    let prevUtil = optimalUtil.clone();

    for (const [bp, weight] of segments) {
      const segmentEnd = bp.gt(DRIFT_UTILIZATION_PRECISION)
        ? DRIFT_UTILIZATION_PRECISION
        : bp;
      const segmentRange = segmentEnd.sub(prevUtil);

      const segmentRateTotal = totalExtraRate.mul(weight).div(weightsDivisor);

      if (utilization.lte(segmentEnd)) {
        const partialUtil = utilization.sub(prevUtil);
        const partialRate = segmentRateTotal.mul(partialUtil).div(segmentRange);
        rate = rate.add(partialRate);
        break;
      } else {
        rate = rate.add(segmentRateTotal);
        prevUtil = segmentEnd;
      }
    }
  }

  return BN.max(minRate, rate);
};

/**
 * Calculates the scaling factor used by Drift for token amounts based on decimals
 * @param tokenDecimals - Number of decimals for the token (6 for USDC, 9 for SOL)
 * @returns Scaling factor as BN (e.g., 1000 for USDC, 1 for SOL)
 */
export const getDriftScalingFactor = (tokenDecimals: number): BN => {
  // Based on observed behavior: 10^(9 - token_decimals)
  // For USDC (6 decimals): 10^(9-6) = 10^3 = 1,000
  // For SOL (9 decimals): 10^(9-9) = 10^0 = 1
  const exponent = 9 - tokenDecimals;
  return new BN(Math.pow(10, exponent));
};

/**
 * Formats token amounts with proper decimal places and commas for display
 * @param amount - The raw token amount to format
 * @param decimals - Number of decimal places for the token
 * @param symbol - Optional token symbol to append (e.g., "USDC")
 * @returns Formatted string with commas and decimal places
 */
export const formatTokenAmount = (
  amount: BN | number | string,
  decimals: number,
  symbol: string = "",
): string => {
  const amountStr = amount.toString();
  const divisor = Math.pow(10, decimals);
  const wholeAmount = parseFloat(amountStr) / divisor;

  // Format with commas and full decimal precision
  const formatted = wholeAmount.toLocaleString("en-US", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });

  return symbol ? `${formatted} ${symbol}` : formatted;
};

// Drift deposit formatting precision - constant across all tokens
const DRIFT_DISPLAY_DECIMALS = 9;

// Export the scaled balance decimals constant from drift-mocks
export const DRIFT_SCALED_BALANCE_DECIMALS = 9;

export const USDC_SCALING_FACTOR = getDriftScalingFactor(
  ecosystem.usdcDecimals,
); // 10^3 = 1,000
export const TOKEN_A_SCALING_FACTOR = getDriftScalingFactor(
  ecosystem.tokenADecimals,
); // 10^1 = 10
export const TOKEN_B_SCALING_FACTOR = getDriftScalingFactor(
  ecosystem.tokenBDecimals,
); // 10^3 = 1000

/**
 * Formats Drift internal deposit amounts with consistent 9-decimal precision
 * @param amount - The raw deposit amount from Drift
 * @param symbol - Optional token symbol to append
 * @returns Formatted string with 9 decimal places for Drift internal precision
 */
export const formatDepositAmount = (
  amount: BN | number | string,
  symbol: string = "",
): string => {
  const amountStr = amount.toString();
  const divisor = Math.pow(10, DRIFT_DISPLAY_DECIMALS);
  const wholeAmount = parseFloat(amountStr) / divisor;

  // Format with commas and consistent decimal precision
  // Drift maintains consistent internal precision regardless of token decimals
  const formatted = wholeAmount.toLocaleString("en-US", {
    minimumFractionDigits: DRIFT_DISPLAY_DECIMALS,
    maximumFractionDigits: DRIFT_DISPLAY_DECIMALS,
  });

  return symbol ? `${formatted} ${symbol}` : formatted;
};

/**
 * Formats raw amounts (like scaled balances) with comma separators
 * @param amount - The raw amount to format
 * @returns Formatted string with commas but no decimal places
 */
export const formatRawAmount = (amount: BN | number | string): string => {
  return parseInt(amount.toString()).toLocaleString("en-US");
};

/**
 * Checks if a Drift spot market has any active positions (deposits or borrows)
 * @param spotMarket - The Drift spot market to check
 * @returns True if the market has non-zero deposit or borrow balances
 */
export const hasActivePositions = (spotMarket: DriftSpotMarket): boolean => {
  return (
    !spotMarket.depositBalance.isZero() || !spotMarket.borrowBalance.isZero()
  );
};

/**
 * Gets all active positions for a Drift user account, formatted for display
 * @param program - The Drift program instance
 * @param authority - The wallet authority that owns the user account
 * @param subAccountId - The sub-account ID to check
 * @returns Array of position objects for active positions only
 */
export const getUserPositions = async (
  program: Program<Drift>,
  authority: PublicKey,
  subAccountId: number,
) => {
  const user = await getUserAccount(program, authority, subAccountId);

  return user.spotPositions.filter(
    (pos) => pos.marketIndex !== 0 || !pos.scaledBalance.isZero(),
  );
};

/**
 * Convert token amount to scaled balance for Drift liquidations
 *
 * @param tokenAmount The actual token amount to convert
 * @param spotMarket The Drift spot market account
 * @returns The scaled balance amount to use in Drift operations
 */
export const tokenAmountToScaledBalance = (
  tokenAmount: BN,
  spotMarket: DriftSpotMarket,
): BN => {
  const decimals = spotMarket.decimals;
  const cumulativeInterest = new BN(
    spotMarket.cumulativeDepositInterest.toString(),
  );

  // Calculate precision increase: 10^(19 - decimals)
  const precisionIncrease = new BN(10).pow(new BN(19 - decimals));

  // Calculate scaled balance: token_amount * precision_increase / cumulative_interest
  let scaledBalance = tokenAmount
    .mul(precisionIncrease)
    .div(cumulativeInterest);

  return scaledBalance;
};

/**
 * Convert scaled balance to token amount
 *
 * @param scaledBalance The scaled balance from Drift
 * @param spotMarket The Drift spot market account
 * @param isDeposit Whether this is a deposit (uses deposit interest) or withdrawal (uses borrow interest)
 * @returns The actual token amount
 */
export const scaledBalanceToTokenAmount = (
  scaledBalance: BN,
  spotMarket: DriftSpotMarket,
  isDeposit: boolean = true,
): BN => {
  const decimals = spotMarket.decimals;
  const cumulativeInterest = isDeposit
    ? new BN(spotMarket.cumulativeDepositInterest.toString())
    : new BN(spotMarket.cumulativeBorrowInterest.toString());
  if (decimals > DRIFT_PRECISION_EXP) {
    console.error("decimals > drift precision, likely invalid");
  }

  const precisionIncrease = TEN.pow(new BN(DRIFT_PRECISION_EXP - decimals));
  const product = scaledBalance.mul(cumulativeInterest);
  const quotient = product.div(precisionIncrease);
  return quotient;
};

export interface DriftConfigCompact {
  oracle: PublicKey;
  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;
  depositLimit: BN;
  oracleSetup: { driftPythPull: {} } | { driftSwitchboardPull: {} };
  operationalState: { operational: {} } | { paused: {} } | { reduceOnly: {} };
  riskTier: { collateral: {} } | { isolated: {} };
  configFlags: number;
  totalAssetValueInitLimit: BN;
  oracleMaxAge: number;
  oracleMaxConfidence: number;
}

/**
 * Creates a default configuration for a Drift bank following the same pattern as Kamino
 *
 * @param oracle The oracle pubkey to use for price feeds
 * @returns A DriftConfigCompact with sensible defaults for testing
 */
export const defaultDriftBankConfig = (
  oracle: PublicKey,
): DriftConfigCompact => {
  const config: DriftConfigCompact = {
    oracle: oracle,
    assetWeightInit: I80F48_ONE, // 100% asset weight
    assetWeightMaint: I80F48_ONE, // 100% asset weight
    // 100 million tokens in native decimals (will be converted to 9-decimal scaled balance in program)
    depositLimit: new BN(100_000_000_000_000),
    oracleSetup: {
      driftPythPull: {}, // Use Pyth Pull oracle by default
    },
    operationalState: { operational: {} }, // Start operational by default
    riskTier: { collateral: {} }, // Collateral by default
    configFlags: 1, // (PYTH_PUSH_MIGRATED_DEPRECATED)
    totalAssetValueInitLimit: new BN(10_000_000_000_000), // 10 million USD equivalent
    oracleMaxAge: 100, // 100 seconds max oracle age
    oracleMaxConfidence: 0, // Use default 10% confidence
  };
  return config;
};
/**
 * Get Drift user account from a marginfi bank's integration_acc_2 field
 *
 * @param program The Drift program
 * @param driftUserPubkey The drift user pubkey from the marginfi bank
 * @returns The Drift user account
 */
export const getDriftUserAccount = async (
  program: Program<Drift>,
  driftUserPubkey: PublicKey,
): Promise<DriftUser> => {
  return await program.account.user.fetch(driftUserPubkey);
};

/**
 * Refresh Drift-specific oracles that use mainnet Pyth program ID. This is needed after time
 * warping to prevent oracle staleness.
 *
 * @param oracles The oracles object containing Token A price data
 * @param driftAccounts Map containing Drift-specific oracle accounts
 * @param bankrunContext The bankrun context
 * @param banksClient The banks client to get clock from
 */
export async function refreshDriftOracles(
  oracles: Oracles,
  driftAccounts: Map<string, PublicKey>,
  bankrunContext: ProgramTestContext,
  banksClient: BanksClient,
) {
  // Get the Drift Token A oracle and feed from the map
  const tokenAOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);
  const tokenAFeed = driftAccounts.get(DRIFT_TOKEN_A_PULL_FEED);

  const tokenBOracle = driftAccounts.get(DRIFT_TOKEN_B_PULL_ORACLE);
  const tokenBFeed = driftAccounts.get(DRIFT_TOKEN_B_PULL_FEED);

  if (tokenAOracle && tokenAFeed) {
    // console.log("refresh oracle A");
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      tokenAOracle,
      tokenAFeed,
      oracles.tokenAPrice,
      oracles.tokenADecimals,
      ORACLE_CONF_INTERVAL,
    );
  }

  if (tokenBOracle && tokenBFeed) {
    // console.log("refresh oracle B");
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      tokenBOracle,
      tokenBFeed,
      oracles.tokenBPrice,
      oracles.tokenBDecimals,
      ORACLE_CONF_INTERVAL,
    );
  }
}

// -------------- Everything below here is slop to be deleted ------

// Comprehensive Drift account valuation tracking
export interface DriftPositionValuation {
  marketIndex: number;
  tokenSymbol: string;
  scaledBalance: BN;
  cumulativeInterest: BN;
  tokenAmount: BN;
  oraclePrice: BN;
  adjustedOraclePrice: BN;
  tokenValue: BN;
  interestEarned: BN;
  scalingFactor: BN;
  decimals: number;
  balanceType: string; // "deposit" or "borrow"
}

export interface DriftAccountValuation {
  userPubkey: PublicKey;
  positions: DriftPositionValuation[];
  totalValue: BN;
  totalCollateral: BN;
  totalLiabilities: BN;
  netValue: BN;
  timestamp: number;
}

// Import additional types we need
import { decodePriceUpdateV2 } from "./pyth-pull-mocks";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { Marginfi } from "../../target/types/marginfi";
import { assert } from "chai";
import BigNumber from "bignumber.js";

// Constants for Drift
export const DRIFT_SCALED_BALANCE_PRECISION = new BN(1_000_000_000); // 10^9
export const DRIFT_SPOT_CUMULATIVE_INTEREST_PRECISION = new BN(10_000_000_000); // 10^10

// Calculate comprehensive valuation for a Drift position
export const calculateDriftPositionValuation = async (
  position: DriftSpotPosition,
  spotMarket: DriftSpotMarket,
  oraclePrice: BN,
  tokenSymbol: string,
  decimals: number,
  initialScaledBalance?: BN,
): Promise<DriftPositionValuation> => {
  const scalingFactor = getDriftScalingFactor(decimals);
  const scaledBalance = new BN(position.scaledBalance.toString());

  // Determine balance type based on cumulative deposits
  const isDeposit = new BN(position.cumulativeDeposits.toString()).gte(
    new BN(0),
  );
  const balanceType = isDeposit ? "deposit" : "borrow";

  const cumulativeInterest = isDeposit
    ? spotMarket.cumulativeDepositInterest
    : spotMarket.cumulativeBorrowInterest;

  // Calculate actual token amount
  const tokenAmount = scaledBalanceToTokenAmount(
    scaledBalance,
    spotMarket,
    isDeposit,
  );

  // Calculate adjusted oracle price (what MarginFi sees)
  // This matches the adjust_oracle_price function in drift-mocks
  const adjustedOraclePrice = oraclePrice
    .mul(cumulativeInterest)
    .div(DRIFT_SCALED_BALANCE_PRECISION);

  // Calculate token value (in base currency units, typically 6 decimals for USD)
  const tokenValue = tokenAmount
    .mul(oraclePrice)
    .div(new BN(10).pow(new BN(decimals)));

  // Calculate interest earned (if we have initial balance)
  let interestEarned = new BN(0);
  if (initialScaledBalance) {
    const initialTokenAmount = initialScaledBalance
      .mul(DRIFT_SCALED_BALANCE_PRECISION)
      .div(cumulativeInterest);
    interestEarned = tokenAmount.sub(initialTokenAmount);
  }

  return {
    marketIndex: position.marketIndex,
    tokenSymbol,
    scaledBalance,
    cumulativeInterest,
    tokenAmount,
    oraclePrice,
    adjustedOraclePrice,
    tokenValue,
    interestEarned,
    scalingFactor,
    decimals,
    balanceType,
  };
};

// Get comprehensive valuation for a Drift user account
export const getDriftAccountValuation = async (
  driftProgram: Program<Drift>,
  banksClient: BanksClient,
  userPubkey: PublicKey,
  oracles: {
    [marketIndex: number]: {
      oracle: PublicKey;
      symbol: string;
      decimals: number;
    };
  },
): Promise<DriftAccountValuation> => {
  // Fetch user account
  const driftUser = await getDriftUserAccount(driftProgram, userPubkey);

  // Get all positions with valuations
  const positions: DriftPositionValuation[] = [];
  let totalCollateral = new BN(0);
  let totalLiabilities = new BN(0);

  for (const position of driftUser.spotPositions) {
    if (position.scaledBalance.toString() === "0") continue;

    const marketInfo = oracles[position.marketIndex];
    if (!marketInfo) continue;

    // Get spot market
    const spotMarket = await getSpotMarketAccount(
      driftProgram,
      position.marketIndex,
    );

    // Get oracle price
    const oracleAccount = await banksClient.getAccount(marketInfo.oracle);
    if (!oracleAccount) {
      console.warn(
        `Oracle account not found for market ${position.marketIndex}`,
      );
      continue;
    }

    const oracleData = decodePriceUpdateV2(
      Buffer.from(oracleAccount.data).toString("base64"),
    );
    const oraclePrice = new BN(oracleData.price_message.price.toString());

    // Calculate full valuation
    const valuation = await calculateDriftPositionValuation(
      position,
      spotMarket,
      oraclePrice,
      marketInfo.symbol,
      marketInfo.decimals,
    );

    positions.push(valuation);

    // Track collateral vs liabilities
    if (valuation.balanceType === "deposit") {
      totalCollateral = totalCollateral.add(valuation.tokenValue);
    } else {
      totalLiabilities = totalLiabilities.add(valuation.tokenValue);
    }
  }

  const totalValue = totalCollateral;
  const netValue = totalCollateral.sub(totalLiabilities);

  return {
    userPubkey,
    positions,
    totalValue,
    totalCollateral,
    totalLiabilities,
    netValue,
    timestamp: Date.now(),
  };
};

// Pretty print valuation for debugging
export const printDriftAccountValuation = (
  valuation: DriftAccountValuation,
  title: string = "Drift Account Valuation",
) => {
  console.log(`\n${"=".repeat(80)}`);
  console.log(`${title}`);
  console.log(`${"=".repeat(80)}`);
  console.log(`User: ${valuation.userPubkey.toString()}`);
  console.log(`Timestamp: ${new Date(valuation.timestamp).toISOString()}`);
  console.log(`\nPositions:`);

  for (const pos of valuation.positions) {
    console.log(`\n  Market ${pos.marketIndex} (${pos.tokenSymbol}):`);
    console.log(`    Type: ${pos.balanceType}`);
    console.log(`    Scaled Balance: ${formatRawAmount(pos.scaledBalance)}`);
    console.log(
      `    Cumulative Interest: ${formatRawAmount(pos.cumulativeInterest)}`,
    );
    console.log(
      `    Token Amount: ${formatTokenAmount(
        pos.tokenAmount,
        pos.decimals,
        pos.tokenSymbol,
      )}`,
    );
    console.log(
      `    Oracle Price: $${
        pos.oraclePrice.toNumber() / Math.pow(10, 6)
      } (raw: ${formatRawAmount(pos.oraclePrice)})`,
    );
    console.log(
      `    Adjusted Oracle Price: $${
        pos.adjustedOraclePrice.toNumber() / Math.pow(10, 6)
      } (raw: ${formatRawAmount(pos.adjustedOraclePrice)})`,
    );
    console.log(
      `    Token Value: $${pos.tokenValue.toNumber() / Math.pow(10, 6)}`,
    );
    if (!pos.interestEarned.isZero()) {
      console.log(
        `    Interest Earned: ${formatTokenAmount(
          pos.interestEarned,
          pos.decimals,
          pos.tokenSymbol,
        )}`,
      );
    }
  }

  console.log(`\nSummary:`);
  console.log(
    `  Total Collateral: $${
      valuation.totalCollateral.toNumber() / Math.pow(10, 6)
    }`,
  );
  console.log(
    `  Total Liabilities: $${
      valuation.totalLiabilities.toNumber() / Math.pow(10, 6)
    }`,
  );
  console.log(
    `  Net Value: $${valuation.netValue.toNumber() / Math.pow(10, 6)}`,
  );
  console.log(`${"=".repeat(80)}\n`);
};

/**
 * Print detailed health cache information from a MarginFi account
 */
export const printMarginfiHealthCache = async (
  program: Program<Marginfi>,
  marginfiAccount: PublicKey,
  label: string = "MarginFi Account Health Cache",
) => {
  const account = await program.account.marginfiAccount.fetch(marginfiAccount);
  const healthCache = account.healthCache;

  console.log(`\n💊 ${label}`);
  console.log("=".repeat(60));

  // Extract values from health cache
  const assetValue = wrappedI80F48toBigNumber(healthCache.assetValue);
  const liabilityValue = wrappedI80F48toBigNumber(healthCache.liabilityValue);
  const assetValueMaint = wrappedI80F48toBigNumber(healthCache.assetValueMaint);
  const liabilityValueMaint = wrappedI80F48toBigNumber(
    healthCache.liabilityValueMaint,
  );

  // Health status
  const flags = healthCache.flags;
  const isHealthy = (flags & 1) !== 0; // HEALTH_CACHE_HEALTHY = 1
  const engineOk = (flags & 2) !== 0; // HEALTH_CACHE_ENGINE_OK = 2
  const oracleOk = (flags & 4) !== 0; // HEALTH_CACHE_ORACLE_OK = 4

  console.log("Health Status:");
  console.log(`  Healthy: ${isHealthy ? "✅ YES" : "❌ NO"}`);
  console.log(`  Engine OK: ${engineOk ? "✅" : "❌"}`);
  console.log(`  Oracle OK: ${oracleOk ? "✅" : "❌"}`);
  console.log(`  Flags: ${flags}`);

  console.log("\nInit Values (for opening positions):");
  console.log(`  Asset Value: $${assetValue.toFixed(6)}`);
  console.log(`  Liability Value: $${liabilityValue.toFixed(6)}`);
  console.log(`  Net Value: $${assetValue.minus(liabilityValue).toFixed(6)}`);

  console.log("\nMaint Values (for liquidations):");
  console.log(`  Asset Value: $${assetValueMaint.toFixed(6)}`);
  console.log(`  Liability Value: $${liabilityValueMaint.toFixed(6)}`);
  console.log(
    `  Net Value: $${assetValueMaint.minus(liabilityValueMaint).toFixed(6)}`,
  );

  // Health ratios
  if (liabilityValue.gt(0)) {
    const initHealthRatio = assetValue.div(liabilityValue);
    console.log(
      `\nInit Health Ratio: ${initHealthRatio.toFixed(4)} (${(
        initHealthRatio.toNumber() * 100
      ).toFixed(2)}%)`,
    );
  }

  if (liabilityValueMaint.gt(0)) {
    const maintHealthRatio = assetValueMaint.div(liabilityValueMaint);
    console.log(
      `Maint Health Ratio: ${maintHealthRatio.toFixed(4)} (${(
        maintHealthRatio.toNumber() * 100
      ).toFixed(2)}%)`,
    );
  }

  // List active balances with their contributions
  console.log("\nActive Positions:");
  const balances = account.lendingAccount.balances;
  for (let i = 0; i < balances.length; i++) {
    const balance = balances[i];
    if (balance.active === 1) {
      const assetShares = wrappedI80F48toBigNumber(balance.assetShares);
      const liabilityShares = wrappedI80F48toBigNumber(balance.liabilityShares);

      if (assetShares.gt(0) || liabilityShares.gt(0)) {
        console.log(`  Bank ${i}: ${balance.bankPk.toString().slice(0, 8)}...`);
        if (assetShares.gt(0)) {
          console.log(`    Asset Shares: ${assetShares.toFixed(2)}`);
        }
        if (liabilityShares.gt(0)) {
          console.log(`    Liability Shares: ${liabilityShares.toFixed(2)}`);
        }
      }
    }
  }

  console.log("=".repeat(60));

  return {
    assetValue,
    liabilityValue,
    assetValueMaint,
    liabilityValueMaint,
    isHealthy,
    flags,
  };
};

/**
 * Create a pulse health instruction to update the health cache
 */
export const makePulseHealthIx = async (
  program: Program<Marginfi>,
  marginfiAccount: PublicKey,
  remainingAccounts: AccountMeta[] = [],
): Promise<TransactionInstruction> => {
  return program.methods
    .lendingAccountPulseHealth()
    .accounts({
      marginfiAccount,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();
};

// Compare MarginFi's valuation with our calculated valuation
export const compareDriftValuations = async (
  marginfiProgram: Program<Marginfi>,
  driftProgram: Program<Drift>,
  banksClient: BanksClient,
  marginfiAccount: PublicKey,
  driftBank: PublicKey,
  integrationAcc2: PublicKey,
  oracleInfo: { oracle: PublicKey; symbol: string; decimals: number },
): Promise<{
  marginfiAssetShares: BN;
  marginfiLiabilityShares: BN;
  marginfiTokenAmount: BN;
  marginfiValuation: BN;
  calculatedValuation: BN;
  difference: BN;
  percentDiff: number;
}> => {
  // Get MarginFi's view of the account
  const mfiAccount = await marginfiProgram.account.marginfiAccount.fetch(
    marginfiAccount,
  );
  const balance = mfiAccount.lendingAccount.balances.find(
    (b: any) => b.active && b.bankPk.equals(driftBank),
  );

  if (!balance) {
    throw new Error("No active balance found for drift bank");
  }

  // Convert shares to BigNumber/BN
  const assetSharesBigNumber = wrappedI80F48toBigNumber(balance.assetShares);
  const liabilitySharesBigNumber = wrappedI80F48toBigNumber(
    balance.liabilityShares,
  );
  const assetShares = new BN(assetSharesBigNumber.toString());
  const liabilityShares = new BN(liabilitySharesBigNumber.toString());

  // For Drift banks, shares are actually the scaled balance from Drift
  // MarginFi stores Drift's scaled balance directly as shares (1:1 mapping)
  const scaledBalance = assetShares.sub(liabilityShares);

  // Get Drift spot market to convert scaled balance to token amount
  const driftUserAccount = await getDriftUserAccount(driftProgram, driftUser);

  // Find the active position (position with non-zero scaled balance)
  let spotPosition = null;
  let marketIndex = 0;
  for (const pos of driftUserAccount.spotPositions) {
    if (pos.scaledBalance.toString() !== "0") {
      spotPosition = pos;
      marketIndex = pos.marketIndex;
      break;
    }
  }

  if (!spotPosition) {
    // No active position found - return zeros
    return {
      marginfiAssetShares: new BN(0),
      marginfiLiabilityShares: new BN(0),
      marginfiTokenAmount: new BN(0),
      marginfiValuation: new BN(0),
      calculatedValuation: new BN(0),
      difference: new BN(0),
      percentDiff: 0,
    };
  }

  const spotMarket = await getSpotMarketAccount(driftProgram, marketIndex);

  // Convert scaled balance to token amount using Drift's method
  const isDeposit = scaledBalance.gte(new BN(0));
  const tokenAmount = scaledBalanceToTokenAmount(
    scaledBalance.abs(),
    spotMarket,
    isDeposit,
  );

  // Get oracle price to calculate USD value
  const oracleAccount = await banksClient.getAccount(oracleInfo.oracle);
  if (!oracleAccount) {
    throw new Error("Oracle account not found");
  }

  const oracleData = decodePriceUpdateV2(
    Buffer.from(oracleAccount.data).toString("base64"),
  );
  const oraclePrice = new BN(oracleData.price_message.price.toString());

  // Calculate USD value: token amount * oracle price / 10^decimals
  // Oracle price is in 6 decimals (USD cents * 10^6)
  const marginfiValuation = tokenAmount
    .mul(oraclePrice)
    .div(new BN(10).pow(new BN(oracleInfo.decimals)));

  // Get our calculated valuation
  const driftValuation = await getDriftAccountValuation(
    driftProgram,
    banksClient,
    driftUser,
    { [marketIndex]: oracleInfo },
  );

  const calculatedValuation = driftValuation.netValue;
  const difference = marginfiValuation.sub(calculatedValuation).abs();
  const percentDiff = marginfiValuation.isZero()
    ? 0
    : (difference.toNumber() / marginfiValuation.toNumber()) * 100;

  console.log(`\n📊 Valuation Comparison:`);
  console.log(`  MarginFi asset shares: ${formatRawAmount(assetShares)}`);
  console.log(
    `  MarginFi liability shares: ${formatRawAmount(liabilityShares)}`,
  );
  console.log(`  Net scaled balance: ${formatRawAmount(scaledBalance)}`);
  console.log(
    `  Token amount: ${formatTokenAmount(
      tokenAmount,
      oracleInfo.decimals,
      oracleInfo.symbol,
    )}`,
  );
  console.log(`  Oracle price: $${oraclePrice.toNumber() / Math.pow(10, 6)}`);
  console.log(
    `  MarginFi valuation: $${marginfiValuation.toNumber() / Math.pow(10, 6)}`,
  );
  console.log(
    `  Our calculation: $${calculatedValuation.toNumber() / Math.pow(10, 6)}`,
  );
  console.log(
    `  Difference: $${
      difference.toNumber() / Math.pow(10, 6)
    } (${percentDiff.toFixed(4)}%)`,
  );

  return {
    marginfiAssetShares: assetShares,
    marginfiLiabilityShares: liabilityShares,
    marginfiTokenAmount: tokenAmount,
    marginfiValuation,
    calculatedValuation,
    difference,
    percentDiff,
  };
};

// Get Drift user from PDA
export const getDriftUser = async (
  driftProgram: Program<Drift>,
  authority: PublicKey,
  subAccountId: number,
): Promise<DriftUser> => {
  const [userPDA] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("user"),
      authority.toBuffer(),
      new BN(subAccountId).toArrayLike(Buffer, "le", 2),
    ],
    driftProgram.programId,
  );

  return await driftProgram.account.user.fetch(userPDA);
};

export async function assertBankBalance(
  marginfiAccount: PublicKey,
  bankPubkey: PublicKey,
  expectedBalance: BN | number | null,
  isLiability: boolean = false,
) {
  const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
    marginfiAccount,
  );

  const balance = userAcc.lendingAccount.balances.find(
    (b) => b.bankPk.equals(bankPubkey) && b.active === 1,
  );

  if (expectedBalance === null) {
    assert(balance === undefined);
    return;
  }

  assert(balance);

  let shares: BigNumber;
  if (isLiability) {
    shares = wrappedI80F48toBigNumber(balance.liabilityShares);
  } else {
    shares = wrappedI80F48toBigNumber(balance.assetShares);
  }

  if (typeof expectedBalance === "number") {
    assert.approximately(shares.toNumber(), expectedBalance, 1);
  } else {
    assert.equal(shares.toString(), expectedBalance.toString());
  }
}

export const createIntermediaryTokenAccountIfNeeded = async (
  bank: PublicKey,
  mint: PublicKey,
) => {
  const [liquidityVaultAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault_auth"), bank.toBuffer()],
    bankrunProgram.programId,
  );

  const intermediaryTokenAccount = getAssociatedTokenAddressSync(
    mint,
    liquidityVaultAuthority,
    true,
  );

  const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
    groupAdmin.wallet.publicKey,
    intermediaryTokenAccount,
    liquidityVaultAuthority,
    mint,
  );

  const tx = new Transaction().add(createAtaIx);
  await processBankrunTransaction(
    bankrunContext,
    tx,
    [groupAdmin.wallet],
    false,
    true,
  );
};

export const createGlobalFeeWalletTokenAccount = async (mint: PublicKey) => {
  const destinationAta = getAssociatedTokenAddressSync(mint, globalFeeWallet);

  const tx = new Transaction().add(
    createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      destinationAta,
      globalFeeWallet,
      mint,
    ),
  );

  await processBankrunTransaction(
    bankrunContext,
    tx,
    [groupAdmin.wallet],
    false,
    true,
  );
};

export const createDriftSpotMarketWithOracle = async (
  tokenMint: PublicKey,
  tokenSymbol: string,
  marketIndex: number,
  price: number,
  decimals: number,
) => {
  const config = defaultSpotMarketConfig();
  const normalizedSymbol = tokenSymbol.toLowerCase();

  const [spotMarketPDA] = deriveSpotMarketPDA(
    driftBankrunProgram.programId,
    marketIndex,
  );

  const pullOracle = Keypair.generate();
  const pullFeed = Keypair.generate();

  await createBankrunPythOracleAccount(
    bankrunContext,
    banksClient,
    pullOracle,
    DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
  );

  driftAccounts.set(
    `drift_${normalizedSymbol}_pull_oracle`,
    pullOracle.publicKey,
  );
  driftAccounts.set(`drift_${normalizedSymbol}_pull_feed`, pullFeed.publicKey);

  await setPythPullOraclePrice(
    bankrunContext,
    banksClient,
    pullOracle.publicKey,
    pullFeed.publicKey,
    price,
    decimals,
    ORACLE_CONF_INTERVAL,
    DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
  );

  const initMarketIx = await makeInitializeSpotMarketIx(
    driftBankrunProgram,
    {
      admin: groupAdmin.wallet.publicKey,
      spotMarketMint: tokenMint,
      oracle: pullOracle.publicKey,
    },
    {
      optimalUtilization: config.optimalUtilization,
      optimalRate: config.optimalRate,
      maxRate: config.maxRate,
      oracleSource: DriftOracleSourceValues.pythPull,
      initialAssetWeight: config.initialAssetWeight,
      maintenanceAssetWeight: config.maintenanceAssetWeight,
      initialLiabilityWeight: config.initialLiabilityWeight,
      maintenanceLiabilityWeight: config.maintenanceLiabilityWeight,
      marketIndex,
    },
  );

  const tx = new Transaction().add(initMarketIx);
  await processBankrunTransaction(
    bankrunContext,
    tx,
    [groupAdmin.wallet],
    false,
    true,
  );

  driftAccounts.set(`drift_${normalizedSymbol}_spot_market`, spotMarketPDA);

  return spotMarketPDA;
};

export const fundAndDepositAdminReward = async (
  driftAdmin: Keypair,
  bank: PublicKey,
  tokenMint: PublicKey,
  marketIndex: number,
  amount: BN,
) => {
  await createIntermediaryTokenAccountIfNeeded(bank, tokenMint);
  await createGlobalFeeWalletTokenAccount(tokenMint);

  const adminTokenAccount = getAssociatedTokenAddressSync(
    tokenMint,
    driftAdmin.publicKey,
  );

  const createAdminAtaIx = createAssociatedTokenAccountIdempotentInstruction(
    globalProgramAdmin.wallet.publicKey,
    adminTokenAccount,
    driftAdmin.publicKey,
    tokenMint,
  );

  const mintToAdminIx = createMintToInstruction(
    tokenMint,
    adminTokenAccount,
    globalProgramAdmin.wallet.publicKey,
    amount.toNumber(),
  );

  const fundTx = new Transaction().add(createAdminAtaIx).add(mintToAdminIx);
  await processBankrunTransaction(
    bankrunContext,
    fundTx,
    [globalProgramAdmin.wallet],
    false,
    true,
  );

  const bankAccount = await bankrunProgram.account.bank.fetch(bank);
  const spotMarket = await getSpotMarketAccount(
    driftBankrunProgram,
    marketIndex,
  );
  const [spotMarketPDA] = deriveSpotMarketPDA(
    driftBankrunProgram.programId,
    marketIndex,
  );

  const remainingAccounts: PublicKey[] = [];
  if (!spotMarket.oracle.equals(PublicKey.default)) {
    remainingAccounts.push(spotMarket.oracle);
  }
  remainingAccounts.push(spotMarketPDA);

  const adminDepositIx = await makeAdminDepositIx(
    driftBankrunProgram,
    {
      admin: driftAdmin.publicKey,
      integrationAcc2: bankAccount.integrationAcc2,
      adminTokenAccount: adminTokenAccount,
    },
    {
      marketIndex,
      amount,
      remainingAccounts,
    },
  );

  const depositTx = new Transaction().add(adminDepositIx);
  await processBankrunTransaction(
    bankrunContext,
    depositTx,
    [driftAdmin],
    false,
    true,
  );
};

export const createThrowawayMarginfiAccount = async (
  user: MockUser,
  group: PublicKey,
) => {
  const accountKeypair = Keypair.generate();
  const initAccountIx = await accountInit(user.mrgnBankrunProgram, {
    marginfiGroup: group,
    marginfiAccount: accountKeypair.publicKey,
    authority: user.wallet.publicKey,
    feePayer: user.wallet.publicKey,
  });

  const initTx = new Transaction().add(initAccountIx);
  await processBankrunTransaction(
    bankrunContext,
    initTx,
    [user.wallet, accountKeypair],
    false,
    true,
  );

  return accountKeypair.publicKey;
};
