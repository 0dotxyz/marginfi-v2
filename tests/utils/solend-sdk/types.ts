import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

export type InputReserveConfigParams = {
  optimalUtilizationRate: number;
  maxUtilizationRate: number;
  loanToValueRatio: number;
  liquidationBonus: number;
  liquidationThreshold: number;
  minBorrowRate: number;
  optimalBorrowRate: number;
  maxBorrowRate: number;
  superMaxBorrowRate: BN;
  fees: {
    borrowFeeWad: BN;
    flashLoanFeeWad: BN;
    hostFeePercentage: number;
  };
  depositLimit: BN;
  borrowLimit: BN;
  feeReceiver: PublicKey;
  protocolLiquidationFee: number;
  protocolTakeRate: number;
  addedBorrowWeightBPS: BN;
  reserveType: number;
  maxLiquidationBonus: number;
  maxLiquidationThreshold: number;
  scaledPriceOffsetBPS: BN;
  extraOracle?: PublicKey;
  attributedBorrowLimitOpen: BN;
  attributedBorrowLimitClose: BN;
};

// Reserve types from Solend
export const RESERVE_TYPE_REGULAR = 0;
export const RESERVE_TYPE_ISOLATED = 1;
