import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

import { Marginfi } from "../../target/types/marginfi";
import { Drift } from "../fixtures/drift";
import {
  deriveDriftStatePDA,
  deriveLendingMarketAuthority as deriveKaminoLendingMarketAuthority,
  deriveLiquidityVaultAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
  deriveSpotMarketVaultPDA,
} from "./pdas";
import {
  DRIFT_PROGRAM_ID,
  FARMS_PROGRAM_ID,
  KLEND_PROGRAM_ID,
  SOLEND_PROGRAM_ID,
} from "./types";
import {
  deriveLendingMarketAuthority as deriveSolendLendingMarketAuthority,
  parseSolendReserve,
  SOLEND_NULL_PUBKEY,
} from "./solend-utils";
import type { JuplendPoolKeys } from "./juplend/types";
import {
  JUPLEND_LENDING_PROGRAM_ID,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
} from "./juplend/juplend-pdas";

export type IntegrationOpMode =
  | { kamino: {} }
  | { drift: {} }
  | { solend: {} }
  | { jupLend: {} };

const meta = (pubkey: PublicKey, isWritable = false): AccountMeta => ({
  pubkey,
  isSigner: false,
  isWritable,
});

/** An unset optional layout slot is filled with the system program sentinel. */
const optionalMeta = (
  pubkey: PublicKey | null | undefined,
  isWritable = false,
): AccountMeta => meta(pubkey ?? SystemProgram.programId, pubkey ? isWritable : false);

const readonlyMetas = (keys: PublicKey[]): AccountMeta[] =>
  keys.map((pubkey) => meta(pubkey));

const resolveIntegrationAccounts = async (
  program: Program<Marginfi>,
  marginfiAccountPk: PublicKey,
  bankPk: PublicKey,
) => {
  const [marginfiAccount, bank] = await Promise.all([
    program.account.marginfiAccount.fetch(marginfiAccountPk),
    program.account.bank.fetch(bankPk),
  ]);
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bankPk,
  );

  return {
    group: marginfiAccount.group,
    liquidityVaultAuthority,
    liquidityVault: bank.liquidityVault,
    mint: bank.mint,
    integrationAcc1: bank.integrationAcc1,
    integrationAcc2: bank.integrationAcc2,
    integrationAcc3: bank.integrationAcc3.equals(PublicKey.default)
      ? null
      : bank.integrationAcc3,
  };
};

export interface IntegrationDepositAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  authority?: PublicKey;
  tokenProgram?: PublicKey;
  /** Overrides the bank-resolved integration accounts, for negative tests. */
  integrationAccOverrides?: {
    integrationAcc1?: PublicKey;
    integrationAcc2?: PublicKey;
    integrationAcc3?: PublicKey | null;
  };
}

/**
 * Build `integrationDeposit(amount, opMode)`. The named accounts (including the bank's
 * integration accounts) are resolved from the bank; `protocolMetas` is the venue's protocol
 * layout minus the integration-account slots (see the per-venue helpers below).
 */
export const makeIntegrationDepositIx = async (
  program: Program<Marginfi>,
  accounts: IntegrationDepositAccounts,
  amount: BN,
  opMode: IntegrationOpMode,
  protocolMetas: AccountMeta[],
): Promise<TransactionInstruction> => {
  const common = await resolveIntegrationAccounts(
    program,
    accounts.marginfiAccount,
    accounts.bank,
  );
  const authority =
    accounts.authority ?? (program.provider.publicKey as PublicKey);

  const overrides = accounts.integrationAccOverrides ?? {};

  return program.methods
    .integrationDeposit(amount, opMode)
    .accountsStrict({
      group: common.group,
      marginfiAccount: accounts.marginfiAccount,
      authority,
      bank: accounts.bank,
      integrationAcc1: overrides.integrationAcc1 ?? common.integrationAcc1,
      integrationAcc2: overrides.integrationAcc2 ?? common.integrationAcc2,
      integrationAcc3:
        overrides.integrationAcc3 !== undefined
          ? overrides.integrationAcc3
          : common.integrationAcc3,
      signerTokenAccount: accounts.signerTokenAccount,
      liquidityVaultAuthority: common.liquidityVaultAuthority,
      liquidityVault: common.liquidityVault,
      mint: common.mint,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(protocolMetas)
    .instruction();
};

export interface IntegrationWithdrawAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  destinationTokenAccount: PublicKey;
  authority?: PublicKey;
  tokenProgram?: PublicKey;
}

export interface IntegrationWithdrawArgs {
  amount: BN;
  withdrawAll?: boolean;
  /** Oracle accounts for the health check (and rate limiting, if enabled). */
  oracleKeys?: PublicKey[];
}

/**
 * Build `integrationWithdraw(amount, withdrawAll, opMode)`. `protocolMetas` is the venue's
 * protocol layout minus the integration-account slots; the oracle accounts follow it in
 * `remaining_accounts`.
 */
export const makeIntegrationWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: IntegrationWithdrawAccounts,
  args: IntegrationWithdrawArgs,
  opMode: IntegrationOpMode,
  protocolMetas: AccountMeta[],
): Promise<TransactionInstruction> => {
  const common = await resolveIntegrationAccounts(
    program,
    accounts.marginfiAccount,
    accounts.bank,
  );
  const authority =
    accounts.authority ?? (program.provider.publicKey as PublicKey);

  return program.methods
    .integrationWithdraw(args.amount, args.withdrawAll ? true : null, opMode)
    .accountsStrict({
      group: common.group,
      marginfiAccount: accounts.marginfiAccount,
      authority,
      bank: accounts.bank,
      integrationAcc1: common.integrationAcc1,
      integrationAcc2: common.integrationAcc2,
      integrationAcc3: common.integrationAcc3,
      destinationTokenAccount: accounts.destinationTokenAccount,
      liquidityVaultAuthority: common.liquidityVaultAuthority,
      liquidityVault: common.liquidityVault,
      mint: common.mint,
      tokenProgram: accounts.tokenProgram ?? TOKEN_PROGRAM_ID,
    })
    .remainingAccounts([
      ...protocolMetas,
      ...readonlyMetas(args.oracleKeys ?? []),
    ])
    .instruction();
};

/**
 * Kamino protocol layout minus the integration-account slots (obligation at 0, reserve at 3).
 * Identical for deposit and withdraw.
 */
export const kaminoIntegrationProtocolMetas = (accounts: {
  lendingMarket: PublicKey;
  reserve: PublicKey;
  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
}): AccountMeta[] => {
  const [lendingMarketAuthority] = deriveKaminoLendingMarketAuthority(
    KLEND_PROGRAM_ID,
    accounts.lendingMarket,
  );
  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    KLEND_PROGRAM_ID,
    accounts.reserve,
  );
  const [reserveCollateralMint] = deriveReserveCollateralMint(
    KLEND_PROGRAM_ID,
    accounts.reserve,
  );
  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    accounts.reserve,
  );

  return [
    meta(accounts.lendingMarket),
    meta(lendingMarketAuthority),
    meta(reserveLiquiditySupply, true),
    meta(reserveCollateralMint, true),
    meta(reserveCollateralSupply, true),
    meta(KLEND_PROGRAM_ID),
    meta(FARMS_PROGRAM_ID),
    meta(TOKEN_PROGRAM_ID),
    meta(SYSVAR_INSTRUCTIONS_PUBKEY),
    optionalMeta(accounts.obligationFarmUserState, true),
    optionalMeta(accounts.reserveFarmState, true),
  ];
};

/**
 * Drift protocol layout minus the integration-account slots (user at 1, user stats at 2,
 * spot market at 3).
 */
export const driftIntegrationProtocolMetas = async (
  driftProgram: Program<Drift>,
  spotMarket: PublicKey,
  direction: "deposit" | "withdraw",
  optionalAccounts: {
    oracle?: PublicKey | null;
    rewardOracle?: PublicKey | null;
    rewardOracle2?: PublicKey | null;
    rewardSpotMarket?: PublicKey | null;
    rewardSpotMarket2?: PublicKey | null;
    rewardMint?: PublicKey | null;
    rewardMint2?: PublicKey | null;
  } = {},
): Promise<AccountMeta[]> => {
  const marketIndex = (
    await driftProgram.account.spotMarket.fetch(spotMarket)
  ).marketIndex;
  const [driftState] = deriveDriftStatePDA(DRIFT_PROGRAM_ID);
  const [spotMarketVault] = deriveSpotMarketVaultPDA(
    DRIFT_PROGRAM_ID,
    marketIndex,
  );

  if (direction === "deposit") {
    return [
      meta(driftState),
      meta(spotMarketVault, true),
      meta(DRIFT_PROGRAM_ID),
      meta(SystemProgram.programId),
      optionalMeta(optionalAccounts.oracle),
    ];
  }

  const [driftSigner] = PublicKey.findProgramAddressSync(
    [Buffer.from("drift_signer")],
    DRIFT_PROGRAM_ID,
  );
  return [
    meta(driftState),
    meta(spotMarketVault, true),
    meta(driftSigner),
    meta(DRIFT_PROGRAM_ID),
    meta(SystemProgram.programId),
    optionalMeta(optionalAccounts.oracle),
    optionalMeta(optionalAccounts.rewardOracle),
    optionalMeta(optionalAccounts.rewardOracle2),
    optionalMeta(optionalAccounts.rewardSpotMarket),
    optionalMeta(optionalAccounts.rewardSpotMarket2),
    optionalMeta(optionalAccounts.rewardMint),
    optionalMeta(optionalAccounts.rewardMint2),
  ];
};

/**
 * Solend protocol layout minus the integration-account slots (obligation at 0, reserve at 3).
 * Deposits additionally carry the pyth/switchboard slots the Solend CPI expects.
 */
export const solendIntegrationProtocolMetas = async (
  program: Program<Marginfi>,
  accounts: {
    bank: PublicKey;
    lendingMarket: PublicKey;
    pythPrice: PublicKey;
  },
  direction: "deposit" | "withdraw",
): Promise<AccountMeta[]> => {
  const bank = await program.account.bank.fetch(accounts.bank);
  const reserveData = await program.provider.connection.getAccountInfo(
    bank.integrationAcc1,
  );
  if (!reserveData) {
    throw new Error("Solend reserve not found");
  }
  const reserve = parseSolendReserve(bank.integrationAcc1, reserveData);
  const [lendingMarketAuthority] = deriveSolendLendingMarketAuthority(
    accounts.lendingMarket,
    SOLEND_PROGRAM_ID,
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank,
  );
  const userCollateral = getAssociatedTokenAddressSync(
    reserve.info.collateral.mintPubkey,
    liquidityVaultAuthority,
    true,
  );

  const sharedTail = [
    meta(reserve.info.liquidity.supplyPubkey, true),
    meta(reserve.info.collateral.mintPubkey, true),
    meta(reserve.info.collateral.supplyPubkey, true),
    meta(userCollateral, true),
  ];

  if (direction === "deposit") {
    return [
      meta(accounts.lendingMarket),
      meta(lendingMarketAuthority),
      ...sharedTail,
      meta(accounts.pythPrice),
      meta(SOLEND_NULL_PUBKEY),
      meta(SOLEND_PROGRAM_ID),
    ];
  }

  return [
    meta(accounts.lendingMarket, true),
    meta(lendingMarketAuthority),
    ...sharedTail,
    meta(SOLEND_PROGRAM_ID),
  ];
};

/**
 * JupLend protocol layout minus the integration-account slots (lending at 0, fToken vault at 2,
 * and for withdrawals the intermediary ATA at 3).
 */
export const juplendIntegrationProtocolMetas = (
  pool: JuplendPoolKeys,
  direction: "deposit" | "withdraw",
  claimAccount?: PublicKey,
): AccountMeta[] => {
  const sharedTail = [
    meta(pool.rateModel),
    meta(pool.vault, true),
    ...(direction === "withdraw" ? [meta(claimAccount!, true)] : []),
    meta(pool.liquidity, true),
    meta(JUPLEND_LIQUIDITY_PROGRAM_ID),
    meta(pool.lendingRewardsRateModel),
    meta(JUPLEND_LENDING_PROGRAM_ID),
    meta(ASSOCIATED_TOKEN_PROGRAM_ID),
    meta(SystemProgram.programId),
  ];

  return [
    meta(pool.fTokenMint, true),
    meta(pool.lendingAdmin),
    meta(pool.tokenReserve, true),
    meta(pool.supplyPositionOnLiquidity, true),
    ...sharedTail,
  ];
};
