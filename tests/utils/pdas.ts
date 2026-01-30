import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { KLEND_PROGRAM_ID } from "./types";

export const deriveLiquidityVaultAuthority = (
  programId: PublicKey,
  bank: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveLiquidityVault = (programId: PublicKey, bank: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity_vault", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveInsuranceVaultAuthority = (
  programId: PublicKey,
  bank: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("insurance_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveInsuranceVault = (programId: PublicKey, bank: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("insurance_vault", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveFeeVaultAuthority = (
  programId: PublicKey,
  bank: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("fee_vault_auth", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveFeeVault = (programId: PublicKey, bank: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("fee_vault", "utf-8"), bank.toBuffer()],
    programId
  );
};

export const deriveEmissionsAuth = (
  programId: PublicKey,
  bank: PublicKey,
  mint: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("emissions_auth_seed", "utf-8"),
      bank.toBuffer(),
      mint.toBuffer(),
    ],
    programId
  );
};

export const deriveEmissionsTokenAccount = (
  programId: PublicKey,
  bank: PublicKey,
  mint: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("emissions_token_account_seed", "utf-8"),
      bank.toBuffer(),
      mint.toBuffer(),
    ],
    programId
  );
};

export const deriveBankWithSeed = (
  programId: PublicKey,
  group: PublicKey,
  bankMint: PublicKey,
  seed: BN
) => {
  return PublicKey.findProgramAddressSync(
    [group.toBuffer(), bankMint.toBuffer(), seed.toArrayLike(Buffer, "le", 8)],
    programId
  );
};

// ************* Below this line, not yet included in package ****************

export const deriveGlobalFeeState = (programId: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("feestate", "utf-8")],
    programId
  );
};

export const deriveStakedSettings = (
  programId: PublicKey,
  group: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("staked_settings", "utf-8"), group.toBuffer()],
    programId
  );
};

export const deriveLiquidationRecord = (
  programId: PublicKey,
  marginfiAccount: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liq_record", "utf-8"), marginfiAccount.toBuffer()],
    programId
  );
};

export const deriveMarginfiAccountPda = (
  programId: PublicKey,
  group: PublicKey,
  authority: PublicKey,
  accountIndex: number,
  thirdPartyId?: number
) => {
  const accountIndexBuffer = Buffer.allocUnsafe(2);
  accountIndexBuffer.writeUInt16LE(accountIndex, 0);

  const thirdPartyIdBuffer = Buffer.allocUnsafe(2);
  thirdPartyIdBuffer.writeUInt16LE(thirdPartyId || 0, 0);

  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("marginfi_account", "utf-8"),
      group.toBuffer(),
      authority.toBuffer(),
      accountIndexBuffer,
      thirdPartyIdBuffer,
    ],
    programId
  );
};

export const deriveBankMetadata = (
  programId: PublicKey,
  bank: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("metadata", "utf-8"), bank.toBuffer()],
    programId
  );
};

// ************* Kamino Related ****************

export const SEED_LENDING_MARKET_AUTH = "lma";
export const SEED_RESERVE_LIQ_SUPPLY = "reserve_liq_supply";
export const SEED_FEE_RECEIVER = "fee_receiver";
export const SEED_RESERVE_COLL_MINT = "reserve_coll_mint";
export const SEED_RESERVE_COLL_SUPPLY = "reserve_coll_supply";
export const SEED_BASE_REFERRER_TOKEN_STATE = "referrer_acc";
export const SEED_BASE_USER_METADATA = "user_meta";
export const SEED_BASE_REFERRER_STATE = "ref_state";
export const SEED_BASE_SHORT_URL = "short_url";
export const SEED_USER_STATE = "user";

export function deriveLendingMarketAuthority(
  programId: PublicKey,
  lendingMarket: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_LENDING_MARKET_AUTH), lendingMarket.toBuffer()],
    programId
  );
}

export function deriveReserveLiquiditySupply(
  programId: PublicKey,
  lendingMarket: PublicKey,
  reserveLiquidityMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from(SEED_RESERVE_LIQ_SUPPLY),
      lendingMarket.toBuffer(),
      reserveLiquidityMint.toBuffer(),
    ],
    programId
  );
}

export function deriveFeeReceiver(
  programId: PublicKey,
  lendingMarket: PublicKey,
  reserveLiquidityMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from(SEED_FEE_RECEIVER),
      lendingMarket.toBuffer(),
      reserveLiquidityMint.toBuffer(),
    ],
    programId
  );
}

export function deriveReserveCollateralMint(
  programId: PublicKey,
  lendingMarket: PublicKey,
  reserveLiquidityMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from(SEED_RESERVE_COLL_MINT),
      lendingMarket.toBuffer(),
      reserveLiquidityMint.toBuffer(),
    ],
    programId
  );
}

export function deriveReserveCollateralSupply(
  programId: PublicKey,
  lendingMarket: PublicKey,
  reserveLiquidityMint: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from(SEED_RESERVE_COLL_SUPPLY),
      lendingMarket.toBuffer(),
      reserveLiquidityMint.toBuffer(),
    ],
    programId
  );
}

export function deriveReferrerTokenState(
  programId: PublicKey,
  referrer: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_BASE_REFERRER_TOKEN_STATE), referrer.toBuffer()],
    programId
  );
}

export function deriveUserMetadata(
  programId: PublicKey,
  user: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_BASE_USER_METADATA), user.toBuffer()],
    programId
  );
}

export function deriveReferrerState(
  programId: PublicKey,
  user: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_BASE_REFERRER_STATE), user.toBuffer()],
    programId
  );
}

export function deriveShortUrl(
  programId: PublicKey,
  identifier: Buffer
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_BASE_SHORT_URL), identifier],
    programId
  );
}

/**
 * Typically the obligation for each bank will have tag and id = 0
 * @param ownerPublicKey
 * @param marketPublicKey
 * @param programId - Default KLEND_PROGRAM_ID
 * @param seed1AccountKey - Default PublicKey.default
 * @param seed2AccountKey - Default PublicKey.default
 * @param tag - Default 0
 * @param id - Default 0
 * @returns
 */
export const deriveBaseObligation = (
  ownerPublicKey: PublicKey,
  marketPublicKey: PublicKey,
  programId: PublicKey = KLEND_PROGRAM_ID,
  seed1AccountKey: PublicKey = PublicKey.default,
  seed2AccountKey: PublicKey = PublicKey.default,
  tag: number = 0,
  id: number = 0
) => {
  return deriveObligation(
    programId,
    tag,
    id,
    ownerPublicKey,
    marketPublicKey,
    seed1AccountKey,
    seed2AccountKey
  );
};

export const deriveObligation = (
  programId: PublicKey,
  tag: number,
  id: number,
  ownerPublicKey: PublicKey,
  marketPublicKey: PublicKey,
  seed1AccountKey: PublicKey,
  seed2AccountKey: PublicKey
) => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from([tag]),
      Buffer.from([id]),
      ownerPublicKey.toBuffer(),
      marketPublicKey.toBuffer(),
      seed1AccountKey.toBuffer(),
      seed2AccountKey.toBuffer(),
    ],
    programId
  );
};

/**
 * Somewhat contrary to the name, this is the rewards state of the farms program for an obligation
 * (like one owned by a bank), and has nothing to do with "users" in a margin context.
 * @param programId 
 * @param farmState 
 * @param obligation 
 * @returns 
 */
export function deriveUserState(
  programId: PublicKey,
  farmState: PublicKey,
  obligation: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(SEED_USER_STATE), farmState.toBuffer(), obligation.toBuffer()],
    programId
  );
}

// ************* Drift Related ****************

export const deriveDriftStatePDA = (programId: PublicKey): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("drift_state")],
    programId
  );
};

export const deriveDriftSignerPDA = (programId: PublicKey): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("drift_signer")],
    programId
  );
};

export const deriveSpotMarketPDA = (
  programId: PublicKey,
  marketIndex: number
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("spot_market"),
      new BN(marketIndex).toArrayLike(Buffer, "le", 2),
    ],
    programId
  );
};

export const deriveSpotMarketVaultPDA = (
  programId: PublicKey,
  marketIndex: number
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("spot_market_vault"),
      new BN(marketIndex).toArrayLike(Buffer, "le", 2),
    ],
    programId
  );
};

export const deriveInsuranceFundVaultPDA = (
  programId: PublicKey,
  marketIndex: number
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("insurance_fund_vault"),
      new BN(marketIndex).toArrayLike(Buffer, "le", 2),
    ],
    programId
  );
};

export const deriveUserPDA = (
  programId: PublicKey,
  authority: PublicKey,
  subAccountId: number
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("user"),
      authority.toBuffer(),
      new BN(subAccountId).toArrayLike(Buffer, "le", 2),
    ],
    programId
  );
};

export const deriveUserStatsPDA = (
  programId: PublicKey,
  authority: PublicKey
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("user_stats"), authority.toBuffer()],
    programId
  );
};

export const deriveSolendObligation = (
  programId: PublicKey,
  bank: PublicKey
): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("solend_obligation"), bank.toBuffer()],
    programId
  );
};
