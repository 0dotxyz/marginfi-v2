import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  banksClient,
  groupAdmin,
  marginfiGroup,
  oracles,
  users,
} from "./rootHooks";
import { tokenANative, usdcNative } from "./utils/token-utils";
import type { MockUser } from "./utils/mocks";
import {
  configureBank,
  configureBankRateLimits,
  configureGroupRateLimits,
  updateGroupRateLimiter,
} from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  withdrawIx,
  repayIx,
} from "./utils/user-instructions";
import {
  assertBNApproximately,
  assertBNEqual,
  expectFailedTxWithError,
  expectFailedTxWithMessage,
} from "./utils/genericTests";
import {
  advanceBankrunClock,
  getBankrunTime,
  processBankrunTransaction,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assert } from "chai";
import { blankBankConfigOptRaw } from "./utils/types";
import { dummyIx } from "./utils/bankrunConnection";

const RATE_LIMIT_ACCOUNT = "rate_limit_account";
const WITHDRAW_ACCOUNT = "withdraw_account";
const HOURLY_WINDOW_SECONDS = 60 * 60;
const DAILY_WINDOW_SECONDS = 24 * 60 * 60;

const usdcRemainingAccounts = (): PublicKey[] =>
  composeRemainingAccounts([
    [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
    [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
  ]);

const usdcOnlyRemainingAccounts = (): PublicKey[] =>
  composeRemainingAccounts([
    [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
  ]);

/**
 * Parse RateLimitFlowEvent events from transaction log messages.
 * Anchor emits events as "Program data: <base64>" in logs.
 */
type RateLimitFlowEvent = {
  group: PublicKey;
  bank: PublicKey;
  mint: PublicKey;
  flowDirection: number;
  nativeAmount: BN;
  mintDecimals: number;
  currentTimestamp: BN;
};

function parseRateLimitFlowEvents(
  program: Program<Marginfi>,
  logMessages: string[],
): RateLimitFlowEvent[] {
  const events: RateLimitFlowEvent[] = [];
  const DATA_PREFIX = "Program data: ";

  for (const log of logMessages) {
    if (!log.startsWith(DATA_PREFIX)) continue;
    const base64Data = log.slice(DATA_PREFIX.length);
    try {
      const decoded = program.coder.events.decode(base64Data);
      if (decoded && decoded.name === "rateLimitFlowEvent") {
        events.push(decoded.data as unknown as RateLimitFlowEvent);
      }
    } catch {
      // Not an event we care about
    }
  }
  return events;
}

/**
 * Aggregate RateLimitFlowEvent events into total inflow/outflow USD amounts.
 * In production, the admin would use oracle prices to convert native amounts to USD.
 * For testing with USDC ($1 = 1 USDC), native amount / 10^decimals = USD value.
 */
function aggregateFlowEvents(
  events: RateLimitFlowEvent[],
): { totalOutflowUsd: number; totalInflowUsd: number } {
  let totalOutflowUsd = 0;
  let totalInflowUsd = 0;

  for (const event of events) {
    const usdValue = event.nativeAmount.toNumber() / Math.pow(10, event.mintDecimals);
    if (event.flowDirection === 0) {
      totalOutflowUsd += usdValue;
    } else {
      totalInflowUsd += usdValue;
    }
  }

  return { totalOutflowUsd, totalInflowUsd };
}

async function getCurrentBankrunSlot(): Promise<BN> {
  const clock = await bankrunContext.banksClient.getClock();
  return new BN(clock.slot.toString());
}

let program: Program<Marginfi>;
let rateLimitAccount: PublicKey | null = null;
let withdrawAccount: PublicKey | null = null;
let rateLimitUser: MockUser;

describe("Rate limiter", () => {
  before(async () => {
    program = bankrunProgram;
    const user = users[2];
    assert.ok(user, "rate limit user (users[2]) must exist");
    rateLimitUser = user;

    // Initialize rate limit account (for borrow tests)
    if (!rateLimitUser.accounts.has(RATE_LIMIT_ACCOUNT)) {
      const accountKeypair = Keypair.generate();
      rateLimitUser.accounts.set(RATE_LIMIT_ACCOUNT, accountKeypair.publicKey);
      rateLimitAccount = accountKeypair.publicKey;

      await userProgram().provider.sendAndConfirm(
        new Transaction().add(
          await accountInit(userProgram(), {
            marginfiGroup: marginfiGroup.publicKey,
            marginfiAccount: accountKeypair.publicKey,
            authority: rateLimitUser.wallet.publicKey,
            feePayer: rateLimitUser.wallet.publicKey,
          }),
        ),
        [accountKeypair],
      );
    } else {
      const existing = rateLimitUser.accounts.get(RATE_LIMIT_ACCOUNT);
      assert.ok(existing, "rate limit account missing from accounts map");
      rateLimitAccount = existing;
    }

    // Initialize withdraw account (for withdraw tests - needs USDC deposits)
    if (!rateLimitUser.accounts.has(WITHDRAW_ACCOUNT)) {
      const accountKeypair = Keypair.generate();
      rateLimitUser.accounts.set(WITHDRAW_ACCOUNT, accountKeypair.publicKey);
      withdrawAccount = accountKeypair.publicKey;

      await userProgram().provider.sendAndConfirm(
        new Transaction().add(
          await accountInit(userProgram(), {
            marginfiGroup: marginfiGroup.publicKey,
            marginfiAccount: accountKeypair.publicKey,
            authority: rateLimitUser.wallet.publicKey,
            feePayer: rateLimitUser.wallet.publicKey,
          }),
        ),
        [accountKeypair],
      );
    } else {
      const existing = rateLimitUser.accounts.get(WITHDRAW_ACCOUNT);
      assert.ok(existing, "withdraw account missing from accounts map");
      withdrawAccount = existing;
    }

    // Prior suites can leave restrictive caps; raise these so deposits in this suite are deterministic.
    const highCapacity = new BN("1000000000000000");
    const usdcCapConfig = blankBankConfigOptRaw();
    usdcCapConfig.depositLimit = highCapacity;
    usdcCapConfig.totalAssetValueInitLimit = highCapacity;
    const tokenACapConfig = blankBankConfigOptRaw();
    tokenACapConfig.depositLimit = highCapacity;
    tokenACapConfig.totalAssetValueInitLimit = highCapacity;

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBank(groupAdmin.mrgnProgram, {
          bank: bankKeypairUsdc.publicKey,
          bankConfigOpt: usdcCapConfig,
        }),
        await configureBank(groupAdmin.mrgnProgram, {
          bank: bankKeypairA.publicKey,
          bankConfigOpt: tokenACapConfig,
        }),
      ),
    );

    // Deposit Token A collateral to rate limit account (for borrow tests)
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: requireRateLimitAccount(),
          bank: bankKeypairA.publicKey,
          tokenAccount: rateLimitUser.tokenAAccount,
          amount: tokenANative(5),
          depositUpToLimit: false,
        }),
      ),
    );

    // Deposit USDC collateral to withdraw account (for withdraw tests)
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: requireWithdrawAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          amount: usdcNative(20),
          depositUpToLimit: false,
        }),
      ),
    );
  });

  const requireRateLimitAccount = (): PublicKey => {
    assert.ok(rateLimitAccount, "rate limit account not initialized");
    return rateLimitAccount!;
  };

  const requireWithdrawAccount = (): PublicKey => {
    assert.ok(withdrawAccount, "withdraw account not initialized");
    return withdrawAccount!;
  };

  const userProgram = (): Program<Marginfi> => {
    const prog = rateLimitUser.mrgnProgram;
    assert.ok(prog, "rate limit user program not initialized");
    return prog!;
  };

  /**
   * Borrow USDC from the rate limit account
   */
  const borrowUsdc = async (amount: BN): Promise<void> => {
    const prog = userProgram();
    await prog.provider.sendAndConfirm(
      new Transaction().add(
        dummyIx(prog.provider.publicKey, users[0].wallet.publicKey),
        await borrowIx(prog, {
          marginfiAccount: requireRateLimitAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcRemainingAccounts(),
          amount,
        }),
      ),
    );
  };

  /**
   * Borrow USDC and return parsed RateLimitFlowEvents from the transaction logs
   */
  const borrowUsdcWithEvents = async (amount: BN): Promise<RateLimitFlowEvent[]> => {
    const prog = userProgram();
    const tx = new Transaction().add(
      dummyIx(prog.provider.publicKey, users[0].wallet.publicKey),
      await borrowIx(prog, {
        marginfiAccount: requireRateLimitAccount(),
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: rateLimitUser.usdcAccount,
        remaining: usdcRemainingAccounts(),
        amount,
      }),
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [rateLimitUser.wallet, users[0].wallet],
    );
    return parseRateLimitFlowEvents(prog, result.logMessages);
  };

  /**
   * Repay USDC to the rate limit account
   */
  const repayUsdc = async (amount: BN): Promise<void> => {
    const prog = userProgram();
    await prog.provider.sendAndConfirm(
      new Transaction().add(
        dummyIx(prog.provider.publicKey, users[0].wallet.publicKey),
        await repayIx(prog, {
          marginfiAccount: requireRateLimitAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcRemainingAccounts(),
          amount,
        }),
      ),
    );
  };

  /**
   * Repay USDC and return parsed RateLimitFlowEvents from the transaction logs
   */
  const repayUsdcWithEvents = async (amount: BN): Promise<RateLimitFlowEvent[]> => {
    const prog = userProgram();
    const tx = new Transaction().add(
      dummyIx(prog.provider.publicKey, users[0].wallet.publicKey),
      await repayIx(prog, {
        marginfiAccount: requireRateLimitAccount(),
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: rateLimitUser.usdcAccount,
        remaining: usdcRemainingAccounts(),
        amount,
      }),
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [rateLimitUser.wallet, users[0].wallet],
    );
    return parseRateLimitFlowEvents(prog, result.logMessages);
  };

  const adminUpdateGroupRateLimiter = async (args: {
    outflowUsd?: BN;
    inflowUsd?: BN;
  }) => {
    const groupState = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    const updateSeq = groupState.rateLimiterLastAdminUpdateSeq.add(new BN(1));
    const eventStartSlot = groupState.rateLimiterLastAdminUpdateSlot;
    const eventEndSlot = await getCurrentBankrunSlot();

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await updateGroupRateLimiter(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          outflowUsd: args.outflowUsd ?? null,
          inflowUsd: args.inflowUsd ?? null,
          updateSeq,
          eventStartSlot,
          eventEndSlot,
        }),
      ),
    );
  };

  /**
   * Configure both bank and group rate limits in a single transaction
   */
  const setRateLimits = async (args: {
    bank?: PublicKey;
    bankHourly?: BN | null;
    bankDaily?: BN | null;
    groupHourly?: BN | null;
    groupDaily?: BN | null;
  }): Promise<void> => {
    const bankKey = args.bank ?? bankKeypairUsdc.publicKey;
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankRateLimits(groupAdmin.mrgnProgram, {
          group: marginfiGroup.publicKey,
          bank: bankKey,
          hourlyMaxOutflow: args.bankHourly ?? null,
          dailyMaxOutflow: args.bankDaily ?? null,
        }),
        await configureGroupRateLimits(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          hourlyMaxOutflowUsd: args.groupHourly ?? null,
          dailyMaxOutflowUsd: args.groupDaily ?? null,
        }),
      ),
    );
  };

  /**
   * Advance the bankrun clock and optionally refresh oracles
   */
  const advanceClock = async (
    seconds: number,
    refreshOracles: boolean,
  ): Promise<void> => {
    await advanceBankrunClock(bankrunContext, seconds);

    if (refreshOracles) {
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    }
  };

  it("(admin) configures bank + group rate limits and partial updates preserve existing", async () => {
    // Initial configuration
    await setRateLimits({
      bankHourly: usdcNative(50),
      bankDaily: usdcNative(100),
      groupHourly: new BN(50),
      groupDaily: new BN(200),
    });

    const [bank, group] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      program.account.marginfiGroup.fetch(marginfiGroup.publicKey),
    ]);
    const now = await getBankrunTime(bankrunContext);

    // Verify bank rate limiter configuration
    assertBNEqual(bank.rateLimiter.hourly.maxOutflow, usdcNative(50));
    assertBNEqual(bank.rateLimiter.daily.maxOutflow, usdcNative(100));
    assertBNEqual(
      bank.rateLimiter.hourly.windowDuration,
      HOURLY_WINDOW_SECONDS,
    );
    assertBNEqual(bank.rateLimiter.daily.windowDuration, DAILY_WINDOW_SECONDS);
    assertBNApproximately(bank.rateLimiter.hourly.windowStart, now, 2);
    assertBNApproximately(bank.rateLimiter.daily.windowStart, now, 2);
    assertBNEqual(bank.rateLimiter.hourly.prevWindowOutflow, 0);
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, 0);

    // Verify group rate limiter configuration
    assertBNEqual(group.rateLimiter.hourly.maxOutflow, 50);
    assertBNEqual(group.rateLimiter.daily.maxOutflow, 200);
    assertBNEqual(
      group.rateLimiter.hourly.windowDuration,
      HOURLY_WINDOW_SECONDS,
    );
    assertBNEqual(group.rateLimiter.daily.windowDuration, DAILY_WINDOW_SECONDS);
    assertBNApproximately(group.rateLimiter.hourly.windowStart, now, 2);
    assertBNApproximately(group.rateLimiter.daily.windowStart, now, 2);
    assertBNEqual(group.rateLimiter.hourly.prevWindowOutflow, 0);
    assertBNEqual(group.rateLimiter.hourly.curWindowOutflow, 0);

    // Partial update: only change bankHourly and groupDaily, preserve others with null
    await setRateLimits({
      bankHourly: usdcNative(75),
      bankDaily: null,
      groupHourly: null,
      groupDaily: new BN(300),
    });

    const [bankAfter, groupAfter] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      program.account.marginfiGroup.fetch(marginfiGroup.publicKey),
    ]);

    assertBNEqual(bankAfter.rateLimiter.hourly.maxOutflow, usdcNative(75)); // updated
    assertBNEqual(bankAfter.rateLimiter.daily.maxOutflow, usdcNative(100)); // preserved
    assertBNEqual(groupAfter.rateLimiter.hourly.maxOutflow, 50); // preserved
    assertBNEqual(groupAfter.rateLimiter.daily.maxOutflow, 300); // updated
  });

  it("(user 2) bank hourly limit blocks excess outflow", async () => {
    const bankHourlyLimit = usdcNative(1);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: new BN(100),
      groupDaily: new BN(0),
    });

    await borrowUsdc(bankHourlyLimit);

    const bankAfterBorrow = await program.account.bank.fetch(
      bankKeypairUsdc.publicKey,
    );
    assertBNEqual(
      bankAfterBorrow.rateLimiter.hourly.curWindowOutflow,
      bankHourlyLimit,
    );

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(new BN(1));
    }, "Bank hourly rate limit exceeded");
  });

  it("(user 2) bank hourly limit blocks excess withdraw", async () => {
    await setRateLimits({
      bankHourly: usdcNative(2),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(userProgram(), {
          marginfiAccount: requireWithdrawAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcOnlyRemainingAccounts(),
          amount: usdcNative(1),
        }),
      ),
    );

    await expectFailedTxWithMessage(async () => {
      await userProgram().provider.sendAndConfirm(
        new Transaction().add(
          await withdrawIx(userProgram(), {
            marginfiAccount: requireWithdrawAccount(),
            bank: bankKeypairUsdc.publicKey,
            tokenAccount: rateLimitUser.usdcAccount,
            remaining: usdcOnlyRemainingAccounts(),
            amount: usdcNative(2),
          }),
        ),
      );
    }, "Bank hourly rate limit exceeded");
  });

  it("(user 2) group hourly limit blocks borrow - event-driven admin update flow", async () => {
    // Set high bank limit so only group limit is the constraint
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(10),
      groupDaily: new BN(0),
    });

    // Borrow 5 USDC and capture emitted events
    const borrowEvents = await borrowUsdcWithEvents(usdcNative(5));

    // Verify a RateLimitFlowEvent was emitted with the correct values
    assert.equal(borrowEvents.length, 1, "Expected 1 RateLimitFlowEvent");
    assert.equal(borrowEvents[0].flowDirection, 0, "Expected outflow (0)");
    assertBNEqual(borrowEvents[0].nativeAmount, usdcNative(5));
    assert.equal(borrowEvents[0].mintDecimals, 6, "USDC has 6 decimals");
    assert.ok(borrowEvents[0].currentTimestamp.toNumber() > 0, "Timestamp should be set");

    // Admin aggregates events and updates group rate limiter (simulating off-chain flow)
    const { totalOutflowUsd } = aggregateFlowEvents(borrowEvents);
    assert.equal(totalOutflowUsd, 5, "5 USDC at $1 = $5 outflow");

    await adminUpdateGroupRateLimiter({ outflowUsd: new BN(totalOutflowUsd) });

    const groupAfterUpdate = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(groupAfterUpdate.rateLimiter.hourly.curWindowOutflow, 5);

    // Try to borrow 8 more - should fail (5 + 8 = 13 > 10 limit)
    await expectFailedTxWithError(
      async () => {
        await borrowUsdc(usdcNative(8));
      },
      "GroupHourlyRateLimitExceeded",
      6117,
    );

    // Repay 3 USDC and capture events
    const repayEvents = await repayUsdcWithEvents(usdcNative(3));

    assert.equal(repayEvents.length, 1, "Expected 1 RateLimitFlowEvent for repay");
    assert.equal(repayEvents[0].flowDirection, 1, "Expected inflow (1)");
    assertBNEqual(repayEvents[0].nativeAmount, usdcNative(3));

    // Admin aggregates the inflow and updates group rate limiter
    const { totalInflowUsd } = aggregateFlowEvents(repayEvents);
    assert.equal(totalInflowUsd, 3, "3 USDC at $1 = $3 inflow");

    await adminUpdateGroupRateLimiter({ inflowUsd: new BN(totalInflowUsd) });

    const groupAfterInflow = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(groupAfterInflow.rateLimiter.hourly.curWindowOutflow, 2);

    // Now borrow 3 more should succeed (2 + 3 = 5 < 10 limit)
    await borrowUsdc(usdcNative(3));

    // Repay what we borrowed
    await repayUsdc(usdcNative(5));
  });

  it("(user 2) bank daily limit blocks excess outflow", async () => {
    const bankDailyLimit = usdcNative(2);

    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: bankDailyLimit,
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await borrowUsdc(bankDailyLimit);

    const bankAfter = await program.account.bank.fetch(
      bankKeypairUsdc.publicKey,
    );
    assertBNEqual(bankAfter.rateLimiter.daily.curWindowOutflow, bankDailyLimit);

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(1));
    }, "Bank daily rate limit exceeded");

    // Repay to keep account healthy for subsequent tests
    await repayUsdc(bankDailyLimit);
  });

  it("(user 2) group daily limit blocks borrow after admin records outflow", async () => {
    // Group daily limit is in USD, so 5 = $5
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(5),
    });

    // Borrow 3 USDC and capture events
    const events = await borrowUsdcWithEvents(usdcNative(3));
    const { totalOutflowUsd } = aggregateFlowEvents(events);
    assert.equal(totalOutflowUsd, 3, "3 USDC at $1 = $3 outflow");

    // Admin aggregates and updates group daily rate limiter
    await adminUpdateGroupRateLimiter({ outflowUsd: new BN(totalOutflowUsd) });

    const groupAfter = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(groupAfter.rateLimiter.daily.curWindowOutflow, 3);

    // Try to borrow 3 more - should fail (3 + 3 = 6 > 5 limit)
    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(3));
    }, "Group daily rate limit exceeded");

    // Repay to keep account healthy
    await repayUsdc(usdcNative(3));
  });

  it("(user 2) deposit offsets withdraw outflow", async () => {
    await setRateLimits({
      bankHourly: usdcNative(1),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    // Deposit 1 USDC (inflow)
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: requireWithdrawAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          amount: usdcNative(1),
          depositUpToLimit: false,
        }),
      ),
    );

    // Withdraw 2 USDC (outflow)
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(userProgram(), {
          marginfiAccount: requireWithdrawAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcOnlyRemainingAccounts(),
          amount: usdcNative(2),
        }),
      ),
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    // Net outflow = -1 (deposit) + 2 (withdraw) = 1
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, usdcNative(1));
  });

  it("(admin) disabling limits removes enforcement", async () => {
    // Set bank hourly limit to 2 USDC
    await setRateLimits({
      bankHourly: usdcNative(2),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await borrowUsdc(usdcNative(2));

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(1));
    }, "Bank hourly rate limit exceeded");

    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assertBNEqual(bank.rateLimiter.hourly.maxOutflow, 0);

    // Now borrow succeeds (limit disabled)
    await borrowUsdc(usdcNative(3));

    // Repay to keep account healthy for subsequent tests
    await repayUsdc(usdcNative(5));
  });

  it("(user 2) group rate limiter is read-only during user instructions", async () => {
    // Configure group hourly limit
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(100),
      groupDaily: new BN(0),
    });

    const groupBefore = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    const outflowBefore =
      groupBefore.rateLimiter.hourly.curWindowOutflow.toNumber();

    // Borrow 5 USDC - this should succeed (within limit) but NOT update group state
    await borrowUsdc(usdcNative(5));

    const groupAfterBorrow = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    // Group rate limiter should NOT have changed (it's read-only during user instructions)
    assertBNEqual(
      groupAfterBorrow.rateLimiter.hourly.curWindowOutflow,
      outflowBefore,
    );

    // Repay 5 USDC - group state should also remain unchanged
    await repayUsdc(usdcNative(5));

    const groupAfterRepay = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(
      groupAfterRepay.rateLimiter.hourly.curWindowOutflow,
      outflowBefore,
    );
  });

  it("(admin) batched event aggregation - multiple actions then single admin update", async () => {
    // Simulates production flow: multiple user actions happen over a period,
    // admin aggregates all emitted events and calls updateGroupRateLimiter once.
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(10),
      groupDaily: new BN(0),
    });

    const allEvents: RateLimitFlowEvent[] = [];

    // Action 1: Borrow 3 USDC
    const events1 = await borrowUsdcWithEvents(usdcNative(3));
    allEvents.push(...events1);

    // Action 2: Borrow 2 USDC
    const events2 = await borrowUsdcWithEvents(usdcNative(2));
    allEvents.push(...events2);

    // Action 3: Repay 1 USDC
    const events3 = await repayUsdcWithEvents(usdcNative(1));
    allEvents.push(...events3);

    // Action 4: Borrow 1 USDC
    const events4 = await borrowUsdcWithEvents(usdcNative(1));
    allEvents.push(...events4);

    // Verify all events were captured
    assert.equal(allEvents.length, 4, "Should have 4 events total");

    // Admin aggregates all events at once (as would happen in production every ~10 minutes)
    const { totalOutflowUsd, totalInflowUsd } = aggregateFlowEvents(allEvents);
    assert.equal(totalOutflowUsd, 6, "3 + 2 + 1 = 6 USDC outflow");
    assert.equal(totalInflowUsd, 1, "1 USDC inflow");

    // Group state should still be unchanged (read-only during user instructions)
    const groupBeforeAdmin = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(groupBeforeAdmin.rateLimiter.hourly.curWindowOutflow, 0);

    // Admin submits both outflow and inflow in a single update
    await adminUpdateGroupRateLimiter({
      outflowUsd: new BN(totalOutflowUsd),
      inflowUsd: new BN(totalInflowUsd),
    });

    // Net outflow = 6 - 1 = 5
    const groupAfterAdmin = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assertBNEqual(groupAfterAdmin.rateLimiter.hourly.curWindowOutflow, 5);

    // Borrow 2 more USDC and capture events for a second batch
    const events5 = await borrowUsdcWithEvents(usdcNative(2));
    const batch2 = aggregateFlowEvents(events5);
    assert.equal(batch2.totalOutflowUsd, 2);

    // Admin does second batch update
    await adminUpdateGroupRateLimiter({
      outflowUsd: new BN(batch2.totalOutflowUsd),
    });

    const groupAfterBatch2 = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    // 5 (previous) + 2 (new) = 7
    assertBNEqual(groupAfterBatch2.rateLimiter.hourly.curWindowOutflow, 7);

    // Borrow 4 more - should fail the read-only group capacity check (7 + 4 = 11 > 10)
    await expectFailedTxWithError(
      async () => {
        await borrowUsdc(usdcNative(4));
      },
      "GroupHourlyRateLimitExceeded",
      6117,
    );

    // Repay what we borrowed to keep the account healthy for subsequent tests
    await repayUsdc(usdcNative(7));
  });

  it("(user 2) hourly window decays and resets", async () => {
    const bankHourlyLimit = usdcNative(1);
    const decayedBorrow = bankHourlyLimit.div(new BN(HOURLY_WINDOW_SECONDS));

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    // Exhaust limit
    await borrowUsdc(bankHourlyLimit);

    await advanceClock(HOURLY_WINDOW_SECONDS, true);

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(new BN(1));
    }, "Bank hourly rate limit exceeded");

    // Advance 1 more second (small decay)
    await advanceClock(1, true);

    await borrowUsdc(decayedBorrow);

    await advanceClock(HOURLY_WINDOW_SECONDS * 2 + 1, true);

    await borrowUsdc(bankHourlyLimit);

    const bankAfter = await program.account.bank.fetch(
      bankKeypairUsdc.publicKey,
    );
    assertBNEqual(
      bankAfter.rateLimiter.hourly.curWindowOutflow,
      bankHourlyLimit,
    );
    assertBNEqual(bankAfter.rateLimiter.hourly.prevWindowOutflow, 0);
  });

  it("(user 2) skips rate limits during flashloan", async () => {
    const bankHourlyLimit = usdcNative(1);
    const groupHourlyLimit = new BN(1);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: groupHourlyLimit,
      groupDaily: new BN(0),
    });

    const prog = userProgram();

    // Flashloan start instruction
    const startIx = await prog.methods
      .lendingAccountStartFlashloan(new BN(2))
      .accounts({
        marginfiAccount: requireRateLimitAccount(),
      })
      .instruction();

    const borrowIxLocal = await borrowIx(prog, {
      marginfiAccount: requireRateLimitAccount(),
      bank: bankKeypairUsdc.publicKey,
      tokenAccount: rateLimitUser.usdcAccount,
      remaining: usdcRemainingAccounts(),
      amount: usdcNative(5), // 5x the hourly limit
    });

    // Flashloan end instruction
    const endRemaining = usdcRemainingAccounts().map((pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    }));

    const endIx = await prog.methods
      .lendingAccountEndFlashloan()
      .accounts({
        marginfiAccount: requireRateLimitAccount(),
      })
      .remainingAccounts(endRemaining)
      .instruction();

    await prog.provider.sendAndConfirm(
      new Transaction().add(startIx, borrowIxLocal, endIx),
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assert.ok(
      bank.rateLimiter.hourly.curWindowOutflow.toNumber() <=
        bankHourlyLimit.toNumber() * 10,
      "Rate limiter should not have excessive outflow after flashloan",
    );
  });
});
