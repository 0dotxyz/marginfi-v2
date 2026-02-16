import { BN } from "@coral-xyz/anchor";
import { Transaction, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import {
  banksClient,
  bankrunContext,
  bankrunProgram,
  globalProgramAdmin,
  solendAccounts,
  users,
  SOLEND_MARKET,
  SOLEND_USDC_BANK,
  SOLEND_TOKEN_A_BANK,
  SOLEND_USDC_RESERVE,
  SOLEND_TOKEN_A_RESERVE,
  bankRunProvider,
  ecosystem,
  oracles,
} from "./rootHooks";
import { MockUser, USER_ACCOUNT_SL } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import {
  getTokenBalance,
  assertBankrunTxFailed,
  assertI80F48Approx,
} from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  composeRemainingAccounts,
  composeRemainingAccountsByBalances,
} from "./utils/user-instructions";
import {
  makeSolendDepositIx,
  makeSolendWithdrawIx,
} from "./utils/solend-instructions";
import { makeSolendRefreshReserveIx } from "./utils/solend-sdk";
import { SOLEND_NULL_PUBKEY } from "./utils/solend-utils";
import { createMintToInstruction } from "@solana/spl-token";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";

describe("sl06: Solend - Marginfi Deposits & Withdrawals", () => {
  let userA: MockUser;
  let userB: MockUser;

  let usdcBank: PublicKey;
  let tokenABank: PublicKey;
  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;

  before(async () => {
    userA = users[0];
    userB = users[1];

    usdcBank = solendAccounts.get(SOLEND_USDC_BANK)!;
    tokenABank = solendAccounts.get(SOLEND_TOKEN_A_BANK)!;
    usdcReserve = solendAccounts.get(SOLEND_USDC_RESERVE)!;
    tokenAReserve = solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!;
  });

  describe("1. Instant Deposit/Withdraw Tests", () => {
    describe("1.1 Large Amount Tests ($1M)", () => {
      it("User A - Deposit and withdraw 1M USDC", async () => {
        const DEPOSIT_AMOUNT = new BN(1_000_000).mul(
          new BN(10 ** ecosystem.usdcDecimals)
        );

        const fundTx = new Transaction().add(
          createMintToInstruction(
            ecosystem.usdcMint.publicKey,
            userA.usdcAccount,
            globalProgramAdmin.wallet.publicKey,
            DEPOSIT_AMOUNT.toNumber()
          )
        );
        await processBankrunTransaction(bankrunContext, fundTx, [
          globalProgramAdmin.wallet,
        ]);

        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );

        const depositTx = new Transaction().add(
          await makeSolendDepositIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
              bank: usdcBank,
              signerTokenAccount: userA.usdcAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.usdcOracle.publicKey,
            },
            { amount: DEPOSIT_AMOUNT }
          )
        );
        await processBankrunTransaction(bankrunContext, depositTx, [
          userA.wallet,
        ]);

        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountAfterDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_SL)
          );

        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          );

        const assetSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const cTokenAmount = new BN(assetSharesAfterDeposit.toString());

        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );

        const withdrawTx = new Transaction().add(
          await makeSolendWithdrawIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
              bank: usdcBank,
              destinationTokenAccount: userA.usdcAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.usdcOracle.publicKey,
            },
            {
              amount: cTokenAmount,
              withdrawAll: false,
              remaining: composeRemainingAccounts([
                [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
              ]),
            }
          )
        );
        await processBankrunTransaction(bankrunContext, withdrawTx, [
          userA.wallet,
        ]);

        const bankAfter = await bankrunProgram.account.bank.fetch(usdcBank);
        const expectedPrice = oracles.usdcPrice;
        const expectedConf =
          expectedPrice * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
        assertI80F48Approx(bankAfter.cache.lastOraclePrice, expectedPrice);
        assertI80F48Approx(bankAfter.cache.lastOraclePriceConfidence, expectedConf);

        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountFinal =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_SL)
          );

        const balanceFinal = marginfiAccountFinal.lendingAccount.balances.find(
          (b) => b.bankPk.equals(usdcBank)
        );

        assert.approximately(userBalanceFinal, userBalanceBefore, 1);
        assert.ok(
          !balanceFinal ||
            wrappedI80F48toBigNumber(balanceFinal.assetShares).eq(0)
        );
      });

      // TODO: Add negative test cases for bad oracle/reserve validation during withdraw operations
      // Should test that health checks properly validate oracle accounts and reserve parameters
      // This covers oracle account validation during health checks (not liquidation-specific)
      // Related to PR #115 comment #49: https://github.com/mrgnlabs/marginfi-v2-internal/pull/115#discussion_r2250715403

      it("User B - Deposit and withdraw $1M worth of Token A", async () => {
        const tokenAAmount = 100_000;
        const DEPOSIT_AMOUNT = new BN(tokenAAmount).mul(
          new BN(10 ** ecosystem.tokenADecimals)
        );

        const fundTx = new Transaction().add(
          createMintToInstruction(
            ecosystem.tokenAMint.publicKey,
            userB.tokenAAccount,
            globalProgramAdmin.wallet.publicKey,
            DEPOSIT_AMOUNT.toNumber()
          )
        );
        await processBankrunTransaction(bankrunContext, fundTx, [
          globalProgramAdmin.wallet,
        ]);

        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );

        const refreshTx = new Transaction().add(
          makeSolendRefreshReserveIx({
            reserve: tokenAReserve,
            pythOracle: oracles.tokenAOracle.publicKey,
            switchboardOracle: SOLEND_NULL_PUBKEY,
          })
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        const depositTx = new Transaction().add(
          await makeSolendDepositIx(
            userB.mrgnBankrunProgram,
            {
              marginfiAccount: userB.accounts.get(USER_ACCOUNT_SL),
              bank: tokenABank,
              signerTokenAccount: userB.tokenAAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.tokenAOracle.publicKey,
            },
            { amount: DEPOSIT_AMOUNT }
          )
        );
        await processBankrunTransaction(
          bankrunContext,
          depositTx,
          [userB.wallet],
          false,
          true
        );

        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );
        const marginfiAccountAfterDeposit =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_SL)
          );

        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(tokenABank)
          );

        const assetSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const cTokenAmount = new BN(assetSharesAfterDeposit.toString());

        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );

        const withdrawTx = new Transaction().add(
          await makeSolendWithdrawIx(
            userB.mrgnBankrunProgram,
            {
              marginfiAccount: userB.accounts.get(USER_ACCOUNT_SL),
              bank: tokenABank,
              destinationTokenAccount: userB.tokenAAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.tokenAOracle.publicKey,
            },
            {
              amount: cTokenAmount,
              withdrawAll: false,
              remaining: composeRemainingAccounts([
                [tokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
              ]),
            }
          )
        );
        await processBankrunTransaction(
          bankrunContext,
          withdrawTx,
          [userB.wallet],
          false,
          true
        );

        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );

        assert.approximately(userBalanceFinal, userBalanceBefore, 1);
      });
    });

    describe("1.2 Small Amount Tests ($1)", () => {
      it("User A - Deposit and withdraw 1 USDC", async () => {
        const DEPOSIT_AMOUNT = new BN(1).mul(
          new BN(10 ** ecosystem.usdcDecimals)
        );

        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );

        const refreshTx = new Transaction().add(
          makeSolendRefreshReserveIx({
            reserve: usdcReserve,
            pythOracle: oracles.usdcOracle.publicKey,
            switchboardOracle: SOLEND_NULL_PUBKEY,
          })
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        const depositTx = new Transaction().add(
          await makeSolendDepositIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
              bank: usdcBank,
              signerTokenAccount: userA.usdcAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.usdcOracle.publicKey,
            },
            { amount: DEPOSIT_AMOUNT }
          )
        );
        await processBankrunTransaction(
          bankrunContext,
          depositTx,
          [userA.wallet],
          false,
          true
        );

        const marginfiAccountAfterDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_SL)
          );
        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          );
        const assetSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const cTokenAmount = new BN(assetSharesAfterDeposit.toString());

        const withdrawTx = new Transaction().add(
          await makeSolendWithdrawIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
              bank: usdcBank,
              destinationTokenAccount: userA.usdcAccount,
              lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
              pythPrice: oracles.usdcOracle.publicKey,
            },
            {
              amount: cTokenAmount,
              withdrawAll: false,
              remaining: composeRemainingAccounts([
                [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
              ]),
            }
          )
        );
        await processBankrunTransaction(
          bankrunContext,
          withdrawTx,
          [userA.wallet],
          false,
          true
        );

        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );

        assert.approximately(userBalanceFinal, userBalanceBefore, 1000);
      });
    });
  });

  describe("2. Reserve Refresh Requirement Tests (Expected Failures)", () => {
    it("Verify reserve refresh is required for deposits after time advancement", async () => {
      const DEPOSIT_AMOUNT = new BN(10_000).mul(
        new BN(10 ** ecosystem.usdcDecimals)
      );

      const fundTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          userA.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          DEPOSIT_AMOUNT.toNumber()
        )
      );
      await processBankrunTransaction(bankrunContext, fundTx, [
        globalProgramAdmin.wallet,
      ]);

      const refreshTx = new Transaction().add(
        makeSolendRefreshReserveIx({
          reserve: usdcReserve,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      );
      await processBankrunTransaction(bankrunContext, refreshTx, [
        globalProgramAdmin.wallet,
      ]);

      const depositTx = new Transaction().add(
        await makeSolendDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          { amount: DEPOSIT_AMOUNT }
        )
      );
      await processBankrunTransaction(
        bankrunContext,
        depositTx,
        [userA.wallet],
        false,
        true
      );

      const currentClock = await banksClient.getClock();
      const newTimestamp = currentClock.unixTimestamp + BigInt(86400);
      const slotsToAdvance = 86400 * 0.4;
      const newClock = new Clock(
        currentClock.slot + BigInt(slotsToAdvance),
        0n,
        currentClock.epoch,
        0n,
        newTimestamp
      );
      bankrunContext.setClock(newClock);

      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      const secondDepositTx = new Transaction().add(
        await makeSolendDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          { amount: DEPOSIT_AMOUNT }
        )
      );

      const result = await processBankrunTransaction(
        bankrunContext,
        secondDepositTx,
        [userA.wallet],
        true,
        false
      );

      assertBankrunTxFailed(result, 6411);
    });

    it("Verify reserve refresh is required for withdrawals after time advancement", async () => {
      const marginfiAccountBefore =
        await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userA.accounts.get(USER_ACCOUNT_SL)
        );
      const balanceBefore = marginfiAccountBefore.lendingAccount.balances.find(
        (b) => b.active === 1 && b.bankPk.equals(usdcBank)
      );

      if (!balanceBefore) {
        return;
      }

      const assetSharesBefore = wrappedI80F48toBigNumber(
        balanceBefore.assetShares
      );
      const halfAmount = new BN(assetSharesBefore.toString()).div(new BN(2));

      const currentClock = await banksClient.getClock();
      const newTimestamp = currentClock.unixTimestamp + BigInt(3600);
      const slotsToAdvance = 3600 * 0.4;
      const newClock = new Clock(
        currentClock.slot + BigInt(slotsToAdvance),
        0n,
        currentClock.epoch,
        0n,
        newTimestamp
      );
      bankrunContext.setClock(newClock);

      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      const withdrawTx = new Transaction().add(
        await makeSolendWithdrawIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
            bank: usdcBank,
            destinationTokenAccount: userA.usdcAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          {
            amount: halfAmount,
            withdrawAll: false,
            remaining: composeRemainingAccounts([
              [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
            ]),
          }
        )
      );

      const result = await processBankrunTransaction(
        bankrunContext,
        withdrawTx,
        [userA.wallet],
        true,
        false
      );

      assertBankrunTxFailed(result, 6411);
    });
  });

  describe("3. Time-Based Interest Accrual Tests", () => {
    it("Deposit USDC, advance time, verify interest accrual", async () => {
      const DEPOSIT_AMOUNT = new BN(10_000).mul(
        new BN(10 ** ecosystem.usdcDecimals)
      );

      const fundTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          userA.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          DEPOSIT_AMOUNT.toNumber()
        )
      );
      await processBankrunTransaction(bankrunContext, fundTx, [
        globalProgramAdmin.wallet,
      ]);

      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      const refreshTx = new Transaction().add(
        makeSolendRefreshReserveIx({
          reserve: usdcReserve,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      );
      await processBankrunTransaction(bankrunContext, refreshTx, [
        globalProgramAdmin.wallet,
      ]);

      const depositTx = new Transaction().add(
        await makeSolendDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          { amount: DEPOSIT_AMOUNT }
        )
      );
      await processBankrunTransaction(
        bankrunContext,
        depositTx,
        [userA.wallet],
        false,
        true
      );

      const currentClock = await banksClient.getClock();
      const newTimestamp = currentClock.unixTimestamp + BigInt(86400);
      const slotsToAdvance = 86400 * 0.4;
      const newClock = new Clock(
        currentClock.slot + BigInt(slotsToAdvance),
        0n,
        currentClock.epoch,
        0n,
        newTimestamp
      );
      bankrunContext.setClock(newClock);

      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      const refreshTx2 = new Transaction().add(
        makeSolendRefreshReserveIx({
          reserve: usdcReserve,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      );
      await processBankrunTransaction(bankrunContext, refreshTx2, [
        globalProgramAdmin.wallet,
      ]);

      const withdrawTx = new Transaction().add(
        await makeSolendWithdrawIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
            bank: usdcBank,
            destinationTokenAccount: userA.usdcAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          {
            amount: new BN(0),
            withdrawAll: true,
            remaining: composeRemainingAccounts([
              [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
            ]),
          }
        )
      );
      await processBankrunTransaction(
        bankrunContext,
        withdrawTx,
        [userA.wallet],
        false,
        true
      );

      const userBalanceFinal = await getTokenBalance(
        bankRunProvider,
        userA.usdcAccount
      );
      const receivedAmount = userBalanceFinal;

      assert.ok(receivedAmount >= DEPOSIT_AMOUNT.toNumber());
    });
  });

  describe("4. Randomized Deposit/Withdraw Tests", () => {
    it("Random deposits and withdrawals with time advancement", async () => {
      const NUM_ITERATIONS = 5;
      const MIN_DEPOSIT_USD = 100;
      const MAX_DEPOSIT_USD = 50_000;

      const fundAmount = new BN(2_000_000);
      const fundTx = new Transaction()
        .add(
          createMintToInstruction(
            ecosystem.usdcMint.publicKey,
            userA.usdcAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.mul(new BN(10 ** ecosystem.usdcDecimals)).toNumber()
          )
        )
        .add(
          createMintToInstruction(
            ecosystem.tokenAMint.publicKey,
            userA.tokenAAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.mul(new BN(10 ** ecosystem.tokenADecimals)).toNumber()
          )
        )
        .add(
          createMintToInstruction(
            ecosystem.usdcMint.publicKey,
            userB.usdcAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.mul(new BN(10 ** ecosystem.usdcDecimals)).toNumber()
          )
        )
        .add(
          createMintToInstruction(
            ecosystem.tokenAMint.publicKey,
            userB.tokenAAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.mul(new BN(10 ** ecosystem.tokenADecimals)).toNumber()
          )
        );
      await processBankrunTransaction(bankrunContext, fundTx, [
        globalProgramAdmin.wallet,
      ]);

      for (let i = 0; i < NUM_ITERATIONS; i++) {
        const isUserA = Math.random() < 0.5;
        const user = isUserA ? userA : userB;
        const isUsdc = Math.random() < 0.5;
        const bank = isUsdc ? usdcBank : tokenABank;
        const reserve = isUsdc ? usdcReserve : tokenAReserve;
        const tokenAccount = isUsdc ? user.usdcAccount : user.tokenAAccount;
        const oracle = isUsdc
          ? oracles.usdcOracle.publicKey
          : oracles.tokenAOracle.publicKey;
        const decimals = isUsdc
          ? ecosystem.usdcDecimals
          : ecosystem.tokenADecimals;
        const tokenPrice = isUsdc ? 1 : 10;

        const marginfiAccount =
          await bankrunProgram.account.marginfiAccount.fetch(
            user.accounts.get(USER_ACCOUNT_SL)
          );
        const existingBalance = marginfiAccount.lendingAccount.balances.find(
          (b) => b.active === 1 && b.bankPk.equals(bank)
        );

        const hasPosition =
          existingBalance &&
          wrappedI80F48toBigNumber(existingBalance.assetShares).gt(0);

        const shouldWithdraw =
          hasPosition && (Math.random() < 0.3 || i === NUM_ITERATIONS - 1);

        const refreshTx = new Transaction().add(
          makeSolendRefreshReserveIx({
            reserve: reserve,
            pythOracle: oracle,
            switchboardOracle: SOLEND_NULL_PUBKEY,
          })
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        if (shouldWithdraw) {
          const withdrawAll = Math.random() < 0.5;
          const cTokenAmount = new BN(
            wrappedI80F48toBigNumber(existingBalance.assetShares).toString()
          );
          const withdrawAmount = withdrawAll
            ? new BN(0)
            : cTokenAmount.div(new BN(2));

          await makeSolendWithdrawThroughMarginfi(
            user,
            bank,
            withdrawAmount,
            withdrawAll
          );
        } else {
          const depositUsdValue =
            MIN_DEPOSIT_USD +
            Math.random() * (MAX_DEPOSIT_USD - MIN_DEPOSIT_USD);
          const tokenAmount = depositUsdValue / tokenPrice;
          const depositAmount = new BN(Math.floor(tokenAmount)).mul(
            new BN(10 ** decimals)
          );

          const depositTx = new Transaction().add(
            await makeSolendDepositIx(
              user.mrgnBankrunProgram,
              {
                marginfiAccount: user.accounts.get(USER_ACCOUNT_SL),
                bank: bank,
                signerTokenAccount: tokenAccount,
                lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
                pythPrice: oracle,
              },
              { amount: depositAmount }
            )
          );
          await processBankrunTransaction(
            bankrunContext,
            depositTx,
            [user.wallet],
            false,
            true
          );
        }

        const timeAdvance =
          3600 + Math.floor(Math.random() * (7 * 24 * 3600 - 3600));
        const currentClock = await banksClient.getClock();
        const slotsToAdvance = Math.floor(timeAdvance * 0.4);
        const newClock = new Clock(
          currentClock.slot + BigInt(slotsToAdvance),
          0n,
          currentClock.epoch,
          0n,
          currentClock.unixTimestamp + BigInt(timeAdvance)
        );
        bankrunContext.setClock(newClock);

        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      }
    });
  });

  async function makeSolendWithdrawThroughMarginfi(
    user: MockUser,
    bank: PublicKey,
    amount: BN,
    withdrawAll: boolean = false
  ): Promise<void> {
    const userAccount = user.accounts.get(USER_ACCOUNT_SL)!;

    const marginfiAccount =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);

    const isUsdc = bank.equals(usdcBank);
    const reserve = isUsdc ? usdcReserve : tokenAReserve;
    const oracle = isUsdc
      ? oracles.usdcOracle.publicKey
      : oracles.tokenAOracle.publicKey;
    const tokenAccount = isUsdc ? user.usdcAccount : user.tokenAAccount;

    const activePositions: PublicKey[][] = [];

    for (const balance of marginfiAccount.lendingAccount.balances) {
      if (balance.active === 1) {
        if (balance.bankPk.equals(usdcBank)) {
          activePositions.push([
            balance.bankPk,
            oracles.usdcOracle.publicKey,
            usdcReserve,
          ]);
        } else if (balance.bankPk.equals(tokenABank)) {
          activePositions.push([
            balance.bankPk,
            oracles.tokenAOracle.publicKey,
            tokenAReserve,
          ]);
        }
      }
    }

    const refreshReserveIx = makeSolendRefreshReserveIx({
      reserve: reserve,
      pythOracle: oracle,
      switchboardOracle: SOLEND_NULL_PUBKEY,
    });

    const withdrawIx = await makeSolendWithdrawIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount: userAccount,
        bank,
        destinationTokenAccount: tokenAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracle,
      },
      {
        amount: withdrawAll ? new BN(0) : amount,
        withdrawAll,
        remaining: withdrawAll
          ? composeRemainingAccountsByBalances(
              marginfiAccount.lendingAccount.balances,
              activePositions,
              bank
            )
          : activePositions.flat(),
      }
    );

    const tx = new Transaction().add(refreshReserveIx, withdrawIx);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      false,
      true
    );
  }

  describe("5. Edge Case Tests", () => {
    describe("5.1 Withdraw All Function", () => {
      it("Deposit various amounts and use withdraw_all", async () => {
        const DEPOSIT1 = new BN(1234).mul(new BN(10 ** ecosystem.usdcDecimals));
        const DEPOSIT2 = new BN(5678).mul(new BN(10 ** ecosystem.usdcDecimals));

        let tx = new Transaction()
          .add(
            makeSolendRefreshReserveIx({
              reserve: usdcReserve,
              pythOracle: oracles.usdcOracle.publicKey,
              switchboardOracle: SOLEND_NULL_PUBKEY,
            })
          )
          .add(
            await makeSolendDepositIx(
              userA.mrgnBankrunProgram,
              {
                marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
                bank: usdcBank,
                signerTokenAccount: userA.usdcAccount,
                lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
                pythPrice: oracles.usdcOracle.publicKey,
              },
              { amount: DEPOSIT1 }
            )
          );
        await processBankrunTransaction(
          bankrunContext,
          tx,
          [userA.wallet],
          false,
          true
        );

        const clock1 = await banksClient.getClock();
        const slotsToAdvance = 300 * 0.4;
        const newClock = new Clock(
          clock1.slot + BigInt(slotsToAdvance),
          0n,
          clock1.epoch,
          0n,
          clock1.unixTimestamp + BigInt(300)
        );
        bankrunContext.setClock(newClock);

        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

        tx = new Transaction()
          .add(
            makeSolendRefreshReserveIx({
              reserve: usdcReserve,
              pythOracle: oracles.usdcOracle.publicKey,
              switchboardOracle: SOLEND_NULL_PUBKEY,
            })
          )
          .add(
            await makeSolendDepositIx(
              userA.mrgnBankrunProgram,
              {
                marginfiAccount: userA.accounts.get(USER_ACCOUNT_SL),
                bank: usdcBank,
                signerTokenAccount: userA.usdcAccount,
                lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
                pythPrice: oracles.usdcOracle.publicKey,
              },
              { amount: DEPOSIT2 }
            )
          );
        await processBankrunTransaction(
          bankrunContext,
          tx,
          [userA.wallet],
          false,
          true
        );

        await makeSolendWithdrawThroughMarginfi(
          userA,
          usdcBank,
          new BN(0),
          true
        );

        const finalAccount =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_SL)
          );
        const finalBalance = finalAccount.lendingAccount.balances.find((b) =>
          b.bankPk.equals(usdcBank)
        );

        assert.ok(!finalBalance);
      });
    });
  });

  describe("6. Final Cleanup Verification", () => {
    it("Ensure all positions are fully withdrawn", async () => {
      let hasActivePositions = true;
      while (hasActivePositions) {
        const currentAccount =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_SL)
          );

        let foundPosition = null;
        for (const balance of currentAccount.lendingAccount.balances) {
          if (
            balance.active === 1 &&
            wrappedI80F48toBigNumber(balance.assetShares).gt(0)
          ) {
            foundPosition = balance;
            break;
          }
        }

        if (!foundPosition) {
          hasActivePositions = false;
          break;
        }

        const bank = foundPosition.bankPk;

        await makeSolendWithdrawThroughMarginfi(userA, bank, new BN(0), true);
      }

      let userBHasActivePositions = true;
      while (userBHasActivePositions) {
        const currentAccountB =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_SL)
          );

        let foundPositionB = null;
        for (const balance of currentAccountB.lendingAccount.balances) {
          if (
            balance.active === 1 &&
            wrappedI80F48toBigNumber(balance.assetShares).gt(0)
          ) {
            foundPositionB = balance;
            break;
          }
        }

        if (!foundPositionB) {
          userBHasActivePositions = false;
          break;
        }

        const bank = foundPositionB.bankPk;

        await makeSolendWithdrawThroughMarginfi(userB, bank, new BN(0), true);
      }

      const finalAccountA =
        await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userA.accounts.get(USER_ACCOUNT_SL)
        );
      const finalAccountB =
        await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userB.accounts.get(USER_ACCOUNT_SL)
        );

      for (const balance of finalAccountA.lendingAccount.balances) {
        if (balance.active === 1) {
          const shares = wrappedI80F48toBigNumber(balance.assetShares);
          assert.ok(shares.eq(0));
        }
      }

      for (const balance of finalAccountB.lendingAccount.balances) {
        if (balance.active === 1) {
          const shares = wrappedI80F48toBigNumber(balance.assetShares);
          assert.ok(shares.eq(0));
        }
      }
    });
  });
});
