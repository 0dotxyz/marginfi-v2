import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { initLendingMarketInstruction } from "./solend-sdk/initLendingMarket";
import { initReserveInstruction } from "./solend-sdk/instructions/initReserve";
import { initObligationInstruction } from "./solend-sdk/instructions/initObligation";
import { depositReserveLiquidityAndObligationCollateralInstruction } from "./solend-sdk/instructions/depositReserveLiquidityAndObligationCollateral";
import { borrowObligationLiquidityInstruction } from "./solend-sdk/instructions/borrowObligationLiquidity";
import { refreshReserveInstruction } from "./solend-sdk/instructions/refreshReserve";
import { refreshObligationInstruction } from "./solend-sdk/instructions/refreshObligation";
import {
  InputReserveConfigParams,
  RESERVE_TYPE_REGULAR,
} from "./solend-sdk/types";
import { SOLEND_NULL_PUBKEY } from "./solend-utils";
import BN from "bn.js";

// Re-export obligation types and parser from local SDK
export {
  Obligation,
  ObligationCollateral,
  ObligationLiquidity,
  parseObligation,
} from "./solend-sdk/state/obligation";

// Use the production Solend program ID
export const SOLEND_PROGRAM_ID = new PublicKey(
  "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
);

// Use Pyth mainnet program ID for oracle programs in tests
export const MOCKS_PROGRAM_ID = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
);

// Oracle program IDs (using Pyth mainnet program ID for tests)
export const PYTH_ORACLE_PROGRAM_ID = MOCKS_PROGRAM_ID;
export const SWITCHBOARD_ORACLE_PROGRAM_ID = MOCKS_PROGRAM_ID;

/**
 * Initialize a new Solend lending market
 * Creates the core market account that manages reserves and global settings
 *
 * **Setup Required**: lendingMarket must be pre-created with SystemProgram.createAccount:
 * - space: SOLEND_LENDING_MARKET_SIZE (290 bytes)
 * - programId: SOLEND_PROGRAM_ID
 *
 * @param quoteCurrency - Market base currency (e.g., "USD") - converted to 32-byte buffer
 */
export const makeSolendInitMarketIx = (
  accounts: {
    lendingMarket: PublicKey;
    lendingMarketOwner: PublicKey;
  },
  args?: {
    quoteCurrency?: string;
    oracleProgramId?: PublicKey;
    switchboardProgramId?: PublicKey;
  }
): TransactionInstruction => {
  // Convert quote currency string to 32-byte buffer
  const quoteCurrency = args?.quoteCurrency || "USD";
  const quoteCurrencyBuffer = Buffer.alloc(32);
  quoteCurrencyBuffer.write(quoteCurrency);

  return initLendingMarketInstruction(
    accounts.lendingMarketOwner,
    quoteCurrencyBuffer,
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID,
    args?.oracleProgramId || PYTH_ORACLE_PROGRAM_ID,
    args?.switchboardProgramId || SWITCHBOARD_ORACLE_PROGRAM_ID
  );
};

/**
 * Default reserve configuration for test environment
 */
export const getDefaultReserveConfig = (
  feeReceiver: PublicKey,
  decimals: number = 6
): InputReserveConfigParams => {
  return {
    optimalUtilizationRate: 80,
    maxUtilizationRate: 98,
    loanToValueRatio: 75,
    liquidationBonus: 5,
    liquidationThreshold: 85,
    minBorrowRate: 0,
    optimalBorrowRate: 6,
    maxBorrowRate: 15,
    superMaxBorrowRate: new BN(150),
    fees: {
      borrowFeeWad: new BN(100000000000000), // 0.01%
      flashLoanFeeWad: new BN(300000000000000), // 0.03%
      hostFeePercentage: 20,
    },
    depositLimit: new BN(10_000_000 * 10 ** decimals), // 10M tokens (adjusted for decimals)
    borrowLimit: new BN(10_000_000 * 10 ** decimals), // 10M tokens (adjusted for decimals)
    feeReceiver,
    protocolLiquidationFee: 30,
    protocolTakeRate: 10,
    addedBorrowWeightBPS: new BN(0),
    reserveType: RESERVE_TYPE_REGULAR,
    maxLiquidationBonus: 10,
    maxLiquidationThreshold: 90,
    scaledPriceOffsetBPS: new BN(0),
    extraOracle: undefined,
    attributedBorrowLimitOpen: new BN(0),
    attributedBorrowLimitClose: new BN(0),
  };
};

/**
 * Initialize a new Solend reserve for a specific token
 * Creates all required accounts: reserve, liquidity vault, collateral mint, etc.
 *
 * **Setup Required**: These accounts must be pre-created with SystemProgram.createAccount:
 * - reserve: space SOLEND_RESERVE_SIZE (619 bytes), programId SOLEND_PROGRAM_ID
 * - liquiditySupply: space TOKEN_ACCOUNT_SIZE (165 bytes), programId TOKEN_PROGRAM_ID
 * - liquidityFeeReceiver: space TOKEN_ACCOUNT_SIZE, programId TOKEN_PROGRAM_ID
 * - collateralMint: space MINT_SIZE (82 bytes), programId TOKEN_PROGRAM_ID
 * - collateralSupply: space TOKEN_ACCOUNT_SIZE, programId TOKEN_PROGRAM_ID
 *
 * @param liquiditySupply - Empty token account to hold deposited tokens
 * @param liquidityFeeReceiver - Account to receive protocol fees
 * @param collateralMint - New mint for cTokens (created by this instruction)
 * @param collateralSupply - Token account for the collateral mint
 * @param pythOracle - Pyth price oracle for the token
 * @param transferAuthority - Usually the lending market authority
 * @param args.liquidityAmount - Initial liquidity to deposit (optional)
 */
export const makeSolendInitReserveIx = (
  accounts: {
    reserve: PublicKey;
    liquidityMint: PublicKey;
    liquiditySupply: PublicKey;
    liquidityFeeReceiver: PublicKey;
    collateralMint: PublicKey;
    collateralSupply: PublicKey;
    pythOracle: PublicKey;
    lendingMarket: PublicKey;
    lendingMarketOwner: PublicKey;
    transferAuthority: PublicKey;
    sourceLiquidity?: PublicKey;
    destinationCollateral?: PublicKey;
  },
  args?: {
    config?: InputReserveConfigParams;
    initialLiquidityAmount?: number | BN;
  }
): TransactionInstruction => {
  const lendingMarketAuthority = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  )[0];

  const config =
    args?.config || getDefaultReserveConfig(accounts.liquidityFeeReceiver);
  const liquidityAmount = args?.initialLiquidityAmount || 0;

  // For initial reserve creation, source and destination are the same as supply accounts
  const sourceLiquidity = accounts.sourceLiquidity || accounts.liquiditySupply;
  const destinationCollateral =
    accounts.destinationCollateral || accounts.collateralSupply;

  return initReserveInstruction(
    liquidityAmount,
    config,
    sourceLiquidity,
    destinationCollateral,
    accounts.reserve,
    accounts.liquidityMint,
    accounts.liquiditySupply,
    accounts.liquidityFeeReceiver,
    accounts.collateralMint,
    accounts.collateralSupply,
    accounts.pythOracle,
    SOLEND_NULL_PUBKEY, // Always use null for switchboard
    accounts.lendingMarket,
    lendingMarketAuthority,
    accounts.lendingMarketOwner,
    accounts.transferAuthority,
    SOLEND_PROGRAM_ID
  );
};

// Helper to derive market authority
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
 * Initialize a new Solend obligation for a user
 * Creates the user's account to track their deposits and borrows
 *
 * **Setup Required**: obligation account must be pre-created with SystemProgram.createAccount:
 * - space: SOLEND_OBLIGATION_SIZE (1300 bytes)
 * - programId: SOLEND_PROGRAM_ID
 *
 * @param obligation - New keypair for the obligation account
 * @param obligationOwner - User who will own this obligation
 * @param lendingMarket - The market this obligation belongs to
 */
export const makeSolendInitObligationIx = (accounts: {
  obligation: PublicKey;
  lendingMarket: PublicKey;
  obligationOwner: PublicKey;
}): TransactionInstruction => {
  return initObligationInstruction(
    accounts.obligation,
    accounts.lendingMarket,
    accounts.obligationOwner,
    SOLEND_PROGRAM_ID
  );
};

/**
 * Refresh a Solend reserve's interest rates and exchange rates
 * Must be called before deposit/withdraw/borrow operations
 * @param reserve - Reserve account to refresh
 * @param pythOracle - Pyth oracle for price updates
 * @param switchboardOracle - Switchboard oracle (use SOLEND_NULL_PUBKEY if not used)
 */
export const makeSolendRefreshReserveIx = (accounts: {
  reserve: PublicKey;
  pythOracle: PublicKey;
  switchboardOracle?: PublicKey;
  extraOracle?: PublicKey;
}): TransactionInstruction => {
  return refreshReserveInstruction(
    accounts.reserve,
    SOLEND_PROGRAM_ID,
    accounts.pythOracle,
    accounts.switchboardOracle,
    accounts.extraOracle
  );
};

/**
 * Refresh a Solend obligation's accrued interest
 * Updates deposit/borrow amounts based on current exchange rates
 * @param obligation - Obligation account to refresh
 * @param depositReserves - All reserves where user has deposits
 * @param borrowReserves - All reserves where user has borrows
 */
export const makeSolendRefreshObligationIx = (accounts: {
  obligation: PublicKey;
  depositReserves?: PublicKey[];
  borrowReserves?: PublicKey[];
}): TransactionInstruction => {
  return refreshObligationInstruction(
    accounts.obligation,
    accounts.depositReserves || [],
    accounts.borrowReserves || [],
    SOLEND_PROGRAM_ID
  );
};

/**
 * Borrow tokens from a Solend reserve
 * Moves tokens from reserve to user's token account, increases user's borrow balance
 * @param sourceLiquidity - Reserve's liquidity vault (source of borrowed tokens)
 * @param destinationLiquidity - User's token account (receives borrowed tokens)
 * @param borrowReserve - Reserve being borrowed from
 * @param borrowReserveLiquidityFeeReceiver - Account to receive borrow fees
 * @param obligation - User's obligation account (tracks debt)
 * @param depositReserves - All reserves where user has deposits (for health check)
 * @param borrowReserves - All reserves where user has borrows (for health check)
 */
export const makeSolendBorrowObligationLiquidityIx = (
  accounts: {
    sourceLiquidity: PublicKey;
    destinationLiquidity: PublicKey;
    borrowReserve: PublicKey;
    borrowReserveLiquidityFeeReceiver: PublicKey;
    obligation: PublicKey;
    lendingMarket: PublicKey;
    obligationOwner: PublicKey;
    depositReserves?: PublicKey[];
    hostFeeReceiver?: PublicKey;
  },
  args: {
    liquidityAmount: number | BN;
  }
): TransactionInstruction => {
  const lendingMarketAuthority = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  )[0];

  return borrowObligationLiquidityInstruction(
    args.liquidityAmount,
    accounts.sourceLiquidity,
    accounts.destinationLiquidity,
    accounts.borrowReserve,
    accounts.borrowReserveLiquidityFeeReceiver,
    accounts.obligation,
    accounts.lendingMarket,
    lendingMarketAuthority,
    accounts.obligationOwner,
    SOLEND_PROGRAM_ID,
    accounts.depositReserves || [],
    accounts.hostFeeReceiver
  );
};

/**
 * Deposit tokens to Solend reserve and receive collateral tokens
 * Combines deposit + collateral minting in a single instruction
 * @param sourceLiquidity - User's token account (source of deposit)
 * @param sourceCollateral - User's collateral token account (receives cTokens)
 * @param reserve - Reserve being deposited to
 * @param reserveLiquiditySupply - Reserve's liquidity vault
 * @param reserveCollateralMint - Reserve's collateral mint (cToken mint)
 * @param obligation - User's obligation account
 * @param lendingMarket - The lending market
 * @param depositReserves - All reserves where user has deposits
 * @param borrowReserves - All reserves where user has borrows
 */
export const makeSolendDepositReserveLiquidityAndObligationCollateralIx = (
  accounts: {
    sourceLiquidity: PublicKey;
    sourceCollateral: PublicKey;
    reserve: PublicKey;
    reserveLiquiditySupply: PublicKey;
    reserveCollateralMint: PublicKey;
    lendingMarket: PublicKey;
    destinationCollateral: PublicKey;
    obligation: PublicKey;
    obligationOwner: PublicKey;
    pythOracle: PublicKey;
    switchboardOracle: PublicKey;
    userTransferAuthority: PublicKey;
  },
  args: {
    liquidityAmount: number | BN;
  }
): TransactionInstruction => {
  const lendingMarketAuthority = deriveLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID
  )[0];

  return depositReserveLiquidityAndObligationCollateralInstruction(
    args.liquidityAmount,
    accounts.sourceLiquidity,
    accounts.sourceCollateral,
    accounts.reserve,
    accounts.reserveLiquiditySupply,
    accounts.reserveCollateralMint,
    accounts.lendingMarket,
    lendingMarketAuthority,
    accounts.destinationCollateral,
    accounts.obligation,
    accounts.obligationOwner,
    accounts.pythOracle,
    accounts.switchboardOracle,
    accounts.userTransferAuthority,
    SOLEND_PROGRAM_ID
  );
};
