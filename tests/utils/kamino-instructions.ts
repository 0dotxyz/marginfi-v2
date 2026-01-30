import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import BN from "bn.js";
import { Marginfi } from "../../target/types/marginfi";
import { Program } from "@coral-xyz/anchor";
import { KaminoConfigCompact } from "./kamino-utils";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { KLEND_PROGRAM_ID } from "./types";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLendingMarketAuthority,
  deriveLiquidityVaultAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
  deriveUserMetadata,
} from "./pdas";

const DEFAULT_KAMINO_DEPOSIT_OPTIONAL_ACCOUNTS = {
  obligationFarmUserState: null,
  reserveFarmState: null,
} as const;

export interface KaminoDepositAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  reserveLiquidityMint: PublicKey;

  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
}

export const makeKaminoDepositIx = async (
  program: Program<Marginfi>,
  accounts: KaminoDepositAccounts,
  amount: BN
): Promise<TransactionInstruction> => {
  // Merge with defaults...
  const accs = {
    ...DEFAULT_KAMINO_DEPOSIT_OPTIONAL_ACCOUNTS,
    ...accounts,
  };

  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket
  );
  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralMint] = deriveReserveCollateralMint(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );

  return program.methods
    .kaminoDeposit(amount)
    .accounts({
      lendingMarketAuthority,
      reserveLiquiditySupply,
      reserveCollateralMint,
      reserveDestinationDepositCollateral: reserveCollateralSupply,
      liquidityTokenProgram: TOKEN_PROGRAM_ID,
      ...accs,
    })
    .instruction();
};

export interface KaminoHarvestRewardAccounts {
  bank: PublicKey;
  feeState: PublicKey;
  destinationTokenAccount: PublicKey;
  userState: PublicKey;
  farmState: PublicKey;
  globalConfig: PublicKey;
  rewardMint: PublicKey;
  userRewardAta: PublicKey;
  rewardsVault: PublicKey;
  rewardsTreasuryVault: PublicKey;
  farmVaultsAuthority: PublicKey;
  scopePrices?: PublicKey | null;
}

export const makeKaminoHarvestRewardIx = async (
  program: Program<Marginfi>,
  accounts: KaminoHarvestRewardAccounts,
  rewardIndex: BN
): Promise<TransactionInstruction> => {
  return program.methods
    .kaminoHarvestReward(rewardIndex)
    .accounts({
      bank: accounts.bank,
      feeState: accounts.feeState,
      userState: accounts.userState,
      farmState: accounts.farmState,
      globalConfig: accounts.globalConfig,
      rewardMint: accounts.rewardMint,
      userRewardAta: accounts.userRewardAta,
      rewardsVault: accounts.rewardsVault,
      rewardsTreasuryVault: accounts.rewardsTreasuryVault,
      farmVaultsAuthority: accounts.farmVaultsAuthority,
      scopePrices: accounts.scopePrices || null,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .accountsPartial({
      destinationTokenAccount: accounts.destinationTokenAccount,
    })
    .instruction();
};

export interface AddKaminoBankAccounts {
  group: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  integrationAcc1: PublicKey;
  kaminoMarket: PublicKey;
  oracle: PublicKey;
  tokenProgram?: PublicKey;
}

/**
 * Arguments for adding a Kamino bank
 */
export interface AddKaminoBankArgs {
  seed: BN;
  config: KaminoConfigCompact;
}

/**
 * Adds a Kamino-type bank to a marginfi group
 *
 * @param program The marginfi program
 * @param accounts The main body of accounts needed
 * @param args Arguments for adding the Kamino bank
 * @returns Instruction to add the Kamino bank
 */
export const makeAddKaminoBankIx = (
  program: Program<Marginfi>,
  accounts: AddKaminoBankAccounts,
  args: AddKaminoBankArgs
): Promise<TransactionInstruction> => {
  const oracleMeta: AccountMeta = {
    pubkey: accounts.oracle,
    isSigner: false,
    isWritable: false,
  };
  const reserveMeta: AccountMeta = {
    pubkey: accounts.integrationAcc1,
    isSigner: false,
    isWritable: false,
  };

  const [bankKey] = deriveBankWithSeed(
    program.programId,
    accounts.group,
    accounts.bankMint,
    args.seed
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bankKey
  );
  const [kaminoObligation] = deriveBaseObligation(
    liquidityVaultAuthority,
    accounts.kaminoMarket
  );

  const ix = program.methods
    .lendingPoolAddBankKamino(args.config, args.seed)
    .accounts({
      integrationAcc2: kaminoObligation,
      tokenProgram: accounts.tokenProgram || TOKEN_PROGRAM_ID,
      ...accounts,
    })
    .remainingAccounts([oracleMeta, reserveMeta])
    .instruction();

  return ix;
};

const DEFAULT_INIT_OBLIGATION_OPTIONAL_ACCOUNTS = {
  obligationFarmUserState: null,
  reserveFarmState: null,
  referrerUserMetadata: null,
  pythOracle: null,
  switchboardPriceOracle: null,
  switchboardTwapOracle: null,
  scopePrices: null,
} as const;

export interface InitObligationAccounts {
  feePayer: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  reserveLiquidityMint: PublicKey;

  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
  referrerUserMetadata?: PublicKey | null;
  // Oracle accounts for refreshing the reserve, pick just one.
  pythOracle?: PublicKey | null;
  switchboardPriceOracle?: PublicKey | null;
  switchboardTwapOracle?: PublicKey | null;
  scopePrices?: PublicKey | null;
}

/**
 * Initialize a Kamino obligation for a marginfi account
 *
 * This instruction creates the user metadata and obligation accounts in the Kamino program. It
 * requires:
 * - feePayer: The account that will pay for the transaction, and owns `signerTokenAccount` doesn't
 *   have to be the admin
 * - bank: The bank account that the obligation is for
 * - lendingMarket: The Kamino lending market the bank's reserve falls under.
 *
 * @param program The marginfi program
 * @param accounts
 * @param amount - Any nominal amount is fine. Default 100 (NO DECIMALS, just 100 exactly)
 * @returns The instruction to initialize a Kamino obligation
 */
export const makeInitObligationIx = async (
  program: Program<Marginfi>,
  accounts: InitObligationAccounts,
  amount?: BN
): Promise<TransactionInstruction> => {
  // Merge with defaults...
  const accs = {
    ...DEFAULT_INIT_OBLIGATION_OPTIONAL_ACCOUNTS,
    ...accounts,
  };

  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank
  );
  const [userMetadata] = deriveUserMetadata(
    KLEND_PROGRAM_ID,
    liquidityVaultAuthority
  );
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket
  );
  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralMint] = deriveReserveCollateralMint(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );

  const ix = await program.methods
    .kaminoInitObligation(amount ?? new BN(100))
    .accounts({
      // Derived
      userMetadata,
      lendingMarketAuthority,
      reserveLiquiditySupply,
      reserveCollateralMint,
      reserveDestinationDepositCollateral: reserveCollateralSupply,
      liquidityTokenProgram: TOKEN_PROGRAM_ID,
      ...accs,
    })
    .instruction();

  return ix;
};

const DEFAULT_KAMINO_WITHDRAW_OPTIONAL_ACCOUNTS = {
  obligationFarmUserState: null,
  reserveFarmState: null,
} as const;

export interface KaminoWithdrawAccounts {
  marginfiAccount: PublicKey;
  authority: PublicKey;
  bank: PublicKey;
  destinationTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  reserveLiquidityMint: PublicKey;

  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
}

export interface KaminoWithdrawArgs {
  amount: BN;
  isFinalWithdrawal: boolean;
  /** Oracle and other remaining accounts needed for health checks */
  remaining: PublicKey[];
}

export const makeKaminoWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: KaminoWithdrawAccounts,
  args: KaminoWithdrawArgs
): Promise<TransactionInstruction> => {
  // Merge with defaults...
  const accs = {
    ...DEFAULT_KAMINO_WITHDRAW_OPTIONAL_ACCOUNTS,
    ...accounts,
  };

  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));

  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket
  );
  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralMint] = deriveReserveCollateralMint(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );
  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
    accounts.reserveLiquidityMint
  );

  const ix = await program.methods
    .kaminoWithdraw(args.amount, args.isFinalWithdrawal)
    .accounts({
      lendingMarketAuthority, // derived
      reserveLiquiditySupply, // derived
      reserveCollateralMint, // derived
      reserveSourceCollateral: reserveCollateralSupply, // derived
      liquidityTokenProgram: TOKEN_PROGRAM_ID,
      ...accs,
    })
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};
