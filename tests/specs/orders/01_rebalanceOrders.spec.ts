import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
  MessageV0,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  bigNumberToWrappedI80F48,
  TOKEN_PROGRAM_ID,
} from "@mrgnlabs/mrgn-common";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMintToInstruction,
  createTransferInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  driftBankrunProgram,
  klendBankrunProgram,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
} from "../../utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
} from "../../utils/group-instructions";
import { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } from "../../utils/types";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
  deriveSpotMarketPDA,
} from "../../utils/pdas";
import { USER_ACCOUNT } from "../../utils/mocks";
import {
  expectFailedTxWithError,
  getTokenBalance,
} from "../../utils/genericTests";
import {
  bigIntToBnSafe,
  bnToBigIntSafe,
  I80F48_FRACTIONAL_BITS,
  nativeToI80Scaled,
  toI80Scaled,
} from "../../utils/bn-utils";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
  processBankrunV0Transaction,
  safeGetAccountInfo,
} from "../../utils/tools";

// Kamino
import { createKaminoMarket, createReserve } from "../../utils/kamino-reserve-setup";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "../../utils/kamino-instructions";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "../../utils/kamino-utils";

// Drift
import {
  makeAddDriftBankIx,
  makeDriftDepositIx,
  makeDriftWithdrawIx,
  makeInitDriftUserIx,
} from "../../utils/drift-instructions";
import {
  makeInitializeDriftIx,
  makeInitializeSpotMarketIx,
  makeInitializeUserIx,
  makeInitializeUserStatsIx,
  makeDepositIx as makeDriftNativeDepositIx,
  makeWithdrawIx as makeDriftNativeWithdrawIx,
  makeUpdateSpotMarketCumulativeInterestIx,
} from "../../utils/drift-sdk";
import {
  defaultDriftBankConfig,
  defaultSpotMarketConfig,
  quoteAssetSpotMarketConfig,
  DriftOracleSourceValues,
  getDriftStateAccount,
  tokenAmountToScaledBalance,
} from "../../utils/drift-utils";
import {
  createBankrunPythOracleAccount,
  refreshPullOraclesBankrun,
  setPythPullOraclePrice,
} from "../../utils/bankrun-oracles";
import { advanceOneHour } from "../../utils/bankrunConnection";
import {
  DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
  ORACLE_CONF_INTERVAL,
} from "../../utils/types";

// JupLend
import {
  configureJuplendProtocolPermissions,
  fetchJuplendPool,
  initJuplendGlobals,
  initJuplendPool,
} from "../../utils/juplend/jlr-pool-setup";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "../../utils/juplend/group-instructions";
import {
  makeJuplendDepositIx,
  makeJuplendNativeBorrowIx,
  makeJuplendNativeLendingDepositIx,
  makeJuplendNativePreOperateIx,
} from "../../utils/juplend/user-instructions";
import { refreshJupSimple } from "../../utils/juplend/shorthand-instructions";
import {
  deriveJuplendFTokenVault,
  deriveJuplendGlobalKeys,
  deriveJuplendLendingPdas,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "../../utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "../../utils/juplend/programs";
import {
  DEFAULT_BORROW_CONFIG,
  DEFAULT_BORROW_CONFIG_MIN,
  DEFAULT_RATE_CONFIG,
  defaultJuplendBankConfig,
  JuplendPoolKeys,
  percent,
} from "../../utils/juplend/types";
import {
  initJuplendProtocolPositionsIx,
  updateJuplendUserBorrowConfigIx,
  updateJuplendUserClassIx,
} from "../../utils/juplend/admin-instructions";

const REBALANCE_ORDER_SEED = "rebalance_order";
const REBALANCE_RECORD_SEED = "rebalance_record";

const deriveRebalanceOrder = (
  programId: PublicKey,
  marginfiAccount: PublicKey,
  mint: PublicKey,
) =>
  PublicKey.findProgramAddressSync(
    [
      Buffer.from(REBALANCE_ORDER_SEED),
      marginfiAccount.toBuffer(),
      mint.toBuffer(),
    ],
    programId,
  );

const deriveRebalanceRecord = (
  programId: PublicKey,
  marginfiAccount: PublicKey,
  seq: bigint,
) =>
  PublicKey.findProgramAddressSync(
    [
      Buffer.from(REBALANCE_RECORD_SEED),
      marginfiAccount.toBuffer(),
      bigIntToBnSafe(seq).toArrayLike(Buffer, "le", 8),
    ],
    programId,
  );

/** The account's next execution sequence, which seeds this execution's record. */
const nextSeqOf = async (
  program: Program<Marginfi>,
  marginfiAccount: PublicKey,
): Promise<bigint> =>
  bnToBigIntSafe(
    (await program.account.marginfiAccount.fetch(marginfiAccount))
      .rebalanceExecutionSeq,
  );

/** The sequence `start_rebalance` last consumed, i.e. the most recent execution's record. */
const lastSeqOf = async (
  program: Program<Marginfi>,
  marginfiAccount: PublicKey,
): Promise<bigint> => (await nextSeqOf(program, marginfiAccount)) - 1n;

const REBALANCE_FEE_POOL_SEED = "rebalance_fee_pool";
const deriveRebalanceFeePool = (
  programId: PublicKey,
  marginfiAccount: PublicKey,
) =>
  PublicKey.findProgramAddressSync(
    [Buffer.from(REBALANCE_FEE_POOL_SEED), marginfiAccount.toBuffer()],
    programId,
  );

/** A declared N->N move from referenced-bank `srcIndex` to `dstIndex`, of `value` USD (== UI USDC at
 * the $1 test oracle). */
const buildMove = (srcIndex: number, dstIndex: number, value: number) => ({
  srcIndex,
  dstIndex,
  pad0: Array(6).fill(0),
  amount: bigNumberToWrappedI80F48(value),
});

const toMeta = (keys: PublicKey[]): AccountMeta[] =>
  keys.map((pubkey) => ({ pubkey, isSigner: false, isWritable: false }));

/** A referenced bank block for remaining_accounts: the bank (writable, for native accrue) + its
 * oracle. */
const bankBlock = (bank: PublicKey, oracle: PublicKey): AccountMeta[] => [
  { pubkey: bank, isSigner: false, isWritable: true },
  { pubkey: oracle, isSigner: false, isWritable: false },
];

/** Solana's per-transaction account lock limit (`MAX_TX_ACCOUNT_LOCKS`). */
const TX_ACCOUNT_LOCK_LIMIT = 64;

/** Accounts a compiled v0 message locks: its static keys plus everything the lookup table resolves. */
const lockedAccounts = (message: MessageV0): number =>
  message.staticAccountKeys.length +
  message.addressTableLookups.reduce(
    (n, l) => n + l.writableIndexes.length + l.readonlyIndexes.length,
    0,
  );

/** A sandwich fits a transaction when it clears both the compute and the account lock budget. */
const assertFitsTransaction = (
  cu: number,
  accounts: number,
  cuCeiling: number,
) => {
  assert.isBelow(cu, cuCeiling, `sandwich burned ${cu} CU`);
  assert.isAtMost(
    accounts,
    TX_ACCOUNT_LOCK_LIMIT,
    `sandwich locked ${accounts} accounts`,
  );
};

describe("Auto-rebalance orders (native -> native)", () => {
  let program: Program<Marginfi>;


  const rebalanceGroup = Keypair.generate();
  const group = rebalanceGroup.publicKey;
  const usdcMint = ecosystem.usdcMint.publicKey;
  let usdcOracle: PublicKey;
  let wsolOracle: PublicKey;

  const SRC_SEED = new BN(8_801);
  const DST_SEED = new BN(8_802);
  const SOL_SEED = new BN(8_803);
  const SRC2_SEED = new BN(8_804);
  const DST2_SEED = new BN(8_805);
  let srcBank: PublicKey; // src USDC bank (util 0 -> rate 0)
  let solBank: PublicKey; // borrower collateral
  let dstBank: PublicKey; // dst USDC bank (util ~50% -> rate > 0)
  let src2Bank: PublicKey; // second src USDC bank (util 0 -> rate 0), for N->1 consolidation
  let dst2Bank: PublicKey; // second dst USDC bank (util > 0 -> rate > 0), for 1->N splitting

  // users[0] = rebalancing user, users[1] = keeper, users[2] = dst lender, users[3] = borrower
  type MockUser = (typeof users)[number];
  let owner: MockUser;
  let keeper: MockUser;
  let lender: MockUser;
  let borrower: MockUser;

  let ownerAcc: PublicKey;
  let lenderAcc: PublicKey;
  let borrowerAcc: PublicKey;
  let borrower2Acc: PublicKey; // separate account so dst2's borrow stays single-liability

  const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const sol = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);

  const REBALANCE_AMOUNT = usdc(1000);

  const sendOwner = (tx: Transaction) =>
    owner.mrgnProgram.provider.sendAndConfirm(tx);
  const sendKeeper = (tx: Transaction) =>
    keeper.mrgnProgram.provider.sendAndConfirm(tx);

  /** A bank's asset shares (0 if the owner holds no active balance there). */
  const sharesOf = (acc: any, bank: PublicKey): bigint => {
    const bal = acc.lendingAccount.balances.find(
      (b: any) => b.active && b.bankPk.equals(bank),
    );
    return bal ? toI80Scaled(bal.assetShares) : 0n;
  };
  const assetShares = async (bank: PublicKey): Promise<bigint> =>
    sharesOf(await program.account.marginfiAccount.fetch(ownerAcc), bank);

  const nextSeq = () => nextSeqOf(program, ownerAcc);
  const lastSeq = () => lastSeqOf(program, ownerAcc);

  /** The `start_rebalance` instruction for `moves` over `refBanks`, at the account's next sequence. */
  const buildStartIx = async (
    order: PublicKey,
    moves: ReturnType<typeof buildMove>[],
    refBanks: AccountMeta[],
  ) => {
    const seq = await nextSeq();
    const [record] = deriveRebalanceRecord(program.programId, ownerAcc, seq);
    return program.methods
      .marginfiAccountStartRebalance(moves, bigIntToBnSafe(seq))
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(refBanks)
      .instruction();
  };

  /** The `end_rebalance` instruction with `endRemaining`, for the account's current record. */
  const buildEndIx = async (order: PublicKey, endRemaining: AccountMeta[]) => {
    const [record] = deriveRebalanceRecord(
      program.programId,
      ownerAcc,
      await nextSeq(),
    );
    const [feePool] = deriveRebalanceFeePool(program.programId, ownerAcc);
    return program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        feePool,
      })
      .remainingAccounts(endRemaining)
      .instruction();
  };

  /** The `settle_rebalance_tip` instruction over `refBanks`, for the last execution's record. */
  const buildSettleIx = async (order: PublicKey, refBanks: AccountMeta[]) => {
    const [record] = deriveRebalanceRecord(
      program.programId,
      ownerAcc,
      await lastSeq(),
    );
    const [feePool] = deriveRebalanceFeePool(program.programId, ownerAcc);
    return program.methods
      .marginfiAccountSettleRebalanceTip()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        feePool,
        caller: keeper.wallet.publicKey,
      })
      .remainingAccounts(refBanks)
      .instruction();
  };

  /** The referenced-bank block for the standard [src, dst, dst2] set (writable banks + oracle). */
  const standardRefBanks = (src: PublicKey = srcBank): AccountMeta[] => [
    ...bankBlock(src, usdcOracle),
    ...bankBlock(dstBank, usdcOracle),
    ...bankBlock(dst2Bank, usdcOracle),
  ];

  /** Advance past the settle delay, refresh oracles, then settle the escrowed tip. */
  const settleStandard = async (order: PublicKey, src: PublicKey = srcBank) => {
    await advanceOneHour(banksClient, bankrunContext);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await sendKeeper(
      new Transaction().add(await buildSettleIx(order, standardRefBanks(src))),
    );
  };

  /**
   * Build the keeper-signed start -> withdraw -> deposit -> end sandwich. N->N by default: the whole
   * source position fans out across TWO destinations (a 1->2 split), so every test that runs the
   * sandwich exercises a multi-move execution rather than a trivial one-to-one move.
   */
  const buildSandwich = async (opts: {
    order: PublicKey;
    extraTrailingIx?: boolean;
    src?: PublicKey;
  }) => {
    const src = opts.src ?? srcBank;
    const half = REBALANCE_AMOUNT.div(new BN(2));
    // Referenced banks (indexed 0=src, 1=dst, 2=dst2): [bank, oracle] per bank.
    const refBanks: AccountMeta[] = [
      ...bankBlock(src, usdcOracle),
      ...bankBlock(dstBank, usdcOracle),
      ...bankBlock(dst2Bank, usdcOracle),
    ];
    // The full source position split across two destinations, half to each.
    const moves = [
      buildMove(0, 1, half.toNumber() / 1e6),
      buildMove(0, 2, half.toNumber() / 1e6),
    ];
    // After the split only the two destinations are active; the post-move health set lists the active
    // balances in their stored (descending pubkey) order.
    const [hi, lo] =
      dstBank.toBuffer().compare(dst2Bank.toBuffer()) > 0
        ? [dstBank, dst2Bank]
        : [dst2Bank, dstBank];
    const endRemaining: AccountMeta[] = [
      ...refBanks,
      ...bankBlock(hi, usdcOracle),
      ...bankBlock(lo, usdcOracle),
    ];

    const startIx = await buildStartIx(opts.order, moves, refBanks);

    const withdrawIx = await program.methods
      .lendingAccountWithdraw(new BN(0), true)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: src,
        destinationTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(toMeta(composeRemainingAccounts([[src, usdcOracle]])))
      .instruction();

    const depositIx1 = await program.methods
      .lendingAccountDeposit(half, false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: dstBank,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    const depositIx2 = await program.methods
      .lendingAccountDeposit(half, false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: dst2Bank,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    const endIx = await buildEndIx(opts.order, endRemaining);

    const tx = new Transaction()
      .add(startIx)
      .add(withdrawIx)
      .add(depositIx1)
      .add(depositIx2)
      .add(endIx);
    if (opts.extraTrailingIx) {
      // Token program is allowlisted, so this clears the program check but leaves end_rebalance no
      // longer last -> validate_ix_last fails. Keeper self-authorizes (no extra signer needed).
      tx.add(
        createTransferInstruction(
          keeper.usdcAccount,
          keeper.usdcAccount,
          keeper.wallet.publicKey,
          0,
        ),
      );
    }
    return tx;
  };

  const placeOrder = async (opts: {
    allowedBanks: PublicKey[];
    minImprovement: number;
    cooldownSeconds: number;
    amount?: BN;
    keeperTip?: BN;
  }) => {
    const [order] = deriveRebalanceOrder(program.programId, ownerAcc, usdcMint);
    const ix = await program.methods
      .marginfiAccountPlaceRebalanceOrder(
        opts.allowedBanks,
        bigNumberToWrappedI80F48(opts.minImprovement),
        new BN(opts.cooldownSeconds),
        opts.amount ?? null,
        opts.keeperTip ?? null,
      )
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        authority: owner.wallet.publicKey,
        mint: usdcMint,
        rebalanceOrder: order,
        feePayer: owner.wallet.publicKey,
      })
      .instruction();
    await sendOwner(new Transaction().add(ix));
    return order;
  };

  /** Fund the account's rebalance fee pool with `lamports` SOL, for keeper tips. */
  const topUpPool = async (lamports: number) => {
    const [feePool] = deriveRebalanceFeePool(program.programId, ownerAcc);
    const ix = await program.methods
      .marginfiAccountTopUpRebalanceFeePool(new BN(lamports))
      .accountsPartial({
        marginfiAccount: ownerAcc,
        feePool,
        payer: owner.wallet.publicKey,
      })
      .instruction();
    await sendOwner(new Transaction().add(ix));
    return feePool;
  };

  const closeOrder = async (order: PublicKey) => {
    const ix = await program.methods
      .marginfiAccountCloseRebalanceOrder()
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: owner.wallet.publicKey,
        feeRecipient: owner.wallet.publicKey,
        rebalanceOrder: order,
      })
      .instruction();
    await sendOwner(new Transaction().add(ix));
  };

  /** Partial update; pass `null` for fields to leave unchanged. */
  const updateOrder = async (
    order: PublicKey,
    opts: {
      allowedBanks?: PublicKey[] | null;
      minImprovement?: number | null;
      cooldownSeconds?: number | null;
      amount?: BN | null;
    },
  ) => {
    const ix = await program.methods
      .marginfiAccountUpdateRebalanceOrder(
        opts.allowedBanks ?? null,
        opts.minImprovement == null
          ? null
          : bigNumberToWrappedI80F48(opts.minImprovement),
        opts.cooldownSeconds == null ? null : new BN(opts.cooldownSeconds),
        opts.amount ?? null,
        null,
      )
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: owner.wallet.publicKey,
        rebalanceOrder: order,
      })
      .instruction();
    await sendOwner(new Transaction().add(ix));
  };

  /** Empty every USDC bank the owner may hold from a prior move, then restore exactly REBALANCE_AMOUNT
   * in src, so each test starts from a clean single-source position. */
  const resetOwnerToSrc = async () => {
    const drainBank = async (bank: PublicKey) => {
      const acc = await program.account.marginfiAccount.fetch(ownerAcc);
      const held = acc.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(bank),
      );
      if (!held) return;
      // Withdraw-all closes the bank; the post-withdraw health set is the still-active banks, ordered
      // descending by pubkey to match their stored balance slots.
      const others = acc.lendingAccount.balances
        .filter((b: any) => b.active && !b.bankPk.equals(bank))
        .map((b: any) => b.bankPk as PublicKey)
        .sort((a: PublicKey, b: PublicKey) => b.toBuffer().compare(a.toBuffer()));
      await sendOwner(
        new Transaction().add(
          await program.methods
            .lendingAccountWithdraw(new BN(0), true)
            .accountsPartial({
              marginfiAccount: ownerAcc,
              authority: owner.wallet.publicKey,
              bank,
              destinationTokenAccount: owner.usdcAccount,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .remainingAccounts(
              toMeta(
                composeRemainingAccounts(
                  others.map((b: PublicKey) => [b, usdcOracle]),
                ),
              ),
            )
            .instruction(),
        ),
      );
    };
    // Drain the destinations first, then any second source, then the primary source, so the final
    // withdraw leaves the account empty. Strict conservation later rejects a full-move sandwich whose
    // declared amount doesn't match a short source, so src is restored to exactly REBALANCE_AMOUNT.
    await drainBank(dstBank);
    await drainBank(dst2Bank);
    await drainBank(src2Bank);
    await drainBank(srcBank);
    await sendOwner(
      new Transaction().add(
        await depositIx(owner.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: srcBank,
          tokenAccount: owner.usdcAccount,
          amount: REBALANCE_AMOUNT,
        }),
      ),
    );
  };

  before(async () => {
    program = bankrunProgram;
    usdcOracle = oracles.usdcOracle.publicKey;
    wsolOracle = oracles.wsolOracle.publicKey;
    [owner, keeper, lender, borrower] = [users[0], users[1], users[2], users[3]];
    [srcBank] = deriveBankWithSeed(program.programId, group, usdcMint, SRC_SEED);
    [dstBank] = deriveBankWithSeed(program.programId, group, usdcMint, DST_SEED);
    [solBank] = deriveBankWithSeed(
      program.programId,
      group,
      ecosystem.wsolMint.publicKey,
      SOL_SEED,
    );
    [src2Bank] = deriveBankWithSeed(program.programId, group, usdcMint, SRC2_SEED);
    [dst2Bank] = deriveBankWithSeed(program.programId, group, usdcMint, DST2_SEED);

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupInitialize(program, {
          marginfiGroup: group,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [rebalanceGroup],
    );

    // src USDC bank (PDA via seed)
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: group,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: usdcMint,
          config: defaultBankConfig(),
          seed: SRC_SEED,
        }),
      ),
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: srcBank,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: usdcOracle,
        }),
      ),
    );

    // dst USDC bank (PDA via seed)
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: group,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: usdcMint,
          config: defaultBankConfig(),
          seed: DST_SEED,
        }),
      ),
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: dstBank,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: usdcOracle,
        }),
      ),
    );

    // SOL bank for borrower collateral (default config: usable as collateral)
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: group,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          config: defaultBankConfig(),
          seed: SOL_SEED,
        }),
      ),
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank: solBank,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: wsolOracle,
        }),
      ),
    );

    // Second src/dst USDC banks so the suite exercises N->N (many-source, many-destination) moves.
    const addUsdcBank = async (seed: BN, bank: PublicKey) => {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await addBankWithSeed(groupAdmin.mrgnProgram, {
            marginfiGroup: group,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: usdcMint,
            config: defaultBankConfig(),
            seed,
          }),
        ),
      );
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await configureBankOracle(groupAdmin.mrgnProgram, {
            bank,
            type: ORACLE_SETUP_PYTH_PUSH,
            oracle: usdcOracle,
          }),
        ),
      );
    };
    await addUsdcBank(SRC2_SEED, src2Bank);
    await addUsdcBank(DST2_SEED, dst2Bank);

    const mintAuth = bankrunContext.payer.publicKey;
    const fundTx = new Transaction();
    for (const u of [owner, keeper, lender, borrower]) {
      fundTx.add(
        createMintToInstruction(usdcMint, u.usdcAccount, mintAuth, 1e12),
      );
      fundTx.add(
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          u.wsolAccount,
          mintAuth,
          1e12,
        ),
      );
    }
    await bankRunProvider.sendAndConfirm(fundTx);

    const initAcc = async (u: typeof owner) => {
      const kp = Keypair.generate();
      u.accounts.set(USER_ACCOUNT, kp.publicKey);
      await u.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await accountInit(program, {
            marginfiGroup: group,
            marginfiAccount: kp.publicKey,
            authority: u.wallet.publicKey,
            feePayer: u.wallet.publicKey,
          }),
        ),
        [kp],
      );
      return kp.publicKey;
    };
    ownerAcc = await initAcc(owner);
    lenderAcc = await initAcc(lender);
    borrowerAcc = await initAcc(borrower);
    borrower2Acc = await initAcc(borrower);

    // Both dst banks carry ~50% utilization (rate > 0): a lender supplies each, and a distinct
    // SOL-collateralized account borrows from each. src banks stay at 0 utilization (rate 0).
    const driveDstUtilization = async (
      dst: PublicKey,
      borrowAcc: PublicKey,
    ) => {
      await lender.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await depositIx(lender.mrgnProgram, {
            marginfiAccount: lenderAcc,
            bank: dst,
            tokenAccount: lender.usdcAccount,
            amount: usdc(2000),
          }),
        ),
      );
      await borrower.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await depositIx(borrower.mrgnProgram, {
            marginfiAccount: borrowAcc,
            bank: solBank,
            tokenAccount: borrower.wsolAccount,
            amount: sol(25),
          }),
        ),
      );
      await borrower.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await borrowIx(borrower.mrgnProgram, {
            marginfiAccount: borrowAcc,
            bank: dst,
            tokenAccount: borrower.usdcAccount,
            amount: usdc(1000),
            remaining: composeRemainingAccounts([
              [dst, usdcOracle],
              [solBank, wsolOracle],
            ]),
          }),
        ),
      );
    };
    await driveDstUtilization(dstBank, borrowerAcc);
    await driveDstUtilization(dst2Bank, borrower2Acc);

    await sendOwner(
      new Transaction().add(
        await depositIx(owner.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: srcBank,
          tokenAccount: owner.usdcAccount,
          amount: REBALANCE_AMOUNT,
        }),
      ),
    );
  });

  it("splits the source across two destinations and keeps the order - happy path", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    const before = await program.account.marginfiAccount.fetch(ownerAcc);
    const oldSrc = sharesOf(before, srcBank);
    assert.equal(sharesOf(before, dstBank), 0n, "dst empty before the move");
    assert.equal(sharesOf(before, dst2Bank), 0n, "dst2 empty before the move");

    const tx = await buildSandwich({ order });
    await sendKeeper(tx);

    // The source fans out equally into both destinations; value conserved, source drained.
    const after = await program.account.marginfiAccount.fetch(ownerAcc);
    const half = oldSrc / 2n;
    assert.equal(sharesOf(after, srcBank), 0n, "src drained after the move");
    assert.equal(sharesOf(after, dstBank), half, "dst holds half");
    assert.equal(sharesOf(after, dst2Bank), half, "dst2 holds half");

    // Order persists; a tip-free execution escrows nothing, so its record closes at end.
    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc));
    const [record] = deriveRebalanceRecord(
      program.programId,
      ownerAcc,
      await lastSeq(),
    );
    assert.isNull(
      await safeGetAccountInfo(bankRunProvider.connection, record),
      "tip-free record closed at end",
    );

    await closeOrder(order);
  });

  it("conserves value exactly and pays the keeper a SOL tip", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const tip = new BN(200_000);
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      keeperTip: tip,
    });
    const feePool = await topUpPool(5_000_000);

    const oldBalance = await assetShares(srcBank);
    const poolBefore = await bankRunProvider.connection.getBalance(feePool);

    // Honest full move: the whole source fans out across both destinations (strict conservation). The
    // full tip is escrowed out of the pool at end, pending settlement.
    await sendKeeper(await buildSandwich({ order }));

    const moved = (await assetShares(dstBank)) + (await assetShares(dst2Bank));
    const escrowed =
      poolBefore - (await bankRunProvider.connection.getBalance(feePool));

    // Value conserved to the atomic unit across the whole destination set, no skim.
    assert.equal(
      moved,
      oldBalance,
      "the two destinations must sum to the original src position exactly",
    );
    // A full move escrows exactly the configured tip out of the SOL pool.
    assert.equal(escrowed, tip.toNumber(), "the full tip is escrowed at end");

    await closeOrder(order);
  });

  it("consolidates two sources into one destination (N->1)", async () => {
    await resetOwnerToSrc();
    // Fund a second source so the owner holds REBALANCE_AMOUNT in each of two banks.
    await sendOwner(
      new Transaction().add(
        await depositIx(owner.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: src2Bank,
          tokenAccount: owner.usdcAccount,
          amount: REBALANCE_AMOUNT,
        }),
      ),
    );
    const tip = new BN(200_000);
    const order = await placeOrder({
      allowedBanks: [srcBank, src2Bank, dstBank],
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      keeperTip: tip,
    });
    const feePool = await topUpPool(5_000_000);

    const oldSrc = await assetShares(srcBank);
    const oldSrc2 = await assetShares(src2Bank);
    const poolBefore = await bankRunProvider.connection.getBalance(feePool);

    // Referenced banks indexed 0=src, 1=src2, 2=dst: both sources move into the single destination.
    const refBanks: AccountMeta[] = [
      ...bankBlock(srcBank, usdcOracle),
      ...bankBlock(src2Bank, usdcOracle),
      ...bankBlock(dstBank, usdcOracle),
    ];
    const amt = REBALANCE_AMOUNT.toNumber() / 1e6;
    const moves = [buildMove(0, 2, amt), buildMove(1, 2, amt)];

    const startIx = await buildStartIx(order, moves, refBanks);
    const drainSrc = async (bank: PublicKey) =>
      program.methods
        .lendingAccountWithdraw(new BN(0), true)
        .accountsPartial({
          marginfiAccount: ownerAcc,
          authority: keeper.wallet.publicKey,
          bank,
          destinationTokenAccount: keeper.usdcAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .remainingAccounts(toMeta(composeRemainingAccounts([[bank, usdcOracle]])))
        .instruction();
    const withdraw1 = await drainSrc(srcBank);
    const withdraw2 = await drainSrc(src2Bank);
    const depositIxn = await program.methods
      .lendingAccountDeposit(REBALANCE_AMOUNT.mul(new BN(2)), false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: dstBank,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    const endIx = await buildEndIx(order, [
      ...refBanks,
      ...bankBlock(dstBank, usdcOracle),
    ]);

    await sendKeeper(
      new Transaction()
        .add(startIx)
        .add(withdraw1)
        .add(withdraw2)
        .add(depositIxn)
        .add(endIx),
    );

    // Both sources drained into the single destination; value conserved across the set, one tip escrowed.
    assert.equal(await assetShares(srcBank), 0n, "src drained");
    assert.equal(await assetShares(src2Bank), 0n, "src2 drained");
    assert.equal(
      await assetShares(dstBank),
      oldSrc + oldSrc2,
      "dst holds both sources' value exactly",
    );
    const escrowed =
      poolBefore - (await bankRunProvider.connection.getBalance(feePool));
    assert.equal(
      escrowed,
      tip.toNumber(),
      "one full tip escrowed for the whole consolidation",
    );

    await closeOrder(order);
  });

  it("moves only the ordered amount on a bounded order", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    // Order half the 1000 USDC position; the rest must stay in src.
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      amount: usdc(500),
    });

    const oldSrc = await assetShares(srcBank);
    // The whole allowlist is priced, so every allowed bank is referenced: 0=src, 1=dst, 2=dst2.
    const refBanks: AccountMeta[] = [
      ...bankBlock(srcBank, usdcOracle),
      ...bankBlock(dstBank, usdcOracle),
      ...bankBlock(dst2Bank, usdcOracle),
    ];

    const startIx = await buildStartIx(order, [buildMove(0, 1, 500)], refBanks);

    // Partial withdraw of exactly the ordered amount (not withdraw-all).
    const withdrawIx = await program.methods
      .lendingAccountWithdraw(usdc(500), false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: srcBank,
        destinationTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(
        toMeta(composeRemainingAccounts([[srcBank, usdcOracle]])),
      )
      .instruction();

    const depositIxn = await program.methods
      .lendingAccountDeposit(usdc(500), false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: dstBank,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    // A partial move leaves src active, so the post-move observation set spans BOTH banks. It follows
    // the referenced-bank blocks and is matched positionally against active balances in their stored
    // (descending pubkey) slot order, so list the two banks descending here.
    const [hi, lo] =
      srcBank.toBuffer().compare(dstBank.toBuffer()) > 0
        ? [srcBank, dstBank]
        : [dstBank, srcBank];
    const endIx = await buildEndIx(order, [
      ...refBanks,
      ...bankBlock(hi, usdcOracle),
      ...bankBlock(lo, usdcOracle),
    ]);

    await sendKeeper(
      new Transaction()
        .add(startIx)
        .add(withdrawIx)
        .add(depositIxn)
        .add(endIx),
    );

    const srcAfter = await assetShares(srcBank);
    const dstAfter = await assetShares(dstBank);
    // Share value is 1 in-tx, so shares == native; with no skim same-mint shares are conserved.
    assert.equal(
      dstAfter,
      nativeToI80Scaled(usdc(500)),
      "exactly the ordered amount moved to dst",
    );
    assert.equal(
      srcAfter + dstAfter,
      oldSrc,
      "no skim -> same-mint shares conserved",
    );

    await closeOrder(order);
  });

  it("updates an order's min improvement in place - RebalanceNotImproving", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    // Raise the bar to +100% APR in place; the next rebalance must now fail as not-improving.
    await updateOrder(order, { minImprovement: 1.0 });

    await expectFailedTxWithError(
      async () => {
        await sendKeeper(await buildSandwich({ order }));
      },
      "RebalanceNotImproving",
      6703,
    );

    await closeOrder(order);
  });

  it("rejects when dst is not improving enough - RebalanceNotImproving", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 1.0, // require +100% APR improvement
      cooldownSeconds: 0,
    });

    await expectFailedTxWithError(
      async () => {
        await sendKeeper(await buildSandwich({ order }));
      },
      "RebalanceNotImproving",
      6703,
    );

    await closeOrder(order);
  });

  it("enforces the per-order cooldown - RebalanceCooldown", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    // The tip-free record closes at end, so the 24h cooldown is all that rejects a second execution.
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 86_400,
    });

    // First execution succeeds and stamps last_exec_timestamp.
    await sendKeeper(await buildSandwich({ order }));

    // Move it back to src, then the cooldown (still hours away) rejects a second execution.
    await resetOwnerToSrc();
    await expectFailedTxWithError(
      async () => {
        await sendKeeper(await buildSandwich({ order }));
      },
      "RebalanceCooldown",
      6701,
    );

    await closeOrder(order);
  });

  it("settle_rebalance_tip pays the keeper when the move realized yield", async () => {
    await resetOwnerToSrc();
    const tip = new BN(200_000);
    const order = await placeOrder({
      allowedBanks: [srcBank, dstBank, dst2Bank],
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      keeperTip: tip,
    });
    const feePool = await topUpPool(5_000_000);

    const poolBefore = await bankRunProvider.connection.getBalance(feePool);
    await sendKeeper(await buildSandwich({ order }));
    // The escrow that left the pool at end equals the tip the record will settle (not paid yet).
    const poolAfterEnd = await bankRunProvider.connection.getBalance(feePool);
    const [record] = deriveRebalanceRecord(
      program.programId,
      ownerAcc,
      await lastSeq(),
    );
    const pendingTip = (
      await program.account.rebalanceRecord.fetch(record)
    ).pendingTip.toNumber();
    assert.equal(
      poolBefore - poolAfterEnd,
      pendingTip,
      "escrow out of the pool equals the record's pending tip",
    );

    // The idle source is out-yielded by the borrow-carrying destinations, so settlement pays the
    // keeper and does not refund the pool; the record is closed.
    await settleStandard(order);
    assert.equal(
      await bankRunProvider.connection.getBalance(feePool),
      poolAfterEnd,
      "realized settlement pays the keeper, leaving the pool untouched",
    );
    assert.isNull(
      await safeGetAccountInfo(bankRunProvider.connection, record),
      "record closed after settlement",
    );

    await closeOrder(order);
  });

  it("settle_rebalance_tip is rejected before the settle delay - RebalanceSettleTooEarly", async () => {
    await resetOwnerToSrc();
    const order = await placeOrder({
      allowedBanks: [srcBank, dstBank, dst2Bank],
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      keeperTip: new BN(200_000),
    });
    await topUpPool(5_000_000);
    await sendKeeper(await buildSandwich({ order }));

    // No clock advance: the settle delay has not elapsed.
    await expectFailedTxWithError(
      async () => {
        await sendKeeper(
          new Transaction().add(await buildSettleIx(order, standardRefBanks())),
        );
      },
      "RebalanceSettleTooEarly",
      6711,
    );

    // Settle properly to unblock, then clean up.
    await settleStandard(order);
    await closeOrder(order);
  });

  it("rejects a tampered instruction sandwich - end_rebalance must be last", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    await expectFailedTxWithError(
      async () => {
        await sendKeeper(
          await buildSandwich({ order, extraTrailingIx: true }),
        );
      },
      "EndNotLast",
      6088,
    );

    await closeOrder(order);
  });

  it("rejects banks outside the order allowlist - RebalanceBankNotAllowed", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank, dst2Bank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    // The on-chain allowlist is {srcBank, dstBank}; a src bank outside it is rejected.
    await expectFailedTxWithError(
      async () => {
        await sendKeeper(await buildSandwich({ order, src: solBank }));
      },
      "RebalanceBankNotAllowed",
      6707,
    );

    await closeOrder(order);
  });
});

/** A keeper-signed sandwich leg targeting one venue (the source it withdraws from or the dst it deposits into). */
type VenueLeg = {
  /** Crank instruction(s) that refresh the venue's rate-bearing account in the same slot. */
  cranks: TransactionInstruction[];
  /** The bank's rate/health tail: [priceOracle, venueAccount]. */
  tail: PublicKey[];
  /** JupLend TokenReserve for the start/end ix arg; null for Kamino/Drift. */
  tokenReserve: PublicKey | null;
  /** Venue accounts the supply rate needs to price reward emissions; empty when the venue has none. */
  rewards: PublicKey[];
};

describe("Auto-rebalance orders (venue -> venue)", () => {
  let program: Program<Marginfi>;
  const group = Keypair.generate();
  const groupPk = group.publicKey;
  const mint = ecosystem.usdcMint.publicKey;
  let usdcOracle: PublicKey;

  const KAMINO_SEED = new BN(7_701);
  const DRIFT_SEED = new BN(7_702);
  const JUPLEND_SEED = new BN(7_703);

  const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const wsol = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);
  const REBALANCE_AMOUNT = usdc(1_000);

  // users[0] = owner (rebalances), users[1] = keeper, users[2] = drift borrower (utilization)
  type MockUser = (typeof users)[number];
  let owner: MockUser;
  let keeper: MockUser;
  let driftBorrower: MockUser;
  let ownerAcc: PublicKey;

  // Kamino
  let kaminoMarket: PublicKey;
  let kaminoReserve: PublicKey;
  let kaminoBank: PublicKey;

  // Drift
  let driftMMarket: PublicKey; // spot market for the shared mint
  let driftMMarketIndex: number;
  let driftCollatMarket: PublicKey; // wsol collateral market
  let driftCollatMarketIndex: number;
  let driftCollatOracle: PublicKey; // drift-receiver-owned pyth oracle for the wsol market
  let driftCollatFeed: PublicKey;
  let driftBank: PublicKey;

  // JupLend
  let juplendPool: JuplendPoolKeys;
  let juplendBank: PublicKey;
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;

  const sendOwner = (tx: Transaction) =>
    owner.mrgnProgram.provider.sendAndConfirm(tx);

  const fundUsdc = async (acc: PublicKey, amount: BN) => {
    const ix = createMintToInstruction(
      mint,
      acc,
      bankrunContext.payer.publicKey,
      bnToBigIntSafe(amount),
    );
    await bankRunProvider.sendAndConfirm(new Transaction().add(ix));
  };

  const fundWsol = async (acc: PublicKey, amount: BN) => {
    const ix = createMintToInstruction(
      ecosystem.wsolMint.publicKey,
      acc,
      bankrunContext.payer.publicKey,
      bnToBigIntSafe(amount),
    );
    await bankRunProvider.sendAndConfirm(new Transaction().add(ix));
  };

  const initAcc = async (u: typeof owner): Promise<PublicKey> => {
    const kp = Keypair.generate();
    u.accounts.set(USER_ACCOUNT, kp.publicKey);
    await u.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(program, {
          marginfiGroup: groupPk,
          marginfiAccount: kp.publicKey,
          authority: u.wallet.publicKey,
          feePayer: u.wallet.publicKey,
        }),
      ),
      [kp],
    );
    return kp.publicKey;
  };

  /** Own Kamino market + reserve + mrgn bank + obligation; kept at ~0 utilization (rate ~ 0). */
  const setupKamino = async () => {
    kaminoMarket = await createKaminoMarket("USDC");
    const reserveKp = Keypair.generate();
    await createReserve(
      reserveKp,
      kaminoMarket,
      mint,
      "venue_usdc_reserve",
      ecosystem.usdcDecimals,
      usdcOracle,
      groupAdmin.usdcAccount,
    );
    kaminoReserve = reserveKp.publicKey;

    [kaminoBank] = deriveBankWithSeed(
      program.programId,
      groupPk,
      mint,
      KAMINO_SEED,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeAddKaminoBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: groupPk,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            kaminoReserve,
            kaminoMarket,
            oracle: usdcOracle,
          },
          { seed: KAMINO_SEED, config: defaultKaminoBankConfig(usdcOracle) },
        ),
      ),
      [groupAdmin.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
        await simpleRefreshReserve(
          klendBankrunProgram,
          kaminoReserve,
          kaminoMarket,
          usdcOracle,
        ),
        await makeInitObligationIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: kaminoBank,
            signerTokenAccount: groupAdmin.usdcAccount,
            lendingMarket: kaminoMarket,
            reserve: kaminoReserve,
          },
          new BN(500),
        ),
      ),
      [groupAdmin.wallet],
    );
  };

  /** fetch-or-init the singleton Drift `State`. */
  const ensureDriftState = async () => {
    const [statePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("drift_state")],
      driftBankrunProgram.programId,
    );
    const existing = await safeGetAccountInfo(
      bankRunProvider.connection,
      statePda,
    );
    if (!existing) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await makeInitializeDriftIx(driftBankrunProgram, {
            admin: groupAdmin.wallet.publicKey,
            usdcMint: mint,
          }),
        ),
        [groupAdmin.wallet],
      );
    }
  };

  /**
   * Own Drift spot markets (shared mint + a wsol collateral market) and a mrgn Drift bank on the
   * shared-mint market. A native borrower then borrows the shared mint against wsol collateral to
   * push the market's utilization (and hence its deposit rate) above zero.
   */
  const setupDrift = async () => {
    await ensureDriftState();

    // Append two markets at the next free indices (markets are global, index-keyed).
    let state = await getDriftStateAccount(driftBankrunProgram);
    driftMMarketIndex = state.numberOfSpotMarkets;
    [driftMMarket] = deriveSpotMarketPDA(
      driftBankrunProgram.programId,
      driftMMarketIndex,
    );
    const mConfig = quoteAssetSpotMarketConfig();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitializeSpotMarketIx(
          driftBankrunProgram,
          {
            admin: groupAdmin.wallet.publicKey,
            spotMarketMint: mint,
            oracle: PublicKey.default,
          },
          {
            optimalUtilization: mConfig.optimalUtilization,
            optimalRate: mConfig.optimalRate,
            maxRate: mConfig.maxRate,
            oracleSource: DriftOracleSourceValues.quoteAsset,
            initialAssetWeight: mConfig.initialAssetWeight,
            maintenanceAssetWeight: mConfig.maintenanceAssetWeight,
            initialLiabilityWeight: mConfig.initialLiabilityWeight,
            maintenanceLiabilityWeight: mConfig.maintenanceLiabilityWeight,
            marketIndex: driftMMarketIndex,
          },
        ),
      ),
      [groupAdmin.wallet],
    );

    state = await getDriftStateAccount(driftBankrunProgram);
    driftCollatMarketIndex = state.numberOfSpotMarkets;
    [driftCollatMarket] = deriveSpotMarketPDA(
      driftBankrunProgram.programId,
      driftCollatMarketIndex,
    );
    const collatOracleKp = Keypair.generate();
    const collatFeedKp = Keypair.generate();
    driftCollatOracle = collatOracleKp.publicKey;
    driftCollatFeed = collatFeedKp.publicKey;
    await createBankrunPythOracleAccount(
      bankrunContext,
      banksClient,
      collatOracleKp,
      DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
    );
    await refreshDriftCollatOracle();
    const collatConfig = defaultSpotMarketConfig();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitializeSpotMarketIx(
          driftBankrunProgram,
          {
            admin: groupAdmin.wallet.publicKey,
            spotMarketMint: ecosystem.wsolMint.publicKey,
            oracle: driftCollatOracle,
          },
          {
            optimalUtilization: collatConfig.optimalUtilization,
            optimalRate: collatConfig.optimalRate,
            maxRate: collatConfig.maxRate,
            oracleSource: DriftOracleSourceValues.pythPull,
            initialAssetWeight: collatConfig.initialAssetWeight,
            maintenanceAssetWeight: collatConfig.maintenanceAssetWeight,
            initialLiabilityWeight: collatConfig.initialLiabilityWeight,
            maintenanceLiabilityWeight: collatConfig.maintenanceLiabilityWeight,
            marketIndex: driftCollatMarketIndex,
          },
        ),
      ),
      [groupAdmin.wallet],
    );

    [driftBank] = deriveBankWithSeed(
      program.programId,
      groupPk,
      mint,
      DRIFT_SEED,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeAddDriftBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: groupPk,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            integrationAcc1: driftMMarket,
            oracle: usdcOracle,
          },
          { seed: DRIFT_SEED, config: defaultDriftBankConfig(usdcOracle) },
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitDriftUserIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: driftBank,
            signerTokenAccount: groupAdmin.usdcAccount,
          },
          { amount: usdc(1) },
          driftMMarketIndex,
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    await driveDriftUtilization();
  };

  const refreshDriftCollatOracle = async () => {
    await setPythPullOraclePrice(
      bankrunContext,
      banksClient,
      driftCollatOracle,
      driftCollatFeed,
      ecosystem.wsolPrice,
      ecosystem.wsolDecimals,
      ORACLE_CONF_INTERVAL,
      DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
    );
  };

  /** Create the native drift subaccount-0 (userStats + user) for a wallet so it can deposit/borrow. */
  const initNativeDriftUser = async (u: typeof owner) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitializeUserStatsIx(driftBankrunProgram, {
          authority: u.wallet.publicKey,
          payer: u.wallet.publicKey,
        }),
        await makeInitializeUserIx(
          driftBankrunProgram,
          {
            authority: u.wallet.publicKey,
            payer: u.wallet.publicKey,
            user: PublicKey.findProgramAddressSync(
              [
                Buffer.from("user"),
                u.wallet.publicKey.toBuffer(),
                new BN(0).toArrayLike(Buffer, "le", 2),
              ],
              driftBankrunProgram.programId,
            )[0],
          },
          { subAccountId: 0, name: Array(32).fill(0) },
        ),
      ),
      [u.wallet],
      false,
      true,
    );
  };

  /** Native drift borrower: deposit wsol collateral, borrow the shared mint -> market utilization > 0. */
  const driveDriftUtilization = async () => {
    await initNativeDriftUser(groupAdmin);
    await initNativeDriftUser(driftBorrower);

    // Seed the shared-mint market with lender liquidity so there is something to borrow.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeDriftNativeDepositIx(
          driftBankrunProgram,
          {
            authority: groupAdmin.wallet.publicKey,
            userTokenAccount: groupAdmin.usdcAccount,
          },
          {
            marketIndex: driftMMarketIndex,
            amount: usdc(10_000),
            subAccountId: 0,
            reduceOnly: false,
            remainingMarkets: [driftMMarket],
          },
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    await fundWsol(driftBorrower.wsolAccount, wsol(200));
    await refreshDriftCollatOracle();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeDriftNativeDepositIx(
          driftBankrunProgram,
          {
            authority: driftBorrower.wallet.publicKey,
            userTokenAccount: driftBorrower.wsolAccount,
          },
          {
            marketIndex: driftCollatMarketIndex,
            amount: wsol(100),
            subAccountId: 0,
            reduceOnly: false,
            remainingOracles: [driftCollatOracle],
            remainingMarkets: [driftCollatMarket],
          },
        ),
      ),
      [driftBorrower.wallet],
      false,
      true,
    );
    // Borrow the shared mint against the wsol deposit (~30% utilization on a 50% optimal curve).
    await refreshDriftCollatOracle();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeDriftNativeWithdrawIx(
          driftBankrunProgram,
          {
            authority: driftBorrower.wallet.publicKey,
            userTokenAccount: driftBorrower.usdcAccount,
          },
          {
            marketIndex: driftMMarketIndex,
            amount: usdc(3_000),
            subAccountId: 0,
            reduceOnly: false,
            remainingOracles: [driftCollatOracle],
            remainingMarkets: [driftMMarket, driftCollatMarket],
          },
        ),
      ),
      [driftBorrower.wallet],
      false,
      true,
    );
  };

  /** fetch-or-init the singleton JupLend globals. */
  const ensureJuplendGlobals = async () => {
    const { liquidity } = deriveJuplendGlobalKeys();
    const existing = await safeGetAccountInfo(
      bankRunProvider.connection,
      liquidity,
    );
    if (!existing) {
      await initJuplendGlobals({ admin: groupAdmin.wallet });
    }
  };

  /**
   * Own JupLend pool on the shared mint (fetch-or-init since the pool is mint-keyed and may already
   * exist when this spec runs inside the full suite) + a mrgn JupLend bank, then a native borrower
   * pushes utilization so the supply rate is comfortably above Drift's.
   */
  const setupJuplend = async () => {
    juplendPrograms = getJuplendPrograms();
    await ensureJuplendGlobals();

    const { lending } = deriveJuplendLendingPdas(mint);
    const poolExists = await safeGetAccountInfo(
      bankRunProvider.connection,
      lending,
    );
    if (poolExists) {
      juplendPool = (await fetchJuplendPool({ mint })).keys;
    } else {
      // Aggressive curve so even modest utilization yields a high supply rate.
      juplendPool = await initJuplendPool({
        admin: groupAdmin.wallet,
        mint,
        symbol: "VUSDC",
        decimals: ecosystem.usdcDecimals,
        rateConfig: {
          ...DEFAULT_RATE_CONFIG,
          rateAtUtilizationZero: percent(20),
          rateAtUtilizationKink: percent(60),
        },
      });
      await configureJuplendProtocolPermissions({
        admin: groupAdmin.wallet,
        mint,
        lending: juplendPool.lending,
        rateModel: juplendPool.rateModel,
        tokenReserve: juplendPool.tokenReserve,
        supplyPositionOnLiquidity: juplendPool.supplyPositionOnLiquidity,
        borrowPositionOnLiquidity: juplendPool.borrowPositionOnLiquidity,
        tokenProgram: juplendPool.tokenProgram,
        borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
      });
    }

    [juplendBank] = deriveBankWithSeed(
      program.programId,
      groupPk,
      mint,
      JUPLEND_SEED,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
          group: groupPk,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: mint,
          bankSeed: JUPLEND_SEED,
          oracle: usdcOracle,
          jupLendingState: juplendPool.lending,
          fTokenMint: juplendPool.fTokenMint,
          config: defaultJuplendBankConfig(usdcOracle, ecosystem.usdcDecimals),
        }),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          signerTokenAccount: groupAdmin.usdcAccount,
          bank: juplendBank,
          pool: juplendPool,
          seedDepositAmount: usdc(1),
        }),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    await driveJuplendUtilization();
  };

  /** Native JupLend lender + borrower (admin) so the pool's supply rate is positive and high. */
  const driveJuplendUtilization = async () => {
    const adminFToken = getAssociatedTokenAddressSync(
      juplendPool.fTokenMint,
      groupAdmin.wallet.publicKey,
      false,
      juplendPool.tokenProgram,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createAssociatedTokenAccountIdempotentInstruction(
          groupAdmin.wallet.publicKey,
          adminFToken,
          groupAdmin.wallet.publicKey,
          juplendPool.fTokenMint,
          juplendPool.tokenProgram,
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeJuplendNativeLendingDepositIx(juplendPrograms.lending, {
          signer: groupAdmin.wallet.publicKey,
          depositorTokenAccount: groupAdmin.usdcAccount,
          recipientTokenAccount: adminFToken,
          pool: juplendPool,
          assets: usdc(10_000),
        }),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    const [supplyPos] = findJuplendLiquiditySupplyPositionPda(
      mint,
      groupAdmin.wallet.publicKey,
    );
    const [borrowPos] = findJuplendLiquidityBorrowPositionPda(
      mint,
      groupAdmin.wallet.publicKey,
    );
    const debtCeiling = usdc(100_000);
    // The protocol borrow position is a singleton keyed by (protocol, mint); the jl* specs already
    // init it for groupAdmin on this mint in the full suite. Only run InitProtocolPositions when it
    // is absent; the class/borrow-config updates are idempotent and keep our debt ceiling high enough
    // for the borrow below.
    const borrowPosExists = await safeGetAccountInfo(
      bankRunProvider.connection,
      borrowPos,
    );
    const setupIxs = [
      await updateJuplendUserClassIx(juplendPrograms, {
        authority: groupAdmin.wallet.publicKey,
        authList: juplendPool.authList,
        entries: [{ addr: groupAdmin.wallet.publicKey, value: 1 }],
      }),
      await updateJuplendUserBorrowConfigIx(juplendPrograms, {
        authority: groupAdmin.wallet.publicKey,
        protocol: groupAdmin.wallet.publicKey,
        authList: juplendPool.authList,
        rateModel: juplendPool.rateModel,
        mint,
        tokenReserve: juplendPool.tokenReserve,
        userBorrowPosition: borrowPos,
        config: {
          ...DEFAULT_BORROW_CONFIG,
          baseDebtCeiling: debtCeiling,
          maxDebtCeiling: debtCeiling,
        },
      }),
    ];
    if (!borrowPosExists) {
      setupIxs.unshift(
        await initJuplendProtocolPositionsIx(juplendPrograms, {
          authority: groupAdmin.wallet.publicKey,
          authList: juplendPool.authList,
          supplyMint: mint,
          borrowMint: mint,
          protocol: groupAdmin.wallet.publicKey,
        }),
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...setupIxs),
      [groupAdmin.wallet],
      false,
      true,
    );
    // Drive utilization to a high target so the supply rate clears the Drift quote-asset market.
    // The borrow amount is computed from the live reserve totals (rather than a fixed 70%) because in
    // the full suite this pool is shared with the jl* specs: their existing deposits/borrows make a
    // fixed borrow yield a much lower utilization than in isolation, leaving the rate below Drift's.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(juplendPrograms.lending, { pool: juplendPool }),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    const reserve = await juplendPrograms.liquidity.account.tokenReserve.fetch(
      juplendPool.tokenReserve,
    );
    const totalSupply = reserve.totalSupplyWithInterest.add(
      reserve.totalSupplyInterestFree,
    );
    const totalBorrow = reserve.totalBorrowWithInterest.add(
      reserve.totalBorrowInterestFree,
    );
    // Target 90% of the reserve's max utilization, capped by both the debt ceiling and the available
    // liquidity so the borrow can never exceed what the pool holds.
    const targetBorrow = totalSupply
      .muln(Math.round(reserve.maxUtilization * 0.9))
      .divn(10_000);
    const headroom = totalSupply.sub(totalBorrow);
    let borrowAmount = targetBorrow.sub(totalBorrow);
    if (borrowAmount.gt(headroom)) borrowAmount = headroom;
    const ceilingRoom = debtCeiling.sub(totalBorrow);
    if (borrowAmount.gt(ceilingRoom)) borrowAmount = ceilingRoom;
    if (borrowAmount.gtn(0)) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await makeJuplendNativePreOperateIx(juplendPrograms.liquidity, {
            protocol: groupAdmin.wallet.publicKey,
            mint,
            pool: juplendPool,
            userSupplyPosition: supplyPos,
            userBorrowPosition: borrowPos,
          }),
          await makeJuplendNativeBorrowIx(juplendPrograms.liquidity, {
            protocol: groupAdmin.wallet.publicKey,
            pool: juplendPool,
            userSupplyPosition: supplyPos,
            userBorrowPosition: borrowPos,
            borrowTo: groupAdmin.wallet.publicKey,
            borrowAmount,
          }),
        ),
        [groupAdmin.wallet],
        false,
        true,
      );
    }
  };

  const driftTail = (): PublicKey[] => [usdcOracle, driftMMarket];
  const kaminoTail = (): PublicKey[] => [usdcOracle, kaminoReserve];
  const juplendTail = (): PublicKey[] => [usdcOracle, juplendPool.lending];

  // Kamino is slot-based: refresh the reserve (and its obligation) before the start_rebalance rate
  // gate reads it. One refresh in the sandwich tx keeps it fresh for both the gate and the withdraw.
  const kaminoCranks = async (): Promise<TransactionInstruction[]> => {
    const [lva] = deriveLiquidityVaultAuthority(program.programId, kaminoBank);
    const [obligation] = deriveBaseObligation(lva, kaminoMarket);
    return [
      await simpleRefreshReserve(
        klendBankrunProgram,
        kaminoReserve,
        kaminoMarket,
        usdcOracle,
      ),
      await simpleRefreshObligation(klendBankrunProgram, kaminoMarket, obligation, [
        kaminoReserve,
      ]),
    ];
  };
  const driftCranks = async (): Promise<TransactionInstruction[]> => [
    await makeUpdateSpotMarketCumulativeInterestIx(
      driftBankrunProgram,
      {},
      driftMMarketIndex,
    ),
  ];
  const juplendCranks = async (): Promise<TransactionInstruction[]> => [
    await refreshJupSimple(juplendPrograms.lending, { pool: juplendPool }),
  ];

  const kaminoSrcWithdraw = async (): Promise<TransactionInstruction[]> => [
    await makeKaminoWithdrawIx(
      keeper.mrgnProgram,
      {
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: kaminoBank,
        mint,
        destinationTokenAccount: keeper.usdcAccount,
        lendingMarket: kaminoMarket,
        reserve: kaminoReserve,
      },
      {
        amount: new BN(0),
        isWithdrawAll: true,
        remaining: composeRemainingAccounts([[kaminoBank, usdcOracle, kaminoReserve]]),
      },
    ),
  ];

  const driftSrcWithdraw = async (): Promise<TransactionInstruction[]> => [
    await makeDriftWithdrawIx(
      keeper.mrgnProgram,
      {
        marginfiAccount: ownerAcc,
        bank: driftBank,
        destinationTokenAccount: keeper.usdcAccount,
      },
      {
        amount: new BN(0),
        withdrawAll: true,
        remaining: composeRemainingAccounts([[driftBank, usdcOracle, driftMMarket]]),
      },
      driftBankrunProgram,
    ),
  ];

  const driftDstDeposit = async (): Promise<TransactionInstruction> =>
    makeDriftDepositIx(
      keeper.mrgnProgram,
      { marginfiAccount: ownerAcc, bank: driftBank, signerTokenAccount: keeper.usdcAccount },
      REBALANCE_AMOUNT,
      driftMMarketIndex,
    );

  const juplendDstDeposit = async (): Promise<TransactionInstruction> =>
    makeJuplendDepositIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      signerTokenAccount: keeper.usdcAccount,
      bank: juplendBank,
      pool: juplendPool,
      amount: REBALANCE_AMOUNT,
    });

  const placeOrder = async (allowedBanks: PublicKey[]) => {
    const [order] = deriveRebalanceOrder(program.programId, ownerAcc, mint);
    await sendOwner(
      new Transaction().add(
        await program.methods
          .marginfiAccountPlaceRebalanceOrder(
            allowedBanks,
            bigNumberToWrappedI80F48(0.0001),
            new BN(0),
            null,
            null,
          )
          .accountsPartial({
            group: groupPk,
            marginfiAccount: ownerAcc,
            authority: owner.wallet.publicKey,
            mint,
            rebalanceOrder: order,
            feePayer: owner.wallet.publicKey,
          })
          .instruction(),
      ),
    );
    return order;
  };

  const closeOrder = async (order: PublicKey) => {
    await sendOwner(
      new Transaction().add(
        await program.methods
          .marginfiAccountCloseRebalanceOrder()
          .accountsPartial({
            marginfiAccount: ownerAcc,
            authority: owner.wallet.publicKey,
            feeRecipient: owner.wallet.publicKey,
            rebalanceOrder: order,
          })
          .instruction(),
      ),
    );
  };

  const nextSeq = () => nextSeqOf(program, ownerAcc);
  const lastSeq = () => lastSeqOf(program, ownerAcc);

  /**
   * Build and execute the keeper sandwich: cranks -> start -> src withdraw -> dst deposit -> end.
   * Packed into a v0 transaction + lookup table because the two full venue account sets exceed the
   * legacy 1232-byte limit.
   */
  const runSandwich = async (opts: {
    order: PublicKey;
    src: { bank: PublicKey; leg: VenueLeg; withdraw: TransactionInstruction[] };
    dst: { bank: PublicKey; leg: VenueLeg; deposit: TransactionInstruction };
  }) => {
    const { order, src, dst } = opts;
    const seq = await nextSeq();
    const [record] = deriveRebalanceRecord(program.programId, ownerAcc, seq);
    const [feePool] = deriveRebalanceFeePool(program.programId, ownerAcc);

    // A referenced venue bank block: [bank, (JupLend reserve), rewards..., priceOracle, venueAccount].
    const venueBlock = (bank: PublicKey, leg: VenueLeg): AccountMeta[] => [
      { pubkey: bank, isSigner: false, isWritable: true },
      ...(leg.tokenReserve
        ? [{ pubkey: leg.tokenReserve, isSigner: false, isWritable: false }]
        : []),
      ...toMeta(leg.rewards),
      ...toMeta(leg.tail),
    ];
    // Referenced banks, indexed 0=src, 1=dst.
    const refBanks: AccountMeta[] = [
      ...venueBlock(src.bank, src.leg),
      ...venueBlock(dst.bank, dst.leg),
    ];
    const moves = [buildMove(0, 1, REBALANCE_AMOUNT.toNumber() / 1e6)];
    // After a full move only dst is active, so the post-move health set is the dst bank block.
    const endRemaining: AccountMeta[] = [
      ...refBanks,
      ...toMeta(composeRemainingAccounts([[dst.bank, ...dst.leg.tail]])),
    ];

    const startIx = await program.methods
      .marginfiAccountStartRebalance(moves, bigIntToBnSafe(seq))
      .accountsPartial({
        group: groupPk,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(refBanks)
      .instruction();

    const endIx = await program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group: groupPk,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        feePool,
      })
      .remainingAccounts(endRemaining)
      .instruction();

    const ixs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ...src.leg.cranks,
      ...dst.leg.cranks,
      startIx,
      ...src.withdraw,
      dst.deposit,
      // The withdraw/deposit legs mark the venue reserves stale, so re-refresh them before
      // end_rebalance reads their post-move rates (the staleness check rejects a stale reserve).
      ...src.leg.cranks,
      ...dst.leg.cranks,
      endIx,
    ];

    const lut = await createLookupTableForInstructions(keeper.wallet, ixs);
    const messageV0 = new TransactionMessage({
      payerKey: keeper.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: ixs,
    }).compileToV0Message([lut]);
    const tx = new VersionedTransaction(messageV0);
    await processBankrunV0Transaction(bankrunContext, tx, [keeper.wallet], false, true);
  };

  const getBalance = async (bank: PublicKey) => {
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    return acc.lendingAccount.balances.find(
      (b: any) => b.active && b.bankPk.equals(bank),
    );
  };

  before(async () => {
    program = bankrunProgram;
    usdcOracle = oracles.usdcOracle.publicKey;
    [owner, keeper, driftBorrower] = [users[0], users[1], users[2]];

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupInitialize(program, {
          marginfiGroup: groupPk,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [group],
    );

    // Admin and keeper need the shared mint; the keeper is the sandwich's token conduit.
    await fundUsdc(groupAdmin.usdcAccount, usdc(1_000_000));
    await fundUsdc(keeper.usdcAccount, usdc(100_000));
    await fundUsdc(owner.usdcAccount, usdc(100_000));

    await setupKamino();
    await setupDrift();
    await setupJuplend();
  });

  it("rebalances Kamino (src) -> Drift (dst)", async () => {
    // Fresh marginfi account per scenario so balances don't carry over between the two moves.
    ownerAcc = await initAcc(owner);

    // Owner deposits into the Kamino bank (low-util source).
    await sendOwner(
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          kaminoReserve,
          kaminoMarket,
          usdcOracle,
        ),
        await (async () => {
          const [lva] = deriveLiquidityVaultAuthority(program.programId, kaminoBank);
          const [obligation] = deriveBaseObligation(lva, kaminoMarket);
          return simpleRefreshObligation(klendBankrunProgram, kaminoMarket, obligation, [
            kaminoReserve,
          ]);
        })(),
        await makeKaminoDepositIx(
          owner.mrgnProgram,
          {
            marginfiAccount: ownerAcc,
            bank: kaminoBank,
            signerTokenAccount: owner.usdcAccount,
            lendingMarket: kaminoMarket,
            reserve: kaminoReserve,
          },
          REBALANCE_AMOUNT,
        ),
      ),
    );

    const order = await placeOrder([kaminoBank, driftBank]);

    assert.isUndefined(await getBalance(driftBank), "dst empty before move");
    assert.exists(await getBalance(kaminoBank), "src funded before move");

    await runSandwich({
      order,
      src: {
        bank: kaminoBank,
        leg: {
          cranks: await kaminoCranks(),
          tail: kaminoTail(),
          tokenReserve: null,
          rewards: [kaminoMarket],
        },
        withdraw: await kaminoSrcWithdraw(),
      },
      dst: {
        bank: driftBank,
        leg: {
          cranks: await driftCranks(),
          tail: driftTail(),
          tokenReserve: null,
          rewards: [],
        },
        deposit: await driftDstDeposit(),
      },
    });

    assert.isUndefined(await getBalance(kaminoBank), "src drained after move");
    // The clock is frozen after the sandwich, so this cumulative deposit interest is the one the
    // deposit leg used.
    const spotMarket =
      await driftBankrunProgram.account.spotMarket.fetch(driftMMarket);
    const expectedDriftShares = tokenAmountToScaledBalance(
      REBALANCE_AMOUNT,
      spotMarket,
    );
    assert.equal(
      toI80Scaled((await getBalance(driftBank)).assetShares) >>
        I80F48_FRACTIONAL_BITS,
      bnToBigIntSafe(expectedDriftShares),
      "dst holds exactly the moved amount as drift scaled balance",
    );

    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc), "order persists");
    const [record] = deriveRebalanceRecord(
      program.programId,
      ownerAcc,
      await lastSeq(),
    );
    assert.isNull(
      await safeGetAccountInfo(bankRunProvider.connection, record),
      "tip-free record closed at end",
    );

    await closeOrder(order);
  });

  it("rebalances Drift (src) -> JupLend (dst)", async () => {
    ownerAcc = await initAcc(owner);
    // Owner deposits into the Drift bank (source for this scenario).
    await sendOwner(
      new Transaction().add(
        await makeDriftDepositIx(
          owner.mrgnProgram,
          { marginfiAccount: ownerAcc, bank: driftBank, signerTokenAccount: owner.usdcAccount },
          REBALANCE_AMOUNT,
          driftMMarketIndex,
        ),
      ),
    );

    const order = await placeOrder([driftBank, juplendBank]);

    assert.isUndefined(await getBalance(juplendBank), "dst empty before move");
    assert.exists(await getBalance(driftBank), "src funded before move");

    const [fTokenVault] = deriveJuplendFTokenVault(
      program.programId,
      juplendBank,
    );
    const fTokensBefore = await getTokenBalance(bankRunProvider, fTokenVault);

    await runSandwich({
      order,
      src: {
        bank: driftBank,
        leg: {
          cranks: await driftCranks(),
          tail: driftTail(),
          tokenReserve: null,
          rewards: [],
        },
        withdraw: await driftSrcWithdraw(),
      },
      dst: {
        bank: juplendBank,
        leg: {
          cranks: await juplendCranks(),
          tail: juplendTail(),
          tokenReserve: juplendPool.tokenReserve,
          rewards: [juplendPool.lendingRewardsRateModel, juplendPool.fTokenMint],
        },
        deposit: await juplendDstDeposit(),
      },
    });

    assert.isUndefined(await getBalance(driftBank), "src drained after move");
    const mintedShares =
      (await getTokenBalance(bankRunProvider, fTokenVault)) - fTokensBefore;
    assert.isAbove(mintedShares, 0, "the deposit minted fTokens");
    assert.equal(
      toI80Scaled((await getBalance(juplendBank)).assetShares) >>
        I80F48_FRACTIONAL_BITS,
      BigInt(mintedShares),
      "dst shares equal the fTokens minted by the move",
    );

    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc), "order persists");

    await closeOrder(order);
  });
  const MAX_BALANCES = 16;
  /** `MAX_INTEGRATION_POSITIONS`, the most integration balances an account may hold. */
  const INTEGRATION_BALANCES = 8;
  const WORST_CASE_SEED_BASE = 7_710;
  /** Measures ~923k. A bound rather than the figure, which every program edit shifts. */
  const CU_CEILING = 1_100_000;

  /** A Drift bank on the shared mint, on its own spot market, with its mrgn-side drift user. */
  const addDriftBank = async (seed: BN) => {
    const marketIndex = (await getDriftStateAccount(driftBankrunProgram))
      .numberOfSpotMarkets;
    const [market] = deriveSpotMarketPDA(
      driftBankrunProgram.programId,
      marketIndex,
    );
    const cfg = quoteAssetSpotMarketConfig();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitializeSpotMarketIx(
          driftBankrunProgram,
          {
            admin: groupAdmin.wallet.publicKey,
            spotMarketMint: mint,
            oracle: PublicKey.default,
          },
          {
            optimalUtilization: cfg.optimalUtilization,
            optimalRate: cfg.optimalRate,
            maxRate: cfg.maxRate,
            oracleSource: DriftOracleSourceValues.quoteAsset,
            initialAssetWeight: cfg.initialAssetWeight,
            maintenanceAssetWeight: cfg.maintenanceAssetWeight,
            initialLiabilityWeight: cfg.initialLiabilityWeight,
            maintenanceLiabilityWeight: cfg.maintenanceLiabilityWeight,
            marketIndex,
          },
        ),
      ),
      [groupAdmin.wallet],
    );
    const [bank] = deriveBankWithSeed(program.programId, groupPk, mint, seed);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeAddDriftBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: groupPk,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            integrationAcc1: market,
            oracle: usdcOracle,
          },
          { seed, config: defaultDriftBankConfig(usdcOracle) },
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeInitDriftUserIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank,
            signerTokenAccount: groupAdmin.usdcAccount,
          },
          { amount: usdc(1) },
          marketIndex,
        ),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
    return { bank, market, marketIndex };
  };

  /** A plain USDC bank on this group, for padding the account out to its balance limit. */
  const addNativeBank = async (seed: BN) => {
    const [bank] = deriveBankWithSeed(program.programId, groupPk, mint, seed);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: groupPk,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: mint,
          config: defaultBankConfig(),
          seed,
        }),
      ),
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: usdcOracle,
        }),
      ),
    );
    return bank;
  };

  /**
   * The costliest set the runtime can be handed: every balance slot filled, the integration limit
   * saturated, and the move itself running Kamino legs (the heaviest) into Drift.
   */
  it("executes at max balances with the integration limit saturated", async () => {
    ownerAcc = await initAcc(owner);
    const drifts = [];
    for (let i = 0; i < INTEGRATION_BALANCES - 2; i++) {
      drifts.push(await addDriftBank(new BN(WORST_CASE_SEED_BASE + i)));
    }
    const natives = [];
    for (let i = 0; i < MAX_BALANCES - INTEGRATION_BALANCES; i++) {
      natives.push(await addNativeBank(new BN(WORST_CASE_SEED_BASE + 100 + i)));
    }

    // The move runs Kamino -> the utilized drift bank; the rest only weigh down the health set.
    for (const d of drifts) {
      await sendOwner(
        new Transaction().add(
          await makeDriftDepositIx(
            owner.mrgnProgram,
            {
              marginfiAccount: ownerAcc,
              bank: d.bank,
              signerTokenAccount: owner.usdcAccount,
            },
            REBALANCE_AMOUNT,
            d.marketIndex,
          ),
        ),
      );
    }
    await sendOwner(
      new Transaction().add(
        await makeDriftDepositIx(
          owner.mrgnProgram,
          {
            marginfiAccount: ownerAcc,
            bank: driftBank,
            signerTokenAccount: owner.usdcAccount,
          },
          REBALANCE_AMOUNT,
          driftMMarketIndex,
        ),
      ),
    );
    await sendOwner(
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          kaminoReserve,
          kaminoMarket,
          usdcOracle,
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          kaminoMarket,
          deriveBaseObligation(
            deriveLiquidityVaultAuthority(program.programId, kaminoBank)[0],
            kaminoMarket,
          )[0],
          [kaminoReserve],
        ),
        await makeKaminoDepositIx(
          owner.mrgnProgram,
          {
            marginfiAccount: ownerAcc,
            bank: kaminoBank,
            signerTokenAccount: owner.usdcAccount,
            lendingMarket: kaminoMarket,
            reserve: kaminoReserve,
          },
          REBALANCE_AMOUNT,
        ),
      ),
    );
    for (const bank of natives) {
      await sendOwner(
        new Transaction().add(
          await depositIx(owner.mrgnProgram, {
            marginfiAccount: ownerAcc,
            bank,
            tokenAccount: owner.usdcAccount,
            amount: usdc(10),
          }),
        ),
      );
    }

    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const active = acc.lendingAccount.balances.filter((b: any) => b.active);
    assert.equal(
      active.length,
      MAX_BALANCES,
      "account is at its balance limit",
    );

    const order = await placeOrder([kaminoBank, driftBank]);

    const venues = [
      ...drifts,
      { bank: driftBank, market: driftMMarket },
      { bank: kaminoBank, market: kaminoReserve },
    ];
    const blockOf = (bank: PublicKey): PublicKey[] => {
      const venue = venues.find((v) => v.bank.equals(bank));
      return venue ? [bank, usdcOracle, venue.market] : [bank, usdcOracle];
    };
    const healthSet = toMeta(
      composeRemainingAccounts(active.map((b: any) => blockOf(b.bankPk))),
    );
    const refBanks: AccountMeta[] = [
      { pubkey: kaminoBank, isSigner: false, isWritable: true },
      ...toMeta([kaminoMarket, usdcOracle, kaminoReserve]),
      { pubkey: driftBank, isSigner: false, isWritable: true },
      ...toMeta([usdcOracle, driftMMarket]),
    ];

    const half = REBALANCE_AMOUNT.div(new BN(2));
    const seq = await nextSeq();
    const [record] = deriveRebalanceRecord(program.programId, ownerAcc, seq);
    const [feePool] = deriveRebalanceFeePool(program.programId, ownerAcc);
    const [lva] = deriveLiquidityVaultAuthority(program.programId, kaminoBank);
    const [obligation] = deriveBaseObligation(lva, kaminoMarket);
    const cranks = [
      await simpleRefreshReserve(
        klendBankrunProgram,
        kaminoReserve,
        kaminoMarket,
        usdcOracle,
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        kaminoMarket,
        obligation,
        [kaminoReserve],
      ),
      await makeUpdateSpotMarketCumulativeInterestIx(
        driftBankrunProgram,
        {},
        driftMMarketIndex,
      ),
    ];
    const ixs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ...cranks,
      await program.methods
        .marginfiAccountStartRebalance(
          [buildMove(0, 1, half.toNumber() / 1e6)],
          bigIntToBnSafe(seq),
        )
        .accountsPartial({
          group: groupPk,
          marginfiAccount: ownerAcc,
          rebalanceOrder: order,
          executor: keeper.wallet.publicKey,
          rebalanceRecord: record,
          feePayer: keeper.wallet.publicKey,
          instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .remainingAccounts(refBanks)
        .instruction(),
      await makeKaminoWithdrawIx(
        keeper.mrgnProgram,
        {
          marginfiAccount: ownerAcc,
          authority: keeper.wallet.publicKey,
          bank: kaminoBank,
          mint,
          destinationTokenAccount: keeper.usdcAccount,
          lendingMarket: kaminoMarket,
          reserve: kaminoReserve,
        },
        {
          amount: half,
          isWithdrawAll: false,
          remaining: healthSet.map((m) => m.pubkey),
        },
      ),
      await makeDriftDepositIx(
        keeper.mrgnProgram,
        {
          marginfiAccount: ownerAcc,
          bank: driftBank,
          signerTokenAccount: keeper.usdcAccount,
        },
        half,
        driftMMarketIndex,
      ),
      ...cranks,
      await program.methods
        .marginfiAccountEndRebalance()
        .accountsPartial({
          group: groupPk,
          marginfiAccount: ownerAcc,
          rebalanceOrder: order,
          rebalanceRecord: record,
          executor: keeper.wallet.publicKey,
          feePool,
        })
        .remainingAccounts([...refBanks, ...healthSet])
        .instruction(),
    ];

    const lut = await createLookupTableForInstructions(keeper.wallet, ixs);
    const messageV0 = new TransactionMessage({
      payerKey: keeper.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: ixs,
    }).compileToV0Message([lut]);
    const accounts = lockedAccounts(messageV0);
    const result: any = await processBankrunV0Transaction(
      bankrunContext,
      new VersionedTransaction(messageV0),
      [keeper.wallet],
      false,
      true,
    );
    assertFitsTransaction(
      Number(
        result?.meta?.computeUnitsConsumed ?? result?.computeUnitsConsumed ?? 0,
      ),
      accounts,
      CU_CEILING,
    );
  });
});

/**
 * An account at `MAX_LENDING_ACCOUNT_BALANCES` in native banks, as deposits and as a deposit against
 * borrows. Own group and banks, so the padding stays out of the other suites' shared account.
 */
describe("Auto-rebalance orders (worst-case balance sets)", () => {
  const MAX_BALANCES = 16;
  const PAD_BANKS = 15;
  const WORST_CASE_SEED_BASE = 8_900;

  let program: Program<Marginfi>;
  let usdcOracle: PublicKey;
  const group = Keypair.generate();
  const groupPk = group.publicKey;

  let srcBank: PublicKey;
  let dstBank: PublicKey;
  const padBanks: PublicKey[] = [];

  let owner: (typeof users)[number];
  let keeper: (typeof users)[number];
  let lender: (typeof users)[number];
  let utilBorrower: (typeof users)[number];
  let depositAcc: PublicKey; // 16 deposits
  let borrowAcc: PublicKey; // 1 deposit + 15 borrows
  let lenderAcc: PublicKey;
  let utilAcc: PublicKey;

  const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const MOVE_AMOUNT = usdc(1000);
  const COLLATERAL = usdc(50_000);
  /** Both shapes measure ~470k. A bound rather than the figure, which every program edit shifts. */
  const CU_CEILING = 700_000;

  const initAcc = async (u: (typeof users)[number]): Promise<PublicKey> => {
    const kp = Keypair.generate();
    await u.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(program, {
          marginfiGroup: groupPk,
          marginfiAccount: kp.publicKey,
          authority: u.wallet.publicKey,
          feePayer: u.wallet.publicKey,
        }),
      ),
      [kp],
    );
    return kp.publicKey;
  };

  const addUsdcBank = async (seed: BN): Promise<PublicKey> => {
    const [bank] = deriveBankWithSeed(
      program.programId,
      groupPk,
      ecosystem.usdcMint.publicKey,
      seed,
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: groupPk,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          config: defaultBankConfig(),
          seed,
        }),
      ),
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnProgram, {
          bank,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: usdcOracle,
        }),
      ),
    );
    return bank;
  };

  /** The account's active balances, in the stored order the health check consumes them in. */
  const activeBanks = async (acc: PublicKey): Promise<PublicKey[]> => {
    const fetched = await program.account.marginfiAccount.fetch(acc);
    return fetched.lendingAccount.balances
      .filter((b: any) => b.active)
      .map((b: any) => b.bankPk);
  };

  /** `banks` as a health observation set, ordered the way the account stores them. */
  const healthSet = (banks: PublicKey[]): AccountMeta[] =>
    toMeta(
      composeRemainingAccounts(
        [...banks]
          .sort((a, b) => b.toBuffer().compare(a.toBuffer()))
          .map((b) => [b, usdcOracle]),
      ),
    );

  const fundUsdc = async (acc: PublicKey, amount: BN) => {
    await bankRunProvider.sendAndConfirm(
      new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          acc,
          bankrunContext.payer.publicKey,
          bnToBigIntSafe(amount),
        ),
      ),
    );
  };

  const deposit = async (
    u: (typeof users)[number],
    acc: PublicKey,
    bank: PublicKey,
    amount: BN,
  ) =>
    u.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(u.mrgnProgram, {
          marginfiAccount: acc,
          bank,
          tokenAccount: u.usdcAccount,
          amount,
        }),
      ),
    );

  before(async () => {
    program = bankrunProgram;
    usdcOracle = oracles.usdcOracle.publicKey;
    [owner, keeper, lender, utilBorrower] = [
      users[0],
      users[1],
      users[2],
      users[3],
    ];
    for (const u of [owner, keeper, lender, utilBorrower]) {
      await fundUsdc(u.usdcAccount, usdc(200_000));
    }

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupInitialize(program, {
          marginfiGroup: groupPk,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [group],
    );

    srcBank = await addUsdcBank(new BN(WORST_CASE_SEED_BASE));
    dstBank = await addUsdcBank(new BN(WORST_CASE_SEED_BASE + 1));
    for (let i = 0; i < PAD_BANKS; i++) {
      padBanks.push(await addUsdcBank(new BN(WORST_CASE_SEED_BASE + 2 + i)));
    }

    depositAcc = await initAcc(owner);
    borrowAcc = await initAcc(owner);
    lenderAcc = await initAcc(lender);

    // Supply for the borrow shape to draw on.
    for (const bank of [dstBank, ...padBanks]) {
      await deposit(lender, lenderAcc, bank, usdc(5_000));
    }

    // Utilization on the destination, so it out-rates the untouched source.
    utilAcc = await initAcc(utilBorrower);
    await deposit(utilBorrower, utilAcc, padBanks[PAD_BANKS - 1], usdc(10_000));
    await utilBorrower.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(utilBorrower.mrgnProgram, {
          marginfiAccount: utilAcc,
          bank: dstBank,
          tokenAccount: utilBorrower.usdcAccount,
          amount: usdc(2_500),
          remaining: composeRemainingAccounts([
            [padBanks[PAD_BANKS - 1], usdcOracle],
            [dstBank, usdcOracle],
          ]),
        }),
      ),
    );
  });

  /** start -> withdraw half of src -> deposit into dst -> end, over a v0 tx + lookup table. */
  const runSandwich = async (
    acc: PublicKey,
    order: PublicKey,
    post: PublicKey[],
    move: BN,
    withdrawAll = false,
  ) => {
    const refBanks: AccountMeta[] = [
      ...bankBlock(srcBank, usdcOracle),
      ...bankBlock(dstBank, usdcOracle),
    ];
    // The withdraw leg observes the balances as they stand; only `end_rebalance` sees the post set.
    const preSet = healthSet(await activeBanks(acc));
    const seq = await nextSeqOf(program, acc);
    const [record] = deriveRebalanceRecord(program.programId, acc, seq);
    const [feePool] = deriveRebalanceFeePool(program.programId, acc);

    const startIx = await program.methods
      .marginfiAccountStartRebalance(
        [buildMove(0, 1, move.toNumber() / 1e6)],
        bigIntToBnSafe(seq),
      )
      .accountsPartial({
        group: groupPk,
        marginfiAccount: acc,
        rebalanceOrder: order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(refBanks)
      .instruction();

    const withdrawIx = await program.methods
      .lendingAccountWithdraw(withdrawAll ? new BN(0) : move, withdrawAll)
      .accountsPartial({
        marginfiAccount: acc,
        authority: keeper.wallet.publicKey,
        bank: srcBank,
        destinationTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(preSet)
      .instruction();

    const depositToDstIx = await program.methods
      .lendingAccountDeposit(move, false)
      .accountsPartial({
        marginfiAccount: acc,
        authority: keeper.wallet.publicKey,
        bank: dstBank,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    const endIx = await program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group: groupPk,
        marginfiAccount: acc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        feePool,
      })
      .remainingAccounts([...refBanks, ...healthSet(post)])
      .instruction();

    const ixs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      startIx,
      withdrawIx,
      depositToDstIx,
      endIx,
    ];
    const lut = await createLookupTableForInstructions(keeper.wallet, ixs);
    const messageV0 = new TransactionMessage({
      payerKey: keeper.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: ixs,
    }).compileToV0Message([lut]);
    const result: any = await processBankrunV0Transaction(
      bankrunContext,
      new VersionedTransaction(messageV0),
      [keeper.wallet],
      false,
      true,
    );
    return {
      accounts: lockedAccounts(messageV0),
      cu: Number(
        result?.meta?.computeUnitsConsumed ?? result?.computeUnitsConsumed ?? 0,
      ),
    };
  };

  const placeOrder = async (acc: PublicKey, u: (typeof users)[number]) => {
    const [order] = deriveRebalanceOrder(
      program.programId,
      acc,
      ecosystem.usdcMint.publicKey,
    );
    await u.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await program.methods
          .marginfiAccountPlaceRebalanceOrder(
            [srcBank, dstBank],
            bigNumberToWrappedI80F48(0.0001),
            new BN(0),
            null,
            null,
          )
          .accountsPartial({
            group: groupPk,
            marginfiAccount: acc,
            authority: u.wallet.publicKey,
            mint: ecosystem.usdcMint.publicKey,
            rebalanceOrder: order,
            feePayer: u.wallet.publicKey,
          })
          .instruction(),
      ),
    );
    return order;
  };

  it("executes with the account at max balances, all deposits", async () => {
    await deposit(owner, depositAcc, srcBank, MOVE_AMOUNT);
    for (const bank of padBanks.slice(0, MAX_BALANCES - 2)) {
      await deposit(owner, depositAcc, bank, usdc(10));
    }
    const start = await activeBanks(depositAcc);
    assert.equal(start.length, MAX_BALANCES - 1, "one slot left for dst");

    const order = await placeOrder(depositAcc, owner);
    // A partial move keeps src open, so dst takes the account's last free slot.
    const { cu, accounts } = await runSandwich(
      depositAcc,
      order,
      [...start, dstBank],
      MOVE_AMOUNT.div(new BN(2)),
    );

    assertFitsTransaction(cu, accounts, CU_CEILING);
    assert.equal(
      (await activeBanks(depositAcc)).length,
      MAX_BALANCES,
      "the move filled the last balance slot",
    );
  });

  it("executes with the account at max balances, a deposit against 15 borrows", async () => {
    await deposit(owner, borrowAcc, srcBank, COLLATERAL);
    for (const bank of padBanks) {
      const observed = [...(await activeBanks(borrowAcc)), bank]
        .filter((b, i, all) => all.findIndex((x) => x.equals(b)) === i)
        .sort((a, b) => b.toBuffer().compare(a.toBuffer()));
      await owner.mrgnProgram.provider.sendAndConfirm(
        // A borrow prices every active balance, so the last few exceed the default 200k budget.
        new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          await borrowIx(owner.mrgnProgram, {
            marginfiAccount: borrowAcc,
            bank,
            tokenAccount: owner.usdcAccount,
            amount: usdc(100),
            remaining: composeRemainingAccounts(
              observed.map((b) => [b, usdcOracle]),
            ),
          }),
        ),
      );
    }
    const start = await activeBanks(borrowAcc);
    assert.equal(start.length, MAX_BALANCES, "1 deposit + 15 borrows");

    const order = await placeOrder(borrowAcc, owner);
    // Every slot is taken, so the whole source has to move: src closes and dst takes its place.
    const { cu, accounts } = await runSandwich(
      borrowAcc,
      order,
      [...start.filter((b) => !b.equals(srcBank)), dstBank],
      COLLATERAL,
      true,
    );

    assertFitsTransaction(cu, accounts, CU_CEILING);
    assert.equal(
      (await activeBanks(borrowAcc)).length,
      MAX_BALANCES,
      "dst replaced src; the borrows are untouched",
    );
  });
});
