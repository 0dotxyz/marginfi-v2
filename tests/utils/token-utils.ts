import { BN } from "@coral-xyz/anchor";
import { ecosystem } from "../rootHooks";
import { toNative } from "./tools";

/** Convert a human-readable USDC amount to native units */
export const usdcNative = (amount: number): BN =>
  toNative(amount, ecosystem.usdcDecimals);

/** Convert a human-readable token A amount to native units */
export const tokenANative = (amount: number): BN =>
  toNative(amount, ecosystem.tokenADecimals);

/** Convert a human-readable token B amount to native units */
export const tokenBNative = (amount: number): BN =>
  toNative(amount, ecosystem.tokenBDecimals);

/** Convert a human-readable wSOL amount to native units */
export const wsolNative = (amount: number): BN =>
  toNative(amount, ecosystem.wsolDecimals);
