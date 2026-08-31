import { BN, Program } from "@coral-xyz/anchor";
import { AccountMeta, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
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
  armOrderInterestIx,
  borrowIx,
  closeOrderIx,
  composeRemainingAccounts,
  depositIx,
  endExecuteOrderIx,
  InterestTriggerArgs,
  placeOrderIx,
  repayIx,
  startExecuteOrderIx,
  withdrawIx,
} from "../../utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
} from "../../utils/group-instructions";
import { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } from "../../utils/types";
import {
  deriveBankWithSeed,
  deriveExecuteOrderPda,
  deriveOrderPda,
} from "../../utils/pdas";
import { expectFailedTxWithError } from "../../utils/genericTests";
import { Clock } from "../../utils/litesvm";
import { toI80Scaled } from "../../utils/bn-utils";

/** Matches `INTEREST_MIN_WINDOW_SECONDS`. */
const MIN_WINDOW = 21_600;
/** Matches `INTEREST_DEFAULT_PATIENCE_SECONDS`. */
const DEFAULT_PATIENCE = 1_209_600;

/**
 * This spec advances the shared clock past an interest window, so it lives in the orders mocha
 * process (see the `all-tests` note in Anchor.toml) and stands up its own group and banks rather
 * than perturbing the ecosystem the earlier specs share.
 */
describe("Interest trigger orders", () => {
  let program: Program<Marginfi>;

  const interestGroup = Keypair.generate();
  const group = interestGroup.publicKey;
  const usdcMint = ecosystem.usdcMint.publicKey;
  const wsolMint = ecosystem.wsolMint.publicKey;
  const SOL_SEED = new BN(8_901);
  const USDC_SEED = new BN(8_902);

  let solBank: PublicKey; // the lend leg
  let usdcBank: PublicKey; // the borrow leg
  let owner: (typeof users)[number];
  let lender: (typeof users)[number];
  let ownerAcc: PublicKey;
  let order: PublicKey;
  let keeper: (typeof users)[number];

  const bankKeys = (): PublicKey[] => [solBank, usdcBank];

  const U32_MAX = 0xffff_ffff;
  const maxSlippage = Math.floor((100 / 10_000) * U32_MAX);
  /** Far below the pair's value, so only the carry condition is ever live here. */
  const stopLossThreshold = bigNumberToWrappedI80F48(1);

  const interest = (
    windowSeconds: number | null,
    patienceSeconds: number | null,
  ): InterestTriggerArgs => ({
    windowSeconds,
    patienceSeconds,
    minNegativeApr: null,
  });

  /** The health observation set in the account's own balance order. `extra` is a balance the
   *  instruction is about to open, which takes the next free slot and so trails the active ones. */
  const remainingKeys = async (
    account: PublicKey,
    extra?: PublicKey,
  ): Promise<PublicKey[]> => {
    const acc = await program.account.marginfiAccount.fetch(account);
    const oracleFor = new Map<string, PublicKey>([
      [solBank.toBase58(), oracles.wsolOracle.publicKey],
      [usdcBank.toBase58(), oracles.usdcOracle.publicKey],
    ]);
    const pairs: [PublicKey, PublicKey][] = acc.lendingAccount.balances
      .filter((b: any) => b.active !== 0)
      .map((b: any) => [b.bankPk, oracleFor.get(b.bankPk.toBase58())!]);
    if (extra && !pairs.some(([bank]) => bank.equals(extra))) {
      pairs.push([extra, oracleFor.get(extra.toBase58())!]);
    }
    return composeRemainingAccounts(pairs);
  };

  /** The same set as metas, banks writable: arming accrues both legs before reading their indices. */
  const remainingMetas = async (): Promise<AccountMeta[]> => {
    const banks = [solBank, usdcBank];
    return (await remainingKeys(ownerAcc)).map((pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: banks.some((bank) => bank.equals(pubkey)),
    }));
  };

  /** Move the clock on. The banks below carry an oracle window wider than the span stepped over
   *  here, so no republish is needed. */
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

  const place = async (cfg: InterestTriggerArgs | null): Promise<PublicKey> => {
    const ix = await placeOrderIx(program, {
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      feePayer: owner.wallet.publicKey,
      bankKeys: bankKeys(),
      trigger: { stopLoss: { threshold: stopLossThreshold, maxSlippage } },
      interest: cfg,
    });
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
    return deriveOrderPda(program.programId, ownerAcc, bankKeys())[0];
  };

  const arm = async () => {
    const ix = await armOrderInterestIx(program, {
      marginfiAccount: ownerAcc,
      group,
      order,
      remaining: await remainingMetas(),
    });
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
  };

  before(async () => {
    program = bankrunProgram;
    [owner, lender, keeper] = [users[0], users[2], users[1]];
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

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await groupInitialize(program, {
          marginfiGroup: group,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [interestGroup],
    );

    const addBank = async (
      mint: PublicKey,
      seed: BN,
      bank: PublicKey,
      oracle: PublicKey,
    ) => {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await addBankWithSeed(groupAdmin.mrgnProgram, {
            marginfiGroup: group,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            config: {
              ...defaultBankConfig(),
              // The field's u16 ceiling, ~18h, which spans every clock step this spec takes so
              // the multiplier read stays fresh without republishing the shared oracles.
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
            oracle,
          }),
        ),
      );
    };
    await addBank(wsolMint, SOL_SEED, solBank, oracles.wsolOracle.publicKey);
    await addBank(usdcMint, USDC_SEED, usdcBank, oracles.usdcOracle.publicKey);

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
    const lenderAcc = await initAcc(lender);

    const mintAuth = bankrunContext.payer.publicKey;
    await bankrunProgram.provider.sendAndConfirm!(
      new Transaction().add(
        createMintToInstruction(
          wsolMint,
          owner.wsolAccount,
          mintAuth,
          10 * 10 ** ecosystem.wsolDecimals,
        ),
        createMintToInstruction(
          usdcMint,
          lender.usdcAccount,
          mintAuth,
          10_000 * 10 ** ecosystem.usdcDecimals,
        ),
        createMintToInstruction(
          usdcMint,
          keeper.usdcAccount,
          mintAuth,
          10_000 * 10 ** ecosystem.usdcDecimals,
        ),
      ),
    );

    // The borrow leg needs liquidity, and its utilization is what gives it a real rate.
    await lender.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(lender.mrgnProgram, {
          marginfiAccount: lenderAcc,
          bank: usdcBank,
          tokenAccount: lender.usdcAccount,
          amount: new BN(1_000 * 10 ** ecosystem.usdcDecimals),
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
          amount: new BN(5 * 10 ** ecosystem.wsolDecimals),
          depositUpToLimit: false,
        }),
      ),
    );

    await owner.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(owner.mrgnProgram, {
          marginfiAccount: ownerAcc,
          bank: usdcBank,
          amount: new BN(100 * 10 ** ecosystem.usdcDecimals),
          tokenAccount: owner.usdcAccount,
          remaining: await remainingKeys(ownerAcc, usdcBank),
        }),
      ),
    );
  });

  it("rejects a window under the floor - OrderInterestInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place(interest(MIN_WINDOW - 1, null));
      },
      "OrderInterestInvalidConfig",
      6804,
    );
  });

  it("rejects zero patience - OrderInterestInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place(interest(null, 0));
      },
      "OrderInterestInvalidConfig",
      6804,
    );
  });

  it("leaves the trigger off when no policy is given", async () => {
    const plain = await place(null);
    const fetched = await program.account.order.fetch(plain);
    assert.equal(fetched.interestFlags, 0);
    assert.equal(fetched.interestWindowSeconds, 0);
    assert.equal(fetched.interestPatienceSeconds, 0);

    // Freed so the pair can carry the interest-bearing order below: one Order per bank pair.
    await owner.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await closeOrderIx(program, {
          marginfiAccount: ownerAcc,
          authority: owner.wallet.publicKey,
          order: plain,
          feeRecipient: owner.wallet.publicKey,
        }),
      ),
    );
  });

  it("carries the interest policy onto the order, unarmed", async () => {
    order = await place(interest(MIN_WINDOW, null));
    const fetched = await program.account.order.fetch(order);

    assert.equal(fetched.interestFlags, 1);
    assert.equal(fetched.interestWindowSeconds, MIN_WINDOW);
    assert.equal(fetched.interestPatienceSeconds, DEFAULT_PATIENCE);
    assert.equal(fetched.interestMinNegativeApr, 0);
    // No anchor yet: the order cannot execute on carry until it is armed.
    assert.isTrue(new BN(fetched.interestAnchorTimestamp).isZero());
    assert.equal(toI80Scaled(fetched.interestAnchorAssetIndex), 0n);
    assert.equal(toI80Scaled(fetched.interestAnchorDebtIndex), 0n);
  });

  it("arms the trigger, writing both anchor indices", async () => {
    await arm();

    const fetched = await program.account.order.fetch(order);
    assert.isTrue(new BN(fetched.interestAnchorTimestamp).gt(new BN(0)));
    // Both banks are native, so their multiplier is 1 and each anchor is exactly the bank's own
    // share value at the moment of arming.
    const sol = await program.account.bank.fetch(solBank);
    const usdc = await program.account.bank.fetch(usdcBank);
    assert.equal(
      toI80Scaled(fetched.interestAnchorAssetIndex),
      toI80Scaled(sol.assetShareValue),
    );
    assert.equal(
      toI80Scaled(fetched.interestAnchorDebtIndex),
      toI80Scaled(usdc.liabilityShareValue),
    );
  });

  it("rejects re-arming inside the window - OrderInterestWindowTooShort", async () => {
    await advance(MIN_WINDOW - 60);
    await expectFailedTxWithError(arm, "OrderInterestWindowTooShort", 6800);
  });

  it("carries a live stop-loss and the carry trigger on one order", async () => {
    // One Order per bank pair, so an order that could not hold both would force the user to choose.
    const fetched = await program.account.order.fetch(order);
    assert.equal(fetched.interestFlags, 1);
    assert.equal(
      toI80Scaled(fetched.stopLoss),
      toI80Scaled(stopLossThreshold),
      "the price threshold should survive alongside the carry policy",
    );
    assert.equal(fetched.maxSlippage, maxSlippage);
  });

  it("re-arms once a full window has passed", async () => {
    const before = await program.account.order.fetch(order);
    await advance(120);
    await arm();

    const after = await program.account.order.fetch(order);
    assert.isTrue(
      new BN(after.interestAnchorTimestamp).gt(
        new BN(before.interestAnchorTimestamp),
      ),
      "the anchor should have advanced past the replaced one",
    );
  });

  it("unwinds the pair through the keeper sandwich", async () => {
    // One more window past the re-arm above. The borrow leg charges over it while the idle lend
    // leg earns nothing, so the measured carry is negative and any negative carry arms this order.
    await advance(MIN_WINDOW + 60);

    const [executeRecord] = deriveExecuteOrderPda(program.programId, order);
    const remaining = await remainingKeys(ownerAcc);

    const start = await startExecuteOrderIx(program, {
      group,
      marginfiAccount: ownerAcc,
      feePayer: keeper.wallet.publicKey,
      executor: keeper.wallet.publicKey,
      order,
      remaining,
      // The trigger accrues both legs, so they cannot arrive read-only.
      bankWritable: [solBank, usdcBank],
    });
    const repay = await repayIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      bank: usdcBank,
      tokenAccount: keeper.usdcAccount,
      amount: new BN(0),
      repayAll: true,
      remaining: [],
    });
    const withdraw = await withdrawIx(keeper.mrgnProgram, {
      marginfiAccount: ownerAcc,
      bank: solBank,
      tokenAccount: keeper.wsolAccount,
      // Just under what the ~100 USDC liability is worth at the SOL oracle, so the keeper covers
      // the repayment out of its own pocket rather than skimming the position.
      amount: new BN(0.6 * 10 ** ecosystem.wsolDecimals),
      withdrawAll: false,
      // The borrow leg is closed by the repay above, so only the lend leg remains observable.
      remaining: composeRemainingAccounts([
        [solBank, oracles.wsolOracle.publicKey],
      ]),
    });
    const end = await endExecuteOrderIx(program, {
      group,
      marginfiAccount: ownerAcc,
      executor: keeper.wallet.publicKey,
      order,
      executeRecord,
      feeRecipient: keeper.wallet.publicKey,
      // The closed borrow leg drops out of the observation set entirely, oracle included.
      remaining: composeRemainingAccounts([
        [solBank, oracles.wsolOracle.publicKey],
      ]),
    });

    await keeper.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(start, repay, withdraw, end),
    );

    assert.isNull(
      await program.account.order.fetchNullable(order),
      "the order should be consumed by its execution",
    );
    const acc = await program.account.marginfiAccount.fetch(ownerAcc);
    assert.isUndefined(
      acc.lendingAccount.balances.find(
        (b: any) => b.active !== 0 && b.bankPk.equals(usdcBank),
      ),
      "the borrow leg should be closed",
    );
  });
});
