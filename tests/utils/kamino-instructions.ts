import {
  AccountMeta,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import BN from "bn.js";
import { Marginfi } from "../../target/types/marginfi";
import { Program } from "@coral-xyz/anchor";
import { KaminoConfigCompact, RESERVE_SIZE, toWeb3Ix } from "./kamino-utils";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { KLEND_PROGRAM_ID } from "./types";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveFeeReceiver,
  deriveLendingMarketAuthority,
  deriveLiquidityVaultAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
  deriveUserMetadata,
} from "./pdas";
import {
  bankrunContext,
  bankRunProvider,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
} from "../rootHooks";
import { createLookupTableForInstructions, getBankrunBlockhash, processBankrunTransaction, processBankrunV0Transaction } from "./tools";
import { AssetReserveConfig, BorrowRateCurve, BorrowRateCurveFields, CurvePoint, LendingMarket, MarketWithAddress, parseForChangesReserveConfigAndGetIxs, PriceFeed, Reserve } from "@kamino-finance/klend-sdk";
import { assert } from "chai";
import { address, createNoopSigner } from "@solana/kit";
import Decimal from "decimal.js";

const DEFAULT_KAMINO_DEPOSIT_OPTIONAL_ACCOUNTS = {
  obligationFarmUserState: null,
  reserveFarmState: null,
} as const;

export interface KaminoDepositAccounts {
  marginfiAccount: PublicKey;
  bank: PublicKey;
  signerTokenAccount: PublicKey;
  lendingMarket: PublicKey;
  reserve: PublicKey;

  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
}

export const makeKaminoDepositIx = async (
  program: Program<Marginfi>,
  accounts: KaminoDepositAccounts,
  amount: BN,
): Promise<TransactionInstruction> => {
  // Merge with defaults...
  const accs = {
    ...DEFAULT_KAMINO_DEPOSIT_OPTIONAL_ACCOUNTS,
    ...accounts,
  };

  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
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
  rewardIndex: BN,
): Promise<TransactionInstruction> => {
  return program.methods
    .kaminoHarvestReward(rewardIndex)
    .accounts({
      bank: accounts.bank,
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
      // feeState: accounts.feeState,
      destinationTokenAccount: accounts.destinationTokenAccount,
    })
    .instruction();
};

export interface AddKaminoBankAccounts {
  group: PublicKey;
  feePayer: PublicKey;
  bankMint: PublicKey;
  kaminoReserve: PublicKey;
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
  args: AddKaminoBankArgs,
): Promise<TransactionInstruction> => {
  const oracleMeta: AccountMeta = {
    pubkey: accounts.oracle,
    isSigner: false,
    isWritable: false,
  };
  const reserveMeta: AccountMeta = {
    pubkey: accounts.kaminoReserve,
    isSigner: false,
    isWritable: false,
  };

  const [bankKey] = deriveBankWithSeed(
    program.programId,
    accounts.group,
    accounts.bankMint,
    args.seed,
  );
  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    bankKey,
  );
  const [kaminoObligation] = deriveBaseObligation(
    liquidityVaultAuthority,
    accounts.kaminoMarket,
  );

  const ix = program.methods
    .lendingPoolAddBankKamino(args.config, args.seed)
    .accounts({
      integrationAcc1: accounts.kaminoReserve,
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
  reserve: PublicKey;

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
  amount?: BN,
): Promise<TransactionInstruction> => {
  // Merge with defaults...
  const accs = {
    ...DEFAULT_INIT_OBLIGATION_OPTIONAL_ACCOUNTS,
    ...accounts,
  };

  const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
    program.programId,
    accounts.bank,
  );
  const [userMetadata] = deriveUserMetadata(
    KLEND_PROGRAM_ID,
    liquidityVaultAuthority,
  );
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
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
  reserve: PublicKey;

  obligationFarmUserState?: PublicKey | null;
  reserveFarmState?: PublicKey | null;
}

export interface KaminoWithdrawArgs {
  amount: BN;
  isWithdrawAll: boolean;
  /** Oracle and other remaining accounts needed for health checks */
  remaining: PublicKey[];
}

export const makeKaminoWithdrawIx = async (
  program: Program<Marginfi>,
  accounts: KaminoWithdrawAccounts,
  args: KaminoWithdrawArgs,
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

  const [reserveSourceCollateral] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    accounts.reserve,
  );

  const ix = await program.methods
    .kaminoWithdraw(args.amount, args.isWithdrawAll)
    .accounts({
      lendingMarketAuthority, // derived
      reserveLiquiditySupply,
      reserveCollateralMint,
      reserveSourceCollateral,
      liquidityTokenProgram: TOKEN_PROGRAM_ID,
      ...accs,
    })
    .remainingAccounts(oracleMeta)
    .instruction();

  return ix;
};

export async function createReserve(
  reserve: Keypair,
  market: PublicKey,
  mint: PublicKey,
  reserveLabel: string,
  decimals: number,
  oracle: PublicKey,
  liquiditySource: PublicKey,
) {
  const [lendingMarketAuthority] = deriveLendingMarketAuthority(
    KLEND_PROGRAM_ID,
    market,
  );

  const [feeReceiver] = deriveFeeReceiver(KLEND_PROGRAM_ID, reserve.publicKey);

  const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
    KLEND_PROGRAM_ID,
    reserve.publicKey,
  );

  const [reserveCollateralMint] = deriveReserveCollateralMint(
    KLEND_PROGRAM_ID,
    reserve.publicKey,
  );

  const [reserveCollateralSupply] = deriveReserveCollateralSupply(
    KLEND_PROGRAM_ID,
    reserve.publicKey,
  );

  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: reserve.publicKey,
      space: RESERVE_SIZE + 8,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          RESERVE_SIZE + 8,
        ),
      programId: KLEND_PROGRAM_ID,
    }),
    await klendBankrunProgram.methods
      .initReserve()
      .accountsStrict({
        signer: groupAdmin.wallet.publicKey,
        lendingMarket: market,
        lendingMarketAuthority,
        reserve: reserve.publicKey,
        reserveLiquidityMint: mint,
        reserveLiquiditySupply,
        feeReceiver,
        reserveCollateralMint,
        reserveCollateralSupply,
        initialLiquiditySource: liquiditySource,
        rent: SYSVAR_RENT_PUBKEY,
        liquidityTokenProgram: TOKEN_PROGRAM_ID,
        collateralTokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction(),
  );

  await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet, reserve]);
  kaminoAccounts.set(reserveLabel, reserve.publicKey);

  console.log("Kamino reserve " + reserveLabel + " " + reserve.publicKey);

  const marketAcc: LendingMarket = LendingMarket.decode(
    (await bankRunProvider.connection.getAccountInfo(market)).data,
  );
  const reserveAcc: Reserve = Reserve.decode(
    (await bankRunProvider.connection.getAccountInfo(reserve.publicKey)).data,
  );

  assert.equal(reserveAcc.lendingMarket.toString(), market.toString());
  // Reserves start in an unconfigured "Hidden" state
  assert.equal(reserveAcc.config.status, 2);

  // Update the reserve to a sane operational state
  const marketWithAddress: MarketWithAddress = {
    address: address(market.toString()),
    state: marketAcc,
  };

  const borrowRateCurve = new BorrowRateCurve({
    points: [
      // At 0% utilization: 50% interest rate
      new CurvePoint({ utilizationRateBps: 0, borrowRateBps: 50000 }),
      // At 50% utilization: 100% interest rate
      new CurvePoint({ utilizationRateBps: 5000, borrowRateBps: 100000 }),
      // At 80% utilization: 500% interest rate
      new CurvePoint({ utilizationRateBps: 8000, borrowRateBps: 500000 }),
      // At 100% utilization: 1000% interest rate
      new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
      // Fill remaining points to complete the curve
      ...Array(7).fill(
        new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
      ),
    ],
  } as BorrowRateCurveFields);
  const assetReserveConfigParams = {
    loanToValuePct: 75, // 75%
    liquidationThresholdPct: 85, // 85%
    borrowRateCurve,
    depositLimit: new Decimal(1_000_000_000),
    borrowLimit: new Decimal(1_000_000_000),
  };

  const priceFeed: PriceFeed = {
    pythPrice: address(oracle.toString()),
    // switchboardPrice: NULL_PUBKEY,
    // switchboardTwapPrice: NULL_PUBKEY,
    // scopePriceConfigAddress: NULL_PUBKEY,
    // scopeChain: [0, 65535, 65535, 65535],
    // scopeTwapChain: [52, 65535, 65535, 65535],
  };

  const assetReserveConfig = new AssetReserveConfig({
    mint: address(mint.toString()),
    mintTokenProgram: address(TOKEN_PROGRAM_ID.toString()),
    tokenName: reserveLabel,
    mintDecimals: decimals,
    priceFeed,
    ...assetReserveConfigParams,
  }).getReserveConfig();

  const addr = address(groupAdmin.wallet.publicKey.toString());
  const signer = createNoopSigner(addr);

  const instructions: TransactionInstruction[] = [
    ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    }),
  ];

  const ixes = await parseForChangesReserveConfigAndGetIxs(
    marketWithAddress,
    reserveAcc,
    address(reserve.publicKey.toString()),
    assetReserveConfig,
    address(klendBankrunProgram.programId.toString()),
    signer,
  );

  for (const ix of ixes) {
    instructions.push(toWeb3Ix(ix.ix as any));
  }

  const lutAccount = await createLookupTableForInstructions(
    groupAdmin.wallet,
    instructions,
  );

  const messageV0 = new TransactionMessage({
    payerKey: groupAdmin.wallet.publicKey,
    recentBlockhash: await getBankrunBlockhash(bankrunContext),
    instructions,
  }).compileToV0Message([lutAccount]);

  const versionedTx = new VersionedTransaction(messageV0);
  await processBankrunV0Transaction(bankrunContext, versionedTx, [groupAdmin.wallet]);
}
