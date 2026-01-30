import { BN } from "@coral-xyz/anchor";
import {
  Transaction,
  PublicKey,
  ComputeBudgetProgram,
  SystemProgram,
} from "@solana/web3.js";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import {
  banksClient,
  bankrunContext,
  globalProgramAdmin,
  kaminoAccounts,
  users,
  KAMINO_USDC_BANK,
  KAMINO_TOKEN_A_BANK,
  USDC_RESERVE,
  TOKEN_A_RESERVE,
  MARKET,
  bankRunProvider,
  ecosystem,
  oracles,
  klendBankrunProgram,
  farmAccounts,
  A_FARM_STATE,
  A_OBLIGATION_USER_STATE,
  groupAdmin,
} from "./rootHooks";
import { MockUser, USER_ACCOUNT_K } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import { getTokenBalance, assertBankrunTxFailed } from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { composeRemainingAccounts } from "./utils/user-instructions";
import {
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  simpleRefreshReserve,
  simpleRefreshObligation,
} from "./utils/kamino-utils";
import { createMintToInstruction } from "@solana/spl-token";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

describe("k15: Kamino - Marginfi Deposits & Withdrawals", () => {
  let userA: MockUser;
  let userB: MockUser;

  // Banks and reserves from previous tests
  let usdcBank: PublicKey;
  let tokenABank: PublicKey;
  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;
  let usdcBankObligation: PublicKey;
  let tokenABankObligation: PublicKey;

  before(async () => {
    userA = users[0];
    userB = users[1];

    // Get banks and reserves from kaminoAccounts map
    usdcBank = kaminoAccounts.get(KAMINO_USDC_BANK)!;
    tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK)!;
    usdcReserve = kaminoAccounts.get(USDC_RESERVE)!;
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE)!;

    // Get bank-specific obligations
    usdcBankObligation = kaminoAccounts.get(
      `${usdcBank.toString()}_OBLIGATION`
    )!;
    tokenABankObligation = kaminoAccounts.get(
      `${tokenABank.toString()}_OBLIGATION`
    )!;
  });

  describe("1. Instant Deposit/Withdraw Tests", () => {
    describe("1.1 Large Amount Tests ($1M)", () => {
      it("User A - Deposit and withdraw 1M USDC", async () => {
        const DEPOSIT_AMOUNT = new BN(1_000_000).mul(
          new BN(10 ** ecosystem.usdcDecimals)
        );

        // Fund user A with 1M USDC
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

        // Refresh oracle prices first
        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

        // Get initial balances
        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountBeforeDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceBeforeDeposit =
          marginfiAccountBeforeDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          );

        const collateralSharesBefore = balanceBeforeDeposit
          ? wrappedI80F48toBigNumber(balanceBeforeDeposit.assetShares)
          : new (require("@mrgnlabs/mrgn-common").BigNumber)(0);

        // Refresh reserves and obligation first (follow k06 pattern)
        const refreshTx = new Transaction().add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            usdcReserve,
            kaminoAccounts.get(MARKET)!,
            oracles.usdcOracle.publicKey
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            kaminoAccounts.get(MARKET)!,
            usdcBankObligation,
            [usdcReserve]
          )
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        // Deposit using correct instruction signature
        const depositTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
          await makeKaminoDepositIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
              bank: usdcBank,
              signerTokenAccount: userA.usdcAccount,
              lendingMarket: kaminoAccounts.get(MARKET)!,
              reserveLiquidityMint: ecosystem.usdcMint.publicKey,
            },
            DEPOSIT_AMOUNT // Separate amount parameter
          )
        );
        await processBankrunTransaction(bankrunContext, depositTx, [
          userA.wallet,
        ]);

        // Verify deposit
        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountAfterDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          )!;

        // Get collateral amount from marginfi balance difference (since user had existing deposits)
        const collateralSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const collateralDifference = collateralSharesAfterDeposit.minus(
          collateralSharesBefore
        );
        const collateralAmount = new BN(collateralDifference.toString());

        // Verify deposit
        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );
        assert.ok(collateralAmount.gt(new BN(0)));

        // Debug: Check what bank the deposit actually went to
        console.log("DEBUG: Expected bank (usdcBank):", usdcBank.toString());
        console.log(
          "DEBUG: Actual bank in account:",
          balanceAfterDeposit.bankPk.toString()
        );
        console.log(
          "DEBUG: Banks match:",
          usdcBank.equals(balanceAfterDeposit.bankPk)
        );

        // Debug: Check ALL active balances on the account
        console.log("DEBUG: All active balances on account:");
        for (const [
          index,
          balance,
        ] of marginfiAccountAfterDeposit.lendingAccount.balances.entries()) {
          if (balance.active === 1) {
            const assetShares = wrappedI80F48toBigNumber(balance.assetShares);
            const liabilityShares = wrappedI80F48toBigNumber(
              balance.liabilityShares
            );
            console.log(
              `  Balance ${index}: Bank ${balance.bankPk.toString()}, Asset shares: ${assetShares.toString()}, Liability shares: ${liabilityShares.toString()}`
            );
          }
        }

        // Use the helper function properly - it should handle the bank selection
        await makeKaminoWithdrawThroughMarginfi(
          userA,
          usdcBank,
          collateralAmount,
          false
        );

        // Verify withdrawal
        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountFinal =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceFinal = marginfiAccountFinal.lendingAccount.balances.find(
          (b) => b.bankPk.equals(usdcBank)
        );

        assert.approximately(
          userBalanceFinal,
          userBalanceBefore,
          10, // Allow small rounding difference
          "User should recover full deposit amount"
        );
      });

      it("User B - Deposit and withdraw $500k worth of Token A", async () => {
        // Calculate Token A amount for $500k (Token A = $10)
        const tokenAPrice = 10;
        const tokenAAmount = 50_000; // 50k Token A = $500k
        const DEPOSIT_AMOUNT = new BN(tokenAAmount).mul(
          new BN(10 ** ecosystem.tokenADecimals)
        );

        // Fund user B with Token A
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

        // Get initial balances
        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );
        const marginfiAccountBeforeDeposit =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceBeforeDeposit =
          marginfiAccountBeforeDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(tokenABank)
          );

        const collateralSharesBefore = balanceBeforeDeposit
          ? wrappedI80F48toBigNumber(balanceBeforeDeposit.assetShares)
          : new (require("@mrgnlabs/mrgn-common").BigNumber)(0);

        // Refresh reserve before deposit
        const refreshTx = new Transaction().add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            tokenAReserve,
            kaminoAccounts.get(MARKET)!,
            oracles.tokenAOracle.publicKey
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            kaminoAccounts.get(MARKET)!,
            tokenABankObligation,
            [tokenAReserve]
          )
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        // Deposit (Token A has farms enabled from k13)
        const depositTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
          await makeKaminoDepositIx(
            userB.mrgnBankrunProgram,
            {
              marginfiAccount: userB.accounts.get(USER_ACCOUNT_K)!,
              bank: tokenABank,
              signerTokenAccount: userB.tokenAAccount,
              lendingMarket: kaminoAccounts.get(MARKET)!,
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
              obligationFarmUserState: farmAccounts.get(
                A_OBLIGATION_USER_STATE
              ),
              reserveFarmState: farmAccounts.get(A_FARM_STATE),
            },
            DEPOSIT_AMOUNT
          )
        );
        await processBankrunTransaction(bankrunContext, depositTx, [
          userB.wallet,
        ]);

        // Get balances after deposit
        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );
        const marginfiAccountAfterDeposit =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_K)!
          );

        // Find the balance for Token A bank
        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(tokenABank)
          )!;

        // Get collateral amount from marginfi balance difference (since user had existing deposits)
        const collateralSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const collateralDifference = collateralSharesAfterDeposit.minus(
          collateralSharesBefore
        );
        const collateralAmount = new BN(collateralDifference.toString());

        // Verify deposit
        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );
        assert.ok(collateralAmount.gt(new BN(0)));

        // Use makeKaminoWithdrawThroughMarginfi helper function
        await makeKaminoWithdrawThroughMarginfi(
          userB,
          tokenABank,
          collateralAmount,
          false
        );

        // Get final balances
        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );

        // Verify withdrawal
        assert.approximately(
          userBalanceFinal,
          userBalanceBefore,
          10,
          "User should recover full deposit amount"
        );
      });
    });

    describe("1.2 Small Amount Tests ($1)", () => {
      it("User A - Deposit and withdraw 10 units (0.000010 USDC)", async () => {
        const DEPOSIT_AMOUNT = new BN(10); // 10 units = 0.000010 USDC

        // Get initial balance
        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        const marginfiAccountBeforeDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceBeforeDeposit =
          marginfiAccountBeforeDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          );

        const collateralSharesBefore = balanceBeforeDeposit
          ? wrappedI80F48toBigNumber(balanceBeforeDeposit.assetShares)
          : new (require("@mrgnlabs/mrgn-common").BigNumber)(0);

        // Refresh reserve
        const refreshTx = new Transaction().add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            usdcReserve,
            kaminoAccounts.get(MARKET)!,
            oracles.usdcOracle.publicKey
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            kaminoAccounts.get(MARKET)!,
            usdcBankObligation,
            [usdcReserve]
          )
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        // Deposit
        const depositTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
          await makeKaminoDepositIx(
            userA.mrgnBankrunProgram,
            {
              marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
              bank: usdcBank,
              signerTokenAccount: userA.usdcAccount,
              lendingMarket: kaminoAccounts.get(MARKET)!,
              reserveLiquidityMint: ecosystem.usdcMint.publicKey,
            },
            DEPOSIT_AMOUNT
          )
        );
        await processBankrunTransaction(bankrunContext, depositTx, [
          userA.wallet,
        ]);

        // Get collateral amount from marginfi balance difference (since user had existing deposits)
        const marginfiAccountAfterDeposit =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );
        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(usdcBank)
          )!;

        const collateralSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const collateralDifference = collateralSharesAfterDeposit.minus(
          collateralSharesBefore
        );
        const collateralAmount = new BN(collateralDifference.toString());

        // Verify deposit
        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );
        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );
        assert.ok(collateralAmount.gt(new BN(0)));

        // Use makeKaminoWithdrawThroughMarginfi helper function
        await makeKaminoWithdrawThroughMarginfi(
          userA,
          usdcBank,
          collateralAmount,
          false
        );

        // Get final balance
        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userA.usdcAccount
        );

        // For tiny amounts (10 units), rounding can be significant but should still be close
        assert.approximately(
          userBalanceFinal,
          userBalanceBefore,
          1, // Very small tolerance for 10 unit amount
          "User should recover deposit amount within rounding tolerance"
        );
      });

      it("User B - Deposit and withdraw 10 units (0.00000010 Token A)", async () => {
        const DEPOSIT_AMOUNT = new BN(10); // 10 units = 0.00000010 Token A (8 decimals)

        // Get initial balance
        const userBalanceBefore = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );
        const marginfiAccountBeforeDeposit =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_K)!
          );

        const balanceBeforeDeposit =
          marginfiAccountBeforeDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(tokenABank)
          );

        const collateralSharesBefore = balanceBeforeDeposit
          ? wrappedI80F48toBigNumber(balanceBeforeDeposit.assetShares)
          : new (require("@mrgnlabs/mrgn-common").BigNumber)(0);

        // Refresh reserve before deposit
        const refreshTx = new Transaction().add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            tokenAReserve,
            kaminoAccounts.get(MARKET)!,
            oracles.tokenAOracle.publicKey
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            kaminoAccounts.get(MARKET)!,
            tokenABankObligation,
            [tokenAReserve]
          )
        );
        await processBankrunTransaction(bankrunContext, refreshTx, [
          globalProgramAdmin.wallet,
        ]);

        // Deposit (Token A has farms enabled from k13)
        const depositTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
          await makeKaminoDepositIx(
            userB.mrgnBankrunProgram,
            {
              marginfiAccount: userB.accounts.get(USER_ACCOUNT_K)!,
              bank: tokenABank,
              signerTokenAccount: userB.tokenAAccount,
              lendingMarket: kaminoAccounts.get(MARKET)!,
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
              obligationFarmUserState: farmAccounts.get(
                A_OBLIGATION_USER_STATE
              ),
              reserveFarmState: farmAccounts.get(A_FARM_STATE),
            },
            DEPOSIT_AMOUNT
          )
        );
        await processBankrunTransaction(bankrunContext, depositTx, [
          userB.wallet,
        ]);

        // Get collateral amount from marginfi balance difference (since user had existing deposits)
        const marginfiAccountAfterDeposit =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_K)!
          );
        const balanceAfterDeposit =
          marginfiAccountAfterDeposit.lendingAccount.balances.find(
            (b) => b.active === 1 && b.bankPk.equals(tokenABank)
          )!;

        const collateralSharesAfterDeposit = wrappedI80F48toBigNumber(
          balanceAfterDeposit.assetShares
        );
        const collateralDifference = collateralSharesAfterDeposit.minus(
          collateralSharesBefore
        );
        const collateralAmount = new BN(collateralDifference.toString());

        // Verify deposit
        const userBalanceAfterDeposit = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );
        assert.ok(
          userBalanceAfterDeposit ===
            userBalanceBefore - DEPOSIT_AMOUNT.toNumber()
        );
        assert.ok(collateralAmount.gt(new BN(0)));

        // Use makeKaminoWithdrawThroughMarginfi helper function
        await makeKaminoWithdrawThroughMarginfi(
          userB,
          tokenABank,
          collateralAmount,
          false
        );

        // Get final balance
        const userBalanceFinal = await getTokenBalance(
          bankRunProvider,
          userB.tokenAAccount
        );

        // For tiny amounts (10 units), rounding can be significant but should still be close
        assert.approximately(
          userBalanceFinal,
          userBalanceBefore,
          1, // Very small tolerance for 10 unit amount
          "User should recover deposit amount within rounding tolerance"
        );
      });
    });
  });

  describe("2. Reserve Refresh Requirement Tests (Expected Failures)", () => {
    it("Verify reserve refresh is required for deposits after time advancement", async () => {
      const DEPOSIT_AMOUNT = new BN(10_000).mul(
        new BN(10 ** ecosystem.usdcDecimals)
      );

      // Fund user
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

      // Initial deposit with proper refresh
      const refreshTx = new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          kaminoAccounts.get(MARKET)!,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          kaminoAccounts.get(MARKET)!,
          usdcBankObligation,
          [usdcReserve]
        )
      );
      await processBankrunTransaction(bankrunContext, refreshTx, [
        globalProgramAdmin.wallet,
      ]);

      const depositTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
        await makeKaminoDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: kaminoAccounts.get(MARKET)!,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          DEPOSIT_AMOUNT
        )
      );
      await processBankrunTransaction(bankrunContext, depositTx, [
        userA.wallet,
      ]);

      // Advance time to make reserve stale (24 hours)
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

      // CRITICAL: Refresh oracles after time advancement
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      // Try to deposit again WITHOUT refreshing reserve - this should fail
      const secondDepositTx = new Transaction().add(
        await makeKaminoDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: kaminoAccounts.get(MARKET)!,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          DEPOSIT_AMOUNT
        )
      );

      const result = await processBankrunTransaction(
        bankrunContext,
        secondDepositTx,
        [userA.wallet],
        true, // trySend=true
        false // dumpLogOnFail=false
      );

      // Assert that we get the expected ReserveStale error (same error as Solend)
      assertBankrunTxFailed(result, 6009);
    });

    it("Verify reserve refresh is required for withdrawals after time advancement", async () => {
      // Get current user position
      const marginfiAccountBefore =
        await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userA.accounts.get(USER_ACCOUNT_K)!
        );
      const balanceBefore = marginfiAccountBefore.lendingAccount.balances.find(
        (b) => b.active === 1 && b.bankPk.equals(usdcBank)
      );

      if (!balanceBefore) {
        return; // Skip if no position
      }

      const assetSharesBefore = wrappedI80F48toBigNumber(
        balanceBefore.assetShares
      );
      const halfAmount = new BN(assetSharesBefore.toString()).div(new BN(2));

      // Advance time again to make reserve even more stale
      const currentClock = await banksClient.getClock();
      const newTimestamp = currentClock.unixTimestamp + BigInt(3600); // 1 hour more
      const slotsToAdvance = 3600 * 0.4;
      const newClock = new Clock(
        currentClock.slot + BigInt(slotsToAdvance),
        0n,
        currentClock.epoch,
        0n,
        newTimestamp
      );
      bankrunContext.setClock(newClock);

      // CRITICAL: Refresh oracles after time advancement
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      // Try to withdraw WITHOUT refreshing reserve - this should fail
      const withdrawTx = new Transaction().add(
        await makeKaminoWithdrawIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
            authority: userA.wallet.publicKey,
            bank: usdcBank,
            destinationTokenAccount: userA.usdcAccount,
            lendingMarket: kaminoAccounts.get(MARKET)!,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          {
            amount: halfAmount,
            isFinalWithdrawal: false,
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
        true, // trySend=true
        false // dumpLogOnFail=false
      );

      // Assert that we get the expected ReserveStale error (6005 = 0x1775)
      assertBankrunTxFailed(result, 6009);
    });
  });

  describe("3. Time-Based Interest Accrual Tests", () => {
    it("Deposit USDC, advance time, verify interest accrual", async () => {
      const DEPOSIT_AMOUNT = new BN(10_000).mul(
        new BN(10 ** ecosystem.usdcDecimals)
      );

      // Fund user
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

      const userBalanceBeforeDeposit = await getTokenBalance(
        bankRunProvider,
        userA.usdcAccount
      );

      // Refresh and deposit
      const refreshTx = new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          kaminoAccounts.get(MARKET)!,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          kaminoAccounts.get(MARKET)!,
          usdcBankObligation,
          [usdcReserve]
        )
      );
      await processBankrunTransaction(bankrunContext, refreshTx, [
        globalProgramAdmin.wallet,
      ]);

      const depositTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
        await makeKaminoDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
            bank: usdcBank,
            signerTokenAccount: userA.usdcAccount,
            lendingMarket: kaminoAccounts.get(MARKET)!,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          DEPOSIT_AMOUNT
        )
      );
      await processBankrunTransaction(bankrunContext, depositTx, [
        userA.wallet,
      ]);

      // Get initial collateral balance
      const marginfiAccountBefore =
        await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userA.accounts.get(USER_ACCOUNT_K)!
        );
      const balanceBefore = marginfiAccountBefore.lendingAccount.balances.find(
        (b) => b.active === 1 && b.bankPk.equals(usdcBank)
      )!;
      const collateralTokensBefore = new BN(
        wrappedI80F48toBigNumber(balanceBefore.assetShares).toString()
      );

      // Advance time by 1 day (86400 seconds)
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

      // CRITICAL: Refresh oracles after time advancement
      await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

      // Check if we can get more tokens when withdrawing (interest accrued)
      // Helper will handle the necessary refresh to update interest
      await makeKaminoWithdrawThroughMarginfi(
        userA,
        usdcBank,
        collateralTokensBefore,
        false
      );

      const userBalanceFinal = await getTokenBalance(
        bankRunProvider,
        userA.usdcAccount
      );
      const receivedAmount = userBalanceFinal - userBalanceBeforeDeposit;

      // Should have received at least the deposit amount (interest might be very small)
      assert.ok(
        receivedAmount >= DEPOSIT_AMOUNT.toNumber(),
        `Expected at least ${DEPOSIT_AMOUNT.toString()}, got ${receivedAmount}`
      );
    });
  });

  describe("4. Randomized Deposit/Withdraw Tests", () => {
    it("Random deposits and withdrawals with time advancement", async () => {
      const NUM_ITERATIONS = 20; // Reduced for test speed
      const MIN_DEPOSIT_USD = 100;
      const MAX_DEPOSIT_USD = 50_000;

      // Fund users with enough tokens
      const fundAmount = new BN(2_000_000); // 2M tokens each
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
        // Randomly choose user and token
        const isUserA = Math.random() < 0.5;
        const user = isUserA ? userA : userB;
        const isUsdc = Math.random() < 0.5;
        const bank = isUsdc ? usdcBank : tokenABank;
        const reserve = isUsdc ? usdcReserve : tokenAReserve;
        const obligation = isUsdc ? usdcBankObligation : tokenABankObligation;
        const tokenAccount = isUsdc ? user.usdcAccount : user.tokenAAccount;
        const oracle = isUsdc
          ? oracles.usdcOracle.publicKey
          : oracles.tokenAOracle.publicKey;
        const decimals = isUsdc
          ? ecosystem.usdcDecimals
          : ecosystem.tokenADecimals;
        const tokenPrice = isUsdc ? 1 : 10;
        const mint = isUsdc
          ? ecosystem.usdcMint.publicKey
          : ecosystem.tokenAMint.publicKey;

        // Check current account value (simplified - just checking if position exists)
        const marginfiAccount =
          await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
            user.accounts.get(USER_ACCOUNT_K)!
          );
        const existingBalance = marginfiAccount.lendingAccount.balances.find(
          (b) => b.active === 1 && b.bankPk.equals(bank)
        );

        const hasPosition =
          existingBalance &&
          wrappedI80F48toBigNumber(existingBalance.assetShares).gt(0);

        // Decide action
        const shouldWithdraw =
          hasPosition && (Math.random() < 0.3 || i === NUM_ITERATIONS - 1);

        if (shouldWithdraw) {
          // Withdraw - helper will handle refresh internally
          const withdrawAll = Math.random() < 0.5;
          const cTokenAmount = new BN(
            wrappedI80F48toBigNumber(existingBalance!.assetShares).toString()
          );
          const withdrawAmount = withdrawAll
            ? cTokenAmount
            : cTokenAmount
                .mul(new BN(40 + Math.floor(Math.random() * 21)))
                .div(new BN(100)); // 40-60%

          await makeKaminoWithdrawThroughMarginfi(
            user,
            bank,
            withdrawAmount,
            false
          );
        } else {
          // Deposit - refresh before deposit
          const refreshTx = new Transaction().add(
            await simpleRefreshReserve(
              klendBankrunProgram,
              reserve,
              kaminoAccounts.get(MARKET)!,
              oracle
            ),
            await simpleRefreshObligation(
              klendBankrunProgram,
              kaminoAccounts.get(MARKET)!,
              obligation,
              [reserve]
            ),
            // dummy tx to trick bankrun
            SystemProgram.transfer({
              fromPubkey: globalProgramAdmin.wallet.publicKey,
              toPubkey: groupAdmin.wallet.publicKey,
              lamports: Math.round(Math.random() * 10000),
            })
          );
          await processBankrunTransaction(bankrunContext, refreshTx, [
            globalProgramAdmin.wallet,
          ]);
          // Deposit
          const depositUsdValue =
            MIN_DEPOSIT_USD +
            Math.random() * (MAX_DEPOSIT_USD - MIN_DEPOSIT_USD);
          const tokenAmount = depositUsdValue / tokenPrice;
          const depositAmount = new BN(Math.floor(tokenAmount)).mul(
            new BN(10 ** decimals)
          );

          const depositTx = new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
            await makeKaminoDepositIx(
              user.mrgnBankrunProgram,
              {
                marginfiAccount: user.accounts.get(USER_ACCOUNT_K)!,
                bank: bank,
                signerTokenAccount: tokenAccount,
                lendingMarket: kaminoAccounts.get(MARKET)!,
                reserveLiquidityMint: mint,
                // Add farm accounts for Token A (has farms enabled from k13)
                obligationFarmUserState: !isUsdc
                  ? farmAccounts.get(A_OBLIGATION_USER_STATE)
                  : null,
                reserveFarmState: !isUsdc
                  ? farmAccounts.get(A_FARM_STATE)
                  : null,
              },
              depositAmount
            ),
            // dummy tx to trick bankrun
            SystemProgram.transfer({
              fromPubkey: user.wallet.publicKey,
              toPubkey: groupAdmin.wallet.publicKey,
              lamports: Math.round(Math.random() * 10000),
            })
          );
          await processBankrunTransaction(bankrunContext, depositTx, [
            user.wallet,
          ]);
        }

        // Advance time randomly between 1 hour and 7 days
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

        // CRITICAL: Refresh oracles after time advancement
        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
      }
    });
  });

  describe("5. Edge Case Tests", () => {
    describe("5.1 Withdraw All Function", () => {
      it("Deposit various amounts and use isFinalWithdrawal", async () => {
        const DEPOSIT1 = new BN(1234).mul(new BN(10 ** ecosystem.usdcDecimals));
        const DEPOSIT2 = new BN(5678).mul(new BN(10 ** ecosystem.usdcDecimals));

        // Make two deposits
        let tx = new Transaction()
          .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }))
          .add(
            await simpleRefreshReserve(
              klendBankrunProgram,
              usdcReserve,
              kaminoAccounts.get(MARKET)!,
              oracles.usdcOracle.publicKey
            )
          )
          .add(
            await simpleRefreshObligation(
              klendBankrunProgram,
              kaminoAccounts.get(MARKET)!,
              usdcBankObligation,
              [usdcReserve]
            )
          )
          .add(
            await makeKaminoDepositIx(
              userA.mrgnBankrunProgram,
              {
                marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
                bank: usdcBank,
                signerTokenAccount: userA.usdcAccount,
                lendingMarket: kaminoAccounts.get(MARKET)!,
                reserveLiquidityMint: ecosystem.usdcMint.publicKey,
              },
              DEPOSIT1
            )
          );
        await processBankrunTransaction(bankrunContext, tx, [userA.wallet]);

        // Advance time slightly
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

        // CRITICAL: Refresh oracles after time advancement
        await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

        // Second deposit
        tx = new Transaction()
          .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }))
          .add(
            await simpleRefreshReserve(
              klendBankrunProgram,
              usdcReserve,
              kaminoAccounts.get(MARKET)!,
              oracles.usdcOracle.publicKey
            )
          )
          .add(
            await simpleRefreshObligation(
              klendBankrunProgram,
              kaminoAccounts.get(MARKET)!,
              usdcBankObligation,
              [usdcReserve]
            )
          )
          .add(
            await makeKaminoDepositIx(
              userA.mrgnBankrunProgram,
              {
                marginfiAccount: userA.accounts.get(USER_ACCOUNT_K)!,
                bank: usdcBank,
                signerTokenAccount: userA.usdcAccount,
                lendingMarket: kaminoAccounts.get(MARKET)!,
                reserveLiquidityMint: ecosystem.usdcMint.publicKey,
              },
              DEPOSIT2
            )
          );
        await processBankrunTransaction(bankrunContext, tx, [userA.wallet]);

        // Use isFinalWithdrawal=true with proper remaining accounts handling
        await makeKaminoWithdrawThroughMarginfi(
          userA,
          usdcBank,
          new BN(0),
          true
        );

        // Verify position is completely closed
        const finalAccount =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );
        const finalBalance = finalAccount.lendingAccount.balances.find((b) =>
          b.bankPk.equals(usdcBank)
        );

        assert.ok(
          !finalBalance || finalBalance.active === 0,
          "Position should be completely closed"
        );
      });
    });
  });

  describe("6. Final Cleanup Verification", () => {
    it("Ensure all positions are fully withdrawn", async () => {
      // Check and withdraw for User A
      let hasActivePositions = true;
      while (hasActivePositions) {
        // Get fresh account state
        const currentAccount =
          await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userA.accounts.get(USER_ACCOUNT_K)!
          );

        // Find first active position with shares > 0
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

        // Use helper function for proper remaining accounts handling
        await makeKaminoWithdrawThroughMarginfi(userA, bank, new BN(0), true);
      }

      // Check and withdraw for User B
      let userBHasActivePositions = true;
      while (userBHasActivePositions) {
        // Get fresh account state for User B
        const currentAccountB =
          await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
            userB.accounts.get(USER_ACCOUNT_K)!
          );

        // Find first active position with shares > 0
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

        // Use helper function for proper remaining accounts handling
        await makeKaminoWithdrawThroughMarginfi(userB, bank, new BN(0), true);
      }

      // Final verification
      const finalAccountA =
        await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userA.accounts.get(USER_ACCOUNT_K)!
        );
      const finalAccountB =
        await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
          userB.accounts.get(USER_ACCOUNT_K)!
        );

      // Check all balances are inactive or zero
      for (const balance of finalAccountA.lendingAccount.balances) {
        if (balance.active === 1) {
          const shares = wrappedI80F48toBigNumber(balance.assetShares);
          assert.ok(shares.eq(0), "User A should have no active positions");
        }
      }

      for (const balance of finalAccountB.lendingAccount.balances) {
        if (balance.active === 1) {
          const shares = wrappedI80F48toBigNumber(balance.assetShares);
          assert.ok(shares.eq(0), "User B should have no active positions");
        }
      }
    });
  });

  // Helper function for proper Kamino withdrawals (following sl06 pattern)
  async function makeKaminoWithdrawThroughMarginfi(
    user: MockUser,
    bank: PublicKey,
    amount: BN,
    isFinalWithdrawal: boolean = false
  ): Promise<void> {
    const userAccount = user.accounts.get(USER_ACCOUNT_K)!;

    // Get fresh account state - NO STALE SNAPSHOTS
    const marginfiAccount =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);

    // Determine bank-specific details from global account maps
    const isUsdc = bank.equals(kaminoAccounts.get(KAMINO_USDC_BANK)!);
    const reserve = isUsdc
      ? kaminoAccounts.get(USDC_RESERVE)!
      : kaminoAccounts.get(TOKEN_A_RESERVE)!;
    const obligation = isUsdc ? usdcBankObligation : tokenABankObligation;
    const oracle = isUsdc
      ? oracles.usdcOracle.publicKey
      : oracles.tokenAOracle.publicKey;
    const tokenAccount = isUsdc ? user.usdcAccount : user.tokenAAccount;
    const mint = isUsdc
      ? ecosystem.usdcMint.publicKey
      : ecosystem.tokenAMint.publicKey;

    // Build remaining accounts from currently active positions (Kamino pattern: [bank, oracle, reserve])
    const activePositions: PublicKey[][] = [];

    for (const balance of marginfiAccount.lendingAccount.balances) {
      if (balance.active === 1) {
        // KEY: Skip the bank we're withdrawing from if doing final withdrawal
        if (isFinalWithdrawal && balance.bankPk.equals(bank)) {
          continue;
        }

        // Include all active Kamino banks with proper pattern
        if (balance.bankPk.equals(kaminoAccounts.get(KAMINO_USDC_BANK)!)) {
          activePositions.push([
            balance.bankPk,
            oracles.usdcOracle.publicKey,
            kaminoAccounts.get(USDC_RESERVE)!,
          ]);
        } else if (
          balance.bankPk.equals(kaminoAccounts.get(KAMINO_TOKEN_A_BANK)!)
        ) {
          activePositions.push([
            balance.bankPk,
            oracles.tokenAOracle.publicKey,
            kaminoAccounts.get(TOKEN_A_RESERVE)!,
          ]);
        }
      }
    }

    // Separate refresh and withdrawal (follow k03 pattern, not complex combined transactions)
    const refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        reserve,
        kaminoAccounts.get(MARKET)!,
        oracle
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        kaminoAccounts.get(MARKET)!,
        obligation,
        [reserve]
      ),
      // dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        toPubkey: groupAdmin.wallet.publicKey,
        lamports: Math.round(Math.random() * 10000),
      })
    );
    await processBankrunTransaction(bankrunContext, refreshTx, [
      globalProgramAdmin.wallet,
    ]);

    // Withdrawal transaction with correct signature
    const withdrawTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          bank,
          destinationTokenAccount: tokenAccount,
          lendingMarket: kaminoAccounts.get(MARKET)!,
          reserveLiquidityMint: mint,
          // Add farm accounts for Token A (has farms enabled from k13)
          obligationFarmUserState: isUsdc
            ? null
            : farmAccounts.get(A_OBLIGATION_USER_STATE),
          reserveFarmState: isUsdc ? null : farmAccounts.get(A_FARM_STATE),
        },
        {
          amount: isFinalWithdrawal ? new BN(0) : amount,
          isFinalWithdrawal,
          remaining: composeRemainingAccounts(activePositions),
        }
      ),
      // dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: user.wallet.publicKey,
        toPubkey: groupAdmin.wallet.publicKey,
        lamports: Math.round(Math.random() * 10000),
      })
    );

    await processBankrunTransaction(bankrunContext, withdrawTx, [user.wallet]);
  }
});
