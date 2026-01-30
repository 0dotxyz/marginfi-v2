import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
} from "@solana/web3.js";
import { Marginfi } from "../../target/types/marginfi";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

export type AccountInitArgs = {
  marginfiGroup: PublicKey;
  marginfiAccount: PublicKey;
  authority: PublicKey;
  feePayer: PublicKey;
};

/**
 * Init a user account for some group.
 * * fee payer and authority must both sign.
 * * account must be a fresh keypair and must also sign
 * @param program
 * @param args
 * @returns
 */
export const accountInit = (
  program: Program<Marginfi>,
  args: AccountInitArgs
) => {
  const ix = program.methods
    .marginfiAccountInitialize()
    .accounts({
      marginfiGroup: args.marginfiGroup,
      marginfiAccount: args.marginfiAccount,
      authority: args.authority,
      feePayer: args.feePayer,
    })
    .instruction();

  return ix;
};

export type TransferAccountAuthorityArgs = {
  oldAccount: PublicKey;
  newAccount: PublicKey;
  newAuthority: PublicKey;
  globalFeeWallet: PublicKey;
  feePayer?: PublicKey;
  authority?: PublicKey;
};

export const transferAccountAuthorityIx = (
  program: Program<Marginfi>,
  args: TransferAccountAuthorityArgs
) => {
  const accounts: any = {
    oldMarginfiAccount: args.oldAccount,
    newMarginfiAccount: args.newAccount,
    // group: args.marginfiGroup,  // implied from oldMarginfiAccount
    newAuthority: args.newAuthority,
    globalFeeWallet: args.globalFeeWallet,
  };

  // Add authority if provided (otherwise implied from oldMarginfiAccount)
  if (args.authority) {
    accounts.authority = args.authority;
  }

  // Add fee payer if provided
  if (args.feePayer) {
    accounts.feePayer = args.feePayer;
  }

  const ix = program.methods
    .transferToNewAccount()
    .accounts(accounts)
    .instruction();

  return ix;
};

export type SetAccountFreezeArgs = {
  group: PublicKey;
  marginfiAccount: PublicKey;
  admin: PublicKey;
  frozen: boolean;
};

export const setAccountFreezeIx = (
  program: Program<Marginfi>,
  args: SetAccountFreezeArgs
) => {
  return program.methods
    .marginfiAccountSetFreeze(args.frozen)
    .accounts({
      group: args.group,
      marginfiAccount: args.marginfiAccount,
      admin: args.admin,
    })
    .instruction();
};

export type DepositArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  tokenAccount: PublicKey;
  amount: BN;
  depositUpToLimit?: boolean;
};

/**
 * Deposit to a bank
 * * `authority`- MarginfiAccount's authority must sign and own the `tokenAccount`
 * @param program
 * @param args
 * @returns
 */
export const depositIx = (program: Program<Marginfi>, args: DepositArgs) => {
  const ix = program.methods
    .lendingAccountDeposit(args.amount, args.depositUpToLimit ?? false)
    .accounts({
      // marginfiGroup: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from marginfiAccount
      bank: args.bank,
      signerTokenAccount: args.tokenAccount,
      // bankLiquidityVault:  deriveLiquidityVault(id, bank)
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export type SettleEmissionsArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
};

/**
 * (Permissionless) Settle emissions for a mrgnfi bank that is emitting some rewards. Generally runs
 * for all users before rates are updated, otherwise past emissions are retroactively credited at
 * the new rate as well. See `withdrawEmissionsIx` to actually claim the emissions to a wallet.
 * * `authority`- MarginfiAccount's authority must sign and own the `tokenAccount`
 * @param program
 * @param args
 * @returns
 */
export const settleEmissionsIx = (
  program: Program<Marginfi>,
  args: SettleEmissionsArgs
) => {
  const ix = program.methods
    .lendingAccountSettleEmissions()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      bank: args.bank,
    })
    .instruction();

  return ix;
};

export type WithdrawEmissionsArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  tokenAccount: PublicKey;
};

/**
 * Settles AND withdraws emissions to the user's given token account. Also see `settleEmissionsIx`, which settles but does not withdraw.
 * * `authority`- MarginfiAccount's authority must sign but does not have to own the `tokenAccount`
 * @param program
 * @param args
 * @returns
 */
export const withdrawEmissionsIx = (
  program: Program<Marginfi>,
  args: WithdrawEmissionsArgs
) => {
  const ix = program.methods
    .lendingAccountWithdrawEmissions()
    .accounts({
      // group: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from marginfiAccount
      bank: args.bank,
      // emissions_mint // implied from bank
      // emissions_auth // pda derived from bank
      // emissions_vault // pda derived from bank
      destinationAccount: args.tokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export type WithdrawEmissionsPermissionlessArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  /** Canonical ATA of `emissions_destination_account` registered on `marginfiAccount` */
  tokenAccount: PublicKey;
};

/**
 * (Permissionless) Settles AND withdraws emissions to the user's given token account. The user must
 * have opted in to this feature by designating a wallet to receive claims with
 * `marginfi_account_update_emissions_destination_account`
 * * `tokenAccount`- must be canonical ATA of `emissions_destination_account`
 * @param program
 * @param args
 * @returns
 */
export const withdrawEmissionsPermissionlessIx = (
  program: Program<Marginfi>,
  args: WithdrawEmissionsPermissionlessArgs
) => {
  const ix = program.methods
    .lendingAccountWithdrawEmissionsPermissionless()
    .accounts({
      // group: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from marginfiAccount
      bank: args.bank,
      // emissions_mint // implied from bank
      // emissions_auth // pda derived from bank
      // emissions_vault // pda derived from bank
      destinationAccount: args.tokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();

  return ix;
};

export type UpdateEmissionsDestinationArgs = {
  marginfiAccount: PublicKey;
  destinationAccount: PublicKey;
};

/**
 * (Permissionless) Opt in to claim permissionless emissions. The designated account/wallet will
 * receive all the funds. Emissions go to the canonical ATA of that account, and if the ATA doesn't
 * exist, they may still not get distributed. We (mrgn) might pay to open SOME atas, or we might
 * open some common ones when you opt in, or we might let the user pay and just let the tx fail it
 * it doesn't exist.
 * @param program
 * @param args
 * @returns
 */
export const updateEmissionsDestination = (
  program: Program<Marginfi>,
  args: UpdateEmissionsDestinationArgs
) => {
  const ix = program.methods
    .marginfiAccountUpdateEmissionsDestinationAccount()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      // authority: //implied from marginfiAccount
      destinationAccount: args.destinationAccount,
    })
    .instruction();

  return ix;
};

export type BorrowIxArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  tokenAccount: PublicKey;
  remaining: PublicKey[];
  amount: BN;
};

/**
 * Borrow from a bank
 * * `authority` - marginfiAccount's authority must sign, but does not have to own the `tokenAccount`
 * * `remaining` - pass bank/oracles for each bank the user is involved with, in the SAME ORDER they
 *   appear in userAcc.balances (e.g. `[bank0, oracle0, bank1, oracle1]`). For Token22 assets, pass
 *   the mint first, then the oracles/banks as described earlier.
 * @param program
 * @param args
 * @returns
 */
export const borrowIx = (program: Program<Marginfi>, args: BorrowIxArgs) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  const ix = program.methods
    .lendingAccountBorrow(args.amount)
    .accounts({
      // marginfiGroup: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from account
      bank: args.bank,
      destinationTokenAccount: args.tokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};

export type WithdrawIxArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  tokenAccount: PublicKey;
  remaining: PublicKey[];
  amount: BN;
  withdrawAll?: boolean;
};

/**
 * Withdraw from a bank
 * * `authority` - marginfiAccount's authority must sign, but does not have to own the `tokenAccount`
 * * `remaining` - pass bank/oracles for each bank the user is involved with, in the SAME ORDER they
 *   appear in userAcc.balances (e.g. `[bank0, oracle0, bank1, oracle1]`). For Token22 assets, pass
 *   the mint first, then the oracles/banks as described earlier.
 * @param program
 * @param args
 * @returns
 */
export const withdrawIx = (
  program: Program<Marginfi>,
  args: WithdrawIxArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  // False is the same as null, so if false we'll just pass null
  const all = args.withdrawAll === true ? true : null;
  const ix = program.methods
    .lendingAccountWithdraw(args.amount, all)
    .accounts({
      // marginfiGroup: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from account
      bank: args.bank,
      destinationTokenAccount: args.tokenAccount,
      // bankLiquidityVaultAuthority = deriveLiquidityVaultAuthority(id, bank);
      // bankLiquidityVault = deriveLiquidityVault(id, bank)
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};

export type RepayIxArgs = {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  tokenAccount: PublicKey;
  amount: BN;
  // TODO repay doesn't actually need these it doesn't check risk
  remaining?: PublicKey[];
  repayAll?: boolean;
};

/**
 * Repay debt to a bank
 * * `authority` - MarginfiAccount's authority must sign and own the `tokenAccount`
 * @param program
 * @param args
 * @returns
 */
export const repayIx = (program: Program<Marginfi>, args: RepayIxArgs) => {
  // False is the same as null, so if false we'll just pass null
  const all = args.repayAll === true ? true : null;
  const ix = program.methods
    .lendingAccountRepay(args.amount, all)
    .accounts({
      // marginfiGroup: args.marginfiGroup, // implied from bank
      marginfiAccount: args.marginfiAccount,
      // authority: args.authority, // implied from account
      bank: args.bank,
      signerTokenAccount: args.tokenAccount,
      // bankLiquidityVaultAuthority = deriveLiquidityVaultAuthority(id, bank);
      // bankLiquidityVault = deriveLiquidityVault(id, bank)
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .instruction();
  return ix;
};

export type InitLiquidationRecordArgs = {
  marginfiAccount: PublicKey;
  feePayer: PublicKey;
};

export const initLiquidationRecordIx = (
  program: Program<Marginfi>,
  args: InitLiquidationRecordArgs
) => {
  return program.methods
    .marginfiAccountInitLiqRecord()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      feePayer: args.feePayer,
      // liquidationRecord: // derived from seeds
      // systemProgram: // hard coded key
    })
    .instruction();
};

export type StartLiquidationArgs = {
  marginfiAccount: PublicKey;
  liquidationReceiver: PublicKey;
  remaining: PublicKey[];
};

export const startLiquidationIx = (
  program: Program<Marginfi>,
  args: StartLiquidationArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  return program.methods
    .startLiquidation()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      // liquidationRecord: // implied from account
      liquidationReceiver: args.liquidationReceiver,
      // instructionSysvar: // hard coded key
      // systemProgram: // hard coded key
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type EndLiquidationArgs = {
  marginfiAccount: PublicKey;
  remaining: PublicKey[];
};

export const endLiquidationIx = (
  program: Program<Marginfi>,
  args: EndLiquidationArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  return program.methods
    .endLiquidation()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      // liquidationRecord: // implied from account
      // liquidationRecord: // implied from record
      // feeState: // static pda
      // globalFeeWallet: // implied from feeState
      // systemProgram: // hard coded key
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type StartDeleverageArgs = {
  marginfiAccount: PublicKey;
  riskAdmin: PublicKey;
  remaining: PublicKey[];
};

export const startDeleverageIx = (
  program: Program<Marginfi>,
  args: StartDeleverageArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  return program.methods
    .startDeleverage()
    .accounts({
      marginfiAccount: args.marginfiAccount,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type EndDeleverageArgs = {
  marginfiAccount: PublicKey;
  remaining: PublicKey[];
};

export const endDeleverageIx = (
  program: Program<Marginfi>,
  args: EndDeleverageArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
  return program.methods
    .endDeleverage()
    .accounts({
      marginfiAccount: args.marginfiAccount,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type LiquidateIxArgs = {
  assetBankKey: PublicKey;
  liabilityBankKey: PublicKey;
  liquidatorMarginfiAccount: PublicKey;
  liquidateeMarginfiAccount: PublicKey;
  remaining: PublicKey[];
  amount: BN;
  liquidateeAccounts: number;
  liquidatorAccounts: number;
};

/**
 * Creates a Liquidate instruction.
 * * `remaining`:
 *     * liab_mint_ai (if token2022 mint),
 *     * asset_oracle_ai,
 *     * liab_oracle_ai,
 *     * liquidator_observation_ais...,
 *     * liquidatee_observation_ais...,
 *
 * @param program - The marginfi program instance.
 * @param args - The arguments required to create the instruction.
 * @returns The TransactionInstruction object.
 */
export const liquidateIx = (
  program: Program<Marginfi>,
  args: LiquidateIxArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => {
    return { pubkey, isSigner: false, isWritable: false };
  });

  return program.methods
    .lendingAccountLiquidate(
      args.amount,
      args.liquidateeAccounts,
      args.liquidatorAccounts
    )
    .accounts({
      assetBank: args.assetBankKey,
      liabBank: args.liabilityBankKey,
      liquidatorMarginfiAccount: args.liquidatorMarginfiAccount,
      liquidateeMarginfiAccount: args.liquidateeMarginfiAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type HealthPulseArgs = {
  marginfiAccount: PublicKey;
  remaining: PublicKey[];
};

export type PulseBankPriceArgs = {
  group: PublicKey;
  bank: PublicKey;
  remaining: PublicKey[];
};

/**
 * Creates a Health pulse instruction. This tx sets the user's risk engine health cache, a read-only
 * way to access the on-chain risk engine's current state, typically for debugging purposes.
 * * `remaining` - pass bank/oracles for each bank the user is involved with, in the SAME ORDER they
 *   appear in userAcc.balances (e.g. `[bank0, oracle0, bank1, oracle1]`). For staked collateral
 *   positions, pass the stake account and lst mint for the single pool as well: [bank0, oracle0,
 *   stakeAcc0, lstmint0]
 * @param program
 * @param args
 * @returns
 */
export const healthPulse = (
  program: Program<Marginfi>,
  args: HealthPulseArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => {
    return { pubkey, isSigner: false, isWritable: false };
  });

  return program.methods
    .lendingAccountPulseHealth()
    .accounts({
      marginfiAccount: args.marginfiAccount,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

/**
 * Creates a bank price pulse instruction. This ix refreshes the cached oracle price
 * for a given bank without modifying user positions.
 * * `remaining` - pass the oracle accounts required for this bank
 *   in the same order as used for other oracle reads for that bank.
 */
export const pulseBankPrice = (
  program: Program<Marginfi>,
  args: PulseBankPriceArgs
) => {
  const oracleMeta: AccountMeta[] = args.remaining.map((pubkey) => {
    return { pubkey, isSigner: false, isWritable: false };
  });

  return program.methods
    .lendingPoolPulseBankPriceCache()
    .accounts({
      bank: args.bank,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type BankAndOracles = PublicKey[]; // [bank, oracle, oracle_2...]

/**
 * Prepares transaction remaining accounts by processing bank-oracle groups:
 * 1. Sorts groups in descending order by bank public key (pushes inactive accounts to end)
 * 2. Flattens the structure into a single public key array
 *
 * Stable on most JS implementations (this shouldn't matter since we do not generally have duplicate
 * banks), in place, and uses the raw 32-byte value to sort in byte-wise lexicographical order (like
 * Rust's b.key.cmp(&a.key))
 *
 * @param banksAndOracles - Array where each element is a bank-oracle group: [bankPubkey,
 *                          oracle1Pubkey, oracle2Pubkey?, ...] Note: SystemProgram keys (111..111)
 *                          represent inactive accounts
 * @returns Flattened array of public keys with inactive accounts at the end, ready for transaction
 *          composition
 */
export const composeRemainingAccounts = (
  banksAndOracles: PublicKey[][]
): PublicKey[] => {
  banksAndOracles.sort((a, b) => {
    const A = a[0].toBytes();
    const B = b[0].toBytes();
    // find the first differing byte
    for (let i = 0; i < 32; i++) {
      if (A[i] !== B[i]) {
        // descending: bigger byte should come first
        return B[i] - A[i];
      }
    }
    return 0; // identical keys
  });

  // flatten out [bank, oracle…, oracle…] → [bank, oracle…, bank, oracle…, …]
  return banksAndOracles.flat();
};

export type AccountInitPdaArgs = {
  marginfiGroup: PublicKey;
  marginfiAccount: PublicKey;
  authority: PublicKey;
  feePayer: PublicKey;
  accountIndex: number;
  thirdPartyId?: number;
};

/**
 * Init a user account using PDA for some group.
 * * fee payer and authority must both sign.
 * * account address is derived as PDA, no keypair needed
 * @param program
 * @param args
 * @returns
 */
export const accountInitPda = (
  program: Program<Marginfi>,
  args: AccountInitPdaArgs
) => {
  const accounts: any = {
    marginfiGroup: args.marginfiGroup,
    marginfiAccount: args.marginfiAccount,
    authority: args.authority,
    feePayer: args.feePayer,
    instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
  };

  const ix = program.methods
    .marginfiAccountInitializePda(args.accountIndex, args.thirdPartyId || null)
    .accounts(accounts)
    .instruction();

  return ix;
};

export type TransferAccountAuthorityPdaArgs = {
  oldAccount: PublicKey;
  newAccount: PublicKey;
  newAuthority: PublicKey;
  globalFeeWallet: PublicKey;
  accountIndex: number;
  thirdPartyId?: number;
};

export const transferAccountAuthorityPdaIx = (
  program: Program<Marginfi>,
  args: TransferAccountAuthorityPdaArgs
) => {
  const accounts: any = {
    oldMarginfiAccount: args.oldAccount,
    newMarginfiAccount: args.newAccount,
    // group: args.marginfiGroup,  // implied from oldMarginfiAccount
    // authority: args.feePayer, // implied from oldMarginfiAccount
    newAuthority: args.newAuthority,
    globalFeeWallet: args.globalFeeWallet,
    instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
  };

  const ix = program.methods
    .transferToNewAccountPda(args.accountIndex, args.thirdPartyId || null)
    .accounts(accounts)
    .instruction();

  return ix;
};

export type PurgeDevelerageArgs = {
  account: PublicKey;
  bank: PublicKey;
};

export const purgeDeveleragedBalance = (
  program: Program<Marginfi>,
  args: PurgeDevelerageArgs
) => {
  const ix = program.methods
    .purgeDeleverageBalance()
    .accounts({
      marginfiAccount: args.account,
      bank: args.bank,
    })
    .instruction();

  return ix;
};
