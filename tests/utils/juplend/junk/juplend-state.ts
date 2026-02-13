import { BorshAccountsCoder, Idl } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

export const EXCHANGE_PRICE_PRECISION = 1_000_000_000_000n; // 1e12

let cachedCoder: BorshAccountsCoder | null = null;

/**
 * Loads the Lending IDL from `idls/juplend_earn.json` and constructs a BorshAccountsCoder.
 *
 * We load from disk at runtime to avoid requiring TS `resolveJsonModule` in the test toolchain.
 */
export function getJuplendLendingCoder(): BorshAccountsCoder {
  if (cachedCoder) return cachedCoder;

  const idlPath = path.join(__dirname, "../../../idls/juplend_earn.json");
  const raw = fs.readFileSync(idlPath, "utf-8");
  const idl = JSON.parse(raw) as Idl;

  cachedCoder = new BorshAccountsCoder(idl);
  return cachedCoder;
}

function toBigInt(x: unknown): bigint {
  // Anchor returns BN for u64/u128.
  // BN has `toString()`; numbers are safe to convert directly.
  if (typeof x === "bigint") return x;
  if (typeof x === "number") return BigInt(x);
  if (typeof x === "string") return BigInt(x);
  if (x && typeof (x as any).toString === "function") {
    return BigInt((x as any).toString());
  }
  throw new Error(`Unable to convert to bigint: ${String(x)}`);
}

export type JuplendLendingState = {
  mint: PublicKey;
  fTokenMint: PublicKey;
  lendingId: number;
  decimals: number;
  rewardsRateModel: PublicKey;
  liquidityExchangePrice: bigint;
  tokenExchangePrice: bigint;
  lastUpdateTimestamp: bigint;
  tokenReservesLiquidity: PublicKey;
  supplyPositionOnLiquidity: PublicKey;
  bump: number;
};

export function decodeJuplendLendingState(data: Buffer): JuplendLendingState {
  const coder = getJuplendLendingCoder();
  const decoded = coder.decode("Lending", data) as any;
  if (!decoded) throw new Error("Failed to decode Lending account");

  return {
    mint: decoded.mint as PublicKey,
    fTokenMint: decoded.f_token_mint as PublicKey,
    lendingId: decoded.lending_id as number,
    decimals: decoded.decimals as number,
    rewardsRateModel: decoded.rewards_rate_model as PublicKey,
    liquidityExchangePrice: toBigInt(decoded.liquidity_exchange_price),
    tokenExchangePrice: toBigInt(decoded.token_exchange_price),
    lastUpdateTimestamp: toBigInt(decoded.last_update_timestamp),
    tokenReservesLiquidity: decoded.token_reserves_liquidity as PublicKey,
    supplyPositionOnLiquidity: decoded.supply_position_on_liquidity as PublicKey,
    bump: decoded.bump as number,
  };
}

/**
 * Expected fToken shares minted when depositing `assets` underlying.
 *
 * Mirrors JupLend's actual deposit flow: **round down** via the liquidity layer.
 *
 * The deposit goes through a two-step conversion in the liquidity layer before
 * computing shares. The intermediate floor divisions can cause up to 1 unit of
 * rounding loss vs the naive single-step formula when exchange prices != 1e12.
 *
 * Formula (1e12 precision):
 * ```text
 * raw   = floor(assets * 1e12 / liquidity_exchange_price)
 * norm  = floor(raw * liquidity_exchange_price / 1e12)
 * shares = floor(norm * 1e12 / token_exchange_price)
 * ```
 */
export function expectedSharesForDeposit(
  assets: bigint,
  tokenExchangePrice: bigint,
  liquidityExchangePrice: bigint
): bigint {
  if (tokenExchangePrice <= 0n) throw new Error("tokenExchangePrice must be > 0");
  if (liquidityExchangePrice <= 0n) throw new Error("liquidityExchangePrice must be > 0");

  const registeredAmountRaw =
    (assets * EXCHANGE_PRICE_PRECISION) / liquidityExchangePrice;

  const registeredAmount =
    (registeredAmountRaw * liquidityExchangePrice) / EXCHANGE_PRICE_PRECISION;

  return (registeredAmount * EXCHANGE_PRICE_PRECISION) / tokenExchangePrice;
}

/**
 * shares = ceil(assets * 1e12 / token_exchange_price)
 */
export function expectedSharesForWithdraw(
  assets: bigint,
  tokenExchangePrice: bigint
): bigint {
  if (tokenExchangePrice <= 0n) throw new Error("tokenExchangePrice must be > 0");
  const num = assets * EXCHANGE_PRICE_PRECISION;
  return (num + tokenExchangePrice - 1n) / tokenExchangePrice;
}

/**
 * Expected underlying assets returned when redeeming `shares` fTokens.
 *
 * Mirrors JupLend's ERC-4626 style `preview_redeem` semantics: **round down**.
 *
 * assets = floor(shares * token_exchange_price / 1e12)
 */
export function expectedAssetsForRedeem(
  shares: bigint,
  tokenExchangePrice: bigint
): bigint {
  if (tokenExchangePrice <= 0n) throw new Error("tokenExchangePrice must be > 0");
  return (shares * tokenExchangePrice) / EXCHANGE_PRICE_PRECISION;
}

/**
 * Same-slot freshness rule (strict):
 *   require(lending.last_update_timestamp == nowTs)
 */
export function isSameSlotFresh(
  lending: JuplendLendingState,
  nowTs: bigint
): boolean {
  return lending.lastUpdateTimestamp === nowTs;
}
