import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
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
  riskAdmin,
} from "./rootHooks";
import {
  configBankEmode,
  configureBank,
  configureDeleverageWithdrawalLimit,
  groupConfigure,
  setFixedPrice,
} from "./utils/group-instructions";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { assert } from "chai";
import {
  CONF_INTERVAL_MULTIPLE,
  defaultBankConfigOptRaw,
  newEmodeEntry,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  liquidateIx,
  initLiquidationRecordIx,
  startLiquidationIx,
  endLiquidationIx,
  withdrawIx,
  repayIx,
  startDeleverageIx,
  endDeleverageIx,
} from "./utils/user-instructions";
import { deriveLiquidationRecord } from "./utils/pdas";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  bytesToF64,
  dumpAccBalances,
  dumpBankrunLogs,
  processBankrunTransaction,
} from "./utils/tools";
import { genericMultiBankTestSetup } from "./genericSetups";
import { getEpochAndSlot } from "./utils/stake-utils";
import {
  assertBankrunTxFailed,
  assertKeyDefault,
  assertKeysEqual,
} from "./utils/genericTests";

const startingSeed: number = 299;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_1234000000M2");

/** This is the program-enforced maximum enforced number of balances per account. */
const MAX_BALANCES = 16;
const USER_ACCOUNT_THROWAWAY = "throwaway_account3";

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;
let remainingAccounts: PublicKey[][] = [];
let lookupTable: PublicKey;

describe("Limits on number of accounts, with emode in effect", () => {
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

  it("(admin) set the group admin as the emode admin too", async () => {
    const tx = new Transaction();
    tx.add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        newAdmin: groupAdmin.wallet.publicKey,
        newEmodeAdmin: groupAdmin.wallet.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(emode admin) Configures bank emodes - happy path", async () => {
    for (let bankIndex = 0; bankIndex < banks.length; bankIndex++) {
      const bank = banks[bankIndex];

      // pick 10 unique, random tags from 0..MAX_BALANCES-1 (excluding the last bank)
      const entryTags = [...Array(MAX_BALANCES - 1).keys()] // [0,1,2,…,14]
        .sort(() => Math.random() - 0.5) // shuffle
        .slice(0, 10); // take first 10

      // build the 10 entries for this bank with random tags and values
      // Banks have liability weights of 1.0, so asset weights must be lower to avoid
      // exceeding leverage limits. Adjusted ranges to stay well under 15x/20x limits:
      const entries = entryTags.map((entryTag) =>
        newEmodeEntry(
          entryTag,
          1, // applies to isolated doesn't matter here
          bigNumberToWrappedI80F48(Math.random() * 0.2 + 0.6), // random 0.6–0.8 (~3.3x-5x leverage)
          bigNumberToWrappedI80F48(Math.random() * 0.1 + 0.8) // random 0.8–0.9 (~5x-10x leverage)
        )
      );

      // construct & send the tx for this bank
      const tx = new Transaction();
      tx.add(
        await configBankEmode(groupAdmin.mrgnBankrunProgram, {
          bank,
          tag: bankIndex, // bank’s own tag = its index
          entries,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      await banksClient.processTransaction(tx);
    }
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
      await banksClient.processTransaction(tx);
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
    await banksClient.processTransaction(tx);

    for (let i = 1; i < banks.length; i += 1) {
      const remainingAccounts: PublicKey[][] = [];
      for (let k = 0; k <= i; k++) {
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

  it("(admin) Vastly increases last bank liability ratio to make user 0 unhealthy", async () => {
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
      // console.log("bank: " + banks[i]);
    }

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
    const liquidateeAccounts = composeRemainingAccounts(remainingAccounts);

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
    // dumpBankrunLogs(result);

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

  it("(user 1) Creates LUT", async () => {
    const liquidator = users[1];
    for (let i = 0; i < MAX_BALANCES; i++) {
      remainingAccounts.push([banks[i], oracles.pythPullLst.publicKey]);
    }

    const recentSlot = Number(await banksClient.getSlot());
    const [createLutIx, lutAddress] =
      AddressLookupTableProgram.createLookupTable({
        authority: liquidator.wallet.publicKey,
        payer: liquidator.wallet.publicKey,
        recentSlot: recentSlot - 1,
      });
    lookupTable = lutAddress;

    let createLutTx = new Transaction().add(createLutIx);
    createLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    createLutTx.sign(liquidator.wallet);
    await banksClient.processTransaction(createLutTx);

    let extendLutTx1 = new Transaction().add(
      AddressLookupTableProgram.extendLookupTable({
        authority: liquidator.wallet.publicKey,
        payer: liquidator.wallet.publicKey,
        lookupTable,
        addresses: remainingAccounts.flat().slice(0, 20),
      })
    );
    extendLutTx1.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    extendLutTx1.sign(liquidator.wallet);
    await banksClient.processTransaction(extendLutTx1);

    let extendLutTx2 = new Transaction().add(
      AddressLookupTableProgram.extendLookupTable({
        authority: liquidator.wallet.publicKey,
        payer: liquidator.wallet.publicKey,
        lookupTable,
        addresses: remainingAccounts.flat().slice(20),
      })
    );
    extendLutTx2.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    extendLutTx2.sign(liquidator.wallet);
    await banksClient.processTransaction(extendLutTx2);

    // We must advance the bankrun slot to allow the lut to activate
    const ONE_MINUTE = 60;
    const slotsToAdvance = ONE_MINUTE * 0.4;
    let { epoch: _, slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(slot + slotsToAdvance));
  });

  it("(user 1) Liquidates user 0 with start/end", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];

    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      liquidateeAccount
    );

    const mrgnAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(liquidateeAccount);
    assertKeyDefault(mrgnAccountBefore.liquidationRecord);
    dumpAccBalances(mrgnAccountBefore);

    let tx = new Transaction();
    tx.add(
      await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        feePayer: liquidator.wallet.publicKey,
        // liquidationRecord: liqRecord,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidator.wallet);
    await banksClient.processTransaction(tx);

    const recordBefore = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    assertKeysEqual(recordBefore.key, liqRecordKey);
    assertKeysEqual(recordBefore.recordPayer, liquidator.wallet.publicKey);
    assertKeysEqual(recordBefore.marginfiAccount, liquidateeAccount);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        // liquidationRecord: liqRecord,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: composeRemainingAccounts(remainingAccounts),
      }),
      await withdrawIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: banks[0],
        tokenAccount: liquidator.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(0.105 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: banks[MAX_BALANCES - 1],
        tokenAccount: liquidator.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(0.1 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
      })
    );
    const blockhash = await getBankrunBlockhash(bankrunContext);
    const lutRaw = await banksClient.getAccount(lookupTable);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lookupTable,
      state: lutState,
    });
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [...tx.instructions],
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([liquidator.wallet]);
    await banksClient.processTransaction(versionedTx);

    const recordAfter = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    const mrgnAccountAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    dumpAccBalances(mrgnAccountAfter);
    assertKeysEqual(mrgnAccountAfter.liquidationRecord, liqRecordKey);

    const entry = recordAfter.entries[3];
    assert(entry.timestamp.toNumber() > 0);

    // Note: asset seized and liability repaid are scaled to the oracle confidence adjustment
    const seized = bytesToF64(entry.assetAmountSeized);
    const repaid = bytesToF64(entry.liabAmountRepaid);
    if (verbose) {
      console.log("asset seized: " + seized);
      console.log("liab repaid: " + repaid);
      console.log("theoretical profit: " + (seized - repaid));
    }
    const expectedAssets =
      0.105 * oracles.lstAlphaPrice -
      0.105 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(seized, expectedAssets, 0.001);
    const expectedLiabs =
      0.1 * oracles.lstAlphaPrice +
      0.1 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(repaid, expectedLiabs, 0.001);

    // other slots (0-2) should still be zero
    for (let i = 0; i < 3; i++) {
      assert(recordAfter.entries[i].timestamp.toNumber() == 0);
    }
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

  it("(admin) Deleverages user 0 by fully repaying bank 2's liabs", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);

    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      deleverageeAccount
    );

    const mrgnAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(deleverageeAccount);
    dumpAccBalances(mrgnAccountBefore);

    const recordBefore = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    assertKeysEqual(recordBefore.key, liqRecordKey);
    assertKeysEqual(recordBefore.marginfiAccount, deleverageeAccount);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccounts(remainingAccounts),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[0],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(1.0 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[2],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[2])
        ),
        amount: new BN(0),
        repayAll: true,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[2])
        ),
      })
    );
    remainingAccounts = remainingAccounts.filter((a) => a[0] != banks[2]);

    const blockhash = await getBankrunBlockhash(bankrunContext);
    const lutRaw = await banksClient.getAccount(lookupTable);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lookupTable,
      state: lutState,
    });
    const messageV0 = new TransactionMessage({
      payerKey: riskAdmin.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [...tx.instructions],
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([riskAdmin.wallet]);
    await banksClient.processTransaction(versionedTx);

    const recordAfter = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    const mrgnAccountAfter = await bankrunProgram.account.marginfiAccount.fetch(
      deleverageeAccount
    );
    dumpAccBalances(mrgnAccountAfter);
    assertKeysEqual(mrgnAccountAfter.liquidationRecord, liqRecordKey);

    const entry = recordAfter.entries[3];
    assert(entry.timestamp.toNumber() > 0);

    // Note: asset seized and liability repaid are scaled to the oracle confidence adjustment
    const seized = bytesToF64(entry.assetAmountSeized);
    const repaid = bytesToF64(entry.liabAmountRepaid);
    if (verbose) {
      console.log("asset seized: " + seized);
      console.log("liab repaid: " + repaid);
      console.log("theoretical profit: " + (seized - repaid));
    }
    const expectedAssets =
      1.0 * oracles.lstAlphaPrice -
      1.0 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(seized, expectedAssets, 0.001);
    const expectedLiabs =
      1.0 * oracles.lstAlphaPrice +
      1.0 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(repaid, expectedLiabs, 0.001);

    // the first two slots (0-1) should still be zero
    for (let i = 0; i < 2; i++) {
      assert(recordAfter.entries[i].timestamp.toNumber() == 0);
    }
  });

  it("(admin) Allows tokenless repayments for banks 3 & 4", async () => {
    let config = defaultBankConfigOptRaw();
    config.tokenlessRepaymentsAllowed = true;

    let tx = new Transaction();
    for (const i of [3, 4]) {
      tx.add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: banks[i],
          bankConfigOpt: config,
        })
      );
    }
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(admin) Deleverages user 0 by fully (tokenlessly) repaying bank 3's liabs", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);

    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      deleverageeAccount
    );

    const mrgnAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(deleverageeAccount);
    dumpAccBalances(mrgnAccountBefore);

    const recordBefore = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    assertKeysEqual(recordBefore.key, liqRecordKey);
    assertKeysEqual(recordBefore.marginfiAccount, deleverageeAccount);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccounts(remainingAccounts),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[0],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(1.0 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[3],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[3])
        ),
        amount: new BN(0),
        repayAll: true,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[3])
        ),
      })
    );
    remainingAccounts = remainingAccounts.filter((a) => a[0] != banks[3]);

    const blockhash = await getBankrunBlockhash(bankrunContext);
    const lutRaw = await banksClient.getAccount(lookupTable);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lookupTable,
      state: lutState,
    });
    const messageV0 = new TransactionMessage({
      payerKey: riskAdmin.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [...tx.instructions],
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([riskAdmin.wallet]);
    await banksClient.processTransaction(versionedTx);

    const recordAfter = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    const mrgnAccountAfter = await bankrunProgram.account.marginfiAccount.fetch(
      deleverageeAccount
    );
    dumpAccBalances(mrgnAccountAfter);
    assertKeysEqual(mrgnAccountAfter.liquidationRecord, liqRecordKey);

    const entry = recordAfter.entries[3];
    assert(entry.timestamp.toNumber() > 0);

    // Note: asset seized and liability repaid are scaled to the oracle confidence adjustment
    const seized = bytesToF64(entry.assetAmountSeized);
    const repaid = bytesToF64(entry.liabAmountRepaid);
    if (verbose) {
      console.log("asset seized: " + seized);
      console.log("liab repaid: " + repaid);
      console.log("theoretical profit: " + (seized - repaid));
    }
    const expectedAssets =
      1.0 * oracles.lstAlphaPrice -
      1.0 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(seized, expectedAssets, 0.001);
    const expectedLiabs =
      1.0 * oracles.lstAlphaPrice +
      1.0 *
        oracles.lstAlphaPrice *
        ORACLE_CONF_INTERVAL *
        CONF_INTERVAL_MULTIPLE;
    assert.approximately(repaid, expectedLiabs, 0.001);

    // the first slot (0) should still be zero
    assert(recordAfter.entries[0].timestamp.toNumber() == 0);
  });

  it("(admin) Sets the group withdrawal limit to $1 less than bank 4's liability", async () => {
    const tx = new Transaction();
    tx.add(
      await configureDeleverageWithdrawalLimit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        limit: 1 * ecosystem.lstAlphaPrice - 1, // borrowAmount is 1 LST Alpha
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(admin) Tries to deleverage user 0 by fully (tokenlessly) repaying bank 4's liabs - limit exceeded", async () => {
    const deleveragee = users[0];
    const deleverageeAccount = deleveragee.accounts.get(USER_ACCOUNT_THROWAWAY);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccounts(remainingAccounts),
      }),
      await withdrawIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[0],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(1.0 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: banks[4],
        tokenAccount: riskAdmin.lstAlphaAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[4])
        ),
        amount: new BN(0),
        repayAll: true,
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: composeRemainingAccounts(
          remainingAccounts.filter((a) => a[0] != banks[4])
        ),
      })
    );
    const blockhash = await getBankrunBlockhash(bankrunContext);
    const lutRaw = await banksClient.getAccount(lookupTable);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lookupTable,
      state: lutState,
    });
    const messageV0 = new TransactionMessage({
      payerKey: riskAdmin.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [...tx.instructions],
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([riskAdmin.wallet]);

    let result = await banksClient.tryProcessTransaction(versionedTx);
    // 6101 (DailyWithdrawalLimitExceeded)
    assertBankrunTxFailed(result, "0x17d5");
  });

  it("(admin) Sets various banks to a fixed price", async () => {
    let tx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        price: oracles.lstAlphaPrice,
      }),
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[5],
        price: oracles.lstAlphaPrice,
      }),
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[6],
        price: oracles.lstAlphaPrice,
      }),
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: banks[MAX_BALANCES - 1],
        price: oracles.lstAlphaPrice,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(user 1) Liquidates user 0 with start/end - some banks use fixed prices", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];

    // Exclude oracles from the fixed-priced banks' remaining accounts
    remainingAccounts = remainingAccounts.map((a) => {
      if (
        a[0] == banks[0] ||
        a[0] == banks[5] ||
        a[0] == banks[6] ||
        a[0] == banks[MAX_BALANCES - 1]
      ) {
        return [a[0]];
      } else {
        return a;
      }
    });

    // Note: Liquidation record already exists from previous round

    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      liquidateeAccount
    );

    const mrgnAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(liquidateeAccount);
    assertKeysEqual(mrgnAccountBefore.liquidationRecord, liqRecordKey);

    const recordBefore = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    assertKeysEqual(recordBefore.key, liqRecordKey);
    assertKeysEqual(recordBefore.recordPayer, liquidator.wallet.publicKey);
    assertKeysEqual(recordBefore.marginfiAccount, liquidateeAccount);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        // liquidationRecord: liqRecord,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: composeRemainingAccounts(remainingAccounts),
      }),
      await withdrawIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: banks[0],
        tokenAccount: liquidator.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(0.105 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: banks[MAX_BALANCES - 1],
        tokenAccount: liquidator.lstAlphaAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
        amount: new BN(0.1 * 10 ** ecosystem.lstAlphaDecimals),
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
      })
    );
    const blockhash = await getBankrunBlockhash(bankrunContext);
    const lutRaw = await banksClient.getAccount(lookupTable);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lookupTable,
      state: lutState,
    });
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [...tx.instructions],
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([liquidator.wallet]);
    //await banksClient.processTransaction(versionedTx);
    let result = await banksClient.tryProcessTransaction(versionedTx);
    dumpBankrunLogs(result);

    const recordAfter = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey
    );
    const mrgnAccountAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    assertKeysEqual(mrgnAccountAfter.liquidationRecord, liqRecordKey);

    // Note: We have the entry from the previous round (which was two deleverages ago) before as well.
    const oldEntry = recordAfter.entries[0];
    assert(oldEntry.timestamp.toNumber() > 0);

    const entry = recordAfter.entries[3];
    assert(entry.timestamp.toNumber() > 0);

    // Note: we did the same liquidation twice: they should be identical, but not quite! The fixed
    // oracle doesn't have a confidence applied, so here we have seized using the raw price with no
    // confidence adjustments. (Of course, the raw amount seized is actually the same)
    const t = 0.00000001;
    const assetsActual = bytesToF64(entry.assetAmountSeized);
    const assetsExpected =
      assetsActual -
      assetsActual * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    assert.approximately(
      assetsExpected,
      bytesToF64(oldEntry.assetAmountSeized),
      t
    );

    // Same for liabilities, we sorta repaid less (in $) because the confidence interval didn't
    // apply here. But again, the actual amount repaid, in token, is unchanged.
    const liabActual = bytesToF64(entry.liabAmountRepaid);
    const liabExpected =
      liabActual + liabActual * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    assert.approximately(
      liabExpected,
      bytesToF64(oldEntry.liabAmountRepaid),
      t
    );

    // Note: asset seized and liability repaid are scaled to the oracle confidence adjustment
    const seized = bytesToF64(entry.assetAmountSeized);
    const repaid = bytesToF64(entry.liabAmountRepaid);
    if (verbose) {
      console.log("asset seized: " + seized);
      console.log("liab repaid: " + repaid);
      console.log("theoretical profit: " + (seized - repaid));
    }
    const expectedAssets = 0.105 * oracles.lstAlphaPrice;
    assert.approximately(seized, expectedAssets, 0.001);
    const expectedLiabs = 0.1 * oracles.lstAlphaPrice;
    assert.approximately(repaid, expectedLiabs, 0.001);

    // All slots are filled now!!
    for (let i = 0; i < 4; i++) {
      assert(recordAfter.entries[i].timestamp.toNumber() != 0);
    }
  });

  // TODO try these with switchboard oracles.
});
