import { BN } from "@coral-xyz/anchor";
import {
  ecosystem,
  kaminoAccounts,
  MARKET,
  USDC_RESERVE,
  TOKEN_A_RESERVE,
  KAMINO_USDC_BANK,
  KAMINO_TOKEN_A_BANK,
  users,
  bankrunContext,
  klendBankrunProgram,
  bankrunProgram,
  globalProgramAdmin,
  oracles,
  bankRunProvider,
  verbose,
  banksClient,
} from "./rootHooks";
import { Reserve } from "@kamino-finance/klend-sdk";
import { PublicKey, Transaction } from "@solana/web3.js";
import { MockUser, USER_ACCOUNT_K } from "./utils/mocks";
import {
  getCollateralExchangeRate,
  getLiquidityExchangeRate,
  simpleRefreshObligation,
  simpleRefreshReserve,
  wrappedU68F60toBigNumber,
} from "./utils/kamino-utils";
import { assert } from "chai";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { processBankrunTransaction } from "./utils/tools";
import {
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import { Clock, ProgramTestContext } from "./utils/litesvm";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { BankrunProvider } from "./utils/litesvm";
import { ONE_WEEK_IN_SECONDS } from "./utils/types";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import Decimal from "decimal.js";
import { getEpochAndSlot } from "./utils/bankrunConnection";

let ctx: ProgramTestContext;
let provider: BankrunProvider;
let bankUsdc: PublicKey;
let market: PublicKey;
let usdcReserve: PublicKey;
let obligation: PublicKey;

describe("k09: Withdraw from Kamino reserve with accrued interest", () => {
  before(async () => {
    ctx = bankrunContext;
    provider = bankRunProvider;
    bankUsdc = kaminoAccounts.get(KAMINO_USDC_BANK);
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    const bankKey = bankUsdc.toString();
    obligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);
  });

  async function getBalances(user: number): Promise<{
    userUsdcBal: number;
    reserveUsdcBal: number;
    reserve: Reserve;
  }> {
    const userAccount = users[user].usdcAccount;
    const reserve = kaminoAccounts.get(USDC_RESERVE);

    const [userUsdcBal, reserveData, reserveUsdcBal] = await Promise.all([
      getTokenBalance(provider, userAccount),
      klendBankrunProgram.account.reserve.fetch(reserve),
      // we could also derive the supply vault pda, just being lazy here
      klendBankrunProgram.account.reserve
        .fetch(reserve)
        .then((r) => getTokenBalance(provider, r.liquidity.supplyVault)),
    ]);

    return {
      userUsdcBal,
      reserveUsdcBal,
      reserve: { ...reserveData } as Reserve,
    };
  }

  function prettyPrintBalances(
    label: string,
    state: {
      userUsdcBal: number;
      reserveUsdcBal: number;
      reserve: Reserve;
    }
  ) {
    if (verbose) {
      console.log(label);
      console.log(" User Token Balance: " + state.userUsdcBal);
      console.log(" Reserve Supply Balance: " + state.reserveUsdcBal);
    }
  }

  async function executeWithdraw(
    user: MockUser,
    withdrawAmt: BN,
    remaining: PublicKey[],
    isFinalWithdrawal: boolean = false
  ): Promise<void> {
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          authority: user.wallet.publicKey,
          bank: bankUsdc,
          mint: ecosystem.usdcMint.publicKey,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        {
          amount: withdrawAmt,
          isWithdrawAll: isFinalWithdrawal,
          remaining,
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
  }

  async function executeDeposit(user: MockUser, amount: BN): Promise<void> {
    const bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    const bankKey = bank.toString();
    const obligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        amount
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
  }

  /** The user's active Kamino-bank asset shares (0 if no active position). */
  async function kaminoAssetShares(user: number): Promise<number> {
    const marginfiAccount = users[user].accounts.get(USER_ACCOUNT_K);
    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
    );
    const bal = acc.lendingAccount.balances.find(
      (b) => b.bankPk.equals(bankUsdc) && b.active === 1
    );
    return bal ? wrappedI80F48toBigNumber(bal.assetShares).toNumber() : 0;
  }

  /** In collateral tokens, as a scaled Fraction */
  let interestAccumulated: number;
  // ??? in rare cases bankrun throws a `Account in use` here.
  it("accrues interest after some time elapses by refreshing the reserve", async () => {
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const initialState = await getBalances(0);
    prettyPrintBalances("---Before interest", initialState);
    const initialBorrowedAmount = wrappedU68F60toBigNumber(
      initialState.reserve.liquidity.borrowedAmountSf
    ).toNumber();

    // Warp 1 week later (NOTE: ends up a few hundred seconds beyond one week, close enough)
    let clock = await banksClient.getClock();
    const timeTarget = clock.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);
    const targetUnix = BigInt(timeTarget);
    const newClock = new Clock(
      clock.slot, // preserve current slot
      clock.epochStartTimestamp,
      clock.epoch,
      clock.leaderScheduleEpoch,
      targetUnix
    );
    bankrunContext.setClock(newClock);
    let { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    // ~241920 slots in 1 week (ONE_WEEK_IN_SECONDS * 0.4)
    const slotsPerWeek = ONE_WEEK_IN_SECONDS * 0.4;
    bankrunContext.warpToSlot(BigInt(slot + slotsPerWeek));
    clock = await banksClient.getClock();

    // Update all pull oracles to bankrun's current time
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh the reserve after warping to accrue interest
    let refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      )
    );
    await processBankrunTransaction(ctx, refreshTx, [
      globalProgramAdmin.wallet,
    ]);

    const finalState = await getBalances(0);
    prettyPrintBalances("---After interest", finalState);
    const finalBorrowedAmount = wrappedU68F60toBigNumber(
      finalState.reserve.liquidity.borrowedAmountSf
    ).toNumber();
    const diffReserve = finalBorrowedAmount - initialBorrowedAmount;
    const exchangeRate = getCollateralExchangeRate(finalState.reserve);
    const diffInCollateral = new Decimal(diffReserve).mul(exchangeRate);
    console.log(
      " diff reserve: " + diffReserve + " in collateral: " + diffInCollateral
    );
    console.log(" exchange rate (collateral per liq): " + exchangeRate);
    interestAccumulated = diffReserve;

    assert(finalBorrowedAmount > initialBorrowedAmount);
    assert.equal(finalState.userUsdcBal - initialState.userUsdcBal, 0);
    assert.equal(finalState.reserveUsdcBal - initialState.reserveUsdcBal, 0);
  });

  it("Should withdraw from Kamino reserve with accrued interest", async () => {
    const preBal = await getBalances(0);
    const sharesBefore = await kaminoAssetShares(0);
    prettyPrintBalances("---Before withdraw", preBal);

    // After k07 test, User 0 should have 200 USDC and User 1 has 150 USDC in the reserve
    // Let's withdraw a small amount (20 USDC) from User 0's remaining balance
    const amt = 20 * 10 ** ecosystem.usdcDecimals;

    await executeWithdraw(
      users[0],
      new BN(amt),
      composeRemainingAccounts([
        [bankUsdc, oracles.usdcOracle.publicKey, usdcReserve],
      ]),
      false
    );

    const postBal = await getBalances(0);
    prettyPrintBalances("After withdraw", postBal);
    const diffUser = postBal.userUsdcBal - preBal.userUsdcBal;
    const diffReserve = postBal.reserveUsdcBal - preBal.reserveUsdcBal;

    // ??? Math question: Do you get the pre withdraw or post withdraw exchange rate?
    const exchangeRate = getLiquidityExchangeRate(preBal.reserve);
    const expected = amt * exchangeRate.toNumber();
    console.log("diff user: " + diffUser + " expected " + expected);
    assert.approximately(diffUser, amt * exchangeRate.toNumber(), amt * 0.0001);
    assert.equal(diffUser, -diffReserve);

    // The withdrawal shrinks the user's Kamino collateral position by ~the withdrawn amount, and
    // the position stays active (partial withdrawal).
    const sharesAfter = await kaminoAssetShares(0);
    assert.isBelow(sharesAfter, sharesBefore);
    assert.approximately(sharesBefore - sharesAfter, amt, amt * 0.0001);
  });

  it("Should deposit to Kamino reserve after interest has accrued", async () => {
    const preDepositBalances = await getBalances(1);
    const sharesBefore = await kaminoAssetShares(1);

    const depositAmount = new BN(50 * 10 ** ecosystem.usdcDecimals); // 50 USDC

    await executeDeposit(users[1], depositAmount);

    const postDepositBalances = await getBalances(1);
    // The deposit moves exactly `depositAmount` USDC from the user into the reserve supply vault,
    // and mints collateral that grows the user's Kamino position.
    assert.equal(
      postDepositBalances.userUsdcBal - preDepositBalances.userUsdcBal,
      -depositAmount.toNumber()
    );
    assert.equal(
      postDepositBalances.reserveUsdcBal - preDepositBalances.reserveUsdcBal,
      depositAmount.toNumber()
    );
    const sharesAfter = await kaminoAssetShares(1);
    assert.isAbove(sharesAfter, sharesBefore);
  });

  it("Should withdraw remaining balance from Kamino reserve", async () => {
    const preWithdrawBalances = await getBalances(0);
    const sharesBefore = await kaminoAssetShares(0);

    const remainingWithdrawAmount = new BN(
      10_000 * 10 ** ecosystem.usdcDecimals
    ); // 10k USDC

    // This is a PARTIAL withdrawal of a large position, so isFinalWithdrawal stays false (the user
    // keeps a balance) despite the "remaining balance" naming.
    await executeWithdraw(
      users[0],
      remainingWithdrawAmount,
      composeRemainingAccounts([
        [bankUsdc, oracles.usdcOracle.publicKey, usdcReserve],
      ]),
      false
    );

    const postWithdrawBalances = await getBalances(0);
    const sharesAfter = await kaminoAssetShares(0);
    const userDelta =
      postWithdrawBalances.userUsdcBal - preWithdrawBalances.userUsdcBal;
    const reserveDelta =
      postWithdrawBalances.reserveUsdcBal - preWithdrawBalances.reserveUsdcBal;
    // The position shrinks by the requested collateral amount, and USDC conservation holds.
    assert.equal(userDelta, -reserveDelta);
    assert.approximately(
      sharesBefore - sharesAfter,
      remainingWithdrawAmount.toNumber(),
      remainingWithdrawAmount.toNumber() * 0.0001
    );
    // Interest check: the user receives MORE USDC than the collateral redeemed, because the
    // collateral has appreciated from the interest accrued earlier in this suite.
    assert.isAbove(userDelta, remainingWithdrawAmount.toNumber());

    const marginfiAccount = users[0].accounts.get(USER_ACCOUNT_K);
    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
    );
    const kaminoBankBalance = acc.lendingAccount.balances.find(
      (b) => b.bankPk.equals(bankUsdc) && b.active === 1
    );
    assert.equal(kaminoBankBalance.active, 1);
  });

  it("(user 0) withdraws token A with accrued interest (non-6-decimal result)", async () => {
    const tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);
    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const tokenAObligation = kaminoAccounts.get(
      `${tokenABank.toString()}_OBLIGATION`
    );
    const user = users[0];
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Deposit token A so user 0 holds a token A Kamino position to withdraw from.
    const depositAmt = new BN(50 * 10 ** ecosystem.tokenADecimals);
    await processBankrunTransaction(
      ctx,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          tokenAReserve,
          market,
          oracles.tokenAOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          tokenAObligation,
          [tokenAReserve]
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount,
            bank: tokenABank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserve: tokenAReserve,
          },
          depositAmt
        )
      ),
      [user.wallet]
    );

    const reserveBefore = await klendBankrunProgram.account.reserve.fetch(
      tokenAReserve
    );
    const userTokenABefore = await getTokenBalance(provider, user.tokenAAccount);

    const withdrawAmt = 20 * 10 ** ecosystem.tokenADecimals;
    // User 0 also holds a USDC Kamino position from earlier, so the post-withdraw health check needs
    // both banks' observations (with fresh reserves) — not just token A's.
    await processBankrunTransaction(
      ctx,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          market,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshReserve(
          klendBankrunProgram,
          tokenAReserve,
          market,
          oracles.tokenAOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          tokenAObligation,
          [tokenAReserve]
        ),
        await makeKaminoWithdrawIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount,
            authority: user.wallet.publicKey,
            bank: tokenABank,
            mint: ecosystem.tokenAMint.publicKey,
            destinationTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserve: tokenAReserve,
          },
          {
            amount: new BN(withdrawAmt),
            isWithdrawAll: false,
            remaining: composeRemainingAccounts([
              [tokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
              [bankUsdc, oracles.usdcOracle.publicKey, usdcReserve],
            ]),
          }
        )
      ),
      [user.wallet]
    );

    const userTokenAAfter = await getTokenBalance(provider, user.tokenAAccount);
    const diffUser = userTokenAAfter - userTokenABefore;

    // The withdrawn collateral redeems for `amt * exchangeRate` of the (interest-appreciated) liquidity.
    const exchangeRate = getLiquidityExchangeRate(reserveBefore as any);
    const expected = withdrawAmt * exchangeRate.toNumber();
    assert.approximately(diffUser, expected, expected * 0.0001);
    // Interest realized: with a >1 exchange rate, the user receives MORE token A than redeemed.
    assert.isAtLeast(diffUser, withdrawAmt);
  });
});
