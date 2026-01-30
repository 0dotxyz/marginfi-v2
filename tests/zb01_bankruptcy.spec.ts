import { BN, Wallet } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  groupAdmin,
  bankrunContext,
  banksClient,
  bankrunProgram,
  ecosystem,
  oracles,
  users,
  verbose,
  globalProgramAdmin,
  riskAdmin,
  emodeAdmin,
} from "./rootHooks";
import {
  accrueInterest,
  configureBank,
  groupConfigure,
  handleBankruptcy,
} from "./utils/group-instructions";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { assert } from "chai";
import {
  blankBankConfigOptRaw,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  liquidateIx,
  repayIx,
  withdrawIx,
} from "./utils/user-instructions";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { Clock } from "solana-bankrun";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  assertBankrunTxFailed,
  assertI80F48Approx,
  assertI80F48Equal,
} from "./utils/genericTests";
import { initOrUpdatePriceUpdateV2 } from "./utils/pyth-pull-mocks";
import { dumpAccBalances, bytesToF64, dumpBankrunLogs } from "./utils/tools";

const USER_ACCOUNT_THROWAWAY = "throwaway_account_zb01";
const ONE_YEAR_IN_SECONDS = 2 * 365 * 24 * 60 * 60;

const startingSeed: number = 699;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_123400000ZB1");

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;

describe("Bank bankruptcy tests", () => {
  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      2,
      USER_ACCOUNT_THROWAWAY,
      groupBuff,
      startingSeed
    );
    banks = result.banks;
    throwawayGroup = result.throwawayGroup;

    // Crank oracles so that the prices are not stale
    // Use bankrun clock instead of wall clock to avoid timestamp mismatch
    const clock = await banksClient.getClock();
    let now = Number(clock.unixTimestamp);
    let priceAlpha = ecosystem.lstAlphaPrice * 10 ** ecosystem.lstAlphaDecimals;
    let confAlpha = priceAlpha * ORACLE_CONF_INTERVAL;
    await initOrUpdatePriceUpdateV2(
      new Wallet(globalProgramAdmin.wallet),
      oracles.pythPullLstOracleFeed.publicKey,
      new BN(priceAlpha),
      new BN(confAlpha),
      now + ONE_YEAR_IN_SECONDS,
      -ecosystem.lstAlphaDecimals,
      oracles.pythPullLst,
      undefined,
      bankrunContext,
      false,
      now + ONE_YEAR_IN_SECONDS
    );
  });

  it("(admin) Seeds liquidity in both banks", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);

    const tx = new Transaction();
    for (const bank of banks) {
      tx.add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank,
          tokenAccount: user.lstAlphaAccount,
          amount,
          depositUpToLimit: false,
        })
      );
    }
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(admin) Sets both banks' asset weights to 0.9", async () => {
    let config = blankBankConfigOptRaw();
    config.assetWeightInit = bigNumberToWrappedI80F48(0.9); // 90%
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.9); // 90%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        bankConfigOpt: config,
      }),
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[1],
        bankConfigOpt: config,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(user 0) Borrows B (bank 1) against A (bank 0)", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    const borrowAmount = new BN(85 * 10 ** ecosystem.lstAlphaDecimals);

    const depositTx = new Transaction();
    depositTx.add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    depositTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    depositTx.sign(user.wallet);
    await banksClient.processTransaction(depositTx);

    const remainingAccounts: PublicKey[][] = [];
    remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
    remainingAccounts.push([banks[1], oracles.pythPullLst.publicKey]);

    const borrowTx = new Transaction();
    borrowTx.add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: borrowAmount,
      })
    );
    borrowTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    borrowTx.sign(user.wallet);
    await banksClient.processTransaction(borrowTx);
  });

  it("One year elapses", async () => {
    // Get current bankrun clock time instead of wall clock
    const currentClock = await banksClient.getClock();
    let now = Number(currentClock.unixTimestamp);
    const targetUnix = BigInt(now + ONE_YEAR_IN_SECONDS + ONE_YEAR_IN_SECONDS);

    // Construct a new Clock; we only care about the unixTimestamp field here.
    const newClock = new Clock(
      0n, // slot
      0n, // epochStartTimestamp
      0n, // epoch
      0n, // leaderScheduleEpoch
      targetUnix
    );

    bankrunContext.setClock(newClock);

    // Crank oracles so that the prices are not stale
    let priceAlpha = ecosystem.lstAlphaPrice * 10 ** ecosystem.lstAlphaDecimals;
    let confAlpha = priceAlpha * ORACLE_CONF_INTERVAL;
    const futureTime = now + ONE_YEAR_IN_SECONDS + ONE_YEAR_IN_SECONDS;
    await initOrUpdatePriceUpdateV2(
      new Wallet(globalProgramAdmin.wallet),
      oracles.pythPullLstOracleFeed.publicKey,
      new BN(priceAlpha),
      new BN(confAlpha),
      futureTime,
      -ecosystem.lstAlphaDecimals,
      oracles.pythPullLst,
      undefined,
      bankrunContext,
      false,
      futureTime
    );
  });

  it("(user 0 - permissionless) Accrues interest on bank 1", async () => {
    const user = users[0];

    const tx = new Transaction();
    tx.add(
      await accrueInterest(user.mrgnBankrunProgram, {
        bank: banks[1],
      }),
      // dummy ix to trick bankrun
      SystemProgram.transfer({
        fromPubkey: users[0].wallet.publicKey,
        toPubkey: bankrunProgram.provider.publicKey,
        lamports: 11,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const bankAcc = await bankrunProgram.account.bank.fetch(banks[1]);
    let bankValues = {
      asset:
        wrappedI80F48toBigNumber(bankAcc.assetShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalAssetShares).toNumber(),
      liability:
        wrappedI80F48toBigNumber(bankAcc.liabilityShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalLiabilityShares).toNumber(),
    };

    if (verbose) {
      console.log("values after one year (in token):");
      console.log(
        `  asset: ${bankValues.asset}, liab: ${bankValues.liability}`
      );
      console.log(`  diff: ${bankValues.liability - bankValues.asset}`);
    }

    assert.isAbove(bankValues.liability, bankValues.asset);
  });

  it("(admin) Tries to withdraw some B (bank 1 - now bankrupt) - should fail", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(0.01 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
    remainingAccounts.push([banks[1], oracles.pythPullLst.publicKey]);

    const tx = new Transaction();
    tx.add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount,
        withdrawAll: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // IllegalUtilizationRatio
    assertBankrunTxFailed(result, "0x178a");
  });

  it("(user 0) Tries to borrow a little more B (bank 1 - now bankrupt) against A (bank 0) - should fail", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const borrowAmount = new BN(0.01 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
    remainingAccounts.push([banks[1], oracles.pythPullLst.publicKey]);

    const borrowTx = new Transaction();
    borrowTx.add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: borrowAmount,
      })
    );
    borrowTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    borrowTx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(borrowTx);
    // IllegalUtilizationRatio
    assertBankrunTxFailed(result, "0x178a");
  });

  it("(admin) Sets the risk admin", async () => {
    const tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        newRiskAdmin: riskAdmin.wallet.publicKey,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);
  });

  it("(admin) Bankrupts user 0 when they still have assets - should fail", async () => {
    const admin = riskAdmin;
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx = new Transaction();
    tx.add(
      await handleBankruptcy(admin.mrgnBankrunProgram, {
        signer: admin.wallet.publicKey,
        marginfiAccount: userAccount,
        bank: banks[1],
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(admin.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // AccountNotBankrupt (until we liquidate them, the user has assets)
    assertBankrunTxFailed(result, 6013);
  });

  let groupAdminAssets: number;

  it("(admin) Liquidates user 0 until he is bankrupt - happy path", async () => {
    const liquidator = groupAdmin;
    const liquidatorMarginfiAccount = liquidator.accounts.get(
      USER_ACCOUNT_THROWAWAY
    );
    const liquidatee = users[0];
    const liquidateeMarginfiAccount = liquidatee.accounts.get(
      USER_ACCOUNT_THROWAWAY
    );
    const liquidateAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
    remainingAccounts.push([banks[1], oracles.pythPullLst.publicKey]);
    const liquidateeAccounts = composeRemainingAccounts(remainingAccounts);
    // Note: same accounts in this case
    const liquidatorAccounts = liquidateeAccounts;

    const liquidateTx = new Transaction();
    liquidateTx.add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: banks[0],
        liabilityBankKey: banks[1],
        liquidatorMarginfiAccount,
        liquidateeMarginfiAccount,
        remaining: [
          oracles.pythPullLst.publicKey,
          oracles.pythPullLst.publicKey,
          ...liquidateeAccounts,
          ...liquidatorAccounts,
        ],
        amount: liquidateAmount,
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: liquidatorAccounts.length,
      })
    );
    liquidateTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    liquidateTx.sign(liquidator.wallet);
    let result = await banksClient.tryProcessTransaction(liquidateTx);
    dumpBankrunLogs(result);

    const tx = new Transaction();
    tx.add(
      await healthPulse(liquidatee.mrgnProgram, {
        marginfiAccount: liquidateeMarginfiAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      }),
      await healthPulse(groupAdmin.mrgnProgram, {
        marginfiAccount: liquidatorMarginfiAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidatee.wallet);
    await banksClient.processTransaction(tx);

    const bankAcc = await bankrunProgram.account.bank.fetch(banks[1]);
    let bankValues = {
      asset:
        wrappedI80F48toBigNumber(bankAcc.assetShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalAssetShares).toNumber(),
      liability:
        wrappedI80F48toBigNumber(bankAcc.liabilityShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalLiabilityShares).toNumber(),
    };

    if (verbose) {
      console.log("values after one liquidation (in token):");
      console.log(
        `  asset: ${bankValues.asset}, liab: ${bankValues.liability}`
      );
      console.log(`  diff: ${bankValues.liability - bankValues.asset}`);
      console.log(
        "asset shares: " +
          wrappedI80F48toBigNumber(bankAcc.totalAssetShares).toNumber()
      );
      console.log(
        "asset share value: " +
          wrappedI80F48toBigNumber(bankAcc.assetShareValue).toNumber()
      );
      console.log(
        "liab shares: " +
          wrappedI80F48toBigNumber(bankAcc.totalLiabilityShares).toNumber()
      );
      console.log(
        "liab share value: " +
          wrappedI80F48toBigNumber(bankAcc.liabilityShareValue).toNumber()
      );

      console.log("good bank (0): " + banks[0]);
      console.log("bankrupt bank (1) : " + banks[1]);
    }
    const groupAdminAcc =
      await groupAdmin.mrgnBankrunProgram.account.marginfiAccount.fetch(
        liquidatorMarginfiAccount
      );
    const user0Acc =
      await groupAdmin.mrgnBankrunProgram.account.marginfiAccount.fetch(
        liquidateeMarginfiAccount
      );

    const bankValueMap = {};
    for (let pk of banks) {
      const acc = await bankrunProgram.account.bank.fetch(pk);
      bankValueMap[pk.toString()] = {
        asset: wrappedI80F48toBigNumber(acc.assetShareValue).toNumber(),
        liability: wrappedI80F48toBigNumber(acc.liabilityShareValue).toNumber(),
      };
    }

    dumpAccBalances(groupAdminAcc, bankValueMap);
    dumpAccBalances(user0Acc, bankValueMap);

    // TODO replace with log health cache function that's somewhere in another branch
    const cA0 = user0Acc.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cA0.assetValue);
    const liabValue = wrappedI80F48toBigNumber(cA0.liabilityValue);
    const aValMaint = wrappedI80F48toBigNumber(cA0.assetValueMaint);
    const lValMaint = wrappedI80F48toBigNumber(cA0.liabilityValueMaint);
    const aValEquity = wrappedI80F48toBigNumber(cA0.assetValueEquity);
    const lValEquity = wrappedI80F48toBigNumber(cA0.liabilityValueEquity);
    const flags = cA0.flags;
    if (verbose) {
      console.log("---user health state---");
      const isHealthy = (flags & HEALTH_CACHE_HEALTHY) !== 0;
      const engineOk = (flags & HEALTH_CACHE_ENGINE_OK) !== 0;
      const oracleOk = (flags & HEALTH_CACHE_ORACLE_OK) !== 0;
      console.log("healthy: " + isHealthy);
      console.log("engine ok: " + engineOk);
      console.log("oracle ok: " + oracleOk);
      console.log("mrgn error: " + cA0.mrgnErr);
      console.log("internal error: " + cA0.internalErr);
      console.log("index of err:   " + cA0.errIndex);
      console.log("bankrpt error: " + cA0.internalBankruptcyErr);
      console.log("asset value: " + assetValue.toString());
      console.log("liab value: " + liabValue.toString());
      console.log("asset value (maint): " + aValMaint.toString());
      console.log("liab value (maint): " + lValMaint.toString());
      console.log("asset value (equity): " + aValEquity.toString());
      console.log("liab value equity): " + lValEquity.toString());
      console.log("prices: ");
      for (let i = 0; i < cA0.prices.length; i++) {
        const price = bytesToF64(cA0.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
      console.log("");
    }

    const cA1 = groupAdminAcc.healthCache;

    const assetValue1 = wrappedI80F48toBigNumber(cA1.assetValue);
    const liabValue1 = wrappedI80F48toBigNumber(cA1.liabilityValue);
    const aValMaint1 = wrappedI80F48toBigNumber(cA1.assetValueMaint);
    const lValMaint1 = wrappedI80F48toBigNumber(cA1.liabilityValueMaint);
    const aValEquity1 = wrappedI80F48toBigNumber(cA1.assetValueEquity);
    const lValEquity1 = wrappedI80F48toBigNumber(cA1.liabilityValueEquity);
    const flags1 = cA1.flags;
    if (verbose) {
      console.log("---admin health state---");
      const isHealthy = (flags1 & HEALTH_CACHE_HEALTHY) !== 0;
      const engineOk = (flags1 & HEALTH_CACHE_ENGINE_OK) !== 0;
      const oracleOk = (flags1 & HEALTH_CACHE_ORACLE_OK) !== 0;
      console.log("healthy: " + isHealthy);
      console.log("engine ok: " + engineOk);
      console.log("oracle ok: " + oracleOk);
      console.log("mrgn error: " + cA1.mrgnErr);
      console.log("internal error: " + cA1.internalErr);
      console.log("index of err:   " + cA1.errIndex);
      console.log("bankrpt error: " + cA1.internalBankruptcyErr);
      console.log("asset value: " + assetValue1.toString());
      console.log("liab value: " + liabValue1.toString());
      console.log("asset value (maint): " + aValMaint1.toString());
      console.log("liab value (maint): " + lValMaint1.toString());
      console.log("asset value (equity): " + aValEquity1.toString());
      console.log("liab value equity): " + lValEquity1.toString());
      console.log("prices: ");
      for (let i = 0; i < cA1.prices.length; i++) {
        const price = bytesToF64(cA1.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
      console.log("");
    }

    // Record the asset value the program thinks that the other depositor in the now-bankrupt bank
    // has, we'll note that it declines post bankruptcy as assets are marked down.
    groupAdminAssets = assetValue1.toNumber();

    // The bankrupt user is now totally broke
    assert.equal(assetValue.toNumber(), 0);
    // He still has debts
    assert.isAbove(liabValue.toNumber(), 0);
    // The bank is still underwater
    assert.isAbove(bankValues.liability, bankValues.asset);
    // The bank has assets to be marked down
    assert.isAbove(bankValues.asset, 0);
  });

  it("(admin) Retries to withdraw, but bank 1 is still bankrupt - should fail", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(0.01 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
    remainingAccounts.push([banks[1], oracles.pythPullLst.publicKey]);

    const tx = new Transaction();
    tx.add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount,
        withdrawAll: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // IllegalUtilizationRatio
    assertBankrunTxFailed(result, 6026);
  });

  // Note: The only way to unblock a "super bankrupt" bank where liabs > assets for the entire bank,
  // like this, is to deposit enough funds to cover the liabilities on its books. Otherwise it must
  // be wiped out (zeroed out)
  it("(admin) Tries to deposit into the bankrupt bank - should succeed ", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(2 * 10 ** ecosystem.lstAlphaDecimals);

    const tx = new Transaction();
    tx.add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        amount,
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });

  // Note: There's no reason for user 0 to do this, they're already bankrupt so there's no risk of
  // further liquidation to them. But, perhaps they're feeling charitable.
  it("(user 0) Tries to repay to bankrupt bank - should succeed ", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(2 * 10 ** ecosystem.lstAlphaDecimals);

    const tx = new Transaction();
    tx.add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        amount,
        remaining: [],
        repayAll: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(admin) Tries to bankrupt user 0 as wrong admin (not group/risk one) - should fail", async () => {
    const admin = emodeAdmin; // sneaky sneaky
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx = new Transaction();
    tx.add(
      await handleBankruptcy(admin.mrgnBankrunProgram, {
        signer: admin.wallet.publicKey,
        marginfiAccount: userAccount,
        bank: banks[1],
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(admin.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(admin) Bankrupts user 0 - happy path", async () => {
    const admin = riskAdmin;
    const user = users[0];
    // Note: this is the non-bankrupt depositor into banks[1] that will be haircut.
    const groupAdminAccount = groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY);
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const user0AccBefore = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const borrowIndex = user0AccBefore.lendingAccount.balances.findIndex(
      (balance) => balance.bankPk.equals(banks[1])
    );
    const balanceBefore = user0AccBefore.lendingAccount.balances[borrowIndex];
    const bankAccBefore = await bankrunProgram.account.bank.fetch(banks[1]);

    const tx = new Transaction();
    tx.add(
      await handleBankruptcy(admin.mrgnBankrunProgram, {
        signer: admin.wallet.publicKey,
        marginfiAccount: userAccount,
        bank: banks[1],
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(admin.wallet);
    await banksClient.processTransaction(tx);

    const pulseTx = new Transaction();
    pulseTx.add(
      await healthPulse(user.mrgnProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      }),
      await healthPulse(groupAdmin.mrgnProgram, {
        marginfiAccount: groupAdminAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    pulseTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    pulseTx.sign(user.wallet);
    await banksClient.processTransaction(pulseTx);

    const bankAcc = await bankrunProgram.account.bank.fetch(banks[1]);
    let bankValues = {
      asset:
        wrappedI80F48toBigNumber(bankAcc.assetShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalAssetShares).toNumber(),
      liability:
        wrappedI80F48toBigNumber(bankAcc.liabilityShareValue).toNumber() *
        wrappedI80F48toBigNumber(bankAcc.totalLiabilityShares).toNumber(),
    };

    if (verbose) {
      console.log("values after bankruptcy (in token):");
      console.log(
        `  asset: ${bankValues.asset}, liab: ${bankValues.liability}`
      );
      console.log(`  diff: ${bankValues.liability - bankValues.asset}`);
      console.log(
        "asset shares: " +
          wrappedI80F48toBigNumber(bankAcc.totalAssetShares).toNumber()
      );
      console.log(
        "asset share value: " +
          wrappedI80F48toBigNumber(bankAcc.assetShareValue).toNumber()
      );
      console.log(
        "liab shares: " +
          wrappedI80F48toBigNumber(bankAcc.totalLiabilityShares).toNumber()
      );
      console.log(
        "liab share value: " +
          wrappedI80F48toBigNumber(bankAcc.liabilityShareValue).toNumber()
      );

      console.log("good bank (0): " + banks[0]);
      console.log("bankrupt bank (1) : " + banks[1]);
    }

    const groupAdminAcc =
      await groupAdmin.mrgnBankrunProgram.account.marginfiAccount.fetch(
        groupAdminAccount
      );
    const user0Acc =
      await groupAdmin.mrgnBankrunProgram.account.marginfiAccount.fetch(
        userAccount
      );

    const bankValueMap = {};
    for (let pk of banks) {
      const acc = await bankrunProgram.account.bank.fetch(pk);
      bankValueMap[pk.toString()] = {
        asset: wrappedI80F48toBigNumber(acc.assetShareValue).toNumber(),
        liability: wrappedI80F48toBigNumber(acc.liabilityShareValue).toNumber(),
      };
    }

    dumpAccBalances(groupAdminAcc, bankValueMap);
    dumpAccBalances(user0Acc, bankValueMap);

    // TODO replace with log health cache function that's somewhere in another branch
    const cA0 = user0Acc.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cA0.assetValue);
    const liabValue = wrappedI80F48toBigNumber(cA0.liabilityValue);
    const aValMaint = wrappedI80F48toBigNumber(cA0.assetValueMaint);
    const lValMaint = wrappedI80F48toBigNumber(cA0.liabilityValueMaint);
    const aValEquity = wrappedI80F48toBigNumber(cA0.assetValueEquity);
    const lValEquity = wrappedI80F48toBigNumber(cA0.liabilityValueEquity);
    const flags = cA0.flags;
    if (verbose) {
      console.log("---user health state---");
      const isHealthy = (flags & HEALTH_CACHE_HEALTHY) !== 0;
      const engineOk = (flags & HEALTH_CACHE_ENGINE_OK) !== 0;
      const oracleOk = (flags & HEALTH_CACHE_ORACLE_OK) !== 0;
      console.log("healthy: " + isHealthy);
      console.log("engine ok: " + engineOk);
      console.log("oracle ok: " + oracleOk);
      console.log("mrgn error: " + cA0.mrgnErr);
      console.log("internal error: " + cA0.internalErr);
      console.log("index of err:   " + cA0.errIndex);
      console.log("bankrpt error: " + cA0.internalBankruptcyErr);
      console.log("asset value: " + assetValue.toString());
      console.log("liab value: " + liabValue.toString());
      console.log("asset value (maint): " + aValMaint.toString());
      console.log("liab value (maint): " + lValMaint.toString());
      console.log("asset value (equity): " + aValEquity.toString());
      console.log("liab value equity): " + lValEquity.toString());
      console.log("prices: ");
      for (let i = 0; i < cA0.prices.length; i++) {
        const price = bytesToF64(cA0.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
      console.log("");
    }

    const cA1 = groupAdminAcc.healthCache;

    const assetValue1 = wrappedI80F48toBigNumber(cA1.assetValue);
    const liabValue1 = wrappedI80F48toBigNumber(cA1.liabilityValue);
    const aValMaint1 = wrappedI80F48toBigNumber(cA1.assetValueMaint);
    const lValMaint1 = wrappedI80F48toBigNumber(cA1.liabilityValueMaint);
    const aValEquity1 = wrappedI80F48toBigNumber(cA1.assetValueEquity);
    const lValEquity1 = wrappedI80F48toBigNumber(cA1.liabilityValueEquity);
    const flags1 = cA1.flags;
    if (verbose) {
      console.log("---admin health state---");
      const isHealthy = (flags1 & HEALTH_CACHE_HEALTHY) !== 0;
      const engineOk = (flags1 & HEALTH_CACHE_ENGINE_OK) !== 0;
      const oracleOk = (flags1 & HEALTH_CACHE_ORACLE_OK) !== 0;
      console.log("healthy: " + isHealthy);
      console.log("engine ok: " + engineOk);
      console.log("oracle ok: " + oracleOk);
      console.log("mrgn error: " + cA1.mrgnErr);
      console.log("internal error: " + cA1.internalErr);
      console.log("index of err:   " + cA1.errIndex);
      console.log("bankrpt error: " + cA1.internalBankruptcyErr);
      console.log("asset value: " + assetValue1.toString());
      console.log("liab value: " + liabValue1.toString());
      console.log("asset value (maint): " + aValMaint1.toString());
      console.log("liab value (maint): " + lValMaint1.toString());
      console.log("asset value (equity): " + aValEquity1.toString());
      console.log("liab value equity): " + lValEquity1.toString());
      console.log("prices: ");
      for (let i = 0; i < cA1.prices.length; i++) {
        const price = bytesToF64(cA1.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
      console.log("");
    }

    // The bank is wiped out, its collateral is worthless
    assertI80F48Equal(bankAcc.assetShareValue, 0);

    // The user's bad debt is cleared
    const balanceAfter = user0Acc.lendingAccount.balances[borrowIndex];
    const debtBefore = wrappedI80F48toBigNumber(
      balanceBefore.liabilityShares
    ).toNumber();
    assert.isAbove(debtBefore, 0);
    // ??? A few shares are left over, is this a rounding issue?
    assertI80F48Approx(balanceAfter.liabilityShares, 0, 100);

    // The user eating the socialized loss takes quite a haircut: the entire value of their deposit
    assert.isAbove(groupAdminAssets, assetValue1.toNumber());

    // The bank's debt was cleared too
    const bankDebtBefore = wrappedI80F48toBigNumber(
      bankAccBefore.totalLiabilityShares
    ).toNumber();
    assertI80F48Approx(
      bankAcc.totalLiabilityShares,
      bankDebtBefore - debtBefore,
      100
    );
    assert.deepEqual(bankAcc.config.operationalState, {
      killedByBankruptcy: {},
    });
  });
});
