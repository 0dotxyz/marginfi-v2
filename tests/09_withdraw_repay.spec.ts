import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import { Clock } from "solana-bankrun";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  assertBNApproximately,
  assertKeysEqual,
  expectFailedTxWithError,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  repayIx,
  withdrawEmissionsIx,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { u64MAX_BN } from "./utils/types";
import { getBankrunTime } from "./utils/tools";

let program: Program<Marginfi>;
let provider: BankrunProvider;

describe("Withdraw funds", () => {
  before(() => {
    provider = bankRunProvider;
    program = bankrunProgram;
  });

  const withdrawAmountTokenA = 0.1;
  const withdrawAmountTokenA_native = new BN(
    withdrawAmountTokenA * 10 ** ecosystem.tokenADecimals
  );

  const repayAmountUsdc = 0.1;
  const repayAmountUsdc_native = new BN(
    repayAmountUsdc * 10 ** ecosystem.usdcDecimals
  );

  /**
   * Advance bankrun clock by specified seconds and refresh oracles.
   * Required for emissions to accrue since bankrun clock is frozen.
   *
   * Note: setTimeout does NOT advance bankrun's Clock sysvar - it's frozen.
   * We must use setClock() to advance blockchain time explicitly.
   */
  const advanceClockAndRefreshOracles = async (seconds: number = 2) => {
    const clock = await banksClient.getClock();
    const newClock = new Clock(
      clock.slot + BigInt(1),
      clock.epochStartTimestamp,
      clock.epoch,
      clock.leaderScheduleEpoch,
      clock.unixTimestamp + BigInt(seconds)
    );
    bankrunContext.setClock(newClock);

    // Refresh oracles so publish times match new clock
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  };

  it("(user 0) withdraws some token A - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairA.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userTokenABefore, vaultTokenABefore] =
      await Promise.all([
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankBefore.liquidityVault),
      ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.tokenAAccount,
          remaining: composeRemainingAccounts([
            [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
            [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
          ]),
          amount: withdrawAmountTokenA_native,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userTokenAAfter, vaultTokenAAfter] = await Promise.all(
      [
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankAfter.liquidityVault),
      ]
    );
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const balancesAfter = userAccAfter.lendingAccount.balances;
    // Partial withdraw only, position is still open
    assert.equal(bankAfter.lendingPositionCount, 1);

    const withdrawExpected = withdrawAmountTokenA_native.toNumber();
    if (verbose) {
      console.log(
        "User 0 withdrew " +
          withdrawAmountTokenA +
          " token A (" +
          withdrawExpected.toString() +
          ") native"
      );
    }

    // user gains the token A, the liquidity vault loses it....
    assert.equal(userTokenAAfter, userTokenABefore + withdrawExpected);
    assert.equal(vaultTokenAAfter, vaultTokenABefore - withdrawExpected);

    // User loses the shares of Token A...
    // Since there hasn't been any interest (no Token A borrowed), shares and Token A are 1:1
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[0].assetShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[0].assetShares
    ).toNumber();
    assert.equal(sharesAfter, sharesBefore - withdrawExpected);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalAssetShares
    ).toNumber();
    assert.equal(bankSharesAfter, bankSharesBefore - withdrawExpected);
  });

  it("(user 0) repays some USDC debt - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairUsdc.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userUsdcBefore, vaultUsdcBefore] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankBefore.liquidityVault),
    ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await repayIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts([
            [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
            [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
          ]),
          amount: repayAmountUsdc_native,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userUsdcAfter, vaultUsdcAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankAfter.liquidityVault),
    ]);
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    // Partial repay only, still has debt
    assert.equal(bankAfter.borrowingPositionCount, 1);
    // Still has deposit in token A
    assert.equal(bankAfter.lendingPositionCount, 1);
    const balancesAfter = userAccAfter.lendingAccount.balances;

    const repayExpected = repayAmountUsdc_native.toNumber();
    if (verbose) {
      console.log(
        "User 0 repaid " +
          repayAmountUsdc +
          " usdc (" +
          repayExpected.toString() +
          ") native"
      );
    }

    // user loses the USDC, the liquidity vault gains it....
    assert.equal(userUsdcAfter, userUsdcBefore - repayExpected);
    assert.equal(vaultUsdcAfter, vaultUsdcBefore + repayExpected);

    // User loses the liability shares of USDC...
    // USDC has some borrows, so there is trivial interest here that affects accounting
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[1].liabilityShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[1].liabilityShares
    ).toNumber();
    assert.approximately(sharesAfter, sharesBefore - repayExpected, 1);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalLiabilityShares
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalLiabilityShares
    ).toNumber();
    assert.approximately(bankSharesAfter, bankSharesBefore - repayExpected, 1);
  });

  it("(user 0) tries to repay all without claiming emissions - should fail", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);
    const bank = bankKeypairUsdc.publicKey;

    // Ensure emissions accrue by advancing bankrun clock
    await advanceClockAndRefreshOracles(2);

    await expectFailedTxWithError(
      async () => {
        await user.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(
            await repayIx(user.mrgnProgram, {
              marginfiAccount: userAccKey,
              bank: bank,
              tokenAccount: user.usdcAccount,
              remaining: composeRemainingAccounts([
                [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
                [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
              ]),
              amount: u64MAX_BN,
              repayAll: true,
            })
          )
        );
      },
      "CannotCloseOutstandingEmissions",
      6033
    );
  });

  it("(user 0) claims emissions (in token B) before repaying their balance - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);
    const bank = bankKeypairUsdc.publicKey;

    // Make this test independent: ensure emissions accrue even when run standalone.
    await advanceClockAndRefreshOracles(2);

    const userBBefore = await getTokenBalance(provider, user.tokenBAccount);
    const userAccBefore = await program.account.marginfiAccount.fetch(userAccKey);

    // Only claim emissions here - the actual repayAll happens in the next test
    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawEmissionsIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.tokenBAccount,
        })
      )
    );

    const userBAfter = await getTokenBalance(provider, user.tokenBAccount);
    const userAccAfter = await program.account.marginfiAccount.fetch(userAccKey);

    let now = await getBankrunTime(bankrunContext);
    assert(userAccBefore.lastUpdate != userAccAfter.lastUpdate);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const diff = userBAfter - userBBefore;
    if (verbose) {
      console.log("Claimed Token B emissions: " + diff);
    }

    // TODO we can probably assert a more specific balance here with some maths...
    assert.ok(diff > 0);

    // TODO assert changes to the emissions accounts...
  });

  it("(user 0) repays all of their USDC debt - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    // Note: In bankrun we don't advance the clock here since the previous test already
    // advanced it and claimed emissions. The withdrawEmissionsIx in this transaction
    // will claim any remaining emissions before repaying.

    const bank = bankKeypairUsdc.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userUsdcBefore, vaultUsdcBefore] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankBefore.liquidityVault),
    ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    const actualOwed =
      wrappedI80F48toBigNumber(balancesBefore[1].liabilityShares).toNumber() *
      wrappedI80F48toBigNumber(bankBefore.liabilityShareValue).toNumber();

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await repayIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts([
            [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
            [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
          ]),
          amount: u64MAX_BN,
          repayAll: true,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userUsdcAfter, vaultUsdcAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankAfter.liquidityVault),
    ]);

    let now = await getBankrunTime(bankrunContext);
    assert(userAccBefore.lastUpdate != userAccAfter.lastUpdate);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const balancesAfter = userAccAfter.lendingAccount.balances;

    if (verbose) {
      console.log(
        "User 0 repaid entire USDC balance: ~" + actualOwed.toLocaleString()
      );
    }

    // USDC has some borrows, so there is trivial interest here that affects accounting

    // user loses the USDC, the liquidity vault gains it....
    assert.approximately(userUsdcAfter, userUsdcBefore - actualOwed, 2);
    assert.approximately(vaultUsdcAfter, vaultUsdcBefore + actualOwed, 2);

    // User loses the liability shares of USDC...
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[1].liabilityShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[1].liabilityShares
    ).toNumber();
    // repayAll should burn *all* liability shares (the amount paid is shares * shareValue).
    assert.approximately(sharesAfter, 0, 0.000001);
    // This balance is now inactive
    assert.equal(balancesAfter[1].active, 0);
    assertKeysEqual(balancesAfter[0].bankPk, bankKeypairA.publicKey);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalLiabilityShares
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalLiabilityShares
    ).toNumber();
    assert.approximately(bankSharesAfter, bankSharesBefore - sharesBefore, 2);
  });

  it("(user 0) withdraws all token A balance - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairA.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userTokenABefore, vaultUsdcBefore] =
      await Promise.all([
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankBefore.liquidityVault),
      ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    const actualDeposited =
      wrappedI80F48toBigNumber(balancesBefore[0].assetShares).toNumber() *
      wrappedI80F48toBigNumber(bankBefore.liabilityShareValue).toNumber();

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.tokenAAccount,
          remaining: [
            bankKeypairA.publicKey,
            oracles.tokenAOracle.publicKey,
            bankKeypairUsdc.publicKey,
            oracles.usdcOracle.publicKey,
          ],
          amount: withdrawAmountTokenA_native,
          withdrawAll: true,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bank);

    const [userAccAfter, vaultUsdcAfter, userTokenAAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, bankAfter.liquidityVault),
      getTokenBalance(provider, user.tokenAAccount),
    ]);
    const balancesAfter = userAccAfter.lendingAccount.balances;

    let now = await getBankrunTime(bankrunContext);
    assert(userAccBefore.lastUpdate != userAccAfter.lastUpdate);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const withdrawExpected = actualDeposited;
    if (verbose) {
      console.log(
        "User 0 withdrew all Token A: " + actualDeposited.toLocaleString()
      );
    }

    // user gains the token A, the liquidity vault loses it....
    assert.equal(userTokenAAfter, userTokenABefore + withdrawExpected);
    assert.equal(vaultUsdcAfter, vaultUsdcBefore - withdrawExpected);

    // User loses the shares of Token A...
    // Since there hasn't been any interest (no Token A borrowed), shares and Token A are 1:1
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[0].assetShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[0].assetShares
    ).toNumber();
    assert.equal(sharesAfter, sharesBefore - withdrawExpected);
    // This balance is now inactive
    assert.equal(balancesAfter[0].active, 0);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalAssetShares
    ).toNumber();
    assert.equal(bankSharesAfter, bankSharesBefore - withdrawExpected);
  });

  it("(user 1) withdraws all SOL balance - happy path", async () => {
    // This is essentially the same test as the previous one but it's necessary to restore the state of the user 1
    // account to only have a single USDC deposit. We don't repeat the checks for exact numbers here though.
    const user = users[1];
    const userAccKey = user.accounts.get(USER_ACCOUNT);
    const bank = bankKeypairSol.publicKey;

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.wsolAccount,
          remaining: [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          amount: new BN(0),
          withdrawAll: true,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const userAccAfter = await program.account.marginfiAccount.fetch(
      userAccKey
    );
    const balancesAfter = userAccAfter.lendingAccount.balances;
    assert.equal(bankAfter.lendingPositionCount, 0);

    // This balance is now inactive
    assert.equal(balancesAfter[1].active, 0);
  });

  it("(user 0) restores previous Token A deposits and USDC borrows", async () => {
    const user = users[0];
    const userAcc = user.accounts.get(USER_ACCOUNT);

    const depositAmountA = 2;
    const depositAmountA_native = new BN(
      depositAmountA * 10 ** ecosystem.tokenADecimals
    );

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKeypairA.publicKey,
          tokenAccount: user.tokenAAccount,
          amount: depositAmountA_native,
        })
      )
    );

    const borrowAmountUsdc = 5;
    const borrowAmountUsdc_native = new BN(
      borrowAmountUsdc * 10 ** ecosystem.usdcDecimals
    );

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(user.mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts([
            [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
            [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          ]),
          amount: borrowAmountUsdc_native,
        })
      )
    );

    const userAccAfter = await program.account.marginfiAccount.fetch(userAcc);
    let balances = userAccAfter.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    assertKeysEqual(balances[0].bankPk, bankKeypairA.publicKey);
    assert.equal(balances[1].active, 1);
    assertKeysEqual(balances[1].bankPk, bankKeypairUsdc.publicKey);
  });
});
