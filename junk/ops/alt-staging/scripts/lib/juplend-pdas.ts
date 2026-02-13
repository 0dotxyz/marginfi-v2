import { PublicKey } from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

/**
 * Jupiter Lend (JupLend / Fluid) program IDs (mainnet).
 */
export const JUPLEND_LENDING_PROGRAM_ID = new PublicKey(
  "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
);

export const JUPLEND_LIQUIDITY_PROGRAM_ID = new PublicKey(
  "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC"
);

export const JUPLEND_EARN_REWARDS_PROGRAM_ID = new PublicKey(
  "jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar"
);

/**
 * LendingAdmin PDA
 * Seeds: ["lending_admin"]
 */
export function findJuplendLendingAdminPda(
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending_admin")],
    lendingProgramId
  );
}

/**
 * fToken Mint PDA
 * Seeds: ["f_token_mint", underlyingMint]
 */
export function findJuplendFTokenMintPda(
  underlyingMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("f_token_mint"), underlyingMint.toBuffer()],
    lendingProgramId
  );
}

/**
 * Lending pool PDA
 * Seeds: ["lending", underlyingMint, fTokenMint]
 */
export function findJuplendLendingPda(
  underlyingMint: PublicKey,
  fTokenMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending"), underlyingMint.toBuffer(), fTokenMint.toBuffer()],
    lendingProgramId
  );
}

export type JuplendLendingPdas = {
  lendingAdmin: PublicKey;
  fTokenMint: PublicKey;
  lending: PublicKey;
};

/**
 * Convenience: derive all Lending-program PDAs from the underlying mint.
 */
export function deriveJuplendLendingPdas(
  underlyingMint: PublicKey,
  lendingProgramId: PublicKey = JUPLEND_LENDING_PROGRAM_ID
): JuplendLendingPdas {
  const [lendingAdmin] = findJuplendLendingAdminPda(lendingProgramId);
  const [fTokenMint] = findJuplendFTokenMintPda(underlyingMint, lendingProgramId);
  const [lending] = findJuplendLendingPda(underlyingMint, fTokenMint, lendingProgramId);

  return { lendingAdmin, fTokenMint, lending };
}

/**
 * Liquidity global PDA
 * Seeds: ["liquidity"]
 */
export function findJuplendLiquidityPda(
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("liquidity")],
    liquidityProgramId
  );
}

/**
 * Liquidity TokenReserve PDA
 * Seeds: ["reserve", underlyingMint]
 */
export function findJuplendLiquidityTokenReservePda(
  underlyingMint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("reserve"), underlyingMint.toBuffer()],
    liquidityProgramId
  );
}

/**
 * Liquidity RateModel PDA
 * Seeds: ["rate_model", underlyingMint]
 */
export function findJuplendLiquidityRateModelPda(
  underlyingMint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("rate_model"), underlyingMint.toBuffer()],
    liquidityProgramId
  );
}

/**
 * Liquidity protocol supply position PDA (protocol = lending PDA)
 * Seeds: ["user_supply_position", underlyingMint, lendingPda]
 */
export function findJuplendLiquiditySupplyPositionPda(
  underlyingMint: PublicKey,
  lendingPda: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("user_supply_position"),
      underlyingMint.toBuffer(),
      lendingPda.toBuffer(),
    ],
    liquidityProgramId
  );
}

/**
 * Liquidity vault token account (ATA(liquidity_pda, underlyingMint))
 */
export function deriveJuplendLiquidityVaultAta(
  underlyingMint: PublicKey,
  liquidityPda: PublicKey,
  tokenProgramId: PublicKey = TOKEN_PROGRAM_ID
): PublicKey {
  return getAssociatedTokenAddressSync(
    underlyingMint,
    liquidityPda,
    true, // allowOwnerOffCurve
    tokenProgramId,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
}

/**
 * Rewards Rate Model PDA
 * Seeds: ["lending_rewards_rate_model", underlyingMint]
 */
export function findJuplendRewardsRateModelPda(
  underlyingMint: PublicKey,
  rewardsProgramId: PublicKey = JUPLEND_EARN_REWARDS_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("lending_rewards_rate_model"), underlyingMint.toBuffer()],
    rewardsProgramId
  );
}

/**
 * Liquidity UserClaim PDA
 * Seeds: ["user_claim", user, mint]
 */
export function findJuplendClaimAccountPda(
  user: PublicKey,
  mint: PublicKey,
  liquidityProgramId: PublicKey = JUPLEND_LIQUIDITY_PROGRAM_ID
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("user_claim"), user.toBuffer(), mint.toBuffer()],
    liquidityProgramId
  );
}

/**
 * Derive all JupLend accounts needed for CPI deposit/withdraw.
 */
export type JuplendCpiAccounts = {
  lendingAdmin: PublicKey;
  fTokenMint: PublicKey;
  lending: PublicKey;
  liquidity: PublicKey;
  liquidityProgram: PublicKey;
  tokenReserve: PublicKey;
  rateModel: PublicKey;
  vault: PublicKey;
  supplyPosition: PublicKey;
  rewardsRateModel: PublicKey;
};

export function deriveJuplendCpiAccounts(
  underlyingMint: PublicKey,
  tokenProgramId: PublicKey = TOKEN_PROGRAM_ID
): JuplendCpiAccounts {
  const lendingPdas = deriveJuplendLendingPdas(underlyingMint);
  const [liquidity] = findJuplendLiquidityPda();
  const [tokenReserve] = findJuplendLiquidityTokenReservePda(underlyingMint);
  const [rateModel] = findJuplendLiquidityRateModelPda(underlyingMint);
  const vault = deriveJuplendLiquidityVaultAta(underlyingMint, liquidity, tokenProgramId);
  const [supplyPosition] = findJuplendLiquiditySupplyPositionPda(
    underlyingMint,
    lendingPdas.lending
  );
  const [rewardsRateModel] = findJuplendRewardsRateModelPda(underlyingMint);

  return {
    ...lendingPdas,
    liquidity,
    liquidityProgram: JUPLEND_LIQUIDITY_PROGRAM_ID,
    tokenReserve,
    rateModel,
    vault,
    supplyPosition,
    rewardsRateModel,
  };
}
