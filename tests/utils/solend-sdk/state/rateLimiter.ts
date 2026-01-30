import BigNumber from "bignumber.js";
import BN from "bn.js";
import * as BufferLayout from "buffer-layout";
import * as Layout from "../layout";

export const RATE_LIMITER_LEN = 56;
export interface RateLimiter {
  config: RateLimiterConfig;
  previousQuantity: BN;
  windowStart: BN;
  currentQuantity: BN;
}

export type ParsedRateLimiter = {
  config: {
    windowDuration: BigNumber;
    maxOutflow: BigNumber;
  };
  windowStart: BigNumber;
  previousQuantity: BigNumber;
  currentQuantity: BigNumber;
  remainingOutflow: BigNumber | null;
};

export interface RateLimiterConfig {
  windowDuration: BN;
  maxOutflow: BN;
}

export const RateLimiterLayout = BufferLayout.struct(
  [
    BufferLayout.struct(
      [Layout.uint64("maxOutflow"), Layout.uint64("windowDuration")],
      "config"
    ),
    Layout.uint128("previousQuantity"),
    Layout.uint64("windowStart"),
    Layout.uint128("currentQuantity"),
  ],
  "rateLimiter"
);
