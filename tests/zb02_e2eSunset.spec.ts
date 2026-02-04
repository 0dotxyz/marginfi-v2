import { BN, Wallet } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  groupAdmin,
  bankrunContext,
  ecosystem,
  oracles,
  users,
  globalProgramAdmin,
  riskAdmin,
  bankRunProvider,
  bankrunProgram,
  verbose,
} from "./rootHooks";
import {
  configureBank,
  configureDeleverageWithdrawalLimit,
  groupConfigure,
} from "./utils/group-instructions";
import { assert } from "chai";
import {
  blankBankConfigOptRaw,
  defaultBankConfigOptRaw,
  ORACLE_CONF_INTERVAL,
  TOKENLESS_REPAYMENTS_ALLOWED,
  TOKENLESS_REPAYMENTS_COMPLETE,
} from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  composeRemainingAccountsByBalances,
  depositIx,
  endDeleverageIx,
  initLiquidationRecordIx,
  purgeDeveleragedBalance,
  repayIx,
  startDeleverageIx,
  withdrawIx,
} from "./utils/user-instructions";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  assertBankrunTxFailed,
  assertI80F48Approx,
  assertI80F48Equal,
  getTokenBalance,
} from "./utils/genericTests";
import { initOrUpdatePriceUpdateV2 } from "./utils/pyth-pull-mocks";
import {
  dumpAccBalances,
  dumpBankrunLogs,
  processBankrunTransaction,
} from "./utils/tools";
import { deriveLiquidityVault } from "./utils/pdas";

const USER_ACCOUNT_THROWAWAY = "throwaway_account_zb02";
const ONE_YEAR_IN_SECONDS = 2 * 365 * 24 * 60 * 60;

const startingSeed: number = 799;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_123400000ZB2");

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;

describe("Bank e2e sunset due to illiquid asset", () => {
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
    let now = Number(
      (await bankrunContext.banksClient.getClock()).unixTimestamp
    );
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

  it("(admin) Sets banks 0/1 asset weights to 0.9", async () => {
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
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(user 0) Deposits to bank 0, (user 1) Deposits to bank 1", async () => {
    const user0 = users[0];
    const user0Account = user0.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);

    const tx0 = new Transaction();
    tx0.add(
      await depositIx(user0.mrgnBankrunProgram, {
        marginfiAccount: user0Account,
        bank: banks[0],
        tokenAccount: user0.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, tx0, [user0.wallet]);

    const user1 = users[1];
    const user1Account = user1.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx1 = new Transaction();
    tx1.add(
      await depositIx(user1.mrgnBankrunProgram, {
        marginfiAccount: user1Account,
        bank: banks[1],
        tokenAccount: user1.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, tx1, [user1.wallet]);
  });

  it("(user 2) Deposits to bank 1", async () => {
    const user2 = users[2];
    const user2Account = user2.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx0 = new Transaction();
    tx0.add(
      await depositIx(user2.mrgnBankrunProgram, {
        marginfiAccount: user2Account,
        bank: banks[1],
        tokenAccount: user2.lstAlphaAccount,
        amount: new BN(42),
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, tx0, [user2.wallet]);
  });

  it("(user 0) Borrows bank 1 against bank 0, (user 1) Borrows bank 0 against bank 1", async () => {
    const user0 = users[0];
    const user0Account = user0.accounts.get(USER_ACCOUNT_THROWAWAY);
    const borrowAmount = new BN(85 * 10 ** ecosystem.lstAlphaDecimals);

    const tx0 = new Transaction();
    tx0.add(
      await borrowIx(user0.mrgnBankrunProgram, {
        marginfiAccount: user0Account,
        bank: banks[1],
        tokenAccount: user0.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
        amount: borrowAmount,
      })
    );
    await processBankrunTransaction(bankrunContext, tx0, [user0.wallet]);

    const user1 = users[1];
    const user1Account = user1.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx1 = new Transaction();
    tx1.add(
      await borrowIx(user1.mrgnBankrunProgram, {
        marginfiAccount: user1Account,
        bank: banks[0],
        tokenAccount: user1.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
        amount: borrowAmount,
      })
    );
    await processBankrunTransaction(bankrunContext, tx1, [user1.wallet]);
  });

  it("(admin) Sets the risk admin", async () => {
    const tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        newRiskAdmin: riskAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  // Note: here the admin pays during repayment, because tokenless repay is not enabled yet.
  it("(risk admin) Deleverages user 0 - happy path", async () => {
    const withdrawn = 2000;
    const repaid = 1998;
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const remainingAccounts = [
      [banks[0], oracles.pythPullLst.publicKey],
      [banks[1], oracles.pythPullLst.publicKey],
    ];

    let initRecordTx = new Transaction().add(
      await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        feePayer: riskAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, initRecordTx, [
      riskAdmin.wallet,
    ]);

    const lstBefore = await getTokenBalance(
      bankRunProvider,
      riskAdmin.lstAlphaAccount
    );

    let tx = await deleverageTx(
      banks[0],
      banks[1],
      deleverageeAccount,
      remainingAccounts,
      withdrawn,
      repaid
    );
    await processBankrunTransaction(bankrunContext, tx, [riskAdmin.wallet]);

    const lstAfter = await getTokenBalance(
      bankRunProvider,
      riskAdmin.lstAlphaAccount
    );
    assert.equal(lstAfter - lstBefore, withdrawn - repaid);
  });

  it("(attacker) Tries to set the withdrawal deleverage limit - should fail", async () => {
    const tx = new Transaction().add(
      await configureDeleverageWithdrawalLimit(users[0].mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        limit: 1,
      })
    );
    let result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [users[0].wallet],
      true
    );

    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  // Imagine at this point that all b1 liquidity is gone: there is no b1 token in any AMM! This
  // means we can't repay debts! The risk admin will set it into tokenless repayment mode.
  it("(admin) Allows tokenless repayments for bank 1", async () => {
    let config = defaultBankConfigOptRaw();
    config.tokenlessRepaymentsAllowed = true;
    config.assetWeightInit = bigNumberToWrappedI80F48(0.9);
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.9);

    const bankBefore = await groupAdmin.mrgnBankrunProgram.account.bank.fetch(
      banks[1]
    );
    assert.equal(
      (bankBefore.flags.toNumber() & TOKENLESS_REPAYMENTS_ALLOWED) ===
        TOKENLESS_REPAYMENTS_ALLOWED,
      false
    );

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[1],
        bankConfigOpt: config,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const bankAfter = await groupAdmin.mrgnBankrunProgram.account.bank.fetch(
      banks[1]
    );
    assert.equal(
      (bankAfter.flags.toNumber() & TOKENLESS_REPAYMENTS_ALLOWED) ===
        TOKENLESS_REPAYMENTS_ALLOWED,
      true
    );
  });

  // Note: Typically the bank admin would put the bank in reduce-only mode at this point anyways,
  // this is just to prevent a footgun where they forget and users continue to borrow the asset
  // about to be deleveraged
  it("(user 0) Tries to borrow bank 1 again - should fail", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const tx = new Transaction();
    tx.add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [banks[0], oracles.pythPullLst.publicKey],
          [banks[1], oracles.pythPullLst.publicKey],
        ]),
        amount: new BN(43),
      })
    );
    let result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      true
    );

    // Forbidden ix: no borrows once borrows are "worthless"
    assertBankrunTxFailed(result, 6089);
  });

  // Here the risk admin still pays during repayment, by choice, by not repaying all. This is
  // helpful when the risk admin has some b1 left to get rid of.
  it("(risk admin) Voluntarily repays during deleverage with tokenless enabled", async () => {
    const withdrawn = 2002;
    const repaid = 2000;
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const remainingAccounts = [
      [banks[0], oracles.pythPullLst.publicKey],
      [banks[1], oracles.pythPullLst.publicKey],
    ];

    const lstBefore = await getTokenBalance(
      bankRunProvider,
      riskAdmin.lstAlphaAccount
    );

    let tx = await deleverageTx(
      banks[0],
      banks[1],
      deleverageeAccount,
      remainingAccounts,
      withdrawn,
      repaid
    );
    await processBankrunTransaction(bankrunContext, tx, [riskAdmin.wallet]);

    const lstAfter = await getTokenBalance(
      bankRunProvider,
      riskAdmin.lstAlphaAccount
    );
    assert.equal(lstAfter - lstBefore, withdrawn - repaid);
  });

  // If the admin is to have enough funds to make lenders whole later, it must obtain enough of b0
  // to repay b1 lenders later. In this rare case, the last remaining b1 borrower is a b0 lender,
  // but there's not enough liquidity in b0 to withdraw the equivalent of their b1 liability! the
  // risk admin is stuck (until the next test)
  it("(risk admin) Attempt to repay all b1 - can't, not enough to withdraw in b0!", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);

    // Note: this is what user 0  borrowed up to this point, less one lamport!
    const withdrawn = 85 * 10 ** ecosystem.lstAlphaDecimals - 1998 - 2000 - 1;
    // Note: we're going to repay all, this doesn't matter
    const repaid = 0;
    const remainingAccounts = [
      [banks[0], oracles.pythPullLst.publicKey],
      [banks[1], oracles.pythPullLst.publicKey],
    ];

    let tx = await deleverageTx(
      banks[0],
      banks[1],
      deleverageeAccount,
      remainingAccounts,
      withdrawn,
      repaid,
      true
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [riskAdmin.wallet],
      true
    );

    // Illegal utilization ratio, withdrawing this much puts b0 below the utilization ratio!
    assertBankrunTxFailed(result, 6026);
  });

  // One option for the risk admin is to simply deposit organization or insurance funds into b0 as a
  // regular lender to boost withdraw liquidity. These funds aren't lost, just stuck in b0 until
  // more liquidity opens up to withdraw, which is fine because b0 isn't being deleveraged, it's a
  // "normal" asset in this example.
  it("(admin) Deposits to bank 0", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);

    const tx = new Transaction();
    tx.add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  });

  // Now there's enough b0 to seize to cover the b1 repayment!
  it("(risk admin) Attempt to repay all user b1 again - happy path", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);

    // Note: this is what user 0  borrowed up to this point, less one lamport!
    const withdrawn = 85 * 10 ** ecosystem.lstAlphaDecimals - 1998 - 2000 - 1;
    // Note: we're going to repay all, this doesn't matter
    const repaid = 12345;
    const remainingAccounts = [
      [banks[0], oracles.pythPullLst.publicKey],
      [banks[1], oracles.pythPullLst.publicKey],
    ];
    const [liqVault] = deriveLiquidityVault(bankrunProgram.programId, banks[1]);

    const [bankBefore, lstBefore, liqVaultBefore] = await Promise.all([
      bankrunProgram.account.bank.fetch(banks[1]),
      getTokenBalance(bankRunProvider, riskAdmin.lstAlphaAccount),
      getTokenBalance(bankRunProvider, liqVault),
    ]);

    let tx = await deleverageTx(
      banks[0],
      banks[1],
      deleverageeAccount,
      remainingAccounts,
      withdrawn,
      repaid,
      true,
      false,
      remainingAccounts.filter((a) => a[0] != banks[1])
    );
    await processBankrunTransaction(bankrunContext, tx, [riskAdmin.wallet]);

    const [bankAfter, lstAfter, liqVaultAfter] = await Promise.all([
      bankrunProgram.account.bank.fetch(banks[1]),
      getTokenBalance(bankRunProvider, riskAdmin.lstAlphaAccount),
      getTokenBalance(bankRunProvider, liqVault),
    ]);

    const sharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalLiabilityShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalLiabilityShares
    ).toNumber();
    if (verbose) {
      console.log(
        "liab shares before: " + sharesBefore + " after " + sharesAfter
      );
      console.log("risk admin now holds: " + lstAfter);
    }

    // Free money! No repay here! The risk admin now holds all of the "missing" debt (Note: in this
    // example, both banks use the same asset for simplicity. In production, this would be held in
    // b0, to be repaid to lenders in some substitute asset e.g. usdc, since we cannot buy b1 due to
    // the "liquidity issue" that set off this chain of events.)
    assert.equal(lstAfter - lstBefore, withdrawn);
    // No change to the liquidity vault
    assert.equal(liqVaultBefore, liqVaultAfter);
    // Shares still decline as expected
    // All debts discharged!
    assertI80F48Approx(bankAfter.totalLiabilityShares, 0);
    const expectedShares = 85 * 10 ** ecosystem.lstAlphaDecimals - 1998 - 2000;
    assertI80F48Approx(
      bankBefore.totalLiabilityShares,
      expectedShares,
      expectedShares * 0.001
    );

    // This complete the deleveraging of the bank
    assert.equal(
      (bankBefore.flags.toNumber() & TOKENLESS_REPAYMENTS_COMPLETE) ===
        TOKENLESS_REPAYMENTS_COMPLETE,
      false
    );
    assert.equal(
      (bankAfter.flags.toNumber() & TOKENLESS_REPAYMENTS_COMPLETE) ===
        TOKENLESS_REPAYMENTS_COMPLETE,
      true
    );
  });

  // Note: at this point we have discharged all b1 debts. But wait! There's not enough liquidity in
  // b1 for depositors to withdraw!

  it("(user 1) Attempts to withdraw b1 - gets just what's left in the liquidity vault!", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    // first repay our debt so we can withdraw without interference.
    // For repayAll, include all active balances, including the closing bank.
    const userAccBefore =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const remaining = composeRemainingAccountsByBalances(
      userAccBefore.lendingAccount.balances,
      [
        [banks[0], oracles.pythPullLst.publicKey],
        [banks[1], oracles.pythPullLst.publicKey],
      ]
    );
    const repayTx = new Transaction();
    repayTx.add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        remaining,
        amount: new BN(1234),
        repayAll: true,
      })
    );
    await processBankrunTransaction(bankrunContext, repayTx, [user.wallet]);

    const [liqVault] = deriveLiquidityVault(bankrunProgram.programId, banks[1]);

    const [bankBefore, userBefore, lstBefore, liqVaultBefore] =
      await Promise.all([
        bankrunProgram.account.bank.fetch(banks[1]),
        bankrunProgram.account.marginfiAccount.fetch(userAccount),
        getTokenBalance(bankRunProvider, user.lstAlphaAccount),
        getTokenBalance(bankRunProvider, liqVault),
      ]);

    // For withdrawAll, include all active balances, including the closing bank.
    const userAccBeforeWithdraw =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const remainingWithdraw = composeRemainingAccountsByBalances(
      userAccBeforeWithdraw.lendingAccount.balances,
      [
        [banks[1], oracles.pythPullLst.publicKey],
        [banks[0], oracles.pythPullLst.publicKey],
      ],
      banks[1]
    );
    const tx = new Transaction();
    tx.add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining: remainingWithdraw,
        amount: new BN(12345),
        withdrawAll: true,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const [bankAfter, userAfter, lstAfter, liqVaultAfter] = await Promise.all([
      bankrunProgram.account.bank.fetch(banks[1]),
      bankrunProgram.account.marginfiAccount.fetch(userAccount),
      getTokenBalance(bankRunProvider, user.lstAlphaAccount),
      getTokenBalance(bankRunProvider, liqVault),
    ]);

    const sharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalAssetShares
    ).toNumber();
    if (verbose) {
      console.log(
        "asset shares before: " + sharesBefore + " after " + sharesAfter
      );
      console.log("user before: " + lstBefore + " after: " + lstAfter);
    }

    // User gets what's left!
    assert.equal(lstAfter - lstBefore, liqVaultBefore);
    // Liquidity vault is empty
    assert.equal(liqVaultAfter, 0);
    // Balance closed!
    assert.equal(userBefore.lendingAccount.balances[0].active, 1);
    assert.equal(userAfter.lendingAccount.balances[0].active, 0);
    // Before we have user 1 and 2's balances, after all that's left is user 2's balance.
    assert.equal(sharesBefore, 100 * 10 ** 9 + 42);
    assert.equal(sharesAfter, 42);
  });

  // Here the admin would fund some "claims portal" using the proceeds it secured earlier to make
  // users whole OTC. Any remaining users with lending funds, for example user 2, will be purged so
  // they don't have this position in their accounts.

  // User 2 could also withdraw here, but if they don't bother, we would do this to simply close
  // their balance, simplifying bookkeeping.
  it("(risk admin) Purge user 2's remaining b1 lending account", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const [liqVault] = deriveLiquidityVault(bankrunProgram.programId, banks[1]);
    const [bankBefore, userBefore, lstBefore, liqVaultBefore] =
      await Promise.all([
        bankrunProgram.account.bank.fetch(banks[1]),
        bankrunProgram.account.marginfiAccount.fetch(userAccount),
        getTokenBalance(bankRunProvider, user.lstAlphaAccount),
        getTokenBalance(bankRunProvider, liqVault),
      ]);

    const tx = new Transaction();
    tx.add(
      await purgeDeveleragedBalance(riskAdmin.mrgnBankrunProgram, {
        account: userAccount,
        bank: banks[1],
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [riskAdmin.wallet]);

    const [bankAfter, userAfter, lstAfter, liqVaultAfter] = await Promise.all([
      bankrunProgram.account.bank.fetch(banks[1]),
      bankrunProgram.account.marginfiAccount.fetch(userAccount),
      getTokenBalance(bankRunProvider, user.lstAlphaAccount),
      getTokenBalance(bankRunProvider, liqVault),
    ]);

    // User gets nothing, we're out of money!
    assert.equal(lstAfter - lstBefore, 0);
    // Liquidity vault is empty, and was empty before!
    assert.equal(liqVaultAfter, 0);
    assert.equal(liqVaultBefore, 0);
    // Balance closed!
    assert.equal(userBefore.lendingAccount.balances[0].active, 1);
    assert.equal(userAfter.lendingAccount.balances[0].active, 0);
    // Bank fully cleared!
    assert.ok(
      wrappedI80F48toBigNumber(bankBefore.totalAssetShares).toNumber() > 0
    );
    assertI80F48Equal(bankAfter.totalAssetShares, 0);
  });

  const deleverageTx = async (
    withdrawBank: PublicKey,
    repayBank: PublicKey,
    deleverageeAccount: PublicKey,
    remainingAccounts: PublicKey[][],
    withdrawn: number,
    repaid: number,
    repayAll?: boolean,
    withdrawAll?: boolean,
    // If repaying all, remove the account being repaid.
    endAccounts?: PublicKey[][]
  ) => {
    const accountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(deleverageeAccount);
    const repayRemaining = composeRemainingAccountsByBalances(
      accountBefore.lendingAccount.balances,
      remainingAccounts
    );

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(remainingAccounts),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: withdrawBank,
        tokenAccount: riskAdmin.lstAlphaAccount,
        // Note: A withdraw within a deleverage only requires the oracle accounts for the bank being
        // withdrawn, you can actually omit all the other accounts!
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(withdrawn),
        withdrawAll: withdrawAll,
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: repayBank,
        tokenAccount: riskAdmin.lstAlphaAccount,
        // For repay_all, include all active balances, including the closing bank.
        remaining: repayRemaining,
        amount: new BN(repaid),
        repayAll: repayAll,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccountsMetaBanksOnly(
          endAccounts ? endAccounts : remainingAccounts
        ),
      })
    );
    return tx;
  };
});
