import { BN } from "@coral-xyz/anchor";
import {
  Transaction,
} from "@solana/web3.js";
import {
  ecosystem,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  MARKET,
  oracles,
  USDC_RESERVE,
  users,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  bankRunProvider,
} from "./rootHooks";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import { assert } from "chai";
import { MockUser, USER_ACCOUNT_K } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";
import { makeKaminoWithdrawIx } from "./utils/kamino-instructions";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { assertBNApproximately, getTokenBalance, assertBankrunTxFailed } from "./utils/genericTests";

let ctx: ProgramTestContext;

describe("k07: Kamino Withdraw Tests", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  async function executeWithdraw(
    user: MockUser,
    withdrawAmt: BN,
    userIndex: number,
    isFinalWithdrawal: boolean = false
  ): Promise<void> {
    const bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    const bankKey = bank.toString();
    const obligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);

    // Get initial balances for verification
    const initialUserUsdcBalance = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );

    console.log(
      `Executing withdrawal of ${
        withdrawAmt.toNumber() / 10 ** ecosystem.usdcDecimals
      } USDC for user ${userIndex}...`
    );

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
          bank,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        {
          amount: withdrawAmt,
          isFinalWithdrawal: isFinalWithdrawal,
          remaining: composeRemainingAccounts([
            [bank, oracles.usdcOracle.publicKey, usdcReserve],
          ]),
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const finalUserUsdcBalance = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );

    const userUsdcChange = finalUserUsdcBalance - initialUserUsdcBalance;
    console.log(
      `Withdrawal successful: ${userUsdcChange.toFixed(
        6
      )} USDC transferred to user's account`
    );

    // Check if user balance increased appropriately
    assertBNApproximately(
      withdrawAmt,
      userUsdcChange,
      userUsdcChange * 0.00001
    );

    // Verify marginfi account state remains active for partial withdrawal
    if (!isFinalWithdrawal) {
      const marginfiAccountData =
        await bankrunProgram.account.marginfiAccount.fetch(marginfiAccount);
      const bankBalances = marginfiAccountData.lendingAccount.balances;
      const kaminoBankBalance = bankBalances.find(
        (b) => b.bankPk.equals(bank) && b.active === 1
      );

      // TODO assert balances
      assert.equal(kaminoBankBalance.active, 1);
    }
  }

  it("(user 0) Partial withdrawal from Kamino via Marginfi - happy path", async () => {
    // Withdraw 100 USDC (leaving remaining USDC from previous deposits in the reserve)
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    await executeWithdraw(users[0], withdrawAmount, 0, false);
  });

  it("(user 0) Try to withdraw more than deposited balance - should fail with OperationWithdrawOnly", async () => {
    const user = users[0];
    const bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const bankKey = bank.toString();
    const obligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);

    // Try to withdraw an extremely large amount (10 million USDC)
    const excessiveWithdrawAmount = new BN(10_000_000 * 10 ** ecosystem.usdcDecimals);

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
          bank,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        {
          amount: excessiveWithdrawAmount,
          isFinalWithdrawal: false,
          remaining: composeRemainingAccounts([
            [bank, oracles.usdcOracle.publicKey, usdcReserve],
          ]),
        }
      )
    );

    let result = await processBankrunTransaction(ctx, tx, [user.wallet], true, true);
    // OperationWithdrawOnly error code 6020
    assertBankrunTxFailed(result, 6020);
  });
});
