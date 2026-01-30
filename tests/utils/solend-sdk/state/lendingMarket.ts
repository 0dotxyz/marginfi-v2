import { AccountInfo, PublicKey } from "@solana/web3.js";
import * as BufferLayout from "buffer-layout";
import * as Layout from "../layout";
import { RateLimiter, RateLimiterLayout } from "./rateLimiter";

export interface LendingMarket {
  version: number;
  bumpSeed: number;
  owner: PublicKey;
  quoteTokenMint: PublicKey;
  tokenProgramId: PublicKey;
  oracleProgramId: PublicKey;
  switchboardOracleProgramId: PublicKey;
  rateLimiter: RateLimiter;
  whitelistedLiquidator: PublicKey | null;
  riskAuthority: PublicKey;
}

export type ParsedLendingMarket = ReturnType<typeof parseLendingMarket>;

export const LendingMarketLayout: typeof BufferLayout.Structure =
  BufferLayout.struct([
    BufferLayout.u8("version"),
    BufferLayout.u8("bumpSeed"),
    Layout.publicKey("owner"),
    Layout.publicKey("quoteTokenMint"),
    Layout.publicKey("tokenProgramId"),
    Layout.publicKey("oracleProgramId"),
    Layout.publicKey("switchboardOracleProgramId"),
    RateLimiterLayout,
    Layout.publicKey("whitelistedLiquidator"),
    Layout.publicKey("riskAuthority"),
    BufferLayout.blob(8, "padding"),
  ]);

export const LENDING_MARKET_SIZE = LendingMarketLayout.span;

export const isLendingMarket = (info: AccountInfo<Buffer>) =>
  info.data.length === LendingMarketLayout.span;

export const parseLendingMarket = (
  pubkey: PublicKey,
  info: AccountInfo<Buffer>,
  encoding?: string
) => {
  if (encoding === "base64+zstd") {
    throw new Error(
      "base64+zstd encoding not supported - fzstd dependency not available"
    );
  }
  const { data } = info;
  const buffer = Buffer.from(data);
  const lendingMarket = LendingMarketLayout.decode(buffer) as LendingMarket;

  const details = {
    pubkey,
    account: {
      ...info,
    },
    info: lendingMarket,
  };

  return details;
};
