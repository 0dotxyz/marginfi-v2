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
} from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  pulseBankPrice,
  withdrawIx,
  repayIx,
} from "./utils/user-instructions";
import {
  assertBNApproximately,
  assertBNEqual,
  expectFailedTxWithError,
  expectFailedTxWithMessage,
} from "./utils/genericTests";
import { advanceBankrunClock, getBankrunTime } from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assert } from "chai";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { blankBankConfigOptRaw } from "./utils/types";


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
          })
        ),
        [accountKeypair]
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
          })
        ),
        [accountKeypair]
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
        })
      )
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
        })
      )
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
        })
      )
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
        await borrowIx(prog, {
          marginfiAccount: requireRateLimitAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcRemainingAccounts(),
          amount,
        })
      )
    );
  };

  /**
   * Repay USDC to the rate limit account
   */
  const repayUsdc = async (amount: BN): Promise<void> => {
    const prog = userProgram();
    await prog.provider.sendAndConfirm(
      new Transaction().add(
        await repayIx(prog, {
          marginfiAccount: requireRateLimitAccount(),
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcRemainingAccounts(),
          amount,
        })
      )
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
        })
      )
    );
  };

  /**
   * Advance the bankrun clock and optionally refresh oracles
   */
  const advanceClock = async (
    seconds: number,
    refreshOracles: boolean
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
    assertBNEqual(bank.rateLimiter.hourly.windowDuration, HOURLY_WINDOW_SECONDS);
    assertBNEqual(bank.rateLimiter.daily.windowDuration, DAILY_WINDOW_SECONDS);
    assertBNApproximately(bank.rateLimiter.hourly.windowStart, now, 2);
    assertBNApproximately(bank.rateLimiter.daily.windowStart, now, 2);
    assertBNEqual(bank.rateLimiter.hourly.prevWindowOutflow, 0);
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, 0);

    // Verify group rate limiter configuration
    assertBNEqual(group.rateLimiter.hourly.maxOutflow, 50);
    assertBNEqual(group.rateLimiter.daily.maxOutflow, 200);
    assertBNEqual(group.rateLimiter.hourly.windowDuration, HOURLY_WINDOW_SECONDS);
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
      bankKeypairUsdc.publicKey
    );
    assertBNEqual(
      bankAfterBorrow.rateLimiter.hourly.curWindowOutflow,
      bankHourlyLimit
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
        })
      )
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
          })
        )
      );
    }, "Bank hourly rate limit exceeded");
  });

  it("(user 2) group hourly limit offsets inflows", async () => {
    // Set high bank limit so only group limit is the constraint
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(20),
      groupDaily: new BN(0),
    });

    // Borrow 15 USDC at $1 = 15 USD outflow
    await borrowUsdc(usdcNative(15));

    const groupAfterBorrow = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterBorrow.rateLimiter.hourly.curWindowOutflow, 15);

    // Try to borrow 10 more - should fail (15 + 10 = 25 > 20 limit)
    await expectFailedTxWithError(
      async () => {
        await borrowUsdc(usdcNative(10));
      },
      "GroupHourlyRateLimitExceeded",
      6117
    );

    // Repay 5 USDC at $1 = 5 USD inflow, reducing net outflow to 10
    await repayUsdc(usdcNative(5));

    const groupAfterRepay = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterRepay.rateLimiter.hourly.curWindowOutflow, 10);

    // Now borrow 5 more should succeed (10 + 5 = 15 < 20 limit)
    await borrowUsdc(usdcNative(5));

    const groupFinal = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupFinal.rateLimiter.hourly.curWindowOutflow, 15);
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
      bankKeypairUsdc.publicKey
    );
    assertBNEqual(
      bankAfter.rateLimiter.daily.curWindowOutflow,
      bankDailyLimit
    );

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(1));
    }, "Bank daily rate limit exceeded");
  });

  it("(user 2) group daily limit blocks excess outflow", async () => {
    // Group daily limit is in USD, so 10 = $10
    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(10),
    });

    // Borrow 10 USDC at $1 = 10 USD outflow (exactly at limit)
    await borrowUsdc(usdcNative(10));

    const groupAfter = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfter.rateLimiter.daily.curWindowOutflow, 10);

    // Try to borrow 5 more - should fail (10 + 5 = 15 > 10 limit)
    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(5));
    }, "Group daily rate limit exceeded");
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
        })
      )
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
        })
      )
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    // Net outflow = -1 (deposit) + 2 (withdraw) = 1
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, usdcNative(1));
  });

  it("(admin) disabling limits removes enforcement", async () => {
    // Set bank hourly limit to 1 USDC
    await setRateLimits({
      bankHourly: usdcNative(1),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await borrowUsdc(usdcNative(1));

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
    await borrowUsdc(usdcNative(1));
  });

  it("(user 2) uses cached price for inflows (repay uses bank cache, not fresh oracle)", async () => {
    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(1_000),
      groupDaily: new BN(0),
    });

    // Pulse bank to cache a fresh oracle price
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(userProgram(), {
          group: marginfiGroup.publicKey,
          bank: bankKeypairUsdc.publicKey,
          remaining: [oracles.usdcOracle.publicKey],
        })
      )
    );

    // Verify bank has a valid cached price
    const bankAfterPulse = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const cachedPrice = wrappedI80F48toBigNumber(bankAfterPulse.cache.lastOraclePrice).toNumber();
    assert.ok(cachedPrice > 0, "Bank should have a valid cached price after pulse");

    await borrowUsdc(usdcNative(1));

    const groupAfterBorrow = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterBorrow.rateLimiter.hourly.curWindowOutflow, 1);

    await repayUsdc(usdcNative(1));

    // Verify: inflow was applied directly (not tracked as untracked)
    const bankAfterRepay = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assertBNEqual(bankAfterRepay.rateLimiter.untrackedInflow, 0);

    // Verify: net outflow = 0 (borrow 1, repay 1)
    const groupAfterRepay = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterRepay.rateLimiter.hourly.curWindowOutflow, 0);
  });

  it("(user 2) tracks untracked inflows when price is stale, applies on pulse", async () => {
    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(1_000),
      groupDaily: new BN(0),
    });

    // Pulse bank price to get a fresh cached price
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(userProgram(), {
          group: marginfiGroup.publicKey,
          bank: bankKeypairUsdc.publicKey,
          remaining: [oracles.usdcOracle.publicKey],
        })
      )
    );

    const groupBefore = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    const outflowBefore = groupBefore.rateLimiter.hourly.curWindowOutflow.toNumber();

    // Advance clock past oracle max age (makes cached price stale)
    const bankBefore = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    await advanceClock(bankBefore.config.oracleMaxAge + 1, false);

    await repayUsdc(usdcNative(1));

    const bankAfterRepay = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assertBNEqual(bankAfterRepay.rateLimiter.untrackedInflow, usdcNative(1));

    const groupAfterRepay = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterRepay.rateLimiter.hourly.curWindowOutflow, outflowBefore);

    // Refresh oracles and pulse bank to apply untracked inflows
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(userProgram(), {
          group: marginfiGroup.publicKey,
          bank: bankKeypairUsdc.publicKey,
          remaining: [oracles.usdcOracle.publicKey],
        })
      )
    );

    // Untracked inflow should be applied (reset to 0)
    const bankAfterPulse = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assertBNEqual(bankAfterPulse.rateLimiter.untrackedInflow, 0);

    // Group outflow reduced by 1 USD (1 USDC at $1)
    const groupAfterPulse = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertBNEqual(groupAfterPulse.rateLimiter.hourly.curWindowOutflow, outflowBefore - 1);
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
      bankKeypairUsdc.publicKey
    );
    assertBNEqual(
      bankAfter.rateLimiter.hourly.curWindowOutflow,
      bankHourlyLimit
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
      new Transaction().add(startIx, borrowIxLocal, endIx)
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assert.ok(
      bank.rateLimiter.hourly.curWindowOutflow.toNumber() <=
        bankHourlyLimit.toNumber() * 10,
      "Rate limiter should not have excessive outflow after flashloan"
    );
  });
});
