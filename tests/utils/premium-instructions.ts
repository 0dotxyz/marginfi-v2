import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Marginfi } from "../../target/types/marginfi";
import { u32_MAX } from "./types";

/**
 * Variable-borrow premium test helpers: PDA derivation, rate encoding, and instruction builders
 * for the FeeStateV2 / group-matrix / bank-premium / sweep instructions.
 */

/** `bank.flags` bit 11: premium accrual active for this (liability) bank. */
export const PREMIUM_ACTIVE = 1 << 11; // 2048
/** Storage capacity of the group premium matrix at the current account size. */
export const MAX_PREMIUM_ENTRIES = 64;

/**
 * Encode a premium/cap APR (as a fraction, e.g. 0.01 for 1%) into the on-chain u32.
 * Mirrors `milli_to_u32`: `u32::MAX` == 1000% APR (10.0), so 1% == 4294967.
 */
export const premiumRateToU32 = (fraction: number): number => {
  if (fraction < 0 || fraction > 10) {
    console.error(
      "premium rate out of range, exp 0-1000% (0-10), will clamp: " + fraction,
    );
  }
  const clamped = Math.max(0, Math.min(fraction, 10));
  return Math.round((clamped / 10) * u32_MAX);
};

/** Decode an on-chain premium u32 back into a fraction (inverse of `premiumRateToU32`). */
export const u32ToPremiumRate = (encoded: number): number =>
  (encoded / u32_MAX) * 10;

export type PremiumEntry = {
  collateralTag: number;
  liabilityTag: number;
  rate: number;
};

/**
 * Build a `PremiumEntry` from tags and an APR fraction (e.g. `newPremiumEntry(200, 100, 0.01)`
 * for a 1% surcharge on collateral-tag-200 against liability-tag-100).
 */
export const newPremiumEntry = (
  collateralTag: number,
  liabilityTag: number,
  rateFraction: number,
): PremiumEntry => ({
  collateralTag,
  liabilityTag,
  rate: premiumRateToU32(rateFraction),
});

export const deriveFeeStateV2 = (programId: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("feestate_v2", "utf-8")],
    programId,
  );
};

/**
 * (global fee admin) Create the FeeStateV2 PDA. `payer` also becomes the rent payer and must sign.
 */
export const initGlobalFeeStateV2 = (
  program: Program<Marginfi>,
  args: { payer: PublicKey },
) => {
  return program.methods
    .initGlobalFeeStateV2()
    .accounts({
      payer: args.payer,
      // feeStateV2: derived from constant seed
      // systemProgram: hard coded key
    })
    .instruction();
};

/**
 * (permissionless) Copy the v1 fee-state fields into FeeStateV2. Leaves the premium fields intact.
 */
export const copyFeeStateToV2 = (program: Program<Marginfi>) => {
  return program.methods
    .copyFeeStateToV2()
    .accounts({
      // feeState: derived from constant seed
      // feeStateV2: derived from constant seed
    })
    .instruction();
};

export type EditFeeStateV2PremiumArgs = {
  /** Signer; must match `FeeStateV2.global_fee_admin`. */
  admin: PublicKey;
  /** undefined = leave unchanged. */
  premiumWallet?: PublicKey;
};

/**
 * (global fee admin) Set the premium sweep wallet on FeeStateV2.
 */
export const editFeeStateV2Premium = (
  program: Program<Marginfi>,
  args: EditFeeStateV2PremiumArgs,
) => {
  return program.methods
    .editFeeStateV2Premium(args.premiumWallet ?? null)
    .accounts({
      globalFeeAdmin: args.admin,
      // feeStateV2: derived from constant seed
    })
    .instruction();
};

export type ConfigGroupPremiumArgs = {
  group: PublicKey;
  /** One (collateral, liability) pair; `rate > 0` inserts/updates, `rate == 0` removes it. */
  entry: PremiumEntry;
};

/**
 * (emode admin) Set one pair of the group's premium matrix (like emode, one pair per
 * instruction). The signer (provider wallet) must be the group's `emode_admin`.
 */
export const configGroupPremium = (
  program: Program<Marginfi>,
  args: ConfigGroupPremiumArgs,
) => {
  return program.methods
    .lendingPoolConfigureGroupPremium(
      args.entry.collateralTag,
      args.entry.liabilityTag,
      args.entry.rate,
    )
    .accounts({
      group: args.group,
      // emodeAdmin: signer, implied from provider wallet (checked has_one against group)
    })
    .instruction();
};

export type ConfigBankPremiumArgs = {
  bank: PublicKey;
  premiumTag: number;
  active: boolean;
};

/**
 * (emode admin) Set a bank's premium tag and toggle premium accrual for its borrowers. The signer
 * (provider wallet) must be the group's `emode_admin`.
 */
export const configBankPremium = (
  program: Program<Marginfi>,
  args: ConfigBankPremiumArgs,
) => {
  return program.methods
    .lendingPoolConfigureBankPremium(args.premiumTag, args.active)
    .accounts({
      // group: implied from bank
      // emodeAdmin: signer, implied from provider wallet
      bank: args.bank,
    })
    .instruction();
};

export type CollectBankPremiumFeesArgs = {
  bank: PublicKey;
  /** Canonical ATA of `FeeStateV2.premium_wallet` for the bank's mint. Must already exist. */
  premiumAta: PublicKey;
};

/**
 * (permissionless) Sweep realized premium from the bank's liquidity vault to the premium wallet's
 * canonical ATA.
 */
export const collectBankPremiumFees = (
  program: Program<Marginfi>,
  args: CollectBankPremiumFeesArgs,
) => {
  return program.methods
    .lendingPoolCollectBankPremiumFees()
    .accounts({
      // group: implied from bank
      bank: args.bank,
      // liquidityVaultAuthority / liquidityVault: pdas from bank
      // feeStateV2: derived from constant seed
      premiumAta: args.premiumAta,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();
};
