import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  ecosystem,
  driftAccounts,
  DRIFT_USDC_BANK,
  DRIFT_TOKEN_A_BANK,
  DRIFT_TOKEN_A_PULL_ORACLE,
  users,
  bankrunContext,
  bankrunProgram,
  driftBankrunProgram,
  bankRunProvider,
} from "./rootHooks";
import { USER_ACCOUNT_D } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import { makeDriftDepositIx } from "./utils/drift-instructions";
import {
  assertBNEqual,
  getTokenBalance,
  assertBankrunTxFailed,
} from "./utils/genericTests";
import {
  getDriftUserAccount,
  USDC_MARKET_INDEX,
  TOKEN_A_MARKET_INDEX,
  USDC_SCALING_FACTOR,
  TOKEN_A_SCALING_FACTOR,
  assertBankBalance,
} from "./utils/drift-utils";

describe("d07: Drift Deposit Tests", () => {
  let driftUsdcBank: PublicKey;
  let driftTokenABank: PublicKey;

  before(async () => {
    driftUsdcBank = driftAccounts.get(DRIFT_USDC_BANK);
    driftTokenABank = driftAccounts.get(DRIFT_TOKEN_A_BANK);
  });

  it("(user 0) Deposits 100 USDC and then 50 USDC to Drift - happy path", async () => {
    const user = users[0];
    const amount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    const bank = await bankrunProgram.account.bank.fetch(driftUsdcBank);
    const [userUsdcBefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[0];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const depositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        signerTokenAccount: user.usdcAccount,
      },
      amount,
      USDC_MARKET_INDEX
    );

    const tx = new Transaction().add(depositIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    assertBNEqual(amount, userUsdcBefore - userUsdcAfter);

    const driftUserAfter = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const spotPositionAfter = driftUserAfter.spotPositions[0];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Note: we account here for the initial amount deposited when the Drift user was created
    assertBNEqual(
      scaledBalanceAfter.sub(scaledBalanceBefore),
      amount.mul(USDC_SCALING_FACTOR)
    );

    await assertBankBalance(
      marginfiAccount,
      driftUsdcBank,
      amount.mul(USDC_SCALING_FACTOR)
    );

    const secondAmount = new BN(50 * 10 ** ecosystem.usdcDecimals);

    const secondDepositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        signerTokenAccount: user.usdcAccount,
      },
      secondAmount,
      USDC_MARKET_INDEX
    );

    const secondTx = new Transaction().add(secondDepositIx);
    await processBankrunTransaction(bankrunContext, secondTx, [user.wallet]);

    const userUsdcAfterSecondDeposit = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    assertBNEqual(secondAmount, userUsdcAfter - userUsdcAfterSecondDeposit);

    const driftUserAfterSecondDeposit = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const scaledBalanceAfterSecondDeposit =
      driftUserAfterSecondDeposit.spotPositions[0].scaledBalance;
    assertBNEqual(
      scaledBalanceAfterSecondDeposit,
      scaledBalanceAfter.add(secondAmount.mul(USDC_SCALING_FACTOR))
    );

    await assertBankBalance(
      marginfiAccount,
      driftUsdcBank,
      amount.add(secondAmount).mul(USDC_SCALING_FACTOR)
    );
  });

  it("(user 1) Deposits 200 USDC to Drift - happy path", async () => {
    const user = users[1];
    const amount = new BN(200 * 10 ** ecosystem.usdcDecimals);
    const bank = await bankrunProgram.account.bank.fetch(driftUsdcBank);
    const driftUserBefore = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );

    const spotPositionBefore = driftUserBefore.spotPositions[0];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const depositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        signerTokenAccount: user.usdcAccount,
      },
      amount,
      USDC_MARKET_INDEX
    );

    const tx = new Transaction().add(depositIx);
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const driftUserAfter = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const spotPositionAfter = driftUserAfter.spotPositions[0];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Note: we account here for the initial amount + the amount deposited by user 0
    assertBNEqual(
      scaledBalanceAfter.sub(scaledBalanceBefore),
      amount.mul(USDC_SCALING_FACTOR)
    );

    await assertBankBalance(
      marginfiAccount,
      driftUsdcBank,
      amount.mul(USDC_SCALING_FACTOR)
    );
  });

  it("(user 0) Tries to deposit zero amount - should fail", async () => {
    const user = users[0];
    const amount = new BN(0);

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const depositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        signerTokenAccount: user.usdcAccount,
      },
      amount,
      USDC_MARKET_INDEX
    );

    const tx = new Transaction().add(depositIx);
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      true
    );

    // Drift Error Code: InsufficientDeposit. Error Number: 6002. custom program error: 0x1772
    assertBankrunTxFailed(result, "0x1772");
  });

  it("(user 0) Deposits 5 Token A to Drift - happy path", async () => {
    const user = users[0];
    const amount = new BN(5 * 10 ** ecosystem.tokenADecimals);

    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const [userTokenABefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[1]; // non-USDC -> position 1
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const depositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        signerTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      amount,
      TOKEN_A_MARKET_INDEX
    );

    const tx = new Transaction().add(depositIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );
    assertBNEqual(amount, userTokenABefore - userTokenAAfter);

    const driftUserAfter = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    // Token A (market 1) uses position[1]
    const spotPositionAfter = driftUserAfter.spotPositions[1];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Note: we account here for the initial amount deposited when the Drift user was created
    assertBNEqual(
      scaledBalanceAfter.sub(scaledBalanceBefore),
      amount.mul(TOKEN_A_SCALING_FACTOR)
    );

    await assertBankBalance(
      marginfiAccount,
      driftTokenABank,
      amount.mul(TOKEN_A_SCALING_FACTOR)
    );

    // USDC position is still zero
    const usdcPosition = driftUserAfter.spotPositions[0];
    assertBNEqual(usdcPosition.scaledBalance, 0);
  });
});
