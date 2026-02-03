import { BN, Program } from "@coral-xyz/anchor";
import { AccountMeta, PublicKey } from "@solana/web3.js";
import { Marginfi } from "../../target/types/marginfi";
import { deriveStakedSettings } from "./pdas";
import {
  BankConfig,
  BankConfigOptRaw,
  EmodeEntry,
  I80F48_ZERO,
  MAX_EMODE_ENTRIES,
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
      // admin: args.admin, // implied from group
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
  args: AddBankWithSeedArgs
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
      args.seed ?? new BN(0)
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
      // admin: args.admin, // implied from group
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
 * newAdmin - (Optional) pass null to keep current admin
 * newEModeAdmin - (Optional) pass null to keep current emode admin
 * newCurveAdmin - (Optional) pass null to keep current curve admin
 * newLimitAdmin - (Optional) pass null to keep current limit admin
 * newEmissionsAdmin - (Optional) pass null to keep current emissions admin
 * newMetadataAdmin - (Optional) pass null to keep current meta admin
 * newRiskAdmin - (Optional) pass null to keep current risk admin
 * marginfiGroup's admin - must sign
 */
export type GroupConfigureArgs = {
  newAdmin?: PublicKey | null; // optional; pass null or leave undefined to keep current admin
  newEmodeAdmin?: PublicKey | null;
  newCurveAdmin?: PublicKey | null;
  newLimitAdmin?: PublicKey | null;
  newEmissionsAdmin?: PublicKey | null;
  newMetadataAdmin?: PublicKey | null;
  newRiskAdmin?: PublicKey | null;
  marginfiGroup: PublicKey;
  emodeMaxInitLeverage?: WrappedI80F48 | null;
  emodeMaxMaintLeverage?: WrappedI80F48 | null;
};

export const groupConfigure = async (
  program: Program<Marginfi>,
  args: GroupConfigureArgs
) => {
  const group = await program.account.marginfiGroup.fetch(args.marginfiGroup);
  const newAdmin = args.newAdmin ?? group.admin;
  const newEmodeAdmin = args.newEmodeAdmin ?? group.emodeAdmin;
  const newCurveAdmin = args.newCurveAdmin ?? group.delegateCurveAdmin;
  const newLimitAdmin = args.newLimitAdmin ?? group.delegateLimitAdmin;
  const newEmissionsAdmin =
    args.newEmissionsAdmin ?? group.delegateEmissionsAdmin;
  const newMetadataAdmin = args.newMetadataAdmin ?? group.metadataAdmin;
  const newRiskAdmin = args.newRiskAdmin ?? group.riskAdmin;
  const emodeMaxInitLeverage = args.emodeMaxInitLeverage ?? null;
  const emodeMaxMaintLeverage = args.emodeMaxMaintLeverage ?? null;

  const ix = program.methods
    .marginfiGroupConfigure(
      newAdmin,
      newEmodeAdmin,
      newCurveAdmin,
      newLimitAdmin,
      newEmissionsAdmin,
      newMetadataAdmin,
      newRiskAdmin,
      emodeMaxInitLeverage,
      emodeMaxMaintLeverage
    )
    .accounts({
      marginfiGroup: args.marginfiGroup,
      // admin: // implied from group
    })
    .instruction();

  return ix;
};

export type GroupInitializeArgs = {
  marginfiGroup: PublicKey;
  admin: PublicKey;
};

export const groupInitialize = (
  program: Program<Marginfi>,
  args: GroupInitializeArgs
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

export type ConfigureBankArgs = {
  bank: PublicKey;
  bankConfigOpt: BankConfigOptRaw;
};

export const configureBank = (
  program: Program<Marginfi>,
  args: ConfigureBankArgs
) => {
  const ix = program.methods
    .lendingPoolConfigureBank(args.bankConfigOpt)
    .accounts({
      bank: args.bank,
    })
    .instruction();
  return ix;
};

export type ConfigureBankOracleArgs = {
  bank: PublicKey;
  type: number;
  oracle: PublicKey;
};

export const configureBankOracle = (
  program: Program<Marginfi>,
  args: ConfigureBankOracleArgs
) => {
  const oracleMeta: AccountMeta = {
    pubkey: args.oracle,
    isSigner: false,
    isWritable: false,
  };

  const ix = program.methods
    .lendingPoolConfigureBankOracle(args.type, args.oracle)
    .accounts({
      bank: args.bank,
    })
    .remainingAccounts([oracleMeta])
    .instruction();
  return ix;
};

export type SetupEmissionsArgs = {
  bank: PublicKey;
  emissionsMint: PublicKey;
  fundingAccount: PublicKey;
  emissionsFlags: BN;
  emissionsRate: BN;
  totalEmissions: BN;
};

export const setupEmissions = (
  program: Program<Marginfi>,
  args: SetupEmissionsArgs
) => {
  const ix = program.methods
    .lendingPoolSetupEmissions(
      args.emissionsFlags,
      args.emissionsRate,
      args.totalEmissions
    )
    .accounts({
      bank: args.bank,
      emissionsMint: args.emissionsMint,
      // emissionsAuth: deriveEmissionsAuth()
      // emissionsTokenAccount: deriveEmissionsTokenAccount()
      emissionsFundingAccount: args.fundingAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
      // systemProgram: SystemProgram.programId,
    })
    .instruction();
  return ix;
};

export type UpdateEmissionsArgs = {
  bank: PublicKey;
  emissionsMint: PublicKey;
  fundingAccount: PublicKey;
  emissionsFlags: BN | null;
  emissionsRate: BN | null;
  additionalEmissions: BN | null;
};

export const updateEmissions = (
  program: Program<Marginfi>,
  args: UpdateEmissionsArgs
) => {
  const ix = program.methods
    .lendingPoolUpdateEmissionsParameters(
      args.emissionsFlags,
      args.emissionsRate,
      args.additionalEmissions
    )
    .accounts({
      bank: args.bank,
      emissionsMint: args.emissionsMint,
      // emissionsAuth: deriveEmissionsAuth()
      // emissionsTokenAccount: deriveEmissionsTokenAccount()
      emissionsFundingAccount: args.fundingAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
      // systemProgram: SystemProgram.programId,
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
  args: InitGlobalFeeStateArgs
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
      args.orderExecutionMaxFee
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
  admin: PublicKey;
  wallet: PublicKey;
  bankInitFlatSolFee: number;
  liquidationFlatSolFee: number;
  orderInitFlatFeeDefault: number;
  programFeeFixed: WrappedI80F48;
  programFeeRate: WrappedI80F48;
  liquidationMaxFee: WrappedI80F48;
  orderExecutionMaxFee: WrappedI80F48;
  newAdmin?: PublicKey;
};

// TODO add test for this
export const editGlobalFeeState = (
  program: Program<Marginfi>,
  args: EditGlobalFeeStateArgs
) => {
  const ix = program.methods
    .editGlobalFeeState(
      args.newAdmin ? args.newAdmin : args.admin,
      args.wallet,
      args.bankInitFlatSolFee,
      args.liquidationFlatSolFee,
      args.orderInitFlatFeeDefault,
      args.programFeeFixed,
      args.programFeeRate,
      args.liquidationMaxFee,
      args.orderExecutionMaxFee
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
  args: PropogateFeeStateArgs
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
  args: InitStakedSettingsArgs
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
  args: EditStakedSettingsArgs
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
  args: PropagateStakedSettingsArgs
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
  seed: BN;
};

export const addBankPermissionless = (
  program: Program<Marginfi>,
  args: AddBankPermissionlessArgs
) => {
  const [settingsKey] = deriveStakedSettings(
    program.programId,
    args.marginfiGroup
  );
  const [lstMint] = PublicKey.findProgramAddressSync(
    [Buffer.from("mint"), args.stakePool.toBuffer()],
    SINGLE_POOL_PROGRAM_ID
  );
  const [solPool] = PublicKey.findProgramAddressSync(
    [Buffer.from("stake"), args.stakePool.toBuffer()],
    SINGLE_POOL_PROGRAM_ID
  );

  // Note: oracle and lst mint/pool are also passed in meta for validation
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

  const ix = program.methods
    .lendingPoolAddBankPermissionless(args.seed)
    .accounts({
      // marginfiGroup: args.marginfiGroup, // implied from stakedSettings
      stakedSettings: settingsKey,
      feePayer: args.feePayer,
      bankMint: lstMint,
      solPool: solPool,
      stakePool: args.stakePool,
      // bank: bankKey, // deriveBankWithSeed
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
    .remainingAccounts([oracleMeta, lstMeta, solPoolMeta])
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
  args: ConfigureBankEmodeArgs
) => {
  const paddedEntries = padEmodeEntries(args.entries);

  const ix = program.methods
    .lendingPoolConfigureBankEmode(args.tag, paddedEntries)
    .accounts({
      // group: // implied from bank
      // emode_admin: // implied from group
      bank: args.bank,
    })
    .instruction();

  return ix;
};

const padEmodeEntries = (entries: EmodeEntry[]): EmodeEntry[] => {
  if (entries.length > MAX_EMODE_ENTRIES) {
    throw new Error(
      `Too many entries provided. Maximum allowed is ${MAX_EMODE_ENTRIES}`
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
  args: UpdateBankFeesDestinationAccountArgs
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
  args: WithdrawFeesPermissionlessArgs
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
  args: CollectBankFeesArgs
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
  args: AccrueInterestArgs
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

export type MigrateCurveArgs = {
  bank: PublicKey;
};

export const migrateCurve = (
  program: Program<Marginfi>,
  args: MigrateCurveArgs
) => {
  const ix = program.methods
    .migrateCurve()
    .accounts({
      bank: args.bank,
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
  args: HandleBankruptcyArgs
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
  bank: PublicKey;
};

export const closeBank = (program: Program<Marginfi>, args: CloseBankArgs) => {
  const ix = program.methods
    .lendingPoolCloseBank()
    .accounts({
      // group: args.group, // implied from bank
      bank: args.bank,
      // admin: args.admin, // implied from group
    })
    .instruction();
  return ix;
};

export type PanicPauseArgs = {
  // No args (everything is inferred)...
};

export const panicPause = async (
  program: Program<Marginfi>,
  _args: PanicPauseArgs
) => {
  const ix = await program.methods
    .panicPause()
    .accounts({
      // globalFeeAdmin: args.admin,
      // feeState: args.feeState,
    })
    .instruction();

  return ix;
};

export type PanicUnpauseArgs = {
  // No args (everything is inferred)...
};

export const panicUnpause = async (
  program: Program<Marginfi>,
  _args: PanicUnpauseArgs
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
  _args: PanicUnpausePermissionlessArgs
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
  args: InitBankMetadataArgs
) => {
  const ix = program.methods
    .initBankMetadata()
    .accounts({
      // metadata: args.metadata, // derived from bank
      bank: args.bank,
    })
    .instruction();

  return ix;
};

export type SetFixedPriceArgs = {
  bank: PublicKey;
  price: number;
};

export const setFixedPrice = (
  program: Program<Marginfi>,
  args: SetFixedPriceArgs
) => {
  const ix = program.methods
    .lendingPoolSetFixedOraclePrice(bigNumberToWrappedI80F48(args.price))
    .accounts({
      // group: // implied from bank
      // admin: // implied from group
      bank: args.bank,
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
  args: WriteBankMetadataArgs
) => {
  const TICKER_CAP = 64;
  const DESC_CAP = 128;

  const tickerBuf =
    args.ticker !== undefined ? Buffer.from(args.ticker, "utf8") : null;
  if (tickerBuf && tickerBuf.length > TICKER_CAP) {
    throw new Error(
      `Ticker is ${tickerBuf.length} bytes, exceeds ${TICKER_CAP} byte cap`
    );
  }

  const descBuf =
    args.description !== undefined
      ? Buffer.from(args.description, "utf8")
      : null;
  if (descBuf && descBuf.length > DESC_CAP) {
    throw new Error(
      `Description is ${descBuf.length} bytes, exceeds ${DESC_CAP} byte cap`
    );
  }

  const ix = program.methods
    .writeBankMetadata(
      tickerBuf, // Option<Vec<u8>> -> Some(Buffer) | None(null)
      descBuf // Option<Vec<u8>> -> Some(Buffer) | None(null)
    )
    .accounts({
      // group: args.group, // implied from metadata
      // bank: args.bank, // implied from metadata
      // metadataAdmin: args.metadataAdmin, // implied from metadata
      metadata: args.metadata,
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
  args: ConfigureDeleverageWithdrawalLimitArgs
) => {
  const ix = await program.methods
    .configureDeleverageWithdrawalLimit(args.limit)
    .accounts({
      marginfiGroup: args.marginfiGroup,
    })
    .instruction();

  return ix;
};
