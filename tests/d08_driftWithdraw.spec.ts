import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  ecosystem,
  driftAccounts,
  DRIFT_USDC_BANK,
  DRIFT_TOKEN_A_BANK,
  DRIFT_USDC_SPOT_MARKET,
  DRIFT_TOKEN_A_SPOT_MARKET,
  DRIFT_TOKEN_A_PULL_ORACLE,
  users,
  bankrunContext,
  bankrunProgram,
  driftBankrunProgram,
  bankRunProvider,
  oracles,
} from "./rootHooks";
import { USER_ACCOUNT_D } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import {
  makeDriftDepositIx,
  makeDriftWithdrawIx,
} from "./utils/drift-instructions";
import {
  assertBNEqual,
  assertI80F48Approx,
  getTokenBalance,
  assertBankrunTxFailed,
} from "./utils/genericTests";
import {
  assertBankBalance,
  getDriftUserAccount,
  TOKEN_A_INIT_DEPOSIT_AMOUNT,
  TOKEN_A_MARKET_INDEX,
  TOKEN_A_SCALING_FACTOR,
  scaledBalanceToTokenAmount,
  tokenAmountToScaledBalance,
  USDC_INIT_DEPOSIT_AMOUNT,
  USDC_SCALING_FACTOR,
} from "./utils/drift-utils";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";

describe("d08: Drift Withdraw Tests", () => {
  let driftUsdcBank: PublicKey;
  let driftTokenABank: PublicKey;

  before(async () => {
    driftUsdcBank = driftAccounts.get(DRIFT_USDC_BANK);
    driftTokenABank = driftAccounts.get(DRIFT_TOKEN_A_BANK);
  });

  it("(user 0) Withdraws all of USDC from Drift - happy path", async () => {
    const user = users[0];

    const bank = await bankrunProgram.account.bank.fetch(driftUsdcBank);
    const [userUsdcBefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[0];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        destinationTokenAccount: user.usdcAccount,
        driftOracle: null,
      },
      {
        amount: new BN(0),
        withdraw_all: true,
        remaining: composeRemainingAccounts([
          [
            driftTokenABank,
            oracles.tokenAOracle.publicKey,
            driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );
    const [userUsdcAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    const amountReceived = new BN(userUsdcAfter - userUsdcBefore);

    const spotPositionAfter = driftUserAfter.spotPositions[0];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Note: +1 is expected here due to rounding Drift applies to withdrawals
    assertBNEqual(
      scaledBalanceBefore.sub(scaledBalanceAfter),
      amountReceived.mul(USDC_SCALING_FACTOR).add(new BN(1))
    );

    // The balance should be cleared
    await assertBankBalance(marginfiAccount, driftUsdcBank, null);
  });

  it("(user 1) Withdraws 200 USDC (all) from Drift - happy path", async () => {
    const user = users[1];
    const amount = new BN(200 * 10 ** ecosystem.usdcDecimals);
    const bank = await bankrunProgram.account.bank.fetch(driftUsdcBank);

    const [userUsdcBefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[0];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftUsdcBank,
        destinationTokenAccount: user.usdcAccount,
      },
      {
        amount: new BN(0),
        withdraw_all: true,
        remaining: composeRemainingAccounts([
          [
            driftUsdcBank,
            oracles.usdcOracle.publicKey,
            driftAccounts.get(DRIFT_USDC_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const [userUsdcAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    const amountReceived = new BN(userUsdcAfter - userUsdcBefore);
    // Note: withdraw_all loses 1 lamport to preserve drift's rounding
    assertBNEqual(amount.sub(new BN(1)), amountReceived);

    const spotPositionAfter = driftUserAfter.spotPositions[0];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;
    const scaledBalanceDecrease = scaledBalanceBefore.sub(scaledBalanceAfter);

    // Note: +1 is expected here due to rounding Drift applies to withdrawals
    assertBNEqual(
      scaledBalanceDecrease,
      amountReceived.mul(USDC_SCALING_FACTOR).add(new BN(1))
    );

    // The balance should be cleared
    await assertBankBalance(marginfiAccount, driftUsdcBank, null);

    // Note: underlying Drift position keeps the init deposit plus 1 base unit of dust per
    // withdraw_all due to rounding. There have been two withdraw_alls at this point.
    const dust = USDC_SCALING_FACTOR.sub(new BN(1));
    assertBNEqual(
      scaledBalanceAfter,
      USDC_INIT_DEPOSIT_AMOUNT.mul(USDC_SCALING_FACTOR).add(dust.mul(new BN(2)))
    );
  });

  it("(user 0) Withdraws 2 Token A (out of 5 total) from Drift - happy path", async () => {
    const user = users[0];
    const amount = new BN(2 * 10 ** ecosystem.tokenADecimals);

    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const [userTokenABefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const spotPositionBefore = driftUserBefore.spotPositions[1];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;
    const marginfiAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(marginfiAccount);
    const marginfiBalanceBefore =
      marginfiAccountBefore.lendingAccount.balances.find(
        (b) => b.bankPk.equals(driftTokenABank) && b.active === 1
      );
    const sharesBefore = new BN(
      wrappedI80F48toBigNumber(marginfiBalanceBefore.assetShares).toString()
    );

    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        destinationTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      {
        amount,
        withdraw_all: false,
        remaining: composeRemainingAccounts([
          [
            driftTokenABank,
            oracles.tokenAOracle.publicKey,
            driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const [userTokenAAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    assertBNEqual(amount, userTokenAAfter - userTokenABefore);

    const spotPositionAfter = driftUserAfter.spotPositions[1]; // non-USDC -> position 1
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;
    const scaledBalanceDecrease = scaledBalanceBefore.sub(scaledBalanceAfter);
    const scaledAmount = amount.mul(TOKEN_A_SCALING_FACTOR);

    // Note: +1 is expected here due to rounding Drift applies to withdrawals
    assertBNEqual(scaledBalanceDecrease, scaledAmount.add(new BN(1)));

    const bankAfter = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const expectedPrice = oracles.tokenAPrice;
    const expectedConf =
      expectedPrice * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    assertI80F48Approx(bankAfter.cache.lastOraclePrice, expectedPrice);
    assertI80F48Approx(bankAfter.cache.lastOraclePriceConfidence, expectedConf);

    await assertBankBalance(
      marginfiAccount,
      driftTokenABank,
      sharesBefore.sub(scaledBalanceDecrease)
    );
  });

  it("(user 0) Tries to withdraw more than deposited - should fail", async () => {
    const user = users[0];
    // Due to Drift rounding on previous withdrawal (2 Token A => 2.00000001), there is now only (3
    // * 10 ** ecosystem.tokenADecimals - 1) tokens left.
    const excessiveAmount = new BN(3 * 10 ** ecosystem.tokenADecimals);

    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        destinationTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      {
        amount: excessiveAmount,
        withdraw_all: false,
        remaining: composeRemainingAccounts([
          [
            driftTokenABank,
            oracles.tokenAOracle.publicKey,
            driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      true,
      false
    );

    // OperationWithdrawOnly
    assertBankrunTxFailed(result, 6020);
  });

  it("(user 0) Tries to withdraw 0 tokens - should succeed with no transfer", async () => {
    const user = users[0];
    const zeroAmount = new BN(0);

    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const [userTokenABefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[0];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        destinationTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      {
        amount: zeroAmount,
        withdraw_all: false,
        remaining: composeRemainingAccounts([
          [
            driftTokenABank,
            oracles.tokenAOracle.publicKey,
            driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const [userTokenAAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    assertBNEqual(new BN(userTokenAAfter), new BN(userTokenABefore));
    const spotPositionAfter = driftUserAfter.spotPositions[0];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;
    assertBNEqual(scaledBalanceAfter, scaledBalanceBefore);
  });

  it("(user 0) Withdraws the maximum Token A amount manually", async () => {
    const user = users[0];

    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const [
      marginfiAccountBefore,
      spotMarket,
      userTokenABefore,
      driftUserBefore,
    ] = await Promise.all([
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      driftBankrunProgram.account.spotMarket.fetch(bank.integrationAcc1),
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const marginfiBalanceBefore =
      marginfiAccountBefore.lendingAccount.balances.find(
        (b) => b.bankPk.equals(driftTokenABank) && b.active === 1
      );

    const assetSharesBefore = new BN(
      wrappedI80F48toBigNumber(marginfiBalanceBefore.assetShares).toString()
    );
    const maxTokenAmount = scaledBalanceToTokenAmount(
      assetSharesBefore,
      spotMarket,
      false
    );

    // Note: we expect to be one base unit short of the actual deposit.
    console.log("(max) token to withdraw: " + maxTokenAmount.toString());
    const expectedMax = new BN(3 * 10 ** ecosystem.tokenADecimals).subn(1);
    assertBNEqual(maxTokenAmount, expectedMax);

    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        destinationTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      {
        amount: maxTokenAmount,
        withdraw_all: false,
        remaining: composeRemainingAccounts([
          [
            driftTokenABank,
            oracles.tokenAOracle.publicKey,
            driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET),
          ],
        ]),
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const [userTokenAAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    assertBNEqual(maxTokenAmount, userTokenAAfter - userTokenABefore);

    const scaledBalanceBefore = driftUserBefore.spotPositions[1].scaledBalance;
    const scaledBalanceAfter = driftUserAfter.spotPositions[1].scaledBalance;
    const scaledBalanceDecrease = scaledBalanceBefore.sub(scaledBalanceAfter);
    let expectedScaledDecrement = tokenAmountToScaledBalance(
      maxTokenAmount,
      spotMarket
    );
    if (!maxTokenAmount.isZero()) {
      expectedScaledDecrement = expectedScaledDecrement.add(new BN(1));
    }
    assertBNEqual(scaledBalanceDecrease, expectedScaledDecrement);

    // Restore the balance for the next test...

    const depositIx = await makeDriftDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        signerTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      maxTokenAmount,
      TOKEN_A_MARKET_INDEX
    );
    const depositTx = new Transaction().add(depositIx);
    await processBankrunTransaction(
      bankrunContext,
      depositTx,
      [user.wallet],
      false,
      true
    );
  });

  it("(user 0) Withdraws all of Token A from from Drift - happy path", async () => {
    const user = users[0];

    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const [userTokenABefore, driftUserBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);

    const spotPositionBefore = driftUserBefore.spotPositions[1];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_D);
    const withdrawIx = await makeDriftWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank: driftTokenABank,
        destinationTokenAccount: user.tokenAAccount,
        driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
      },
      {
        amount: new BN(0),
        withdraw_all: true,
        remaining: [],
      },
      driftBankrunProgram
    );

    const tx = new Transaction().add(withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );

    const [userTokenAAfter, driftUserAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.tokenAAccount),
      getDriftUserAccount(driftBankrunProgram, bank.integrationAcc2),
    ]);
    const amountReceived = new BN(userTokenAAfter - userTokenABefore);

    const spotPositionAfter = driftUserAfter.spotPositions[1];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Note: +1 is expected here due to rounding Drift applies to withdrawals
    assertBNEqual(
      scaledBalanceBefore.sub(scaledBalanceAfter),
      amountReceived.mul(TOKEN_A_SCALING_FACTOR).add(new BN(1))
    );

    // The balance should be cleared
    await assertBankBalance(marginfiAccount, driftTokenABank, null);

    // Note: this time the actual underlying Drift position is NOT simply the (initial_deposit -
    // count_of_withdrawals_so_far). This is because Drift uses fixed precision of 9 decimals and we
    // "lose" the lowest (9 - mint_decimals) digits and withdraw less than a full amount as a
    // result.
    //
    // In this specific example: we request to withdraw (3 * 10 ** 8 - 1) tokens (i.e. 2.99999999),
    // and Drift balance decrease is then: 2'999'999'990 (note one extra 0 for that extra
    // precision), but our balance is: 2'999'999'999, which leave us that lowest "9": 2'999'999'999
    // - 2'999'999'990 = 9
    //
    // So the resultant balance becomes equal to: (initial_deposit - count_of_withdrawals_so_far +
    // 9) minus 1 extra scaled unit from the max-withdraw round trip above.
    const nonWithdrawnDust = TOKEN_A_SCALING_FACTOR.sub(new BN(1));
    assertBNEqual(
      scaledBalanceAfter,
      TOKEN_A_INIT_DEPOSIT_AMOUNT.mul(TOKEN_A_SCALING_FACTOR)
        .sub(new BN(2))
        .add(nonWithdrawnDust)
    );
  });
});
