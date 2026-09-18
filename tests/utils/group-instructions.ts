import { BN, Program, AnchorProvider } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import { Marginfi } from "../../target/types/marginfi";
import {
  deriveBankWithSeed,
  deriveOnRampPool,
  deriveSameAssetEmodeRegistry,
  deriveStakedSettings,
} from "./pdas";
import {
  BankConfig,
  BankConfigOptRaw,
  EmodeEntry,
  I80F48_ZERO,
  MAX_EMODE_ENTRIES,
  ORACLE_SETUP_FIXED,
  SINGLE_POOL_PROGRAM_ID,
  StakedSettingsConfig,
  StakedSettingsEdit,
} from "./types";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { bigNumberToWrappedI80F48, WrappedI80F48 } from "@mrgnlabs/mrgn-common";

export const MAX_ORACLE_KEYS = 5;

/**
 * * admin/feePayer - must sign
 * * bank - use a fresh keypair, must sign
 */
export type AddBankArgs = {
  marginfiGroup: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  bank: PublicKey;
  config: BankConfig;
};

export const addBank = (program: Program<Marginfi>, args: AddBankArgs) => {
  const ix = program.methods
    .lendingPoolAddBank({
      assetWeightInit: args.config.assetWeightInit,
      assetWeightMaint: args.config.assetWeightMaint,
      liabilityWeightInit: args.config.liabilityWeightInit,
      liabilityWeightMaint: args.config.liabilityWeightMain,
      depositLimit: args.config.depositLimit,
      interestRateConfig: args.config.interestRateConfig,
      operationalState: args.config.operationalState,
      borrowLimit: args.config.borrowLimit,
      riskTier: args.config.riskTier,
      assetTag: args.config.assetTag,
      configFlags: args.config.configFlags,
      pad0: [0, 0, 0, 0, 0, 0],
      totalAssetValueInitLimit: args.config.totalAssetValueInitLimit,
      oracleMaxAge: args.config.oracleMaxAge,
      oracleMaxConfidence: args.config.oracleMaxConfidence,
    })
    .accounts({
      marginfiGroup: args.marginfiGroup,
      // governanceAdmin: signer, implied from group
      feePayer: args.feePayer,
      bankMint: args.bankMint,
      bank: args.bank,
      // globalFeeState: deriveGlobalFeeState(id),
      // globalFeeWallet: args.globalFeeWallet,
      // liquidityVaultAuthority = deriveLiquidityVaultAuthority(id, bank);
      // liquidityVault = deriveLiquidityVault(id, bank);
      // insuranceVaultAuthority = deriveInsuranceVaultAuthority(id, bank);
      // insuranceVault = deriveInsuranceVault(id, bank);
      // feeVaultAuthority = deriveFeeVaultAuthority(id, bank);
      // feeVault = deriveFeeVault(id, bank);
      // rent = SYSVAR_RENT_PUBKEY
      tokenProgram: TOKEN_PROGRAM_ID,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

/**
 * * admin/feePayer - must sign
 * * bank - use a fresh keypair, must sign
 */
export type AddBankWithSeedArgs = {
  marginfiGroup: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  config: BankConfig;
  seed?: BN;
};

export const addBankWithSeed = (
  program: Program<Marginfi>,
  args: AddBankWithSeedArgs,
) => {
  const ix = program.methods
    .lendingPoolAddBankWithSeed(
      {
        assetWeightInit: args.config.assetWeightInit,
        assetWeightMaint: args.config.assetWeightMaint,
        liabilityWeightInit: args.config.liabilityWeightInit,
        liabilityWeightMaint: args.config.liabilityWeightMain,
        depositLimit: args.config.depositLimit,
        interestRateConfig: args.config.interestRateConfig,
        operationalState: args.config.operationalState,
        borrowLimit: args.config.borrowLimit,
        riskTier: args.config.riskTier,
        assetTag: args.config.assetTag,
        configFlags: args.config.configFlags,
        pad0: [0, 0, 0, 0, 0, 0],
        totalAssetValueInitLimit: args.config.totalAssetValueInitLimit,
        oracleMaxAge: args.config.oracleMaxAge,
        oracleMaxConfidence: args.config.oracleMaxConfidence,
      },
      args.seed ?? new BN(0),
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
      // governanceAdmin: signer, implied from group
      feePayer: args.feePayer,
      bankMint: args.bankMint,
      // bank: args.bank, // derived from seed
      // globalFeeState: deriveGlobalFeeState(id),
      // globalFeeWallet: args.globalFeeWallet,
      // liquidityVaultAuthority = deriveLiquidityVaultAuthority(id, bank);
      // liquidityVault = deriveLiquidityVault(id, bank);
      // insuranceVaultAuthority = deriveInsuranceVaultAuthority(id, bank);
      // insuranceVault = deriveInsuranceVault(id, bank);
      // feeVaultAuthority = deriveFeeVaultAuthority(id, bank);
      // feeVault = deriveFeeVault(id, bank);
      // rent = SYSVAR_RENT_PUBKEY
      tokenProgram: TOKEN_PROGRAM_ID,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

/**
 * Every omitted field is encoded as `null` and left unchanged. Fast-admin configuration and
 * slow-governance configuration use distinct instructions. `newFastAdmin` deliberately routes a
 * fast-admin rotation through the governance instruction. Use `groupConfigureIxs` when a test
 * updates both classes in the same transaction.
 */
export type GroupConfigureArgs = {
  newAdmin?: PublicKey | null; // optional; pass null or leave undefined to keep current admin
  /** Optional fast-admin rotation authorized by the governance admin. */
  newFastAdmin?: PublicKey | null;
  newEmodeAdmin?: PublicKey | null;
  newCurveAdmin?: PublicKey | null;
  newLimitAdmin?: PublicKey | null;
  newFlowAdmin?: PublicKey | null;
  newEmissionsAdmin?: PublicKey | null;
  newMetadataAdmin?: PublicKey | null;
  newRiskAdmin?: PublicKey | null;
  marginfiGroup: PublicKey;
  emodeMaxInitLeverage?: WrappedI80F48 | null;
  emodeMaxMaintLeverage?: WrappedI80F48 | null;
  sameAssetEmodeInitLeverage?: WrappedI80F48 | null;
  sameAssetEmodeMaintLeverage?: WrappedI80F48 | null;
};

const groupConfigureFast = (
  program: Program<Marginfi>,
  args: GroupConfigureArgs,
) => {
  return program.methods
    .marginfiGroupConfigure(
      args.newAdmin ?? null,
      args.newCurveAdmin ?? null,
      args.newLimitAdmin ?? null,
      args.newFlowAdmin ?? null,
      args.newEmissionsAdmin ?? null,
      args.newMetadataAdmin ?? null,
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();
};

const groupConfigureGov = (
  program: Program<Marginfi>,
  args: GroupConfigureArgs,
) => {
  return program.methods
    .marginfiGroupConfigureGov(
      args.newFastAdmin ?? null,
      args.newEmodeAdmin ?? null,
      args.newRiskAdmin ?? null,
      args.emodeMaxInitLeverage ?? null,
      args.emodeMaxMaintLeverage ?? null,
      args.sameAssetEmodeInitLeverage ?? null,
      args.sameAssetEmodeMaintLeverage ?? null,
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();
};

const hasFastGroupConfig = (args: GroupConfigureArgs) =>
  args.newAdmin != null ||
  args.newCurveAdmin != null ||
  args.newLimitAdmin != null ||
  args.newFlowAdmin != null ||
  args.newEmissionsAdmin != null ||
  args.newMetadataAdmin != null;

const hasGovGroupConfig = (args: GroupConfigureArgs) =>
  args.newFastAdmin != null ||
  args.newEmodeAdmin != null ||
  args.newRiskAdmin != null ||
  args.emodeMaxInitLeverage != null ||
  args.emodeMaxMaintLeverage != null ||
  args.sameAssetEmodeInitLeverage != null ||
  args.sameAssetEmodeMaintLeverage != null;

/** Build one explicitly authorized group-configuration instruction. */
export const groupConfigure = async (
  program: Program<Marginfi>,
  args: GroupConfigureArgs,
) => {
  const fast = hasFastGroupConfig(args);
  const gov = hasGovGroupConfig(args);
  if (fast && gov) {
    throw new Error(
      "groupConfigure received fast and governance fields; use groupConfigureIxs",
    );
  }
  return gov
    ? groupConfigureGov(program, args)
    : groupConfigureFast(program, args);
};

/** Build explicit fast and governance instructions for one atomic test transaction. */
export const groupConfigureIxs = async (
  program: Program<Marginfi>,
  args: GroupConfigureArgs,
): Promise<TransactionInstruction[]> => {
  const ixs: TransactionInstruction[] = [];
  if (hasFastGroupConfig(args))
    ixs.push(await groupConfigureFast(program, args));
  if (hasGovGroupConfig(args)) ixs.push(await groupConfigureGov(program, args));
  return ixs;
};

export type GroupInitializeArgs = {
  marginfiGroup: PublicKey;
  admin: PublicKey;
};

export const groupInitialize = (
  program: Program<Marginfi>,
  args: GroupInitializeArgs,
) => {
  const ix = program.methods
    .marginfiGroupInitialize()
    .accounts({
      marginfiGroup: args.marginfiGroup,
      // feeState: deriveGlobalFeeState(id),
      admin: args.admin,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

/** Bootstrap or rotate a group's slow governance admin. */
export const setGovernanceAdmin = (
  program: Program<Marginfi>,
  args: {
    marginfiGroup: PublicKey;
    newGovernanceAdmin: PublicKey;
    signer?: PublicKey;
  },
) => {
  const signer =
    args.signer ?? (program.provider as AnchorProvider).wallet.publicKey;
  return program.methods
    .marginfiGroupSetGovernanceAdmin(args.newGovernanceAdmin)
    .accounts({
      marginfiGroup: args.marginfiGroup,
      signer,
    })
    .instruction();
};

export type ResizeGroupAccountArgs = {
  group: PublicKey;
  /** Funds the rent for the added account space. */
  payer: PublicKey;
};

/**
 * (permissionless) Resize a group account to the v2 layout size. Errors if the account is
 * already at (or above) the target size.
 */
export const resizeGroupAccount = (
  program: Program<Marginfi>,
  args: ResizeGroupAccountArgs,
) => {
  return program.methods
    .lendingPoolResizeGroupAccount()
    .accounts({
      group: args.group,
      payer: args.payer,
      // systemProgram: hard coded key
    })
    .instruction();
};

export type ResizeGlobalFeeStateArgs = {
  /** Funds the rent for the added account space. */
  payer: PublicKey;
};

/**
 * (permissionless) Resize the fee-state account to the v2 layout size. Errors if the account
 * is already at (or above) the target size.
 */
export const resizeGlobalFeeState = (
  program: Program<Marginfi>,
  args: ResizeGlobalFeeStateArgs,
) => {
  return program.methods
    .resizeGlobalFeeState()
    .accounts({
      // feeState: derived from constant seed
      payer: args.payer,
      // systemProgram: hard coded key
    })
    .instruction();
};

export type ConfigureBankArgs = {
  bank: PublicKey;
  bankConfigOpt: BankConfigOptRaw;
  group?: PublicKey;
  signer?: PublicKey;
};

const isOperational = (state: BankConfigOptRaw["operationalState"]) =>
  state != null && "operational" in state;

const hasFastBankConfig = (config: BankConfigOptRaw) =>
  config.depositLimit != null ||
  config.borrowLimit != null ||
  (config.operationalState != null &&
    !isOperational(config.operationalState)) ||
  config.interestRateConfig != null ||
  config.totalAssetValueInitLimit != null ||
  config.permissionlessBadDebtSettlement != null ||
  config.freezeSettings != null ||
  config.liquidationLiquidatorFee != null ||
  config.liquidationInsuranceFee != null ||
  config.circuitBreakerEnabled != null ||
  config.cbDeviationBpsTiers != null ||
  config.cbTierDurationsSeconds != null ||
  config.cbEscalationWindowMult != null ||
  config.cbEmaAlphaBps != null ||
  config.cbWindowSeconds != null ||
  config.cbWindowMaxUpBps != null ||
  config.cbWindowMaxDownBps != null;

const hasGovBankConfig = (config: BankConfigOptRaw) =>
  config.assetWeightInit != null ||
  config.assetWeightMaint != null ||
  config.liabilityWeightInit != null ||
  config.liabilityWeightMaint != null ||
  isOperational(config.operationalState) ||
  config.riskTier != null ||
  config.assetTag != null ||
  config.oracleMaxConfidence != null ||
  config.oracleMaxAge != null ||
  config.tokenlessRepaymentsAllowed != null;

const fastBankConfig = (config: BankConfigOptRaw) => ({
  depositLimit: config.depositLimit,
  borrowLimit: config.borrowLimit,
  operationalState: isOperational(config.operationalState)
    ? null
    : config.operationalState,
  interestRateConfig: config.interestRateConfig,
  totalAssetValueInitLimit: config.totalAssetValueInitLimit,
  permissionlessBadDebtSettlement: config.permissionlessBadDebtSettlement,
  freezeSettings: config.freezeSettings,
  liquidationLiquidatorFee: config.liquidationLiquidatorFee,
  liquidationInsuranceFee: config.liquidationInsuranceFee,
  circuitBreakerEnabled: config.circuitBreakerEnabled,
  cbDeviationBpsTiers: config.cbDeviationBpsTiers,
  cbTierDurationsSeconds: config.cbTierDurationsSeconds,
  cbEscalationWindowMult: config.cbEscalationWindowMult,
  cbEmaAlphaBps: config.cbEmaAlphaBps,
  cbWindowSeconds: config.cbWindowSeconds,
  cbWindowMaxUpBps: config.cbWindowMaxUpBps,
  cbWindowMaxDownBps: config.cbWindowMaxDownBps,
});

const govBankConfig = (config: BankConfigOptRaw) => ({
  assetWeightInit: config.assetWeightInit,
  assetWeightMaint: config.assetWeightMaint,
  liabilityWeightInit: config.liabilityWeightInit,
  liabilityWeightMaint: config.liabilityWeightMaint,
  operationalState: isOperational(config.operationalState)
    ? config.operationalState
    : null,
  riskTier: config.riskTier,
  assetTag: config.assetTag,
  oracleMaxConfidence: config.oracleMaxConfidence,
  oracleMaxAge: config.oracleMaxAge,
  tokenlessRepaymentsAllowed: config.tokenlessRepaymentsAllowed,
});

const configureFastBank = (
  program: Program<Marginfi>,
  args: ConfigureBankArgs,
): Promise<TransactionInstruction> => {
  const admin =
    args.signer || (program.provider as AnchorProvider).wallet.publicKey;
  const accounts: Record<string, PublicKey> = { bank: args.bank, admin };
  if (args.group) accounts.group = args.group;
  return program.methods
    .lendingPoolConfigureBank(fastBankConfig(args.bankConfigOpt))
    .accounts(accounts)
    .instruction();
};

const configureGovBank = (
  program: Program<Marginfi>,
  args: ConfigureBankArgs,
): Promise<TransactionInstruction> => {
  const governanceAdmin =
    args.signer || (program.provider as AnchorProvider).wallet.publicKey;
  const accounts: Record<string, PublicKey> = {
    bank: args.bank,
    governanceAdmin,
  };
  if (args.group) accounts.group = args.group;
  return program.methods
    .lendingPoolConfigureBankGov(govBankConfig(args.bankConfigOpt))
    .accounts(accounts)
    .instruction();
};

/** Build one explicitly authorized bank-configuration instruction. */
export const configureBank = (
  program: Program<Marginfi>,
  args: ConfigureBankArgs,
): Promise<TransactionInstruction> => {
  const fast = hasFastBankConfig(args.bankConfigOpt);
  const gov = hasGovBankConfig(args.bankConfigOpt);
  if (fast && gov) {
    throw new Error(
      "configureBank received fast and governance fields; use configureBankIxs",
    );
  }
  return gov
    ? configureGovBank(program, args)
    : configureFastBank(program, args);
};

/** Build explicit fast and governance instructions for one atomic test transaction. */
export const configureBankIxs = async (
  program: Program<Marginfi>,
  args: ConfigureBankArgs,
): Promise<TransactionInstruction[]> => {
  const ixs: TransactionInstruction[] = [];
  if (hasFastBankConfig(args.bankConfigOpt))
    ixs.push(await configureFastBank(program, args));
  if (hasGovBankConfig(args.bankConfigOpt))
    ixs.push(await configureGovBank(program, args));
  return ixs;
};

export type ConfigureBankRateLimitsArgs = {
  group: PublicKey;
  bank: PublicKey;
  hourlyMaxOutflow?: BN | null;
  dailyMaxOutflow?: BN | null;
};

export const configureBankRateLimits = (
  program: Program<Marginfi>,
  args: ConfigureBankRateLimitsArgs,
) => {
  const ix = program.methods
    .configureBankRateLimits(
      args.hourlyMaxOutflow ?? null,
      args.dailyMaxOutflow ?? null,
    )
    .accounts({
      bank: args.bank,
    })
    .instruction();
  return ix;
};

export type ConfigureGroupRateLimitsArgs = {
  marginfiGroup: PublicKey;
  hourlyMaxOutflowUsd?: BN | null;
  dailyMaxOutflowUsd?: BN | null;
};

export const configureGroupRateLimits = (
  program: Program<Marginfi>,
  args: ConfigureGroupRateLimitsArgs,
) => {
  const ix = program.methods
    .configureGroupRateLimits(
      args.hourlyMaxOutflowUsd ?? null,
      args.dailyMaxOutflowUsd ?? null,
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();
  return ix;
};

export type ConfigureBankOracleArgs = {
  bank: PublicKey;
  type: number;
  oracle: PublicKey;
  // Extra oracle accounts appended after the primary feed, e.g. the Marinade State / SPL StakePool
  // for the mSOL/LST setups. Omit for single-oracle setups.
  remaining?: PublicKey[];
  group?: PublicKey;
  governanceAdmin?: PublicKey;
};

export const configureBankOracle = (
  program: Program<Marginfi>,
  args: ConfigureBankOracleArgs,
) => {
  const metas: AccountMeta[] = [args.oracle, ...(args.remaining ?? [])].map(
    (pubkey) => ({ pubkey, isSigner: false, isWritable: false }),
  );

  const governanceAdmin =
    args.governanceAdmin ||
    (program.provider as AnchorProvider).wallet.publicKey;
  const accounts: Record<string, PublicKey> = {
    bank: args.bank,
    governanceAdmin,
  };

  if (args.group) {
    accounts.group = args.group;
  }

  const ix = program.methods
    .lendingPoolConfigureBankOracle(args.type, args.oracle)
    .accounts(accounts)
    .remainingAccounts(metas)
    .instruction();
  return ix;
};

export type ConfigureBankOracleScopeArgs = {
  bank: PublicKey;
  group?: PublicKey;
  governanceAdmin?: PublicKey;
  /** The scope feed's OraclePrices account */
  oracle: PublicKey;
  /** Which of the 512 entries in that account prices this bank */
  entryIndex: number;
  /**
   * Venue account after the feed: the Kamino reserve for a Kamino bank, the JupLend `Lending`
   * state for a JupLend bank. Omit for a regular bank.
   */
  remaining?: PublicKey[];
};

export const configureBankOracleScope = (
  program: Program<Marginfi>,
  args: ConfigureBankOracleScopeArgs,
) => {
  const metas: AccountMeta[] = [args.oracle, ...(args.remaining ?? [])].map(
    (pubkey) => ({ pubkey, isSigner: false, isWritable: false }),
  );

  const governanceAdmin =
    args.governanceAdmin ||
    (program.provider as AnchorProvider).wallet.publicKey;
  const accounts: Record<string, PublicKey> = {
    bank: args.bank,
    governanceAdmin,
  };

  if (args.group) {
    accounts.group = args.group;
  }

  const ix = program.methods
    .lendingPoolConfigureBankOracleScope(args.oracle, args.entryIndex)
    .accounts(accounts)
    .remainingAccounts(metas)
    .instruction();

  return ix;
};

export type EmissionsDepositArgs = {
  bank: PublicKey;
  mint: PublicKey;
  fundingAccount: PublicKey;
  depositor: PublicKey;
  liquidityVault: PublicKey;
  amount: BN;
};

export const lendingPoolEmissionsDeposit = (
  program: Program<Marginfi>,
  args: EmissionsDepositArgs,
) => {
  const ix = program.methods
    .lendingPoolEmissionsDeposit(args.amount)
    .accounts({
      bank: args.bank,
      depositor: args.depositor,
      // mint: args.mint,
      emissionsFundingAccount: args.fundingAccount,
      // liquidityVault: args.liquidityVault,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

// ************* Below this line, not yet included in package ****************

export type InitGlobalFeeStateArgs = {
  payer: PublicKey;
  admin: PublicKey;
  wallet: PublicKey;
  bankInitFlatSolFee: number;
  liquidationFlatSolFee: number;
  orderInitFlatFeeDefault: number;
  programFeeFixed: WrappedI80F48;
  programFeeRate: WrappedI80F48;
  liquidationMaxFee: WrappedI80F48;
  orderExecutionMaxFee: WrappedI80F48;
};

export const initGlobalFeeState = (
  program: Program<Marginfi>,
  args: InitGlobalFeeStateArgs,
) => {
  const ix = program.methods
    .initGlobalFeeState(
      args.admin,
      args.wallet,
      args.bankInitFlatSolFee,
      args.liquidationFlatSolFee,
      args.orderInitFlatFeeDefault,
      args.programFeeFixed,
      args.programFeeRate,
      args.liquidationMaxFee,
      args.orderExecutionMaxFee,
    )
    .accounts({
      payer: args.payer,
      // feeState = deriveGlobalFeeState(id),
      // rent = SYSVAR_RENT_PUBKEY,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

export type EditGlobalFeeStateArgs = {
  admin: PublicKey; // signer (current global fee admin)
  newAdmin?: PublicKey | null;
  wallet?: PublicKey | null;
  bankInitFlatSolFee?: number | null;
  liquidationFlatSolFee?: number | null;
  orderInitFlatFeeDefault?: number | null;
  programFeeFixed?: WrappedI80F48 | null;
  programFeeRate?: WrappedI80F48 | null;
  liquidationMaxFee?: WrappedI80F48 | null;
  orderExecutionMaxFee?: WrappedI80F48 | null;
  pauseDelegateAdmin?: PublicKey | null; // undefined = no-op, null = clear
  accountTransferFee?: number | null; // u32, in lamports; 0 => use default
};

// Covered by e05_panicMode "(fee admin) edits all global fee fields and restores them".
export const editGlobalFeeState = (
  program: Program<Marginfi>,
  args: EditGlobalFeeStateArgs,
) => {
  const pauseDelegateAdminArg =
    args.pauseDelegateAdmin === undefined
      ? null
      : args.pauseDelegateAdmin ?? PublicKey.default;

  const ix = program.methods
    .editGlobalFeeState(
      args.newAdmin ?? null,
      args.wallet ?? null,
      args.bankInitFlatSolFee ?? null,
      args.liquidationFlatSolFee ?? null,
      args.orderInitFlatFeeDefault ?? null,
      args.programFeeFixed ?? null,
      args.programFeeRate ?? null,
      args.liquidationMaxFee ?? null,
      args.orderExecutionMaxFee ?? null,
      pauseDelegateAdminArg,
      args.accountTransferFee ?? null,
    )
    .accounts({
      globalFeeAdmin: args.admin,
      // feeState = deriveGlobalFeeState(id),
    })
    .instruction();

  return ix;
};

export type PropogateFeeStateArgs = {
  group: PublicKey;
};

export const propagateFeeState = (
  program: Program<Marginfi>,
  args: PropogateFeeStateArgs,
) => {
  const ix = program.methods
    .propagateFeeState()
    .accounts({
      marginfiGroup: args.group,
      // feeState = deriveGlobalFeeState(id),
    })
    .instruction();

  return ix;
};

export type InitStakedSettingsArgs = {
  group: PublicKey;
  feePayer: PublicKey;
  settings: StakedSettingsConfig;
};

export const initStakedSettings = (
  program: Program<Marginfi>,
  args: InitStakedSettingsArgs,
) => {
  const ix = program.methods
    .initStakedSettings(args.settings)
    .accounts({
      marginfiGroup: args.group,
      // admin: args.admin, // implied from group
      feePayer: args.feePayer,
      // staked_settings: deriveStakedSettings()
      // rent = SYSVAR_RENT_PUBKEY,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

export type EditStakedSettingsArgs = {
  settingsKey: PublicKey;
  settings: StakedSettingsEdit;
};

export const editStakedSettings = (
  program: Program<Marginfi>,
  args: EditStakedSettingsArgs,
) => {
  const ix = program.methods
    .editStakedSettings(args.settings)
    .accounts({
      // marginfiGroup: args.group, // implied from stakedSettings
      // admin: args.admin, // implied from group
      stakedSettings: args.settingsKey,
      // rent = SYSVAR_RENT_PUBKEY,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();

  return ix;
};

/**
 * oracle - required only if settings updates the oracle key
 */
export type PropagateStakedSettingsArgs = {
  settings: PublicKey;
  bank: PublicKey;
  oracle?: PublicKey;
};

export const propagateStakedSettings = (
  program: Program<Marginfi>,
  args: PropagateStakedSettingsArgs,
) => {
  const remainingAccounts = args.oracle
    ? [
        {
          pubkey: args.oracle,
          isSigner: false,
          isWritable: false,
        } as AccountMeta,
      ]
    : [];

  const ix = program.methods
    .propagateStakedSettings()
    .accounts({
      // marginfiGroup: args.group, // implied from stakedSettings
      stakedSettings: args.settings,
      bank: args.bank,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();

  return ix;
};

export type AddBankPermissionlessArgs = {
  marginfiGroup: PublicKey;
  feePayer: PublicKey;
  pythOracle: PublicKey;
  stakePool: PublicKey;
  validatorVoteAccount: PublicKey;
  seed: BN;
};

export const addBankPermissionless = (
  program: Program<Marginfi>,
  args: AddBankPermissionlessArgs,
) => {
  const [settingsKey] = deriveStakedSettings(
    program.programId,
    args.marginfiGroup,
  );
  const [lstMint] = PublicKey.findProgramAddressSync(
    [Buffer.from("mint"), args.stakePool.toBuffer()],
    SINGLE_POOL_PROGRAM_ID,
  );
  const [solPool] = PublicKey.findProgramAddressSync(
    [Buffer.from("stake"), args.stakePool.toBuffer()],
    SINGLE_POOL_PROGRAM_ID,
  );
  const [poolOnramp] = deriveOnRampPool(args.stakePool);
  // Note: oracle, lst mint, pool stake, and on-ramp are also passed in meta for validation.
  const oracleMeta: AccountMeta = {
    pubkey: args.pythOracle,
    isSigner: false,
    isWritable: false,
  };
  const lstMeta: AccountMeta = {
    pubkey: lstMint,
    isSigner: false,
    isWritable: false,
  };
  const solPoolMeta: AccountMeta = {
    pubkey: solPool,
    isSigner: false,
    isWritable: false,
  };
  const onrampMeta: AccountMeta = {
    pubkey: poolOnramp,
    isSigner: false,
    isWritable: false,
  };
  const [bank] = deriveBankWithSeed(
    program.programId,
    args.marginfiGroup,
    lstMint,
    args.seed,
  );
  const ix = program.methods
    .lendingPoolAddBankPermissionless(args.seed)
    .accounts({
      feePayer: args.feePayer,
      bankMint: lstMint,
      solPool: solPool,
      poolOnramp,
      stakePool: args.stakePool,
      validatorVoteAccount: args.validatorVoteAccount,
      // globalFeeState: deriveGlobalFeeState(id),
      // globalFeeWallet: // implied from globalFeeState,
      // liquidityVaultAuthority = deriveLiquidityVaultAuthority(id, bank);
      // liquidityVault = deriveLiquidityVault(id, bank);
      // insuranceVaultAuthority = deriveInsuranceVaultAuthority(id, bank);
      // insuranceVault = deriveInsuranceVault(id, bank);
      // feeVaultAuthority = deriveFeeVaultAuthority(id, bank);
      // feeVault = deriveFeeVault(id, bank);
      // rent = SYSVAR_RENT_PUBKEY
      tokenProgram: TOKEN_PROGRAM_ID,
      // systemProgram: SystemProgram.programId,
    })
    .accountsPartial({
      marginfiGroup: args.marginfiGroup,
      stakedSettings: settingsKey,
      bank,
    })
    .remainingAccounts([oracleMeta, lstMeta, solPoolMeta, onrampMeta])
    .instruction();

  return ix;
};

export const disableStakedOracles = (
  program: Program<Marginfi>,
  group: PublicKey,
  governanceAdmin?: PublicKey,
) => {
  const [stakedSettingsKey] = deriveStakedSettings(program.programId, group);
  const ix = program.methods
    .disableStakedOracles()
    .accounts({
      group,
    })
    .accountsPartial({ governanceAdmin, stakedSettings: stakedSettingsKey })
    .instruction();

  return ix;
};

export const enableStakedOracleOnramp = (
  program: Program<Marginfi>,
  group: PublicKey,
  governanceAdmin?: PublicKey,
) => {
  const [stakedSettingsKey] = deriveStakedSettings(program.programId, group);
  const ix = program.methods
    .enableStakedOracleOnramp()
    .accounts({
      group,
    })
    .accountsPartial({ governanceAdmin, stakedSettings: stakedSettingsKey })
    .instruction();

  return ix;
};

export type ConfigureBankEmodeArgs = {
  bank: PublicKey;
  tag: number;
  /** Must be `MAX_EMODE_ENTRIES` or fewer, see `newEmodeEntry` */
  entries: EmodeEntry[];
};

export const configBankEmode = (
  program: Program<Marginfi>,
  args: ConfigureBankEmodeArgs,
) => {
  const paddedEntries = padEmodeEntries(args.entries);

  const ix = program.methods
    .lendingPoolConfigureBankEmode(args.tag, paddedEntries)
    .accounts({
      // group: // implied from bank
      // governanceAdmin: signer, implied from group
      bank: args.bank,
    })
    .instruction();

  return ix;
};

const padEmodeEntries = (entries: EmodeEntry[]): EmodeEntry[] => {
  if (entries.length > MAX_EMODE_ENTRIES) {
    throw new Error(
      `Too many entries provided. Maximum allowed is ${MAX_EMODE_ENTRIES}`,
    );
  }
  const padded = [...entries];
  while (padded.length < MAX_EMODE_ENTRIES) {
    padded.push({
      collateralBankEmodeTag: 0,
      flags: 0,
      pad0: [0, 0, 0, 0, 0],
      assetWeightInit: I80F48_ZERO,
      assetWeightMaint: I80F48_ZERO,
    });
  }
  return padded;
};

export type UpdateBankFeesDestinationAccountArgs = {
  bank: PublicKey;
  /** An ATA of the bank's mint. Otherwise, admin's choice! */
  destination: PublicKey;
};

/**
 * Set a destination for fees. Once set, anyone can sweep fees to this account in a permissionless
 * way buy calling `withdrawFeesPermissionless`. Remember to run `collectBankFees` first.
 * @param program
 * @param args
 * @returns
 */
export const updateBankFeesDestinationAccount = (
  program: Program<Marginfi>,
  args: UpdateBankFeesDestinationAccountArgs,
) => {
  const ix = program.methods
    .lendingPoolUpdateFeesDestinationAccount()
    .accounts({
      // group: // implied from bank
      bank: args.bank,
      // admin: // implied from bank
      destinationAccount: args.destination,
    })
    .instruction();

  return ix;
};

export type WithdrawFeesPermissionlessArgs = {
  bank: PublicKey;
  amount: BN;
};

/**
 * Permissionless, move funds from the fee vault to the account the admin specified as the
 * destination for fees.
 */
export const withdrawFeesPermissionless = (
  program: Program<Marginfi>,
  args: WithdrawFeesPermissionlessArgs,
) => {
  const ix = program.methods
    .lendingPoolWithdrawFeesPermissionless(args.amount)
    .accounts({
      // group: // implied from bank
      bank: args.bank,
      // fee_vault: // implied from bank
      // fee_vault_authority: // implied from bank
      // fee_destination_account: // implied from bank
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export type CollectBankFeesArgs = {
  bank: PublicKey;
  feeAta: PublicKey;
};

/**
 * Permissionless, collect bank fees into their respective vaults.
 * @param program
 * @param args
 * @returns
 */
export const collectBankFees = (
  program: Program<Marginfi>,
  args: CollectBankFeesArgs,
) => {
  const ix = program.methods
    .lendingPoolCollectBankFees()
    .accounts({
      // group: // implied from bank
      bank: args.bank,
      // liquidity_vault: // implied from bank
      // liquidity_vault_authority: // implied from bank
      // fee_vault: // implied from bank
      // fee_vault_authority: // implied from bank
      // insurance_vault: // implied from bank
      // insurance_vault_authority: // implied from bank
      // fee_state: // derived from constant seed
      feeAta: args.feeAta,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export type AccrueInterestArgs = {
  bank: PublicKey;
};

export const accrueInterest = (
  program: Program<Marginfi>,
  args: AccrueInterestArgs,
) => {
  const ix = program.methods
    .lendingPoolAccrueBankInterest()
    .accounts({
      // group: // implied from bank
      bank: args.bank,
    })
    .instruction();
  return ix;
};

export type BackfillBankIsT22FlagArgs = {
  bank: PublicKey;
  bankSeed?: BN | null;
};

export const backfillBankIsT22Flag = (
  program: Program<Marginfi>,
  args: BackfillBankIsT22FlagArgs,
) => {
  const ix = program.methods
    .lendingPoolBackfillBankIsT22Flag(args.bankSeed ?? null)
    .accounts({
      bank: args.bank,
      // group: // implied via has_one on bank
      // mint: // implied via has_one on bank
    })
    .instruction();
  return ix;
};

export type BackfillStakedBankValidatorVoteAccountArgs = {
  bank: PublicKey;
  validatorVoteAccount: PublicKey;
};

export const backfillStakedBankValidatorVoteAccount = (
  program: Program<Marginfi>,
  args: BackfillStakedBankValidatorVoteAccountArgs,
) => {
  const ix = program.methods
    .lendingPoolBackfillStakedBankValidatorVoteAccount()
    .accounts({
      bank: args.bank,
      validatorVoteAccount: args.validatorVoteAccount,
    })
    .instruction();

  return ix;
};

export type HandleBankruptcyArgs = {
  signer: PublicKey;
  bank: PublicKey;
  marginfiAccount: PublicKey;
  remaining: PublicKey[];
};

/**
 * Permissionless, handle bank bankruptcy and settle bad debt using insurance vault. Signer must be
 * group admin unless the `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` is set on the bank.
 * @param program
 * @param args
 * @returns
 */
export const handleBankruptcy = (
  program: Program<Marginfi>,
  args: HandleBankruptcyArgs,
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => {
    return { pubkey, isSigner: false, isWritable: false };
  });

  const ix = program.methods
    .lendingPoolHandleBankruptcy()
    .accounts({
      // group: // implied from bank
      signer: args.signer,
      bank: args.bank,
      marginfiAccount: args.marginfiAccount,
      // liquidityVault: // implied from seed
      // insuranceVault: // implied from seed
      // insuranceVaultAuthority: // implied from seed
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};

export type CloseBankArgs = {
  marginfiGroup: PublicKey;
  bank: PublicKey;
  /** Admin escape hatch: skip the CLOSE_ENABLED_FLAG + open-position checks. */
  forceClose?: boolean;
  admin: PublicKey;
};

export const closeBank = (program: Program<Marginfi>, args: CloseBankArgs) => {
  const ix = program.methods
    .lendingPoolCloseBank(args.forceClose ?? null)
    .accounts({
      // group: args.marginfiGroup,
      bank: args.bank,
      // admin: args.admin,
    })
    .instruction();
  return ix;
};

export type ClearCircuitBreakerArgs = {
  bank: PublicKey;
  /** If true, also zero the EMA reference so the next pulse reseeds from live oracle data. */
  reseedReference?: boolean;
};

export const clearCircuitBreaker = async (
  program: Program<Marginfi>,
  args: ClearCircuitBreakerArgs,
) => {
  return program.methods
    .lendingPoolClearCircuitBreaker(args.reseedReference ?? false)
    .accounts({
      bank: args.bank,
      // group + riskAdmin: inferred from has_one + signer
    })
    .instruction();
};

export type PanicPauseArgs = {
  /** Note: when omitted, Anchor uses the provider, which works in this test suite. */
  pauseAuthority?: PublicKey;
};

export const panicPause = async (
  program: Program<Marginfi>,
  args: PanicPauseArgs,
) => {
  const ix = await program.methods
    .panicPause()
    .accounts({
      pauseAuthority: args.pauseAuthority,
      // feeState: args.feeState,
    })
    .instruction();

  return ix;
};

export type PanicUnpauseArgs = {
  // No args (global fee admin and fee state are inferred)...
};

export const panicUnpause = async (
  program: Program<Marginfi>,
  _args: PanicUnpauseArgs,
) => {
  const ix = await program.methods
    .panicUnpause()
    .accounts({
      // globalFeeAdmin: args.admin,
      // feeState: args.feeState,
    })
    .instruction();

  return ix;
};

export type PanicUnpausePermissionlessArgs = {
  // No args (everything is inferred)...
};

export const panicUnpausePermissionless = async (
  program: Program<Marginfi>,
  _args: PanicUnpausePermissionlessArgs,
) => {
  const ix = await program.methods
    .panicUnpausePermissionless()
    .accounts({
      // feeState: args.feeState,
    })
    .instruction();

  return ix;
};

type InitBankMetadataArgs = {
  bank: PublicKey;
};

export const initBankMetadata = (
  program: Program<Marginfi>,
  args: InitBankMetadataArgs,
) => {
  const ix = program.methods
    .initBankMetadata()
    .accounts({
      // metadata derived from bank
    })
    .accountsPartial({ bank: args.bank })
    .instruction();

  return ix;
};

export type InitSameAssetEmodeRegistryArgs = {
  group: PublicKey;
  governanceAdmin: PublicKey;
};

export const initSameAssetEmodeRegistry = (
  program: Program<Marginfi>,
  args: InitSameAssetEmodeRegistryArgs,
) => {
  const ix = program.methods
    .lendingPoolInitSameAssetEmodeRegistry()
    .accounts({
      group: args.group,
      // governanceAdmin: args.governanceAdmin,
      // sameAssetEmodeRegistry,
    })
    .instruction();

  return ix;
};

export type SetFixedPriceArgs = {
  bank: PublicKey;
  price: number;
  setup?: number;
  group?: PublicKey;
  governanceAdmin?: PublicKey;
  remaining?: PublicKey[];
};

export const setFixedPrice = (
  program: Program<Marginfi>,
  args: SetFixedPriceArgs,
) => {
  const oracleMeta: AccountMeta[] = (args.remaining ?? []).map((pubkey) => {
    return { pubkey, isSigner: false, isWritable: false };
  });

  const governanceAdmin =
    args.governanceAdmin ||
    (program.provider as AnchorProvider).wallet.publicKey;
  const accounts: Record<string, PublicKey> = {
    bank: args.bank,
    governanceAdmin,
  };

  if (args.group) {
    accounts.group = args.group;
  }

  const ix = program.methods
    .lendingPoolSetOraclePrice(
      bigNumberToWrappedI80F48(args.price),
      args.setup ?? ORACLE_SETUP_FIXED,
    )
    .accounts(accounts)
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};

export type SetBankSameAssetEmodeEligibilityArgs = {
  // group: PublicKey;
  governanceAdmin: PublicKey;
  bank: PublicKey;
  enabled: boolean;
};

export const setBankSameAssetEmodeEligibility = (
  program: Program<Marginfi>,
  args: SetBankSameAssetEmodeEligibilityArgs,
) => {
  const ix = program.methods
    .lendingPoolSetBankSameAssetEmodeEligibility(args.enabled)
    .accounts({
      // group: args.group,
      // governanceAdmin: args.governanceAdmin,
      bank: args.bank,
      // sameAssetEmodeRegistry,
    })
    .instruction();

  return ix;
};

type WriteBankMetadataArgs = {
  metadata: PublicKey;
  /// Pass undefined to skip. Limit 64 bytes
  ticker?: string;
  /// Pass undefined to skip. Limit 128 bytes
  description?: string;
};

/**
 * Write bank metadata (ticker / description).
 * - Validates UTF-8 byte length (<=64 / <=128).
 * - Passes Buffer (Some) or null (None) to the program.
 */
export const writeBankMetadata = (
  program: Program<Marginfi>,
  args: WriteBankMetadataArgs,
) => {
  const TICKER_CAP = 64;
  const DESC_CAP = 128;

  const tickerBuf =
    args.ticker !== undefined ? Buffer.from(args.ticker, "utf8") : null;
  if (tickerBuf && tickerBuf.length > TICKER_CAP) {
    throw new Error(
      `Ticker is ${tickerBuf.length} bytes, exceeds ${TICKER_CAP} byte cap`,
    );
  }

  const descBuf =
    args.description !== undefined
      ? Buffer.from(args.description, "utf8")
      : null;
  if (descBuf && descBuf.length > DESC_CAP) {
    throw new Error(
      `Description is ${descBuf.length} bytes, exceeds ${DESC_CAP} byte cap`,
    );
  }

  const ix = program.methods
    .writeBankMetadata(
      tickerBuf, // Option<Vec<u8>> -> Some(Buffer) | None(null)
      descBuf, // Option<Vec<u8>> -> Some(Buffer) | None(null)
    )
    .accounts({
      // group: implied
      // bank: implied from metadata
      // metadataAdmin: args.metadataAdmin, // implied from metadata
      metadata: args.metadata,
    })
    .instruction();

  return ix;
};

type WriteBankMetadataPreInitArgs = {
  group: PublicKey;
  bankMint: PublicKey;
  bankSeed: BN;
  metadata: PublicKey;
  /// Pass undefined to skip. Limit 64 bytes
  ticker?: string;
  /// Pass undefined to skip. Limit 128 bytes
  description?: string;
};

export const writeBankMetadataPreInit = (
  program: Program<Marginfi>,
  args: WriteBankMetadataPreInitArgs,
) => {
  const TICKER_CAP = 64;
  const DESC_CAP = 128;

  const tickerBuf =
    args.ticker !== undefined ? Buffer.from(args.ticker, "utf8") : null;
  if (tickerBuf && tickerBuf.length > TICKER_CAP) {
    throw new Error(
      `Ticker is ${tickerBuf.length} bytes, exceeds ${TICKER_CAP} byte cap`,
    );
  }

  const descBuf =
    args.description !== undefined
      ? Buffer.from(args.description, "utf8")
      : null;
  if (descBuf && descBuf.length > DESC_CAP) {
    throw new Error(
      `Description is ${descBuf.length} bytes, exceeds ${DESC_CAP} byte cap`,
    );
  }

  const ix = program.methods
    .writeBankMetadataPreInit(
      args.bankSeed,
      tickerBuf, // Option<Vec<u8>> -> Some(Buffer) | None(null)
      descBuf, // Option<Vec<u8>> -> Some(Buffer) | None(null)
    )
    .accounts({
      group: args.group,
      bankMint: args.bankMint,
      // bank: derived from seeds
      metadata: args.metadata,
    })
    .instruction();

  return ix;
};

export type UpdateGroupRateLimiterArgs = {
  marginfiGroup: PublicKey;
  outflowUsd?: BN | null;
  inflowUsd?: BN | null;
  updateSeq: BN;
  eventStartSlot: BN;
  eventEndSlot: BN;
};

export const updateGroupRateLimiter = (
  program: Program<Marginfi>,
  args: UpdateGroupRateLimiterArgs,
) => {
  const ix = program.methods
    .updateGroupRateLimiter(
      args.outflowUsd ?? null,
      args.inflowUsd ?? null,
      args.updateSeq,
      args.eventStartSlot,
      args.eventEndSlot,
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();
  return ix;
};

export type UpdateDeleverageWithdrawalsArgs = {
  marginfiGroup: PublicKey;
  outflowUsd: number;
  updateSeq: BN;
  eventStartSlot: BN;
  eventEndSlot: BN;
};

export const updateDeleverageWithdrawals = (
  program: Program<Marginfi>,
  args: UpdateDeleverageWithdrawalsArgs,
) => {
  const ix = program.methods
    .updateDeleverageWithdrawals(
      args.outflowUsd,
      args.updateSeq,
      args.eventStartSlot,
      args.eventEndSlot,
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();
  return ix;
};

export type ConfigureDeleverageWithdrawalLimitArgs = {
  marginfiGroup: PublicKey;
  limit: number;
};

export const configureDeleverageWithdrawalLimit = async (
  program: Program<Marginfi>,
  args: ConfigureDeleverageWithdrawalLimitArgs,
) => {
  const ix = await program.methods
    .configureDeleverageWithdrawalLimit(args.limit)
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();

  return ix;
};
