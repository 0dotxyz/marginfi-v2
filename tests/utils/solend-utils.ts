import { PublicKey } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
import { WrappedI80F48, bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { I80F48_ONE } from "./types";
import BigNumber from "bignumber.js";

// Solend's canonical null pubkey for oracles
export const SOLEND_NULL_PUBKEY = new PublicKey(
  new Uint8Array([
    11, 193, 238, 216, 208, 116, 241, 195, 55, 212, 76, 22, 75, 202, 40, 216,
    76, 206, 27, 169, 138, 64, 177, 28, 19, 90, 156, 0, 0, 0, 0, 0,
  ])
);

// Import and re-export from local solend-sdk
import { parseReserve } from "./solend-sdk/state/reserve";
export {
  LendingMarket as SolendLendingMarket,
  parseLendingMarket as parseSolendLendingMarket,
} from "./solend-sdk/state/lendingMarket";
export {
  Reserve as SolendReserve,
  parseReserve as parseSolendReserve,
} from "./solend-sdk/state/reserve";

// Size constants - these are from the Solend protocol
export const SOLEND_RESERVE_SIZE = 619;
export const SOLEND_OBLIGATION_SIZE = 1300;
export const SOLEND_LENDING_MARKET_SIZE = 290; // Updated to match solend-mocks state.rs

// Helper function to derive lending market authority PDA
export const deriveLendingMarketAuthority = (
  lendingMarket: PublicKey,
  programId: PublicKey
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [lendingMarket.toBuffer()],
    programId
  );
};

/**
 * Constants for deterministic account creation
 */
export const SOLEND_MARKET_SEED = Buffer.from(
  "SOLEND_MARKET_SEED_0000000000000"
);
export const SOLEND_USDC_RESERVE_SEED = Buffer.from(
  "SOLEND_USDC_RESERVE_SEED_0000000"
);
export const SOLEND_TOKEN_A_RESERVE_SEED = Buffer.from(
  "SOLEND_TOKEN_A_RESERVE_SEED_0000"
);
/**
 * Max number of deposits/borrows in an obligation
 */
export const MAX_OBLIGATION_DEPOSITS = 10;
export const MAX_OBLIGATION_BORROWS = 10;

/**
 * WAD = 1e18 (for interest rate calculations)
 */
export const WAD = new BN("1000000000000000000");

// Solend bank configuration type for MarginFi integration
export interface SolendConfigCompact {
  oracle: PublicKey;
  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;
  depositLimit: BN;
  oracleSetup: { solendPythPull: {} } | { solendSwitchboardPull: {} };
  operationalState: { operational: {} } | { paused: {} } | { reduceOnly: {} };
  riskTier: { collateral: {} } | { isolated: {} };
  configFlags: number;
  totalAssetValueInitLimit: BN;
  oracleMaxAge: number;
  oracleMaxConfidence: number;
}

// Default Solend bank configuration
export const defaultSolendBankConfig = (
  oracleFeed: PublicKey,
  decimals: number = 6
): SolendConfigCompact => ({
  oracle: oracleFeed,
  assetWeightInit: bigNumberToWrappedI80F48(0.8), // 80%
  assetWeightMaint: bigNumberToWrappedI80F48(0.9), // 90%
  depositLimit: new BN(10_000_000 * 10 ** decimals), // 10 million tokens (adjusted for decimals)
  oracleSetup: { solendPythPull: {} },
  operationalState: { operational: {} }, // Start operational by default
  riskTier: { collateral: {} }, // Collateral-only by default
  configFlags: 1, // Modern bank flag (PYTH_PUSH_MIGRATED_DEPRECATED)
  totalAssetValueInitLimit: new BN(1_000_000_000_000), // 1 million USD equivalent
  oracleMaxAge: 60,
  oracleMaxConfidence: 0, // Use default 10% confidence
});

/**
 * Print detailed reserve information for debugging using Solend SDK
 */
export const printSolendReserveInfo = (
  reserveAccount: any,
  reserveAddress: PublicKey,
  tokenName: string = "Token"
): void => {
  console.log(`\n🏦 ${tokenName} Reserve Info:`);
  console.log(`  Reserve address: ${reserveAddress.toString()}`);

  if (!reserveAccount) {
    console.log(`  ❌ Reserve account not found`);
    return;
  }

  console.log(`  Account length: ${reserveAccount.data.length}`);

  // Parse using the Solend SDK
  try {
    const reserveParsed = parseReserve(reserveAddress, {
      ...reserveAccount,
      data: Buffer.from(reserveAccount.data),
    });

    console.log(`  📊 Reserve Details:`);
    console.log(
      `    Liquidity mint: ${reserveParsed.info.liquidity.mintPubkey.toString()}`
    );
    console.log(
      `    Liquidity supply: ${reserveParsed.info.liquidity.supplyPubkey.toString()}`
    );
    console.log(
      `    Available amount: ${reserveParsed.info.liquidity.availableAmount.toString()}`
    );
    console.log(
      `    Borrowed amount: ${reserveParsed.info.liquidity.borrowedAmountWads.toString()}`
    );
    console.log(
      `    Market price: ${reserveParsed.info.liquidity.marketPrice.toString()}`
    );
    console.log(
      `    Cumulative borrow rate: ${reserveParsed.info.liquidity.cumulativeBorrowRateWads.toString()}`
    );
    console.log(
      `    Accumulated protocol fees: ${reserveParsed.info.liquidity.accumulatedProtocolFeesWads.toString()}`
    );
    console.log(
      `    Smoothed market price: ${reserveParsed.info.liquidity.smoothedMarketPrice.toString()}`
    );
  } catch (e: any) {
    console.log(`  ❌ SDK parsing failed: ${e.message}`);
    console.log(
      `  Raw data preview (first 32 bytes): ${Array.from(
        reserveAccount.data.slice(0, 32)
      )
        .map((b) => b.toString().padStart(2, "0"))
        .join(" ")}`
    );
  }
};

/**
 * Calculate cToken exchange rate exactly as the original Solend SDK does
 * This is copied verbatim from /solend-sdk/src/core/utils/pools.ts lines 95-99
 */
export function calculateCTokenExchangeRate(
  totalSupply: BigNumber,
  collateralSupply: BigNumber,
  decimals: number
): BigNumber {
  return totalSupply.dividedBy(collateralSupply.shiftedBy(-decimals));
}

/**
 * Calculate expected liquidity amount from collateral using Solend's exchange rate
 */
export function calculateExpectedLiquidity(
  collateralAmount: BigNumber,
  totalSupply: BigNumber,
  collateralSupply: BigNumber,
  decimals: number
): BigNumber {
  const cTokenExchangeRate = calculateCTokenExchangeRate(
    totalSupply,
    collateralSupply,
    decimals
  );
  return collateralAmount.multipliedBy(cTokenExchangeRate);
}

/**
 * Format reserve data exactly like the original Solend SDK
 * Simplified version focusing on the exchange rate calculation
 */
export function formatReserveForDebugging(reserve: any) {
  const decimals = reserve.info.liquidity.mintDecimals;
  const availableAmount = new BigNumber(
    reserve.info.liquidity.availableAmount.toString()
  ).shiftedBy(-decimals);
  const totalBorrow = new BigNumber(
    reserve.info.liquidity.borrowedAmountWads.toString()
  ).shiftedBy(-18 - decimals);
  const accumulatedProtocolFees = new BigNumber(
    reserve.info.liquidity.accumulatedProtocolFeesWads.toString()
  ).shiftedBy(-18 - decimals);

  // This is the key calculation - copied exactly from Solend SDK
  const totalSupply = totalBorrow
    .plus(availableAmount)
    .minus(accumulatedProtocolFees);

  const cTokenExchangeRate = new BigNumber(totalSupply).dividedBy(
    new BigNumber(reserve.info.collateral.mintTotalSupply.toString()).shiftedBy(
      -decimals
    )
  );

  return {
    decimals,
    availableAmount,
    totalBorrow,
    accumulatedProtocolFees,
    totalSupply,
    cTokenExchangeRate,
    collateralSupply: new BigNumber(
      reserve.info.collateral.mintTotalSupply.toString()
    ),
  };
}

/**
 * Debug function to log all the intermediate calculations with nice formatting
 */
export function debugCTokenExchangeRate(
  reserve: any,
  collateralAmount: BigNumber,
  label: string = "SOLEND_EXCHANGE_RATE"
): {
  exchangeRate: BigNumber;
  expectedLiquidity: BigNumber;
  intermediates: {
    decimals: number;
    availableAmount: BigNumber;
    totalBorrow: BigNumber;
    accumulatedProtocolFees: BigNumber;
    totalSupply: BigNumber;
    collateralSupply: BigNumber;
  };
} {
  const formatted = formatReserveForDebugging(reserve);
  const expectedLiquidity = collateralAmount.multipliedBy(
    formatted.cTokenExchangeRate
  );

  console.log(`\n🧮 ${label} DEBUG:`);
  console.log(`   Decimals: ${formatted.decimals}`);
  console.log(`   Available Amount: ${formatted.availableAmount.toString()}`);
  console.log(`   Total Borrow (scaled): ${formatted.totalBorrow.toString()}`);
  console.log(
    `   Protocol Fees (scaled): ${formatted.accumulatedProtocolFees.toString()}`
  );
  console.log(`   Total Supply: ${formatted.totalSupply.toString()}`);
  console.log(`   Collateral Supply: ${formatted.collateralSupply.toString()}`);
  console.log(
    `   cToken Exchange Rate: ${formatted.cTokenExchangeRate.toString()}`
  );
  console.log(`   Input Collateral: ${collateralAmount.toString()}`);
  console.log(`   Expected Liquidity: ${expectedLiquidity.toString()}`);

  return {
    exchangeRate: formatted.cTokenExchangeRate,
    expectedLiquidity,
    intermediates: {
      decimals: formatted.decimals,
      availableAmount: formatted.availableAmount,
      totalBorrow: formatted.totalBorrow,
      accumulatedProtocolFees: formatted.accumulatedProtocolFees,
      totalSupply: formatted.totalSupply,
      collateralSupply: formatted.collateralSupply,
    },
  };
}

/**
 * Enhanced reserve info printer that includes exchange rate calculations
 */
export const printSolendReserveInfoWithExchangeRate = (
  reserveAccount: any,
  reserveAddress: PublicKey,
  tokenName: string = "Token",
  collateralAmount?: BigNumber
): void => {
  console.log(`\n🏦 ${tokenName} Reserve Info (Enhanced):`);
  console.log(`  Reserve address: ${reserveAddress.toString()}`);

  if (!reserveAccount) {
    console.log(`  ❌ Reserve account not found`);
    return;
  }

  console.log(`  Account length: ${reserveAccount.data.length}`);

  // Parse using the Solend SDK
  try {
    const reserveParsed = parseReserve(reserveAddress, {
      ...reserveAccount,
      data: Buffer.from(reserveAccount.data),
    });

    console.log(`  📊 Reserve Details:`);
    console.log(
      `    Liquidity mint: ${reserveParsed.info.liquidity.mintPubkey.toString()}`
    );
    console.log(
      `    Available amount: ${reserveParsed.info.liquidity.availableAmount.toString()}`
    );
    console.log(
      `    Borrowed amount: ${reserveParsed.info.liquidity.borrowedAmountWads.toString()}`
    );
    console.log(
      `    Protocol fees: ${reserveParsed.info.liquidity.accumulatedProtocolFeesWads.toString()}`
    );
    console.log(
      `    Collateral supply: ${reserveParsed.info.collateral.mintTotalSupply.toString()}`
    );

    // Add exchange rate calculation using the official Solend SDK logic
    const formatted = formatReserveForDebugging(reserveParsed);
    console.log(`\n  🧮 Exchange Rate Calculation (Official Solend SDK):`);
    console.log(`    Total Supply: ${formatted.totalSupply.toString()}`);
    console.log(
      `    cToken Exchange Rate: ${formatted.cTokenExchangeRate.toString()}`
    );

    if (collateralAmount) {
      const expectedLiquidity = collateralAmount.multipliedBy(
        formatted.cTokenExchangeRate
      );
      console.log(`    For collateral ${collateralAmount.toString()}:`);
      console.log(`    Expected liquidity: ${expectedLiquidity.toString()}`);
    }
  } catch (e: any) {
    console.log(`  ❌ SDK parsing failed: ${e.message}`);
    console.log(
      `  Raw data preview (first 32 bytes): ${Array.from(
        reserveAccount.data.slice(0, 32)
      )
        .map((b) => b.toString().padStart(2, "0"))
        .join(" ")}`
    );
  }
};
