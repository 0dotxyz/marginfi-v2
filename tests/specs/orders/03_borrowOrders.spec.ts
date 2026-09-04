import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { assert } from "chai";
import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import {
  accountInit,
  borrowIx,
  cancelBorrowOrderIx,
  composeRemainingAccounts,
  depositIx,
  endBorrowOrderCloseIx,
  endBorrowOrderOpenIx,
  placeBorrowOrderIx,
  repayIx,
  startBorrowOrderCloseIx,
  startBorrowOrderOpenIx,
  updateBorrowOrderIx,
  withdrawIx,
} from "../../utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
} from "../../utils/group-instructions";
import { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } from "../../utils/types";
import { deriveBankWithSeed, deriveBorrowOrderPda } from "../../utils/pdas";
import {
  expectFailedTxWithError,
  getTokenBalance,
} from "../../utils/genericTests";
import { Clock } from "../../utils/litesvm";
import {
  bnToBigIntSafe,
  divI80,
  I80F48_SCALE,
  mulI80,
  nativeToI80Scaled,
  toI80Scaled,
} from "../../utils/bn-utils";

const I80F48_ONE = I80F48_SCALE;

/** Matches `INTEREST_MIN_WINDOW_SECONDS` / `INTEREST_MAX_WINDOW_SECONDS`. */
const MIN_WINDOW = 21_600;
const MAX_WINDOW = 172_800;
const U32_MAX = 0xffff_ffff;
/** `milli_to_u32`: a percent on the 0-1000% scale. */
const aprPercent = (pct: number) => Math.floor((pct / 1000) * U32_MAX);

/**
 * Steps the shared clock past a rate window, so this lives in the orders mocha process (see the
 * `all-tests` note in Anchor.toml) with its own group and banks.
 */
describe("Borrow orders", () => {
  let program: Program<Marginfi>;

  const borrowGroup = Keypair.generate();
  const group = borrowGroup.publicKey;
  const usdcMint = ecosystem.usdcMint.publicKey;
  const wsolMint = ecosystem.wsolMint.publicKey;
  const SOL_SEED = new BN(9_101);
  const USDC_SEED = new BN(9_102);
  const USDC_DST_SEED = new BN(9_103);

  let solBank: PublicKey; // the borrower's collateral
  let usdcBank: PublicKey; // the bank borrowed from
  let usdcDstBank: PublicKey; // a second USDC bank a redeploying order deposits into
  let owner: (typeof users)[number];
  let lender: (typeof users)[number];
  let keeper: (typeof users)[number];
  let ownerAcc: PublicKey;
  let keeperAcc: PublicKey; // doubles as the third-party borrower that moves the rate
  let order: PublicKey;

  const FLOAT = new BN(10_000 * 10 ** ecosystem.usdcDecimals);
  /** The whole order fills in one go, which is what a fill under an idle bank must take. */
  const AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const PART = new BN(40 * 10 ** ecosystem.usdcDecimals);
  /** Lifts utilization past 70%, which the test curve prices at ~170%, over the close level. */
  const SPIKE = new BN(7_000 * 10 ** ecosystem.usdcDecimals);
  const CLOSE_LEVEL = aprPercent(120);

  const advance = async (seconds: number) => {
    const before = await banksClient.getClock();
    bankrunContext.setClock(
      new Clock(
        before.slot,
        before.epochStartTimestamp,
        before.epoch,
        before.leaderScheduleEpoch,
        before.unixTimestamp + BigInt(seconds),
      ),
    );
  };

  const oracleFor = (bank: PublicKey) =>
    bank.equals(solBank)
      ? oracles.wsolOracle.publicKey
      : oracles.usdcOracle.publicKey;

  /** The health observation set, with `extra` for balances the sandwich is about to open. */
  const remainingKeys = async (
    extra: PublicKey[] = [],
  ): Promise<PublicKey[]> => {
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const banks: PublicKey[] = acc.lendingAccount.balances
      .filter((b: any) => b.active !== 0)
      .map((b: any) => b.bankPk as PublicKey);
    for (const bank of extra) {
      if (!banks.some((b) => b.equals(bank))) banks.push(bank);
    }
    return composeRemainingAccounts(banks.map((b) => [b, oracleFor(b)]));
  };

  const place = async (args: {
    windowSeconds?: number | null;
    destinationBank?: PublicKey | null;
    openBelowApr?: number;
    closeAboveApr?: number;
  }) => {
    const ix = await placeBorrowOrderIx(program, {
      group,
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      feePayer: owner.wallet.publicKey,
      bank: usdcBank,
      amount: AMOUNT,
      openBelowApr: args.openBelowApr ?? aprPercent(100),
      closeAboveApr: args.closeAboveApr,
      cooldownSeconds: 0,
      windowSeconds: args.windowSeconds,
      destinationBank: args.destinationBank,
    });
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
    return deriveBorrowOrderPda(program.programId, ownerAcc, usdcBank)[0];
  };

  const cancel = async (target: PublicKey) => {
    const ix = await cancelBorrowOrderIx(program, {
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      order: target,
      feeRecipient: owner.wallet.publicKey,
    });
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
  };

  /** The keeper's fill through the sandwich, redeploying when the order names a bank. */
  const fill = async (amount: BN) => {
    const fetched = await program.account.borrowOrder.fetch(order);
    const redeploy = !fetched.destinationBank.equals(PublicKey.default);
    const opened = redeploy ? [usdcBank, usdcDstBank] : [usdcBank];

    const start = await startBorrowOrderOpenIx(program, {
      group,
      marginfiAccount: ownerAcc,
      order,
      bank: usdcBank,
      executor: keeper.wallet.publicKey,
      feePayer: keeper.wallet.publicKey,
    });
    // A wallet fill is pinned to the authority's ATA; a redeploying one flows through the keeper's.
    const borrow = await borrowIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      bank: usdcBank,
      tokenAccount: redeploy ? keeper.usdcAccount : owner.usdcAccount,
      amount,
      remaining: await remainingKeys([usdcBank]),
    });
    const legs = [start, borrow];
    if (redeploy) {
      legs.push(
        await depositIx(keeper.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: usdcDstBank,
          tokenAccount: keeper.usdcAccount,
          amount,
          depositUpToLimit: false,
        }),
      );
    }
    legs.push(
      await endBorrowOrderOpenIx(program, {
        group,
        marginfiAccount: ownerAcc,
        order,
        bank: usdcBank,
        destinationBank: redeploy ? usdcDstBank : null,
        executor: keeper.wallet.publicKey,
        remaining: await remainingKeys(opened),
      }),
    );
    await keeper.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(...legs),
    );
  };

  /** The keeper's close through the sandwich: destination withdraw, repay, proof. */
  const close = async (amount: BN) => {
    const start = await startBorrowOrderCloseIx(program, {
      group,
      marginfiAccount: ownerAcc,
      order,
      bank: usdcBank,
      executor: keeper.wallet.publicKey,
      feePayer: keeper.wallet.publicKey,
    });
    const withdraw = await withdrawIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      bank: usdcDstBank,
      tokenAccount: keeper.usdcAccount,
      amount,
      remaining: await remainingKeys(),
    });
    const repay = await repayIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      bank: usdcBank,
      tokenAccount: keeper.usdcAccount,
      amount,
    });
    const end = await endBorrowOrderCloseIx(program, {
      group,
      marginfiAccount: ownerAcc,
      order,
      bank: usdcBank,
      destinationBank: usdcDstBank,
      executor: keeper.wallet.publicKey,
      remaining: await remainingKeys(),
    });
    await keeper.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(start, withdraw, repay, end),
    );
  };

  const liabilitySharesIn = async (bank: PublicKey): Promise<bigint> => {
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const balance = acc.lendingAccount.balances.find(
      (b: any) => b.active !== 0 && b.bankPk.equals(bank),
    );
    return balance ? toI80Scaled(balance.liabilityShares) : 0n;
  };

  before(async () => {
    program = bankrunProgram;
    [owner, keeper, lender] = [users[0], users[1], users[2]];
    [solBank] = deriveBankWithSeed(
      program.programId,
      group,
      wsolMint,
      SOL_SEED,
    );
    [usdcBank] = deriveBankWithSeed(
      program.programId,
      group,
      usdcMint,
      USDC_SEED,
    );
    [usdcDstBank] = deriveBankWithSeed(
      program.programId,
      group,
      usdcMint,
      USDC_DST_SEED,
    );

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupInitialize(program, {
          marginfiGroup: group,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [borrowGroup],
    );

    const addBank = async (mint: PublicKey, seed: BN, bank: PublicKey) => {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await addBankWithSeed(groupAdmin.mrgnProgram, {
            marginfiGroup: group,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            config: {
              ...defaultBankConfig(),
              // Room for the collateral the third-party borrower posts.
              depositLimit: new BN(1_000_000_000_000),
              // The field's u16 ceiling, ~18h, which spans every clock step this spec takes so
              // the oracle stays fresh without republishing the shared feeds.
              oracleMaxAge: 65_535,
            },
            seed,
          }),
        ),
      );
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await configureBankOracle(groupAdmin.mrgnProgram, {
            bank,
            type: ORACLE_SETUP_PYTH_PUSH,
            oracle: oracleFor(bank),
          }),
        ),
      );
    };
    await addBank(wsolMint, SOL_SEED, solBank);
    await addBank(usdcMint, USDC_SEED, usdcBank);
    await addBank(usdcMint, USDC_DST_SEED, usdcDstBank);

    const initAcc = async (u: typeof owner) => {
      const kp = Keypair.generate();
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
    keeperAcc = await initAcc(keeper);
    const lenderAcc = await initAcc(lender);

    const mintAuth = bankrunContext.payer.publicKey;
    await bankrunProgram.provider.sendAndConfirm!(
      new Transaction().add(
        createMintToInstruction(
          wsolMint,
          owner.wsolAccount,
          mintAuth,
          100 * 10 ** ecosystem.wsolDecimals,
        ),
        createMintToInstruction(
          wsolMint,
          keeper.wsolAccount,
          mintAuth,
          100 * 10 ** ecosystem.wsolDecimals,
        ),
        createMintToInstruction(usdcMint, lender.usdcAccount, mintAuth, FLOAT),
      ),
    );

    await lender.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(lender.mrgnProgram, {
          marginfiAccount: lenderAcc,
          bank: usdcBank,
          tokenAccount: lender.usdcAccount,
          amount: FLOAT,
          depositUpToLimit: false,
        }),
      ),
    );
    await owner.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(owner.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: solBank,
          tokenAccount: owner.wsolAccount,
          amount: new BN(50 * 10 ** ecosystem.wsolDecimals),
          depositUpToLimit: false,
        }),
      ),
    );
    await keeper.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(keeper.mrgnProgram, {
          marginfiAccount: keeperAcc,
          bank: solBank,
          tokenAccount: keeper.wsolAccount,
          amount: new BN(100 * 10 ** ecosystem.wsolDecimals),
          depositUpToLimit: false,
        }),
      ),
    );
  });

  it("rejects a window outside the range the bank ring covers - BorrowOrderInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place({ windowSeconds: MIN_WINDOW - 1 });
      },
      "BorrowOrderInvalidConfig",
      6901,
    );
    await expectFailedTxWithError(
      async () => {
        await place({ windowSeconds: MAX_WINDOW + 1 });
      },
      "BorrowOrderInvalidConfig",
      6901,
    );
  });

  it("rejects redeploying into the borrow bank itself - BorrowOrderInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place({ destinationBank: usdcBank });
      },
      "BorrowOrderInvalidConfig",
      6901,
    );
  });

  it("places a wallet-destination order, live from placement", async () => {
    order = await place({ windowSeconds: MIN_WINDOW });
    const fetched = await program.account.borrowOrder.fetch(order);
    assert.isTrue(fetched.bank.equals(usdcBank));
    assert.equal(bnToBigIntSafe(fetched.amount), bnToBigIntSafe(AMOUNT));
    assert.equal(bnToBigIntSafe(fetched.filled), 0n);
    assert.equal(fetched.windowSeconds, MIN_WINDOW);
    assert.equal(fetched.openBelowApr, aprPercent(100));
    assert.equal(fetched.flags, 1); // DESTINATION_WALLET
    assert.isTrue(fetched.destinationBank.equals(PublicKey.default));
  });

  it("updates the order in place", async () => {
    const ix = await updateBorrowOrderIx(program, {
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      order,
      windowSeconds: MIN_WINDOW * 2,
      openBelowApr: aprPercent(50),
    });
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
    const fetched = await program.account.borrowOrder.fetch(order);
    assert.equal(fetched.windowSeconds, MIN_WINDOW * 2);
    assert.equal(fetched.openBelowApr, aprPercent(50));

    // Back to the minimum window for the fills below.
    const back = await updateBorrowOrderIx(program, {
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      order,
      windowSeconds: MIN_WINDOW,
      openBelowApr: aprPercent(100),
    });
    await owner.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(back),
    );
  });

  it("cannot fill before the bank's reading is a window old - BorrowOrderHistoryTooShort", async () => {
    await advance(MIN_WINDOW - 60);
    await expectFailedTxWithError(
      async () => {
        await fill(AMOUNT);
      },
      "BorrowOrderHistoryTooShort",
      6900,
    );
  });

  it("refuses a wallet fill that delivers to the keeper - BorrowOrderWrongDestination", async () => {
    await advance(120);
    await expectFailedTxWithError(
      async () => {
        const start = await startBorrowOrderOpenIx(program, {
          group,
          marginfiAccount: ownerAcc,
          order,
          bank: usdcBank,
          executor: keeper.wallet.publicKey,
          feePayer: keeper.wallet.publicKey,
        });
        const borrow = await borrowIx(keeper.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: usdcBank,
          tokenAccount: keeper.usdcAccount,
          amount: AMOUNT,
          remaining: await remainingKeys([usdcBank]),
        });
        const end = await endBorrowOrderOpenIx(program, {
          group,
          marginfiAccount: ownerAcc,
          order,
          bank: usdcBank,
          destinationBank: null,
          executor: keeper.wallet.publicKey,
          remaining: await remainingKeys([usdcBank]),
        });
        await keeper.mrgnProgram.provider.sendAndConfirm!(
          new Transaction().add(start, borrow, end),
        );
      },
      "BorrowOrderWrongDestination",
      6912,
    );
  });

  it("fills through the keeper sandwich once the window has passed", async () => {
    const usdcBefore = await getTokenBalance(
      owner.mrgnProgram.provider as any,
      owner.usdcAccount,
    );
    await fill(AMOUNT);

    const fetched = await program.account.borrowOrder.fetch(order);
    assert.equal(bnToBigIntSafe(fetched.filled), bnToBigIntSafe(AMOUNT));

    const usdcAfter = await getTokenBalance(
      owner.mrgnProgram.provider as any,
      owner.usdcAccount,
    );
    assert.equal(
      BigInt(usdcAfter) - BigInt(usdcBefore),
      bnToBigIntSafe(AMOUNT),
    );
    // The debt carries the origination fee at the bank's stored rate, at a share value of 1.
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const liab = acc.lendingAccount.balances.find(
      (b: any) => b.active !== 0 && b.bankPk.equals(usdcBank),
    );
    const bank = await program.account.bank.fetch(usdcBank);
    assert.equal(toI80Scaled(bank.liabilityShareValue), I80F48_ONE);
    const fillScaled = bnToBigIntSafe(AMOUNT) << 48n;
    const feeScaled = toI80Scaled(
      bank.config.interestRateConfig.protocolOriginationFee,
    );
    assert.equal(
      toI80Scaled(liab.liabilityShares),
      fillScaled + mulI80(fillScaled, feeScaled),
    );
  });

  it("cancels the order and returns rent", async () => {
    await cancel(order);
    assert.isNull(await program.account.borrowOrder.fetchNullable(order));
  });

  it("rejects a close level without a destination bank - BorrowOrderNoCloseSide", async () => {
    await expectFailedTxWithError(
      async () => {
        await place({ closeAboveApr: CLOSE_LEVEL });
      },
      "BorrowOrderNoCloseSide",
      6906,
    );
  });

  it("a redeploying order deposits the borrowed funds into the destination bank", async () => {
    order = await place({
      windowSeconds: MIN_WINDOW,
      destinationBank: usdcDstBank,
      closeAboveApr: CLOSE_LEVEL,
    });
    const placed = await program.account.borrowOrder.fetch(order);
    assert.isTrue(placed.destinationBank.equals(usdcDstBank));
    assert.equal(placed.closeAboveApr, CLOSE_LEVEL);
    assert.equal(placed.flags, 0);

    // The wallet fill above left its debt on the account; this fill adds to it.
    const sharesBefore = await liabilitySharesIn(usdcBank);
    await fill(AMOUNT);

    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const deposit = acc.lendingAccount.balances.find(
      (b: any) => b.active !== 0 && b.bankPk.equals(usdcDstBank),
    );
    assert.isDefined(
      deposit,
      "the borrowed USDC should sit in the destination bank",
    );
    const dst = await program.account.bank.fetch(usdcDstBank);
    // A fresh bank's share value is exactly 1, so the shares are the native amount.
    assert.equal(toI80Scaled(dst.assetShareValue), 1n << 48n);
    assert.equal(
      toI80Scaled(deposit.assetShares) >> 48n,
      bnToBigIntSafe(AMOUNT),
    );
    const fetched = await program.account.borrowOrder.fetch(order);
    assert.equal(
      toI80Scaled(fetched.liabilityShares),
      (await liabilitySharesIn(usdcBank)) - sharesBefore,
    );
  });

  it("cannot close while the rate sits under the close level - BorrowOrderRateNotHighEnough", async () => {
    await expectFailedTxWithError(
      async () => {
        await close(AMOUNT);
      },
      "BorrowOrderRateNotHighEnough",
      6914,
    );
  });

  it("closes from the destination once the rate has risen, taking all it holds", async () => {
    // A third party lifts utilization past 70% and holds it there for a window.
    await keeper.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(keeper.mrgnProgram, {
          marginfiAccount: keeperAcc,
          bank: usdcBank,
          tokenAccount: keeper.usdcAccount,
          amount: SPIKE,
          remaining: composeRemainingAccounts([
            [solBank, oracleFor(solBank)],
            [usdcBank, oracleFor(usdcBank)],
          ]),
        }),
      ),
    );
    await advance(MIN_WINDOW);
    const sharesBefore = await liabilitySharesIn(usdcBank);
    const before = await program.account.borrowOrder.fetch(order);
    const orderSharesBefore = toI80Scaled(before.liabilityShares);

    // The destination holds the whole redeployed amount, so a close must take all of it.
    await expectFailedTxWithError(
      async () => {
        await close(PART);
      },
      "BorrowOrderCloseIncomplete",
      6917,
    );
    await close(AMOUNT);

    const fetched = await program.account.borrowOrder.fetch(order);
    // The repay burns the closed amount's shares at the index the close accrued to; the principal
    // comes down by the same fraction.
    const bank = await program.account.bank.fetch(usdcBank);
    const burned = divI80(
      nativeToI80Scaled(AMOUNT),
      toI80Scaled(bank.liabilityShareValue),
    );
    assert.equal(await liabilitySharesIn(usdcBank), sharesBefore - burned);
    assert.equal(
      toI80Scaled(fetched.liabilityShares),
      orderSharesBefore - burned,
    );
    const leftScaled =
      (nativeToI80Scaled(before.filled) * (orderSharesBefore - burned)) /
      orderSharesBefore;
    assert.equal(
      bnToBigIntSafe(fetched.filled),
      (leftScaled + (1n << 47n)) >> 48n,
    );
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    const deposit = acc.lendingAccount.balances.find(
      (b: any) => b.active !== 0 && b.bankPk.equals(usdcDstBank),
    );
    assert.equal(toI80Scaled(deposit.assetShares), 0n);
    await cancel(order);
  });
});
