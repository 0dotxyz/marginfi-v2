import { BN } from "@coral-xyz/anchor";
import {
  ecosystem,
  kaminoAccounts,
  MARKET,
  USDC_RESERVE,
  KAMINO_USDC_BANK,
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
import { processBankrunTransaction } from "./utils/tools";
import {
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import { Clock, ProgramTestContext } from "solana-bankrun";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { BankrunProvider } from "anchor-bankrun";
import { ONE_WEEK_IN_SECONDS } from "./utils/types";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { getEpochAndSlot } from "./utils/stake-utils";
import Decimal from "decimal.js";

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
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        {
          amount: withdrawAmt,
          isFinalWithdrawal,
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
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        amount
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
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
      0n, // slot
      0n, // epochStartTimestamp
      0n, // epoch
      0n, // leaderScheduleEpoch
      targetUnix
    );
    bankrunContext.setClock(newClock);
    let { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    // ~241920 slots in 1 week (ONE_WEEK_IN_SECONDS * 0.4)
    const slotsPerWeek = ONE_WEEK_IN_SECONDS * 0.4;
    const slotTarget = slot + slotsPerWeek;
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

    // TODO moar asserts
  });

  it("Should deposit to Kamino reserve after interest has accrued", async () => {
    const preDepositBalances = await getBalances(1);

    const depositAmount = new BN(50 * 10 ** ecosystem.usdcDecimals); // 50 USDC

    await executeDeposit(users[1], depositAmount);

    const postDepositBalances = await getBalances(1);
    // TODO assert collateral token deposits increase?
    // const initialUserUsdcBalance = preDepositBalances.userTokenBalance;
    // const finalUserUsdcBalance = postDepositBalances.userTokenBalance;
    // const userUsdcChange = finalUserUsdcBalance - initialUserUsdcBalance;
    // assertBNApproximately(
    //   depositAmount,
    //   userUsdcChange,
    //   userUsdcChange * 0.00001
    // );
  });

  it("Should withdraw remaining balance from Kamino reserve", async () => {
    const preWithdrawBalances = await getBalances(0);

    const remainingWithdrawAmount = new BN(
      10_000 * 10 ** ecosystem.usdcDecimals
    ); // 10k USDC

    // TODO mark final?
    // Execute the withdrawal using our helper function - mark as final withdrawal
    await executeWithdraw(
      users[0],
      remainingWithdrawAmount,
      composeRemainingAccounts([
        [bankUsdc, oracles.usdcOracle.publicKey, usdcReserve],
      ]),
      false
    );

    const postWithdrawBalances = await getBalances(0);
    // TODO assert interest....
    // const initialUserUsdcBalance = preWithdrawBalances.userTokenBalance;
    // const finalUserUsdcBalance = postWithdrawBalances.userTokenBalance;
    // const userUsdcChange = finalUserUsdcBalance - initialUserUsdcBalance;
    // assertBNApproximately(
    //   remainingWithdrawAmount,
    //   userUsdcChange,
    //   userUsdcChange * 0.00001
    // );

    const marginfiAccount = users[0].accounts.get(USER_ACCOUNT_K);
    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
    );
    const kaminoBankBalance = acc.lendingAccount.balances.find(
      (b) => b.bankPk.equals(bankUsdc) && b.active === 1
    );
    assert.equal(kaminoBankBalance.active, 1);
  });

  // TODO repeat for token A (for non-6-decimals result)
});
