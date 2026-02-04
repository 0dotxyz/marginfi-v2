import { BN, Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  marginfiGroup,
  oracles,
  users,
} from "./rootHooks";
import type { MockUser } from "./utils/mocks";
import {
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


const RATE_LIMIT_ACCOUNT = "rate_limit_account";
const HOURLY_WINDOW_SECONDS = 60 * 60;
const DAILY_WINDOW_SECONDS = 24 * 60 * 60;


const toNative = (amount: number, decimals: number): BN =>
  new BN(amount).mul(new BN(10).pow(new BN(decimals)));

const usdcNative = (amount: number): BN =>
  toNative(amount, ecosystem.usdcDecimals);

const tokenANative = (amount: number): BN =>
  toNative(amount, ecosystem.tokenADecimals);


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
let rateLimitUser: MockUser;


describe("Rate limiter", () => {
  before(() => {
    program = bankrunProgram;
    const user = users[2];
    assert.ok(user, "rate limit user (users[2]) must exist");
    rateLimitUser = user;
  });

  const requireRateLimitAccount = (): PublicKey => {
    assert.ok(rateLimitAccount, "rate limit account not initialized");
    return rateLimitAccount!;
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


  it("(user 2) initializes a rate limit account and deposits collateral", async () => {
    // Initialize account if not already done
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

    const account = await program.account.marginfiAccount.fetch(
      requireRateLimitAccount()
    );
    assert.ok(account, "marginfi account should exist after initialization");
  });

  it("(admin) configures bank + group rate limits", async () => {
    const bankHourlyLimit = usdcNative(50);
    const bankDailyLimit = usdcNative(100);
    const groupHourlyLimit = new BN(50);
    const groupDailyLimit = new BN(200);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: bankDailyLimit,
      groupHourly: groupHourlyLimit,
      groupDaily: groupDailyLimit,
    });

    const [bank, group] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      program.account.marginfiGroup.fetch(marginfiGroup.publicKey),
    ]);
    const now = await getBankrunTime(bankrunContext);

    // Verify bank rate limiter configuration
    assertBNEqual(bank.rateLimiter.hourly.maxOutflow, bankHourlyLimit);
    assertBNEqual(bank.rateLimiter.daily.maxOutflow, bankDailyLimit);
    assertBNEqual(bank.rateLimiter.hourly.windowDuration, HOURLY_WINDOW_SECONDS);
    assertBNEqual(bank.rateLimiter.daily.windowDuration, DAILY_WINDOW_SECONDS);
    assertBNApproximately(bank.rateLimiter.hourly.windowStart, now, 2);
    assertBNApproximately(bank.rateLimiter.daily.windowStart, now, 2);
    assertBNEqual(bank.rateLimiter.hourly.prevWindowOutflow, 0);
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, 0);

    // Verify group rate limiter configuration
    assertBNEqual(group.rateLimiter.hourly.maxOutflow, groupHourlyLimit);
    assertBNEqual(group.rateLimiter.daily.maxOutflow, groupDailyLimit);
    assertBNEqual(
      group.rateLimiter.hourly.windowDuration,
      HOURLY_WINDOW_SECONDS
    );
    assertBNEqual(group.rateLimiter.daily.windowDuration, DAILY_WINDOW_SECONDS);
    assertBNApproximately(group.rateLimiter.hourly.windowStart, now, 2);
    assertBNApproximately(group.rateLimiter.daily.windowStart, now, 2);
    assertBNEqual(group.rateLimiter.hourly.prevWindowOutflow, 0);
    assertBNEqual(group.rateLimiter.hourly.curWindowOutflow, 0);
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
    const bankHourlyLimit = usdcNative(2);
    const initialDeposit = usdcNative(5);
    const firstWithdraw = usdcNative(1);
    const secondWithdraw = usdcNative(2);

    const withdrawAccount = Keypair.generate();
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(userProgram(), {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: withdrawAccount.publicKey,
          authority: rateLimitUser.wallet.publicKey,
          feePayer: rateLimitUser.wallet.publicKey,
        })
      ),
      [withdrawAccount]
    );

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: withdrawAccount.publicKey,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          amount: initialDeposit,
          depositUpToLimit: false,
        })
      )
    );

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(userProgram(), {
          marginfiAccount: withdrawAccount.publicKey,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcOnlyRemainingAccounts(),
          amount: firstWithdraw,
        })
      )
    );

    await expectFailedTxWithMessage(async () => {
      await userProgram().provider.sendAndConfirm(
        new Transaction().add(
          await withdrawIx(userProgram(), {
            marginfiAccount: withdrawAccount.publicKey,
            bank: bankKeypairUsdc.publicKey,
            tokenAccount: rateLimitUser.usdcAccount,
            remaining: usdcOnlyRemainingAccounts(),
            amount: secondWithdraw,
          })
        )
      );
    }, "Bank hourly rate limit exceeded");
  });

  it("(user 2) group hourly limit offsets inflows", async () => {
    const bankHourlyLimit = usdcNative(1_000);
    const groupHourlyLimit = new BN(20);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: groupHourlyLimit,
      groupDaily: new BN(0),
    });

    await borrowUsdc(usdcNative(15));

    await expectFailedTxWithError(
      async () => {
        await borrowUsdc(usdcNative(10));
      },
      "GroupHourlyRateLimitExceeded",
      6106
    );

    await repayUsdc(usdcNative(5));

    await borrowUsdc(usdcNative(5));

    const group = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assert.ok(
      group.rateLimiter.hourly.curWindowOutflow.toNumber() <= 20,
      "Group outflow should not exceed limit"
    );
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
    const groupDailyLimit = new BN(10);

    await setRateLimits({
      bankHourly: usdcNative(1_000),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: groupDailyLimit,
    });

    await borrowUsdc(usdcNative(10));

    const [groupAfter, bankAfter] = await Promise.all([
      program.account.marginfiGroup.fetch(marginfiGroup.publicKey),
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
    ]);
    const price = wrappedI80F48toBigNumber(
      bankAfter.cache.lastOraclePrice
    ).toNumber();
    const maxExpectedOutflow = Math.floor(price * 10);
    const actualOutflow =
      groupAfter.rateLimiter.daily.curWindowOutflow.toNumber();
    assert.ok(
      actualOutflow > 0,
      "Group daily outflow should be recorded after borrow"
    );
    assert.ok(
      actualOutflow <= maxExpectedOutflow,
      "Group daily outflow should not exceed oracle-priced borrow amount"
    );

    await expectFailedTxWithMessage(async () => {
      await borrowUsdc(usdcNative(5));
    }, "Group daily rate limit exceeded");
  });

  it("(user 2) deposit offsets withdraw outflow", async () => {
    const bankHourlyLimit = usdcNative(1);
    const depositAmount = usdcNative(1);
    const withdrawAmount = usdcNative(2);
    const collateralAmount = usdcNative(5);

    const offsetAccount = Keypair.generate();
    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(userProgram(), {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: offsetAccount.publicKey,
          authority: rateLimitUser.wallet.publicKey,
          feePayer: rateLimitUser.wallet.publicKey,
        })
      ),
      [offsetAccount]
    );

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: offsetAccount.publicKey,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          amount: collateralAmount,
          depositUpToLimit: false,
        })
      )
    );

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(userProgram(), {
          marginfiAccount: offsetAccount.publicKey,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          amount: depositAmount,
          depositUpToLimit: false,
        })
      )
    );

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(userProgram(), {
          marginfiAccount: offsetAccount.publicKey,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: rateLimitUser.usdcAccount,
          remaining: usdcOnlyRemainingAccounts(),
          amount: withdrawAmount,
        })
      )
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const expectedNetOutflow = depositAmount.neg().add(withdrawAmount);
    assertBNEqual(bank.rateLimiter.hourly.curWindowOutflow, expectedNetOutflow);
  });

  it("(admin) partial updates preserve existing windows", async () => {
    const bankHourlyLimit = usdcNative(5);
    const bankDailyLimit = usdcNative(10);
    const groupHourlyLimit = new BN(5);
    const groupDailyLimit = new BN(10);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: bankDailyLimit,
      groupHourly: groupHourlyLimit,
      groupDaily: groupDailyLimit,
    });

    const bankHourlyUpdate = usdcNative(7);
    const groupDailyUpdate = new BN(12);
    await setRateLimits({
      bankHourly: bankHourlyUpdate,
      bankDaily: null, // preserve existing
      groupHourly: null, // preserve existing
      groupDaily: groupDailyUpdate,
    });

    const [bank, group] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      program.account.marginfiGroup.fetch(marginfiGroup.publicKey),
    ]);

    assertBNEqual(bank.rateLimiter.hourly.maxOutflow, bankHourlyUpdate);
    assertBNEqual(bank.rateLimiter.daily.maxOutflow, bankDailyLimit); // unchanged
    assertBNEqual(group.rateLimiter.hourly.maxOutflow, groupHourlyLimit); // unchanged
    assertBNEqual(group.rateLimiter.daily.maxOutflow, groupDailyUpdate);
  });

  it("(admin) disabling limits removes enforcement", async () => {
    const bankHourlyLimit = usdcNative(1);

    await setRateLimits({
      bankHourly: bankHourlyLimit,
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    await borrowUsdc(bankHourlyLimit);

    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assertBNEqual(bank.rateLimiter.hourly.maxOutflow, 0);

    await borrowUsdc(bankHourlyLimit.add(new BN(1)));
  });

  it("(user 2) uses cached price when oracles are omitted for inflows", async () => {
    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(1_000),
      groupDaily: new BN(0),
    });

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(userProgram(), {
          group: marginfiGroup.publicKey,
          bank: bankKeypairUsdc.publicKey,
          remaining: [oracles.usdcOracle.publicKey],
        })
      )
    );

    await borrowUsdc(usdcNative(1));
    await repayUsdc(usdcNative(1));

    const group = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assert.ok(group, "Group should exist after cached price operations");
  });

  it("(user 2) rejects stale cached price when oracles are omitted for inflows", async () => {
    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(1_000),
      groupDaily: new BN(0),
    });

    await userProgram().provider.sendAndConfirm(
      new Transaction().add(
        await pulseBankPrice(userProgram(), {
          group: marginfiGroup.publicKey,
          bank: bankKeypairUsdc.publicKey,
          remaining: [oracles.usdcOracle.publicKey],
        })
      )
    );

    const bank = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    await advanceClock(bank.config.oracleMaxAge + 1, false);

    await expectFailedTxWithMessage(async () => {
      await repayUsdc(usdcNative(1));
    }, "Invalid rate limit price");

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
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


  after(async () => {
    // Reset all rate limits to disabled state
    await setRateLimits({
      bankHourly: new BN(0),
      bankDaily: new BN(0),
      groupHourly: new BN(0),
      groupDaily: new BN(0),
    });

    // Also reset Token A bank limits
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await configureBankRateLimits(groupAdmin.mrgnProgram, {
          group: marginfiGroup.publicKey,
          bank: bankKeypairA.publicKey,
          hourlyMaxOutflow: new BN(0),
          dailyMaxOutflow: new BN(0),
        })
      )
    );
  });
});
