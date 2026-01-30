import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
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
  globalProgramAdmin,
} from "./rootHooks";
import { configureBank, setFixedPrice } from "./utils/group-instructions";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { assert } from "chai";
import { defaultBankConfigOptRaw } from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  liquidateIx,
  withdrawIx,
} from "./utils/user-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { dumpAccBalances, processBankrunTransaction } from "./utils/tools";
import { genericMultiBankTestSetup } from "./genericSetups";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assertI80F48Approx, assertKeyDefault } from "./utils/genericTests";

const startingSeed: number = 199;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_1234000000M1");

/** This is the program-enforced maximum enforced number of balances per account. */
const MAX_BALANCES = 16;
const USER_ACCOUNT_THROWAWAY = "throwaway_account1";

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;

describe("Limits on number of accounts when using Kamino", () => {
  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      MAX_BALANCES,
      USER_ACCOUNT_THROWAWAY,
      groupBuff,
      startingSeed
    );
    banks = result.banks;
    throwawayGroup = result.throwawayGroup;
  });

  it("Refresh oracles", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("(admin) Seeds liquidity in all banks - validates 16 deposits is possible", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    // Note: This is about the max per TX without using LUTs.
    const depositsPerTx = 5;

    for (let i = 0; i < banks.length; i += depositsPerTx) {
      const chunk = banks.slice(i, i + depositsPerTx);
      const tx = new Transaction();
      for (const bank of chunk) {
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
      await banksClient.tryProcessTransaction(tx);
    }
  });

  it("(user 0) Borrows 15 positions against 1 - validates max borrows possible", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    const borrowAmount = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);
    let oomAt = MAX_BALANCES;

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
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.tryProcessTransaction(tx);

    for (let i = 1; i < banks.length; i += 1) {
      const remainingAccounts: PublicKey[][] = [];
      remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
      for (let k = 1; k <= i; k++) {
        remainingAccounts.push([banks[k], oracles.pythPullLst.publicKey]);
      }

      const tx = new Transaction();
      tx.add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
        ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: banks[i],
          tokenAccount: user.lstAlphaAccount,
          remaining: composeRemainingAccounts(remainingAccounts),
          amount: borrowAmount,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(user.wallet);
      let result = await banksClient.tryProcessTransaction(tx);
      console.log("***********" + i + " ***********");
      //dumpBankrunLogs(result);

      // Throws if the error is not OOM.
      if (result.result) {
        const logs = result.meta.logMessages;
        const isOOM = logs.some((msg) =>
          msg.toLowerCase().includes("memory allocation failed, out of memory")
        );

        if (isOOM) {
          oomAt = i + 1;
          console.warn(`⚠️ \t OOM during borrow on bank ${i}: \n`, logs);
          console.log("MAXIMUM ACCOUNTS BEFORE MEMORY FAILURE: " + oomAt);
          assert.ok(false);
        } else {
          // anything other than OOM should blow up the test
          throw new Error(
            `Unexpected borrowIx failure on bank ${banks[i].toBase58()}: ` +
              logs.join("\n")
          );
        }
      }
    }
    console.log("No memory failures detected on " + MAX_BALANCES + " accounts");
  });

  it("(admin) vastly increase last bank liability ratio to make user 0 unhealthy", async () => {
    let config = defaultBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(210); // 21000%
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(200); // 20000%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[MAX_BALANCES - 1],
        bankConfigOpt: config,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(user 1) Liquidates user 0", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    const liquidateAmount = new BN(0.01 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    for (let i = 0; i < MAX_BALANCES; i++) {
      remainingAccounts.push([banks[i], oracles.pythPullLst.publicKey]);
    }
    let liquidateeAccounts = composeRemainingAccounts(remainingAccounts);

    // Deposit some funds to operate as a liquidator...
    let tx = new Transaction();
    tx.add(
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: banks[0],
        tokenAccount: liquidator.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidator.wallet);
    await banksClient.tryProcessTransaction(tx);

    const liquidateeAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    dumpAccBalances(liquidateeAcc);
    const liquidatorAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    dumpAccBalances(liquidatorAcc);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: banks[0],
        liabilityBankKey: banks[MAX_BALANCES - 1],
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.pythPullLst.publicKey, // asset oracle
          oracles.pythPullLst.publicKey, // liab oracle

          ...composeRemainingAccounts([
            // liquidator accounts
            [banks[0], oracles.pythPullLst.publicKey],
            [banks[MAX_BALANCES - 1], oracles.pythPullLst.publicKey],
          ]),

          ...liquidateeAccounts,
        ],
        amount: liquidateAmount,
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: 4,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidator.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    //dumpBankrunLogs(result);

    // Throws if the error is not OOM.
    if (result.result) {
      const logs = result.meta.logMessages;
      const isOOM = logs.some((msg) =>
        msg.toLowerCase().includes("memory allocation failed, out of memory")
      );

      if (isOOM) {
        console.warn(`⚠️ \t OOM during liquidate: \n`, logs);
        assert.ok(false);
      } else {
        // anything other than OOM should blow up the test
        throw new Error(`Unexpected liquidate failure}: ` + logs.join("\n"));
      }
    }
  });

  it("(admin) Sets banks[1] and banks[4] to a fixed price", async () => {
    let tx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[1],
        price: oracles.lstAlphaPrice,
      }),
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[4],
        price: oracles.lstAlphaPrice,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const bankAfter = await bankrunProgram.account.bank.fetch(banks[1]);
    assert.deepEqual(bankAfter.config.oracleSetup, { fixed: {} });
    assertI80F48Approx(
      bankAfter.config.fixedPrice,
      bigNumberToWrappedI80F48(oracles.lstAlphaPrice)
    );
    assertKeyDefault(bankAfter.config.oracleKeys[0]);
  });

  it("(user 1) Liquidates user 0 (with some banks using fixed oracles)", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidateAmount = new BN(0.001 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    // Note: stupid hack because bankrun doesn't have fetchMultiple. In prod, use `fetchMultiple`.
    const bankAccs = await Promise.all(
      banks.map(async (pubkey) => {
        const account = await bankrunProgram.account.bank.fetch(pubkey);
        return { pubkey, account };
      })
    );
    for (let i = 0; i < MAX_BALANCES; i++) {
      const setup = bankAccs[i].account.config.oracleSetup;
      const keys = bankAccs[i].account.config.oracleKeys;
      // Note: there's probably a more clever way to compare the enum kind.
      if ("fixed" in setup) {
        remainingAccounts.push([banks[i]]);
      } else if ("stakedWithPythPush" in setup) {
        remainingAccounts.push([banks[i], keys[0], keys[1], keys[2]]);
      } else if ("kaminoPythPush" in setup) {
        remainingAccounts.push([banks[i], keys[0], keys[1]]);
      } else if ("kaminoSwitchboardPull" in setup) {
        remainingAccounts.push([banks[i], keys[0], keys[1]]);
      } else {
        remainingAccounts.push([banks[i], keys[0]]);
      }
    }
    let liquidateeAccounts = composeRemainingAccounts(remainingAccounts);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: banks[0],
        liabilityBankKey: banks[MAX_BALANCES - 1],
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.pythPullLst.publicKey, // asset oracle
          oracles.pythPullLst.publicKey, // liab oracle

          ...composeRemainingAccounts([
            // liquidator accounts
            [banks[0], oracles.pythPullLst.publicKey],
            [banks[MAX_BALANCES - 1], oracles.pythPullLst.publicKey],
          ]),

          ...liquidateeAccounts,
        ],
        amount: liquidateAmount,
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: 4,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [liquidator.wallet]);
  });

  it("(admin) Sets banks[0] (asset) and banks[MAX-1] (liab) to a fixed price", async () => {
    let tx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        price: oracles.lstAlphaPrice,
      }),
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[MAX_BALANCES - 1],
        price: oracles.lstAlphaPrice,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(user 1) Liquidates user 0 (with asset/liab fixed + other banks fixed)", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidateAmount = new BN(0.0011 * 10 ** ecosystem.lstAlphaDecimals);

    const remainingAccounts: PublicKey[][] = [];
    // Note: stupid hack because bankrun doesn't have fetchMultiple. In prod, use `fetchMultiple`.
    const bankAccs = await Promise.all(
      banks.map(async (pubkey) => {
        const account = await bankrunProgram.account.bank.fetch(pubkey);
        return { pubkey, account };
      })
    );

    for (let i = 0; i < MAX_BALANCES; i++) {
      // Note: there's probably a more clever way to compare the enum kind.
      if ("fixed" in bankAccs[i].account.config.oracleSetup) {
        remainingAccounts.push([banks[i]]);
      } else {
        remainingAccounts.push([banks[i], oracles.pythPullLst.publicKey]);
      }
    }
    let liquidateeAccounts = composeRemainingAccounts(remainingAccounts);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: banks[0],
        liabilityBankKey: banks[MAX_BALANCES - 1],
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          // no asset oracle needed, it's fixed!
          // no liab oracle needed, it's fixed!

          ...composeRemainingAccounts([
            // liquidator accounts
            [banks[0]], // fixed!
            [banks[MAX_BALANCES - 1]], // fixed!
          ]),

          ...liquidateeAccounts,
        ],
        amount: liquidateAmount,
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: 2,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [liquidator.wallet]);
  });

  it("(admin) sets user's main liability to be near-worthless through a fixed price", async () => {
    // Note: we maxed the borrow limit in the previous test, raise it to enable some more borrows
    let config = defaultBankConfigOptRaw();
    config.borrowLimit = config.borrowLimit.muln(2);

    let tx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[MAX_BALANCES - 1],
        price: 0.00001,
      }),
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[MAX_BALANCES - 1],
        bankConfigOpt: config,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const bankAfter = await bankrunProgram.account.bank.fetch(
      banks[MAX_BALANCES - 1]
    );
    assertI80F48Approx(
      bankAfter.config.fixedPrice,
      bigNumberToWrappedI80F48(0.00001)
    );
  });

  it("(user 0) now able to borrow/withdraw again (with multiple fixed banks)", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const remainingAccounts: PublicKey[][] = [];
    const bankAccs = await Promise.all(
      banks.map(async (pubkey) => {
        const account = await bankrunProgram.account.bank.fetch(pubkey);
        return { pubkey, account };
      })
    );

    for (let i = 0; i < MAX_BALANCES; i++) {
      if ("fixed" in bankAccs[i].account.config.oracleSetup) {
        remainingAccounts.push([banks[i]]);
      } else {
        remainingAccounts.push([banks[i], oracles.pythPullLst.publicKey]);
      }
    }

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[MAX_BALANCES - 1],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(42),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(42),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  });

  // TODO try these with switchboard oracles.
});
