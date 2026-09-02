import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
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
  borrowIx,
  closeOrderIx,
  composeRemainingAccounts,
  depositIx,
  endExecuteOrderIx,
  InterestTriggerArgs,
  placeInterestOrderIx,
  placeOrderIx,
  pulseBankPrice,
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
/** Matches `INTEREST_MAX_WINDOW_SECONDS`. */
const MAX_WINDOW = 172_800;
/** Matches `INTEREST_DEFAULT_EXIT_BUDGET_SECONDS`. */
const DEFAULT_EXIT_BUDGET = 1_209_600;

/**
 * Advances the shared clock past an interest window, so this spec runs in the orders mocha process
 * (see the `all-tests` note in Anchor.toml) on its own group and banks.
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
    exitBudgetSeconds: number | null,
  ): InterestTriggerArgs => ({
    windowSeconds,
    exitBudgetSeconds,
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
    const common = {
      marginfiAccount: ownerAcc,
      authority: owner.wallet.publicKey,
      feePayer: owner.wallet.publicKey,
      bankKeys: bankKeys(),
      trigger: { stopLoss: { threshold: stopLossThreshold, maxSlippage } },
    };
    const ix = cfg
      ? await placeInterestOrderIx(program, { ...common, interest: cfg })
      : await placeOrderIx(program, common);
    await owner.mrgnProgram.provider.sendAndConfirm(new Transaction().add(ix));
    return deriveOrderPda(program.programId, ownerAcc, bankKeys())[0];
  };

  /** The keeper's execution: start, repay the USDC borrow, take SOL to cover it, end. */
  const sandwich = async () => {
    const [executeRecord] = deriveExecuteOrderPda(program.programId, order);
    const start = await startExecuteOrderIx(program, {
      group,
      marginfiAccount: ownerAcc,
      feePayer: keeper.wallet.publicKey,
      executor: keeper.wallet.publicKey,
      order,
      remaining: await remainingKeys(ownerAcc),
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
      // the repayment out of its own pocket and skims nothing from the position.
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
      remaining: composeRemainingAccounts([
        [solBank, oracles.wsolOracle.publicKey],
      ]),
    });
    return new Transaction().add(start, repay, withdraw, end);
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

    // The borrow priced the USDC bank, which took its first rate reading. The SOL lend leg has
    // only been deposited into, which prices nothing, so pulse it for its own.
    await owner.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(program, {
          bank: solBank,
          remaining: [oracles.wsolOracle.publicKey],
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
      6803,
    );
  });

  it("rejects a window over the ceiling - OrderInterestInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place(interest(MAX_WINDOW + 1, null));
      },
      "OrderInterestInvalidConfig",
      6803,
    );
  });

  it("rejects a zero exit budget - OrderInterestInvalidConfig", async () => {
    await expectFailedTxWithError(
      async () => {
        await place(interest(null, 0));
      },
      "OrderInterestInvalidConfig",
      6803,
    );
  });

  it("leaves the trigger off when no policy is given", async () => {
    const plain = await place(null);
    const fetched = await program.account.order.fetch(plain);
    assert.equal(fetched.interestFlags, 0);
    assert.equal(fetched.interestWindowSeconds, 0);
    assert.equal(fetched.interestExitBudgetSeconds, 0);

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

  it("carries the interest policy with no accounts beyond a plain order", async () => {
    order = await place(interest(MIN_WINDOW, null));
    const fetched = await program.account.order.fetch(order);

    assert.equal(fetched.interestFlags, 1);
    assert.equal(fetched.interestWindowSeconds, MIN_WINDOW);
    assert.equal(fetched.interestExitBudgetSeconds, DEFAULT_EXIT_BUDGET);
    assert.equal(fetched.interestMinNegativeApr, 0);
  });

  it("carries a live stop-loss and the carry trigger on one order", async () => {
    const fetched = await program.account.order.fetch(order);
    assert.equal(fetched.interestFlags, 1);
    assert.equal(
      toI80Scaled(fetched.stopLoss),
      toI80Scaled(stopLossThreshold),
      "the price threshold should survive alongside the carry policy",
    );
    assert.equal(fetched.maxSlippage, maxSlippage);
  });

  it("cannot execute before the banks hold a window of history - OrderInterestHistoryTooShort", async () => {
    await advance(MIN_WINDOW - 60);
    await expectFailedTxWithError(
      async () => {
        await keeper.mrgnProgram.provider.sendAndConfirm!(await sandwich());
      },
      "OrderInterestHistoryTooShort",
      6800,
    );
  });

  it("unwinds the pair through the keeper sandwich", async () => {
    // A full window since both banks' first readings. The borrow leg charged over it while the
    // idle lend leg earned nothing, so the measured carry is negative and any negative carry fires.
    await advance(120);
    await keeper.mrgnProgram.provider.sendAndConfirm!(await sandwich());

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
