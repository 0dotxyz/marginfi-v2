import { BN } from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";

import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
} from "../pdas";
import type { JuplendPoolKeys } from "./types";

export const JUPLEND_LENDING_PROGRAM_ID = new PublicKey(
  "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9",
);

export const JUPLEND_LIQUIDITY_PROGRAM_ID = new PublicKey(
  "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC",
);

export const JUPLEND_EARN_REWARDS_PROGRAM_ID = new PublicKey(
  "jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar",
);

export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
);

export const JUPLEND_LIQUIDITY_AUTH_LIST_SEED = "auth_list";
export const JUPLEND_LENDING_REWARDS_ADMIN_SEED = "lending_rewards_admin";
export const JUPLEND_F_TOKEN_VAULT_SEED = "f_token_vault";
export const TOKEN_METADATA_SEED = "metadata";

export type JuplendPoolKeysArgs = {
  mint: PublicKey;
  tokenProgram?: PublicKey;
  liquidityProgramId?: PublicKey;
  lendingProgramId?: PublicKey;
  rewardsProgramId?: PublicKey;
};

export type JuplendGlobalKeys = {
  liquidity: PublicKey;
  authList: PublicKey;
  lendingAdmin: PublicKey;
  lendingRewardsAdmin: PublicKey;
};

export function findJuplendLendingAdminPda(
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending_admin")],
    lendingProgramId,
  );
}

export function findJuplendFTokenMintPda(
  underlyingMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("f_token_mint"), underlyingMint.toBuffer()],
    lendingProgramId,
  );
}

export function findJuplendLendingPda(
  underlyingMint: PublicKey,
  fTokenMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending"), underlyingMint.toBuffer(), fTokenMint.toBuffer()],
    lendingProgramId,
  );
}

export type JuplendLendingPdas = {
  lendingAdmin: PublicKey;
  lendingAdminBump: number;
  fTokenMint: PublicKey;
  fTokenMintBump: number;
  lending: PublicKey;
  lendingBump: number;
};

export function deriveJuplendLendingPdas(
  underlyingMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID,
): JuplendLendingPdas {
  const [lendingAdmin, lendingAdminBump] =
    findJuplendLendingAdminPda(lendingProgramId);
  const [fTokenMint, fTokenMintBump] = findJuplendFTokenMintPda(
    underlyingMint,
    lendingProgramId,
  );
  const [lending, lendingBump] = findJuplendLendingPda(
    underlyingMint,
    fTokenMint,
    lendingProgramId,
  );

  return {
    lendingAdmin,
    lendingAdminBump,
    fTokenMint,
    fTokenMintBump,
    lending,
    lendingBump,
  };
}

export function findJuplendLiquidityPda(
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity")],
    liquidityProgramId,
  );
}

export function findJuplendLiquidityAuthListPda(
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(JUPLEND_LIQUIDITY_AUTH_LIST_SEED)],
    liquidityProgramId,
  );
}

export function findJuplendLiquidityTokenReservePda(
  underlyingMint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("reserve"), underlyingMint.toBuffer()],
    liquidityProgramId,
  );
}

export function findJuplendLiquidityRateModelPda(
  underlyingMint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("rate_model"), underlyingMint.toBuffer()],
    liquidityProgramId,
  );
}

export function findJuplendLiquiditySupplyPositionPda(
  underlyingMint: PublicKey,
  lendingPda: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("user_supply_position"),
      underlyingMint.toBuffer(),
      lendingPda.toBuffer(),
    ],
    liquidityProgramId,
  );
}

export function findJuplendLiquidityBorrowPositionPda(
  underlyingMint: PublicKey,
  lendingPda: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("user_borrow_position"),
      underlyingMint.toBuffer(),
      lendingPda.toBuffer(),
    ],
    liquidityProgramId,
  );
}

export function deriveJuplendLiquidityVaultAta(
  underlyingMint: PublicKey,
  liquidityPda: PublicKey,
  tokenProgramId: PublicKey = TOKEN_PROGRAM_ID,
): PublicKey {
  return getAssociatedTokenAddressSync(
    underlyingMint,
    liquidityPda,
    true,
    tokenProgramId,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );
}

export function findJuplendTokenMetadataPda(
  mint: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from(TOKEN_METADATA_SEED),
      TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      mint.toBuffer(),
    ],
    TOKEN_METADATA_PROGRAM_ID,
  );
}

export function findJuplendLendingRewardsAdminPda(
  rewardsProgramId: PublicKey = JUPLEND_EARN_REWARDS_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(JUPLEND_LENDING_REWARDS_ADMIN_SEED)],
    rewardsProgramId,
  );
}

export function findJuplendRewardsRateModelPdaBestEffort(
  underlyingMint: PublicKey,
  rewardsProgramId: PublicKey = JUPLEND_EARN_REWARDS_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending_rewards_rate_model"), underlyingMint.toBuffer()],
    rewardsProgramId,
  );
}

export function findJuplendClaimAccountPda(
  user: PublicKey,
  mint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("user_claim"), user.toBuffer(), mint.toBuffer()],
    liquidityProgramId,
  );
}

export const deriveJuplendFTokenVault = (
  programId: PublicKey,
  bank: PublicKey,
) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(JUPLEND_F_TOKEN_VAULT_SEED, "utf-8"), bank.toBuffer()],
    programId,
  );
};

export const deriveJuplendGlobalKeys = (args?: {
  liquidityProgramId?: PublicKey;
  lendingProgramId?: PublicKey;
  rewardsProgramId?: PublicKey;
}): JuplendGlobalKeys => {
  const liquidityProgramId =
    args?.liquidityProgramId ?? JUPLEND_LIQUIDITY_PROGRAM_ID;
  const lendingProgramId = args?.lendingProgramId ?? JUPLEND_LENDING_PROGRAM_ID;
  const rewardsProgramId =
    args?.rewardsProgramId ?? JUPLEND_EARN_REWARDS_PROGRAM_ID;

  const [liquidity] = findJuplendLiquidityPda(liquidityProgramId);
  const [authList] = findJuplendLiquidityAuthListPda(liquidityProgramId);
  const [lendingAdmin] = findJuplendLendingAdminPda(lendingProgramId);
  const [lendingRewardsAdmin] =
    findJuplendLendingRewardsAdminPda(rewardsProgramId);

  return { liquidity, authList, lendingAdmin, lendingRewardsAdmin };
};

export const deriveJuplendPoolKeys = (
  args: JuplendPoolKeysArgs,
): JuplendPoolKeys => {
  const liquidityProgramId =
    args.liquidityProgramId ?? JUPLEND_LIQUIDITY_PROGRAM_ID;
  const lendingProgramId = args.lendingProgramId ?? JUPLEND_LENDING_PROGRAM_ID;
  const rewardsProgramId =
    args.rewardsProgramId ?? JUPLEND_EARN_REWARDS_PROGRAM_ID;

  const [liquidity] = findJuplendLiquidityPda(liquidityProgramId);
  const [authList] = findJuplendLiquidityAuthListPda(liquidityProgramId);
  const [tokenReserve] = findJuplendLiquidityTokenReservePda(
    args.mint,
    liquidityProgramId,
  );
  const [rateModel] = findJuplendLiquidityRateModelPda(
    args.mint,
    liquidityProgramId,
  );
  const vault = deriveJuplendLiquidityVaultAta(
    args.mint,
    liquidity,
    args.tokenProgram,
  );

  const { fTokenMint, lending, lendingAdmin } = deriveJuplendLendingPdas(
    args.mint,
    lendingProgramId,
  );
  const [fTokenMetadata] = findJuplendTokenMetadataPda(fTokenMint);

  const [lendingRewardsAdmin] =
    findJuplendLendingRewardsAdminPda(rewardsProgramId);
  const [lendingRewardsRateModel] = findJuplendRewardsRateModelPdaBestEffort(
    args.mint,
    rewardsProgramId,
  );

  const [supplyPositionOnLiquidity] = findJuplendLiquiditySupplyPositionPda(
    args.mint,
    lending,
    liquidityProgramId,
  );
  const [borrowPositionOnLiquidity] = findJuplendLiquidityBorrowPositionPda(
    args.mint,
    lending,
    liquidityProgramId,
  );

  return {
    mint: args.mint,
    tokenProgram: args.tokenProgram,
    liquidityProgram: liquidityProgramId,
    lendingProgram: lendingProgramId,
    liquidity,
    authList,
    tokenReserve,
    rateModel,
    vault,
    lendingRewardsAdmin,
    lendingRewardsRateModel,
    lendingAdmin,
    lending,
    fTokenMint,
    fTokenMetadata,
    supplyPositionOnLiquidity,
    borrowPositionOnLiquidity,
    lendingSupplyPositionOnLiquidity: supplyPositionOnLiquidity,
    lendingBorrowPositionOnLiquidity: borrowPositionOnLiquidity,
  };
};

export type JuplendMrgnAddresses = {
  bank: PublicKey;
  liquidityVaultAuthority: PublicKey;
  liquidityVault: PublicKey;
  insuranceVaultAuthority: PublicKey;
  insuranceVault: PublicKey;
  feeVaultAuthority: PublicKey;
  feeVault: PublicKey;
  fTokenVault: PublicKey;
  claimAccount: PublicKey;
};

export type DeriveJuplendMrgnAddressesArgs = {
  mrgnProgramId: PublicKey;
  group: PublicKey;
  bankMint: PublicKey;
  bankSeed: BN;
};

export function deriveJuplendMrgnAddresses(
  args: DeriveJuplendMrgnAddressesArgs,
): JuplendMrgnAddresses {
  const [bank] = deriveBankWithSeed(
    args.mrgnProgramId,
    args.group,
    args.bankMint,
    args.bankSeed,
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    args.mrgnProgramId,
    bank,
  );
  const [liquidityVault] = deriveLiquidityVault(args.mrgnProgramId, bank);

  const [insuranceVaultAuthority] = deriveInsuranceVaultAuthority(
    args.mrgnProgramId,
    bank,
  );
  const [insuranceVault] = deriveInsuranceVault(args.mrgnProgramId, bank);

  const [feeVaultAuthority] = deriveFeeVaultAuthority(args.mrgnProgramId, bank);
  const [feeVault] = deriveFeeVault(args.mrgnProgramId, bank);

  const [fTokenVault] = deriveJuplendFTokenVault(args.mrgnProgramId, bank);

  const [claimAccount] = findJuplendClaimAccountPda(
    liquidityVaultAuthority,
    args.bankMint,
  );

  return {
    bank,
    liquidityVaultAuthority,
    liquidityVault,
    insuranceVaultAuthority,
    insuranceVault,
    feeVaultAuthority,
    feeVault,
    fTokenVault,
    claimAccount,
  };
}
