import { AccountInfo, PublicKey } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import BN from "bn.js";
import { Buffer } from "buffer";
import { RateLimiterLayout, RateLimiter } from "./rateLimiter";
import * as Layout from "../layout";
import { LastUpdate, LastUpdateLayout } from "./lastUpdate";
// Hardcode U64_MAX instead of importing
const U64_MAX = "18446744073709551615"; // 2^64 - 1

const BufferLayout = require("buffer-layout");

export interface Reserve {
  version: number;
  lastUpdate: LastUpdate;
  lendingMarket: PublicKey;
  liquidity: ReserveLiquidity;
  collateral: ReserveCollateral;
  config: ReserveOnchainConfig;
  rateLimiter: RateLimiter;
  pubkey: PublicKey;
}

export type RawReserveType = ReturnType<typeof parseReserve>;
export interface ReserveLiquidity {
  mintPubkey: PublicKey;
  mintDecimals: number;
  supplyPubkey: PublicKey;
  // @FIXME: oracle option
  oracleOption: number;
  pythOracle: PublicKey;
  switchboardOracle: PublicKey;
  availableAmount: BN;
  borrowedAmountWads: BN;
  cumulativeBorrowRateWads: BN;
  accumulatedProtocolFeesWads: BN;
  marketPrice: BN;
  smoothedMarketPrice: BN;
}

export interface ReserveCollateral {
  mintPubkey: PublicKey;
  mintTotalSupply: BN;
  supplyPubkey: PublicKey;
}

export interface ReserveOnchainConfig {
  optimalUtilizationRate: number;
  maxUtilizationRate: number;
  loanToValueRatio: number;
  liquidationBonus: number;
  maxLiquidationBonus: number;
  liquidationThreshold: number;
  maxLiquidationThreshold: number;
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
  borrowWeight: string;
  reserveType: AssetType;
  extraOracle?: PublicKey;
  scaledPriceOffsetBPS: BN;
  attributedBorrowLimitOpen: BN;
  attributedBorrowLimitClose: BN;
  liquidityExtraMarketPriceFlag: number;
  liquidityExtraMarketPrice: BN;
  attributedBorrowValue: BN;
}

export enum AssetType {
  Regular = 0,
  Isolated = 1,
}

export const ReserveLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.u8("version"),
    LastUpdateLayout,
    Layout.publicKey("lendingMarket"),
    Layout.publicKey("liquidityMintPubkey"),
    BufferLayout.u8("liquidityMintDecimals"),
    Layout.publicKey("liquiditySupplyPubkey"),
    // Note: oracle option field omitted - could use BufferLayout.u32('oracleOption') if needed
    Layout.publicKey("liquidityPythOracle"),
    Layout.publicKey("liquiditySwitchboardOracle"),
    Layout.uint64("liquidityAvailableAmount"),
    Layout.uint128("liquidityBorrowedAmountWads"),
    Layout.uint128("liquidityCumulativeBorrowRateWads"),
    Layout.uint128("liquidityMarketPrice"),

    Layout.publicKey("collateralMintPubkey"),
    Layout.uint64("collateralMintTotalSupply"),
    Layout.publicKey("collateralSupplyPubkey"),

    BufferLayout.u8("optimalUtilizationRate"),
    BufferLayout.u8("loanToValueRatio"),
    BufferLayout.u8("liquidationBonus"),
    BufferLayout.u8("liquidationThreshold"),
    BufferLayout.u8("minBorrowRate"),
    BufferLayout.u8("optimalBorrowRate"),
    BufferLayout.u8("maxBorrowRate"),
    Layout.uint64("borrowFeeWad"),
    Layout.uint64("flashLoanFeeWad"),
    BufferLayout.u8("hostFeePercentage"),
    Layout.uint64("depositLimit"),
    Layout.uint64("borrowLimit"),
    Layout.publicKey("feeReceiver"),
    BufferLayout.u8("protocolLiquidationFee"),
    BufferLayout.u8("protocolTakeRate"),
    Layout.uint128("accumulatedProtocolFeesWads"),
    RateLimiterLayout,
    Layout.uint64("addedBorrowWeightBPS"),
    Layout.uint128("liquiditySmoothedMarketPrice"),
    BufferLayout.u8("reserveType"),
    BufferLayout.u8("maxUtilizationRate"),
    Layout.uint64("superMaxBorrowRate"),
    BufferLayout.u8("maxLiquidationBonus"),
    BufferLayout.u8("maxLiquidationThreshold"),
    Layout.int64("scaledPriceOffsetBPS"),
    Layout.publicKey("extraOracle"),
    BufferLayout.u8("liquidityExtraMarketPriceFlag"),
    Layout.uint128("liquidityExtraMarketPrice"),
    Layout.uint128("attributedBorrowValue"),
    Layout.uint64("attributedBorrowLimitOpen"),
    Layout.uint64("attributedBorrowLimitClose"),
    BufferLayout.blob(49, "padding"),
  ]
);

function decodeReserve(buffer: Buffer, pubkey: PublicKey): Reserve {
  const reserve = ReserveLayout.decode(buffer);
  return {
    version: reserve.version,
    lastUpdate: reserve.lastUpdate,
    lendingMarket: reserve.lendingMarket,
    liquidity: {
      mintPubkey: reserve.liquidityMintPubkey,
      mintDecimals: reserve.liquidityMintDecimals,
      supplyPubkey: reserve.liquiditySupplyPubkey,
      // @FIXME: oracle option
      oracleOption: reserve.liquidityOracleOption,
      pythOracle: reserve.liquidityPythOracle,
      switchboardOracle: reserve.liquiditySwitchboardOracle,
      availableAmount: reserve.liquidityAvailableAmount,
      borrowedAmountWads: reserve.liquidityBorrowedAmountWads,
      cumulativeBorrowRateWads: reserve.liquidityCumulativeBorrowRateWads,
      marketPrice: reserve.liquidityMarketPrice,
      accumulatedProtocolFeesWads: reserve.accumulatedProtocolFeesWads,
      smoothedMarketPrice: reserve.smoothedMarketPrice,
    },
    collateral: {
      mintPubkey: reserve.collateralMintPubkey,
      mintTotalSupply: reserve.collateralMintTotalSupply,
      supplyPubkey: reserve.collateralSupplyPubkey,
    },
    config: {
      optimalUtilizationRate: reserve.optimalUtilizationRate,
      maxUtilizationRate: Math.max(
        reserve.maxUtilizationRate,
        reserve.optimalUtilizationRate
      ),
      loanToValueRatio: reserve.loanToValueRatio,
      liquidationBonus: reserve.liquidationBonus,
      maxLiquidationBonus: Math.max(
        reserve.maxLiquidationBonus,
        reserve.liquidationBonus
      ),
      liquidationThreshold: reserve.liquidationThreshold,
      maxLiquidationThreshold: Math.max(
        reserve.maxLiquidationThreshold,
        reserve.liquidationThreshold
      ),
      minBorrowRate: reserve.minBorrowRate,
      optimalBorrowRate: reserve.optimalBorrowRate,
      maxBorrowRate: reserve.maxBorrowRate,
      superMaxBorrowRate:
        reserve.superMaxBorrowRate > reserve.maxBorrowRate
          ? reserve.superMaxBorrowRate
          : new BN(reserve.maxBorrowRate),
      fees: {
        borrowFeeWad: reserve.borrowFeeWad,
        flashLoanFeeWad: reserve.flashLoanFeeWad,
        hostFeePercentage: reserve.hostFeePercentage,
      },
      depositLimit: reserve.depositLimit,
      borrowLimit: reserve.borrowLimit,
      feeReceiver: reserve.feeReceiver,
      // value is stored on-chain as deca bps (10 deca bp = 1 bps)
      protocolLiquidationFee: reserve.protocolLiquidationFee,
      protocolTakeRate: reserve.protocolTakeRate,
      addedBorrowWeightBPS: reserve.addedBorrowWeightBPS,
      borrowWeight:
        reserve.addedBorrowWeightBPS.toString() === U64_MAX
          ? U64_MAX
          : new BigNumber(reserve.addedBorrowWeightBPS.toString())
              .dividedBy(new BigNumber(10000))
              .plus(new BigNumber(1))
              .toString(),
      reserveType:
        reserve.reserveType == 0 ? AssetType.Regular : AssetType.Isolated,
      liquidityExtraMarketPriceFlag: reserve.liquidityExtraMarketPriceFlag,
      liquidityExtraMarketPrice: reserve.liquidityExtraMarketPrice,
      attributedBorrowValue: reserve.attributedBorrowValue,
      scaledPriceOffsetBPS: reserve.scaledPriceOffsetBPS,
      extraOracle: reserve.extraOracle,
      attributedBorrowLimitOpen: reserve.attributedBorrowLimitOpen,
      attributedBorrowLimitClose: reserve.attributedBorrowLimitClose,
    },
    rateLimiter: reserve.rateLimiter,
    pubkey,
  };
}

export const RESERVE_SIZE = ReserveLayout.span;

export const isReserve = (info: AccountInfo<Buffer>) =>
  info.data.length === RESERVE_SIZE;

export const parseReserve = (
  pubkey: PublicKey,
  info: AccountInfo<Buffer>,
  encoding?: string
) => {
  // Skip fzstd decompression - we don't use compressed data in tests
  if (encoding === "base64+zstd") {
    throw new Error("Compressed data not supported in test environment");
  }
  const { data } = info;
  const buffer = Buffer.from(data);
  const reserve = decodeReserve(buffer, pubkey);

  const details = {
    pubkey,
    account: {
      ...info,
    },
    info: reserve,
  };

  return details;
};

export function reserveToString(reserve: Reserve) {
  return JSON.stringify(
    reserve,
    (key, value) => {
      // Skip padding
      if (key === "padding" || key === "oracleOption" || value === undefined) {
        return null;
      }
      switch (value.constructor.name) {
        case "PublicKey":
          return value.toBase58();
        case "BN":
          return value.toString();
        default:
          return value;
      }
    },
    2
  );
}
