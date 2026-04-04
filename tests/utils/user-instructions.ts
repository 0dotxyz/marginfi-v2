import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
} from "@solana/web3.js";
import { Marginfi } from "../../target/types/marginfi";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  deriveExecuteOrderPda,
  deriveGlobalFeeState,
  deriveLiquidationRecord,
  deriveOrderPda,
} from "./pdas";
import { WrappedI80F48 } from "@mrgnlabs/mrgn-common";

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
      // group: args.group,
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
  remaining?: PublicKey[];
  repayAll?: boolean;
};

/**
 * Repay debt to a bank
 * * `authority` - MarginfiAccount's authority must sign and own the `tokenAccount`
 * * `remaining` - pass bank/oracles for any balances that need risk/oracle context in this
 *   instruction. For Token22 assets, pass the mint first.
 * @param program
 * @param args
 * @returns
 */
export const repayIx = (program: Program<Marginfi>, args: RepayIxArgs) => {
  const oracleMeta: AccountMeta[] = (args.remaining || []).map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));
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
    .remainingAccounts(oracleMeta)
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

/**
 * Converts an array that can be either PublicKey[] or AccountMeta[] into AccountMeta[].
 * If the input is already AccountMeta[], it preserves the existing isWritable flags.
 * If the input is PublicKey[], it converts using the provided default isWritable flag.
 */
const toAccountMetas = (
  remaining: Array<PublicKey | AccountMeta>,
  defaultWritable: boolean = false
): AccountMeta[] => {
  if (remaining.length === 0) {
    return [];
  }
  const first = remaining[0] as AccountMeta;
  if (first.pubkey !== undefined) {
    return remaining as AccountMeta[];
  }
  // Convert PublicKey[] to AccountMeta[]
  return (remaining as PublicKey[]).map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: defaultWritable,
  }));
};

export type StartLiquidationArgs = {
  marginfiAccount: PublicKey;
  liquidationReceiver: PublicKey;
  remaining: PublicKey[] | AccountMeta[];
};

export const startLiquidationIx = (
  program: Program<Marginfi>,
  args: StartLiquidationArgs
) => {
  const oracleMeta: AccountMeta[] = toAccountMetas(args.remaining, false);
  const [liquidationRecord] = deriveLiquidationRecord(
    program.programId,
    args.marginfiAccount,
  );
  return program.methods
    .startLiquidation()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      liquidationRecord,
      liquidationReceiver: args.liquidationReceiver,
      // instructionSysvar: // hard coded key
      // systemProgram: // hard coded key
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type EndLiquidationArgs = {
  marginfiAccount: PublicKey;
  remaining: PublicKey[] | AccountMeta[];
};

export const endLiquidationIx = (
  program: Program<Marginfi>,
  args: EndLiquidationArgs
) => {
  const oracleMeta: AccountMeta[] = toAccountMetas(args.remaining, false);
  const [liquidationRecord] = deriveLiquidationRecord(
    program.programId,
    args.marginfiAccount,
  );
  const liquidationReceiver = program.provider.publicKey;
  return program.methods
    .endLiquidation()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      liquidationRecord,
      liquidationReceiver,
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
  remaining: PublicKey[] | AccountMeta[];
};

export const startDeleverageIx = (
  program: Program<Marginfi>,
  args: StartDeleverageArgs
) => {
  const oracleMeta: AccountMeta[] = toAccountMetas(args.remaining, false);
  const [liquidationRecord] = deriveLiquidationRecord(
    program.programId,
    args.marginfiAccount,
  );
  return program.methods
    .startDeleverage()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      liquidationRecord,
      riskAdmin: args.riskAdmin,
    })
    .remainingAccounts(oracleMeta)
    .instruction();
};

export type EndDeleverageArgs = {
  marginfiAccount: PublicKey;
  remaining: PublicKey[] | AccountMeta[];
};

export const endDeleverageIx = (
  program: Program<Marginfi>,
  args: EndDeleverageArgs
) => {
  const oracleMeta: AccountMeta[] = toAccountMetas(args.remaining, false);
  const [liquidationRecord] = deriveLiquidationRecord(
    program.programId,
    args.marginfiAccount,
  );
  const riskAdmin = program.provider.publicKey;
  return program.methods
    .endDeleverage()
    .accounts({
      marginfiAccount: args.marginfiAccount,
      liquidationRecord,
      riskAdmin,
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

/**
 * Use in place of `composeRemainingAccounts` when building Meta for Start Liquidate (marks banks as
 * mutable, which is required)
 * @param banksAndOracles 
 * @returns 
 */
export const composeRemainingAccountsWriteableMeta = (
  banksAndOracles: PublicKey[][]
): AccountMeta[] => {
  banksAndOracles.sort((a, b) => {
    const A = a[0].toBytes();
    const B = b[0].toBytes();
    for (let i = 0; i < 32; i++) {
      if (A[i] !== B[i]) {
        return B[i] - A[i];
      }
    }
    return 0;
  });

  return banksAndOracles.flatMap((accs) =>
    accs.map((pubkey, idx) => ({
      pubkey,
      isSigner: false,
      isWritable: idx === 0,
    }))
  );
};

/**
 * Use in place of `composeRemainingAccounts` when building Meta for End Liquidate (marks banks as
 * mutable and ignores/excludes all other accounts)
 * @param banksAndOracles 
 * @returns 
 */
export const composeRemainingAccountsMetaBanksOnly = (
  banksAndOracles: PublicKey[][]
): AccountMeta[] => {
  banksAndOracles.sort((a, b) => {
    const A = a[0].toBytes();
    const B = b[0].toBytes();
    for (let i = 0; i < 32; i++) {
      if (A[i] !== B[i]) {
        return B[i] - A[i];
      }
    }
    return 0;
  });

  return banksAndOracles.map((accs) => ({
    pubkey: accs[0],
    isSigner: false,
    isWritable: true,
  }));
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

// ---------------------------------------------------------------------------
// Orders
// ---------------------------------------------------------------------------

export type OrderTriggerArgs =
  | { stopLoss: { threshold: WrappedI80F48; maxSlippage: number } }
  | { takeProfit: { threshold: WrappedI80F48; maxSlippage: number } }
  | { both: { stopLoss: WrappedI80F48; takeProfit: WrappedI80F48; maxSlippage: number } };

export type PlaceOrderArgs = {
  marginfiAccount: PublicKey;
  authority: PublicKey;
  feePayer: PublicKey;
  bankKeys: PublicKey[];
  trigger: OrderTriggerArgs;
  feeState?: PublicKey;
  globalFeeWallet?: PublicKey;
};

export const placeOrderIx = async (
  program: Program<Marginfi>,
  args: PlaceOrderArgs
) => {
  const [orderPda] = deriveOrderPda(
    program.programId,
    args.marginfiAccount,
    args.bankKeys
  );

  const feeState = args.feeState ?? deriveGlobalFeeState(program.programId)[0];
  const globalFeeWallet = args.globalFeeWallet
    ?? (await program.account.feeState.fetch(feeState)).globalFeeWallet;

  const accounts = {
    authority: args.authority,
    marginfiAccount: args.marginfiAccount,
    feePayer: args.feePayer,
    order: orderPda,
    feeState,
    globalFeeWallet,
  };

  return program.methods
    .marginfiAccountPlaceOrder(args.bankKeys, args.trigger)
    .accounts(accounts)
    .instruction();
};

export type CloseOrderArgs = {
  marginfiAccount: PublicKey;
  authority: PublicKey;
  order: PublicKey;
  feeRecipient: PublicKey;
};

export const closeOrderIx = (
  program: Program<Marginfi>,
  args: CloseOrderArgs
) => {
  const accounts = {
    marginfiAccount: args.marginfiAccount,
    authority: args.authority,
    order: args.order,
    feeRecipient: args.feeRecipient,
  };

  return program.methods
    .marginfiAccountCloseOrder()
    .accounts(accounts)
    .instruction();
};

export type KeeperCloseOrderArgs = {
  marginfiAccount: PublicKey;
  order: PublicKey;
  feeRecipient: PublicKey;
};

export const keeperCloseOrderIx = (
  program: Program<Marginfi>,
  args: KeeperCloseOrderArgs
) => {
  const accounts = {
    marginfiAccount: args.marginfiAccount,
    order: args.order,
    feeRecipient: args.feeRecipient,
  };

  return program.methods
    .marginfiAccountKeeperCloseOrder()
    .accounts(accounts)
    .instruction();
};

export type SetKeeperCloseFlagsArgs = {
  marginfiAccount: PublicKey;
  authority: PublicKey;
  bankKeysOpt?: PublicKey[] | null;
};

export const setKeeperCloseFlagsIx = (
  program: Program<Marginfi>,
  args: SetKeeperCloseFlagsArgs
) => {
  const accounts: any = {
    marginfiAccount: args.marginfiAccount,
    authority: args.authority,
  };

  return program.methods
    .marginfiAccountSetKeeperCloseFlags(args.bankKeysOpt ?? null)
    .accounts(accounts)
    .instruction();
};

export type StartExecuteOrderArgs = {
  group: PublicKey;
  marginfiAccount: PublicKey;
  feePayer: PublicKey;
  executor: PublicKey;
  order: PublicKey;
  remaining: PublicKey[];
};

export const startExecuteOrderIx = (
  program: Program<Marginfi>,
  args: StartExecuteOrderArgs
) => {
  const [executeRecord] = deriveExecuteOrderPda(program.programId, args.order);

  const rem: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));

  const accounts: any = {
    group: args.group,
    marginfiAccount: args.marginfiAccount,
    feePayer: args.feePayer,
    executor: args.executor,
    order: args.order,
    executeRecord,
    instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
  };

  return program.methods
    .marginfiAccountStartExecuteOrder()
    .accounts(accounts)
    .remainingAccounts(rem)
    .instruction();
};

export type EndExecuteOrderArgs = {
  group: PublicKey;
  marginfiAccount: PublicKey;
  executor: PublicKey;
  order: PublicKey;
  executeRecord: PublicKey;
  feeRecipient: PublicKey;
  remaining: PublicKey[];
  feeState?: PublicKey;
};

export const endExecuteOrderIx = (
  program: Program<Marginfi>,
  args: EndExecuteOrderArgs
) => {
  const feeState = args.feeState ?? deriveGlobalFeeState(program.programId)[0];
  const rem: AccountMeta[] = args.remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));

  const accounts: any = {
    group: args.group,
    marginfiAccount: args.marginfiAccount,
    executor: args.executor,
    feeRecipient: args.feeRecipient,
    order: args.order,
    executeRecord: args.executeRecord,
    feeState,
  };

  return program.methods
    .marginfiAccountEndExecuteOrder()
    .accountsStrict(accounts)
    .remainingAccounts(rem)
    .instruction();
};
