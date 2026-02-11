import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import { Marginfi } from "../../../target/types/marginfi";

import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVaultAuthority,
} from "../pdas";

import { JuplendPoolKeys } from "./juplend-bankrun-builder";
import { findJuplendClaimAccountPda } from "./juplend-pdas";

// Re-export for convenience
export { findJuplendClaimAccountPda } from "./juplend-pdas";

/**
 * Derivations we need when wiring a JupLend bank into a Marginfi group.
 *
 * Mirrors the minimal vault authorities/vault accounts other integrations store/use.
 */
export type JuplendMrgnAddresses = {
  bank: PublicKey;
  liquidityVaultAuthority: PublicKey;
  liquidityVault: PublicKey;
  insuranceVaultAuthority: PublicKey;
  insuranceVault: PublicKey;
  feeVaultAuthority: PublicKey;
  feeVault: PublicKey;
  /**
   * ATA owned by the bank's liquidity vault authority, for the protocol fToken mint.
   * This is where JupLend CPI mints/burns fTokens on deposit/withdraw.
   */
  fTokenVault: PublicKey;
  /**
   * JupLend claim account for the liquidity vault authority.
   * Seeds: ["user_claim", liquidityVaultAuthority, bankMint] on Liquidity program.
   * Required for withdraw despite IDL marking it optional.
   */
  claimAccount: PublicKey;
};

export type DeriveJuplendMrgnAddressesArgs = {
  mrgnProgramId: PublicKey;
  group: PublicKey;
  bankMint: PublicKey;
  bankSeed: BN;
  fTokenMint: PublicKey;
  tokenProgram?: PublicKey;
};

/**
 * Deterministically derive the Marginfi PDAs + fToken ATA used by a JupLend bank.
 */
export function deriveJuplendMrgnAddresses(
  args: DeriveJuplendMrgnAddressesArgs
): JuplendMrgnAddresses {
  const [bank] = deriveBankWithSeed(
    args.mrgnProgramId,
    args.group,
    args.bankMint,
    args.bankSeed
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    args.mrgnProgramId,
    bank
  );
  // TEMPORARY: liquidityVault uses ATA instead of PDA. Mainnet Fluid currently requires ATA for
  // depositor_token_account but this is expected to be removed soon. Revert to deriveLiquidityVault() after.
  const tokenProgram = args.tokenProgram ?? TOKEN_PROGRAM_ID;
  const liquidityVault = getAssociatedTokenAddressSync(
    args.bankMint,
    liquidityVaultAuthority,
    true, // allowOwnerOffCurve
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const [insuranceVaultAuthority] = deriveInsuranceVaultAuthority(
    args.mrgnProgramId,
    bank
  );
  const [insuranceVault] = deriveInsuranceVault(args.mrgnProgramId, bank);

  const [feeVaultAuthority] = deriveFeeVaultAuthority(args.mrgnProgramId, bank);
  const [feeVault] = deriveFeeVault(args.mrgnProgramId, bank);

  // JupLend creates fToken mints using the same token program as the underlying mint,
  // so for Token-2022 underlying mints, fToken mints are also Token-2022.
  const fTokenVault = getAssociatedTokenAddressSync(
    args.fTokenMint,
    liquidityVaultAuthority,
    true,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const [claimAccount] = findJuplendClaimAccountPda(
    liquidityVaultAuthority,
    args.bankMint
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

/**
 * For a JupLend bank, health checks require:
 *   [bank, pyth_oracle_price_update, lending_state]
 * where the lending_state is used both for owner checks and for the fToken price conversion.
 */
export function juplendHealthRemainingAccounts(
  bank: PublicKey,
  pythPriceUpdateV2: PublicKey,
  integrationAcc1: PublicKey
): PublicKey[] {
  return [bank, pythPriceUpdateV2, integrationAcc1];
}

// ---------------------------------------------------------------------------
// Marginfi instruction helpers
// ---------------------------------------------------------------------------

export type AddJuplendBankAccounts = {
  group: PublicKey;
  admin: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  bankSeed: BN;

  /** Pyth price update account (oracle_keys[0]) */
  oracle: PublicKey;
  /** JupLend lending state (oracle_keys[1]) */
  integrationAcc1: PublicKey;
  /** JupLend fToken mint (needed to create fToken vault ATA) */
  fTokenMint: PublicKey;

  /** Compact bank config (same struct used by other add_bank instructions) */
  config: any;

  tokenProgram?: PublicKey;
};

/**
 * Build `lending_pool_add_bank_juplend`.
 */
export const makeAddJuplendBankIx = async (
  program: Program<Marginfi>,
  accounts: AddJuplendBankAccounts
): Promise<TransactionInstruction> => {
  const tokenProgram = accounts.tokenProgram ?? TOKEN_PROGRAM_ID;

  const [bank] = deriveBankWithSeed(
    program.programId,
    accounts.group,
    accounts.bankMint,
    accounts.bankSeed
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bank
  );
  // TEMPORARY: ATA instead of PDA - mainnet requires ATA for now (see deriveJuplendMrgnAddresses)
  const liquidityVault = getAssociatedTokenAddressSync(
    accounts.bankMint,
    liquidityVaultAuthority,
    true,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );
  const [insuranceVaultAuthority] = deriveInsuranceVaultAuthority(
    program.programId,
    bank
  );
  const [insuranceVault] = deriveInsuranceVault(program.programId, bank);
  const [feeVaultAuthority] = deriveFeeVaultAuthority(program.programId, bank);
  const [feeVault] = deriveFeeVault(program.programId, bank);

  // fToken vault is an ATA owned by liquidityVaultAuthority for the fToken mint
  // Note: JupLend creates fToken mints using the same token program as the underlying mint,
  // so for Token-2022 underlying mints, fToken mints are also Token-2022.
  const fTokenVault = getAssociatedTokenAddressSync(
    accounts.fTokenMint,
    liquidityVaultAuthority,
    true,
    tokenProgram,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const oracleMeta: AccountMeta = {
    pubkey: accounts.oracle,
    isSigner: false,
    isWritable: false,
  };

  const lendingMeta: AccountMeta = {
    pubkey: accounts.integrationAcc1,
    isSigner: false,
    isWritable: false,
  };

  return program.methods
    .lendingPoolAddBankJuplend(accounts.config, accounts.bankSeed)
    .accounts({
      group: accounts.group,
      admin: accounts.admin,
      feePayer: accounts.feePayer,
      bankMint: accounts.bankMint,
      bank,
      integrationAcc1: accounts.integrationAcc1,
      liquidityVaultAuthority,
      liquidityVault,
      insuranceVaultAuthority,
      insuranceVault,
      feeVaultAuthority,
      feeVault,
      fTokenMint: accounts.fTokenMint,
      integrationAcc2: fTokenVault,
      tokenProgram,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .remainingAccounts([oracleMeta, lendingMeta])
    .instruction();
};

export type JuplendInitPositionAccounts = {
  feePayer: PublicKey;
  signerTokenAccount: PublicKey;

  bank: PublicKey;
  liquidityVaultAuthority: PublicKey;
  liquidityVault: PublicKey;
  fTokenVault: PublicKey;

  mint: PublicKey;
  pool: JuplendPoolKeys;
  seedDepositAmount: BN;

  tokenProgram?: PublicKey;
  associatedTokenProgram?: PublicKey;
  systemProgram?: PublicKey;
};

/**
 * Build `juplend_init_position`.
 *
 * NOTE: This helper fetches the bank to determine its `group`.
 */
export const makeJuplendInitPositionIx = async (
  program: Program<Marginfi>,
  accounts: JuplendInitPositionAccounts
): Promise<TransactionInstruction> => {
  const bank = await program.account.bank.fetch(accounts.bank);
  const group = bank.group as PublicKey;
  const admin = (program.provider as any).wallet.publicKey as PublicKey;

  return program.methods
    .juplendInitPosition(accounts.seedDepositAmount)
    .accounts({
      group,
      admin,
      feePayer: accounts.feePayer,
      signerTokenAccount: accounts.signerTokenAccount,
      bank: accounts.bank,
      liquidityVaultAuthority: accounts.liquidityVaultAuthority,
      liquidityVault: accounts.liquidityVault,
      integrationAcc2: accounts.fTokenVault,

      mint: accounts.mint,
      lendingAdmin: accounts.pool.lendingAdmin,
      integrationAcc1: accounts.pool.lending,
      fTokenMint: accounts.pool.fTokenMint,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      juplendProgram: accounts.pool.lendingProgram,

      tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram,
      associatedTokenProgram:
        accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: accounts.systemProgram ?? SystemProgram.programId,
    })
    .instruction();
};

export type JuplendDepositAccounts = {
  group: PublicKey;
  marginfiAccount: PublicKey;
  authority: PublicKey;
  signerTokenAccount: PublicKey;

  bank: PublicKey;
  liquidityVaultAuthority: PublicKey;
  liquidityVault: PublicKey;
  fTokenVault: PublicKey;

  mint: PublicKey;
  pool: JuplendPoolKeys;

  amount: BN;

  tokenProgram?: PublicKey;
  associatedTokenProgram?: PublicKey;
  systemProgram?: PublicKey;
};

/**
 * Build `juplend_deposit(amount)`.
 */
export const makeJuplendDepositIx = async (
  program: Program<Marginfi>,
  accounts: JuplendDepositAccounts
): Promise<TransactionInstruction> => {
  return program.methods
    .juplendDeposit(accounts.amount)
    .accounts({
      group: accounts.group,
      marginfiAccount: accounts.marginfiAccount,
      authority: accounts.authority,
      signerTokenAccount: accounts.signerTokenAccount,

      bank: accounts.bank,
      liquidityVaultAuthority: accounts.liquidityVaultAuthority,
      liquidityVault: accounts.liquidityVault,
      integrationAcc2: accounts.fTokenVault,

      mint: accounts.mint,
      lendingAdmin: accounts.pool.lendingAdmin,
      integrationAcc1: accounts.pool.lending,
      fTokenMint: accounts.pool.fTokenMint,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      juplendProgram: accounts.pool.lendingProgram,

      tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram,
      associatedTokenProgram:
        accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: accounts.systemProgram ?? SystemProgram.programId,
    })
    .instruction();
};

export type JuplendWithdrawAccounts = {
  group: PublicKey;
  marginfiAccount: PublicKey;
  authority: PublicKey;
  destinationTokenAccount: PublicKey;

  bank: PublicKey;
  liquidityVaultAuthority: PublicKey;
  liquidityVault: PublicKey;
  fTokenVault: PublicKey;

  mint: PublicKey;
  /** (Optional) used only for readability when constructing remaining accounts */
  underlyingOracle?: PublicKey;
  pool: JuplendPoolKeys;

  amount: BN;
  /** If true, ignore `amount` and withdraw the entire position (burn all shares). */
  withdrawAll?: boolean;
  /** Remaining accounts for risk engine (bank/oracles) */
  remainingAccounts?: PublicKey[];

  /**
   * JupLend claim account for liquidity_vault_authority.
   * TEMPORARY: Mainnet currently requires this (passing None causes ConstraintMut errors),
   * but an upcoming upgrade is expected to make it truly optional.
   */
  claimAccount: PublicKey;

  tokenProgram?: PublicKey;
  associatedTokenProgram?: PublicKey;
  systemProgram?: PublicKey;
};

/** Build `juplend_withdraw(amount, withdraw_all)` */
export const makeJuplendWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: JuplendWithdrawAccounts
): Promise<TransactionInstruction> => {
  const remaining: AccountMeta[] = (accounts.remainingAccounts ?? []).map(
    (pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    })
  );

  return program.methods
    .juplendWithdraw(accounts.amount, accounts.withdrawAll ? true : null)
    .accounts({
      group: accounts.group,
      marginfiAccount: accounts.marginfiAccount,
      authority: accounts.authority,
      destinationTokenAccount: accounts.destinationTokenAccount,

      bank: accounts.bank,
      liquidityVaultAuthority: accounts.liquidityVaultAuthority,
      liquidityVault: accounts.liquidityVault,
      integrationAcc2: accounts.fTokenVault,
      claimAccount: accounts.claimAccount,

      mint: accounts.mint,
      lendingAdmin: accounts.pool.lendingAdmin,
      integrationAcc1: accounts.pool.lending,
      fTokenMint: accounts.pool.fTokenMint,
      supplyTokenReservesLiquidity: accounts.pool.tokenReserve,
      lendingSupplyPositionOnLiquidity:
        accounts.pool.lendingSupplyPositionOnLiquidity,
      rateModel: accounts.pool.rateModel,
      vault: accounts.pool.vault,
      liquidity: accounts.pool.liquidity,
      liquidityProgram: accounts.pool.liquidityProgram,
      rewardsRateModel: accounts.pool.lendingRewardsRateModel,
      juplendProgram: accounts.pool.lendingProgram,

      tokenProgram: accounts.tokenProgram ?? accounts.pool.tokenProgram,
      associatedTokenProgram:
        accounts.associatedTokenProgram ?? ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: accounts.systemProgram ?? SystemProgram.programId,
    })
    .remainingAccounts(remaining)
    .instruction();
};
