import { BN, Program } from "@coral-xyz/anchor";
import BigNumber from "bignumber.js";
import {
  AccountMeta,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  bigNumberToWrappedI80F48,
  TOKEN_PROGRAM_ID,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMintToInstruction,
  createTransferInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { Marginfi } from "../target/types/marginfi";
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
} from "./rootHooks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
} from "./utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
} from "./utils/group-instructions";
import { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } from "./utils/types";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
  deriveSpotMarketPDA,
} from "./utils/pdas";
import { USER_ACCOUNT } from "./utils/mocks";
import { expectFailedTxWithError, getTokenBalance } from "./utils/genericTests";
import { bnToBigIntSafe, bnToDecimalStringSafe } from "./utils/bn-utils";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
  processBankrunV0Transaction,
  safeGetAccountInfo,
} from "./utils/tools";

// Kamino
import { createKaminoMarket, createReserve } from "./utils/kamino-reserve-setup";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";

// Drift
import {
  makeAddDriftBankIx,
  makeDriftDepositIx,
  makeDriftWithdrawIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";
import {
  makeInitializeDriftIx,
  makeInitializeSpotMarketIx,
  makeInitializeUserIx,
  makeInitializeUserStatsIx,
  makeDepositIx as makeDriftNativeDepositIx,
  makeWithdrawIx as makeDriftNativeWithdrawIx,
  makeUpdateSpotMarketCumulativeInterestIx,
} from "./utils/drift-sdk";
import {
  defaultDriftBankConfig,
  defaultSpotMarketConfig,
  quoteAssetSpotMarketConfig,
  DriftOracleSourceValues,
  getDriftStateAccount,
} from "./utils/drift-utils";
import {
  createBankrunPythOracleAccount,
  setPythPullOraclePrice,
} from "./utils/bankrun-oracles";
import {
  DRIFT_ORACLE_RECEIVER_PROGRAM_ID,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";

// JupLend
import {
  configureJuplendProtocolPermissions,
  fetchJuplendPool,
  initJuplendGlobals,
  initJuplendPool,
} from "./utils/juplend/jlr-pool-setup";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/group-instructions";
import {
  makeJuplendDepositIx,
  makeJuplendNativeBorrowIx,
  makeJuplendNativeLendingDepositIx,
  makeJuplendNativePreOperateIx,
} from "./utils/juplend/user-instructions";
import { refreshJupSimple } from "./utils/juplend/shorthand-instructions";
import {
  deriveJuplendGlobalKeys,
  deriveJuplendLendingPdas,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "./utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "./utils/juplend/programs";
import {
  DEFAULT_BORROW_CONFIG,
  DEFAULT_BORROW_CONFIG_MIN,
  DEFAULT_RATE_CONFIG,
  defaultJuplendBankConfig,
  JuplendPoolKeys,
  percent,
} from "./utils/juplend/types";
import {
  initJuplendProtocolPositionsIx,
  updateJuplendUserBorrowConfigIx,
  updateJuplendUserClassIx,
} from "./utils/juplend/admin-instructions";

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

const deriveRebalanceRecord = (programId: PublicKey, order: PublicKey) =>
  PublicKey.findProgramAddressSync(
    [Buffer.from(REBALANCE_RECORD_SEED), order.toBuffer()],
    programId,
  );

const toMeta = (keys: PublicKey[]): AccountMeta[] =>
  keys.map((pubkey) => ({ pubkey, isSigner: false, isWritable: false }));

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
  let srcBank: PublicKey; // src USDC bank (util 0 -> rate 0)
  let solBank: PublicKey; // borrower collateral
  let dstBank: PublicKey; // dst USDC bank (util ~50% -> rate > 0)

  // users[0] = rebalancing user, users[1] = keeper, users[2] = dst lender, users[3] = borrower
  type MockUser = (typeof users)[number];
  let owner: MockUser;
  let keeper: MockUser;
  let lender: MockUser;
  let borrower: MockUser;

  let ownerAcc: PublicKey;
  let lenderAcc: PublicKey;
  let borrowerAcc: PublicKey;

  const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const sol = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);

  const REBALANCE_AMOUNT = usdc(1000);

  const sendOwner = (tx: Transaction) =>
    owner.mrgnProgram.provider.sendAndConfirm(tx);
  const sendKeeper = (tx: Transaction) =>
    keeper.mrgnProgram.provider.sendAndConfirm(tx);

  /** Build the keeper-signed start -> withdraw -> deposit -> end sandwich. */
  const buildSandwich = async (opts: {
    order: PublicKey;
    extraTrailingIx?: boolean;
    src?: PublicKey;
    dst?: PublicKey;
    depositAmount?: BN;
  }) => {
    const src = opts.src ?? srcBank;
    const dst = opts.dst ?? dstBank;
    const depositAmount = opts.depositAmount ?? REBALANCE_AMOUNT;
    const [record] = deriveRebalanceRecord(program.programId, opts.order);
    const oracleRemaining = toMeta([usdcOracle, usdcOracle]);
    // end_rebalance runs a real init-health check, so it also needs the post-move observation set
    // (bank+oracle per active balance) after the [src_oracle, dst_oracle] rate accounts. After a full
    // native->native move the only active balance is dst.
    const endRemaining = toMeta([usdcOracle, usdcOracle, dst, usdcOracle]);

    const startIx = await program.methods
      .marginfiAccountStartRebalance()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        srcBank: src,
        dstBank: dst,
        srcTokenReserve: null,
        dstTokenReserve: null,
        rebalanceOrder: opts.order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(oracleRemaining)
      .instruction();

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

    const depositIxn = await program.methods
      .lendingAccountDeposit(depositAmount, false)
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: keeper.wallet.publicKey,
        bank: dst,
        signerTokenAccount: keeper.usdcAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    const endIx = await program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        rebalanceOrder: opts.order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        srcBank: src,
        dstBank: dst,
        srcTokenReserve: null,
        dstTokenReserve: null,
      })
      .remainingAccounts(endRemaining)
      .instruction();

    const tx = new Transaction()
      .add(startIx)
      .add(withdrawIx)
      .add(depositIxn)
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
  }) => {
    const [order] = deriveRebalanceOrder(program.programId, ownerAcc, usdcMint);
    const ix = await program.methods
      .marginfiAccountPlaceRebalanceOrder(
        opts.allowedBanks,
        bigNumberToWrappedI80F48(opts.minImprovement),
        new BN(opts.cooldownSeconds),
        opts.amount ?? null,
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

  const closeOrder = async (order: PublicKey) => {
    const ix = await program.methods
      .marginfiAccountCloseRebalanceOrder()
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: owner.wallet.publicKey,
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
      )
      .accountsPartial({
        marginfiAccount: ownerAcc,
        authority: owner.wallet.publicKey,
        rebalanceOrder: order,
      })
      .instruction();
    await sendOwner(new Transaction().add(ix));
  };

  /** Ensure the owner holds REBALANCE_AMOUNT in src and nothing in dst. */
  const resetOwnerToSrc = async () => {
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const has = (bank: PublicKey) =>
      acc.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(bank),
      );
    if (has(dstBank)) {
      // Withdraw-all closes dst and re-sorts before the health check, so the post-withdraw
      // observation set is only the banks that stay active (e.g. a partial-move remainder in src).
      const remainingBanks = acc.lendingAccount.balances
        .filter((b: any) => b.active && !b.bankPk.equals(dstBank))
        .map((b: any) => b.bankPk as PublicKey);
      await sendOwner(
        new Transaction().add(
          await program.methods
            .lendingAccountWithdraw(new BN(0), true)
            .accountsPartial({
              marginfiAccount: ownerAcc,
              authority: owner.wallet.publicKey,
              bank: dstBank,
              destinationTokenAccount: owner.usdcAccount,
              tokenProgram: TOKEN_PROGRAM_ID,
            })
            .remainingAccounts(
              toMeta(
                composeRemainingAccounts(
                  remainingBanks.map((b: PublicKey) => [b, usdcOracle]),
                ),
              ),
            )
            .instruction(),
        ),
      );
    }
    if (!has(srcBank)) {
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
    }
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

    // dst utilization: lender supplies, borrower (SOL collateral) borrows ~50%
    await lender.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(lender.mrgnProgram, {
          marginfiAccount: lenderAcc,
          bank: dstBank,
          tokenAccount: lender.usdcAccount,
          amount: usdc(2000),
        }),
      ),
    );
    await borrower.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(borrower.mrgnProgram, {
          marginfiAccount: borrowerAcc,
          bank: solBank,
          tokenAccount: borrower.wsolAccount,
          amount: sol(50),
        }),
      ),
    );
    await borrower.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(borrower.mrgnProgram, {
          marginfiAccount: borrowerAcc,
          bank: dstBank,
          tokenAccount: borrower.usdcAccount,
          amount: usdc(1000),
          remaining: composeRemainingAccounts([
            [dstBank, usdcOracle],
            [solBank, wsolOracle],
          ]),
        }),
      ),
    );

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

  it("moves the deposit src -> dst and keeps the order - happy path", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    const before = await program.account.marginfiAccount.fetch(ownerAcc);
    assert.isUndefined(
      before.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(dstBank),
      ),
      "dst should be empty before the move",
    );

    const tx = await buildSandwich({ order });
    await sendKeeper(tx);

    const after = await program.account.marginfiAccount.fetch(ownerAcc);
    const srcBal = after.lendingAccount.balances.find(
      (b: any) => b.active && b.bankPk.equals(srcBank),
    );
    const dstBal = after.lendingAccount.balances.find(
      (b: any) => b.active && b.bankPk.equals(dstBank),
    );
    assert.isUndefined(srcBal, "src should be drained after the move");
    assert.exists(dstBal, "dst should hold the moved deposit");

    // Order persists; record was closed back to the keeper.
    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc));
    const [record] = deriveRebalanceRecord(program.programId, order);
    assert.isNull(
      await program.provider.connection.getAccountInfo(record),
      "record should be closed",
    );

    await closeOrder(order);
  });

  it("conserves value exactly minus the keeper fee", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
    });

    const assetShares = async (bank: PublicKey) => {
      const acc = await program.account.marginfiAccount.fetch(ownerAcc);
      const bal = acc.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(bank),
      );
      return bal
        ? wrappedI80F48toBigNumber(bal.assetShares)
        : new BigNumber(0);
    };

    const oldBalance = await assetShares(srcBank);
    const keeperBefore = await getTokenBalance(bankRunProvider, keeper.usdcAccount);

    // Keeper skims $0.40 (within the $0.50 flat-fee cap): withdraw the full source, deposit the rest.
    const tx = await buildSandwich({
      order,
      depositAmount: REBALANCE_AMOUNT.sub(usdc(0.4)),
    });
    await sendKeeper(tx);

    const newBalance = await assetShares(dstBank);
    const keeperFee =
      (await getTokenBalance(bankRunProvider, keeper.usdcAccount)) - keeperBefore;

    assert.isAbove(keeperFee, 0, "keeper should have taken a fee");
    assert.equal(
      newBalance.plus(keeperFee).toString(),
      oldBalance.toString(),
      "dst position + keeper fee must equal the original src position exactly",
    );

    await closeOrder(order);
  });

  it("moves only the ordered amount on a bounded order", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
    // Order half the 1000 USDC position; the rest must stay in src.
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 0,
      amount: usdc(500),
    });

    const assetShares = async (bank: PublicKey) => {
      const acc = await program.account.marginfiAccount.fetch(ownerAcc);
      const bal = acc.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(bank),
      );
      return bal
        ? wrappedI80F48toBigNumber(bal.assetShares)
        : new BigNumber(0);
    };

    const oldSrc = await assetShares(srcBank);
    const [record] = deriveRebalanceRecord(program.programId, order);

    const startIx = await program.methods
      .marginfiAccountStartRebalance()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        srcBank,
        dstBank,
        srcTokenReserve: null,
        dstTokenReserve: null,
        rebalanceOrder: order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(toMeta([usdcOracle, usdcOracle]))
      .instruction();

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

    // A partial move leaves src active, so the post-move observation set spans BOTH banks:
    // [src_oracle, dst_oracle] rate accounts, then bank+oracle per active balance. The health check
    // matches the observation set positionally against active balances in their stored (descending
    // pubkey) slot order, so order the two banks descending here.
    const [hi, lo] =
      srcBank.toBuffer().compare(dstBank.toBuffer()) > 0
        ? [srcBank, dstBank]
        : [dstBank, srcBank];
    const endIx = await program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        srcBank,
        dstBank,
        srcTokenReserve: null,
        dstTokenReserve: null,
      })
      .remainingAccounts(
        toMeta([usdcOracle, usdcOracle, hi, usdcOracle, lo, usdcOracle]),
      )
      .instruction();

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
      dstAfter.toString(),
      bnToDecimalStringSafe(usdc(500)),
      "exactly the ordered amount moved to dst",
    );
    assert.equal(
      srcAfter.plus(dstAfter).toString(),
      oldSrc.toString(),
      "no skim -> same-mint shares conserved",
    );

    await closeOrder(order);
  });

  it("updates an order's min improvement in place - RebalanceNotImproving", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
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
      6603,
    );

    await closeOrder(order);
  });

  it("rejects when dst is not improving enough - RebalanceNotImproving", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
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
      6603,
    );

    await closeOrder(order);
  });

  it("enforces the per-order cooldown - RebalanceCooldown", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
    // 24h cooldown: the first exec stamps last_exec_timestamp, so an immediate second exec (same
    // wall-clock second) is rejected.
    const order = await placeOrder({
      allowedBanks,
      minImprovement: 0.0001,
      cooldownSeconds: 86_400,
    });

    // First execution succeeds and stamps last_exec_timestamp.
    await sendKeeper(await buildSandwich({ order }));

    // Move it back to src so a second execution is structurally possible.
    await resetOwnerToSrc();

    await expectFailedTxWithError(
      async () => {
        await sendKeeper(await buildSandwich({ order }));
      },
      "RebalanceCooldown",
      6601,
    );

    await closeOrder(order);
  });

  it("rejects a tampered instruction sandwich - end_rebalance must be last", async () => {
    await resetOwnerToSrc();
    const allowedBanks = [srcBank, dstBank];
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
    const allowedBanks = [srcBank, dstBank];
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
      6607,
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
  /** JupLend Fluid TokenReserve for the start/end ix arg; null for Kamino/Drift. */
  tokenReserve: PublicKey | null;
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
            rebalanceOrder: order,
          })
          .instruction(),
      ),
    );
  };

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
    const [record] = deriveRebalanceRecord(program.programId, order);

    const startRemaining = toMeta([...src.leg.tail, ...dst.leg.tail]);
    // After a full move only dst is active, so the post-move health set is the dst bank tail.
    const endRemaining = toMeta([
      ...src.leg.tail,
      ...dst.leg.tail,
      ...composeRemainingAccounts([[dst.bank, ...dst.leg.tail]]),
    ]);

    const startIx = await program.methods
      .marginfiAccountStartRebalance()
      .accountsPartial({
        group: groupPk,
        marginfiAccount: ownerAcc,
        srcBank: src.bank,
        dstBank: dst.bank,
        srcTokenReserve: src.leg.tokenReserve,
        dstTokenReserve: dst.leg.tokenReserve,
        rebalanceOrder: order,
        executor: keeper.wallet.publicKey,
        rebalanceRecord: record,
        feePayer: keeper.wallet.publicKey,
        instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts(startRemaining)
      .instruction();

    const endIx = await program.methods
      .marginfiAccountEndRebalance()
      .accountsPartial({
        group: groupPk,
        marginfiAccount: ownerAcc,
        rebalanceOrder: order,
        rebalanceRecord: record,
        executor: keeper.wallet.publicKey,
        srcBank: src.bank,
        dstBank: dst.bank,
        srcTokenReserve: src.leg.tokenReserve,
        dstTokenReserve: dst.leg.tokenReserve,
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
        leg: { cranks: await kaminoCranks(), tail: kaminoTail(), tokenReserve: null },
        withdraw: await kaminoSrcWithdraw(),
      },
      dst: {
        bank: driftBank,
        leg: { cranks: await driftCranks(), tail: driftTail(), tokenReserve: null },
        deposit: await driftDstDeposit(),
      },
    });

    assert.isUndefined(await getBalance(kaminoBank), "src drained after move");
    assert.exists(await getBalance(driftBank), "dst holds the moved deposit");

    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc), "order persists");
    const [record] = deriveRebalanceRecord(program.programId, order);
    assert.isNull(
      await safeGetAccountInfo(program.provider.connection, record),
      "record closed",
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

    await runSandwich({
      order,
      src: {
        bank: driftBank,
        leg: { cranks: await driftCranks(), tail: driftTail(), tokenReserve: null },
        withdraw: await driftSrcWithdraw(),
      },
      dst: {
        bank: juplendBank,
        leg: {
          cranks: await juplendCranks(),
          tail: juplendTail(),
          tokenReserve: juplendPool.tokenReserve,
        },
        deposit: await juplendDstDeposit(),
      },
    });

    assert.isUndefined(await getBalance(driftBank), "src drained after move");
    assert.exists(await getBalance(juplendBank), "dst holds the moved deposit");

    const orderAcc = await program.account.rebalanceOrder.fetch(order);
    assert.ok(orderAcc.marginfiAccount.equals(ownerAcc), "order persists");

    await closeOrder(order);
  });
});
