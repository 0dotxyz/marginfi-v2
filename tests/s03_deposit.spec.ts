import { BN, Program } from "@coral-xyz/anchor";
import { Marginfi } from "../target/types/marginfi";
import { Keypair, Transaction } from "@solana/web3.js";
import {
  stakedBankKeypairSol,
  stakedBankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  stakedMarginfiGroup,
  users,
  validators,
} from "./rootHooks";
import { assertBankrunTxFailed, assertKeysEqual } from "./utils/genericTests";
import { assert } from "chai";
import { accountInit, depositIx } from "./utils/user-instructions";
import { LST_ATA, USER_ACCOUNT } from "./utils/mocks";
import { createMintToInstruction } from "@solana/spl-token";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";

let program: Program<Marginfi>;
let marginfiGroup: Keypair;
let bankKeypairSol: Keypair;
let bankKeypairUsdc: Keypair;

describe("Deposit funds (included staked assets)", () => {
  before(() => {
    program = bankrunProgram;
    marginfiGroup = stakedMarginfiGroup;
    bankKeypairSol = stakedBankKeypairSol;
    bankKeypairUsdc = stakedBankKeypairUsdc;
  });

  it("(Fund user 0 and user 1 USDC/WSOL token accounts", async () => {
    const payer = bankrunContext.payer;
    let tx = new Transaction();
    for (let i = 0; i < users.length; i++) {
      // Note: WSOL is really just an spl token in this implementation, we don't simulate the
      // exchange of SOL for WSOL, but that doesn't really matter.
      tx.add(
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          users[i].wsolAccount,
          payer.publicKey,
          100 * 10 ** ecosystem.wsolDecimals
        )
      );
      tx.add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          users[i].usdcAccount,
          payer.publicKey,
          10000 * 10 ** ecosystem.usdcDecimals
        )
      );
    }
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(payer);
    await banksClient.processTransaction(tx);
  });

  it("Initialize user accounts (if needed)", async () => {
    for (let i = 0; i < users.length; i++) {
      // ???: should we be recycling these between the main test suite and bankrun?
      if (users[i].accounts.get(USER_ACCOUNT)) {
        console.log("Warn: Over-writing user acc for user " + i);
      }
      const userAccKeypair = Keypair.generate();
      const userAccount = userAccKeypair.publicKey;
      users[i].accounts.set(USER_ACCOUNT, userAccount);

      let user1Tx: Transaction = new Transaction();
      user1Tx.add(
        await accountInit(program, {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: userAccount,
          authority: users[i].wallet.publicKey,
          feePayer: users[i].wallet.publicKey,
        })
      );
      user1Tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      user1Tx.sign(users[i].wallet, userAccKeypair);
      await banksClient.processTransaction(user1Tx);
    }
  });

  it("(user 0) deposit USDC to bank - happy path", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: user.usdcAccount,
        amount: new BN(10 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    // Verify the deposit worked and the account exists
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    assertKeysEqual(balances[0].bankPk, bankKeypairUsdc.publicKey);
  });

  it("(user 0) cannot deposit to staked bank if regular deposits exists - should fail", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(1 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // AssetTagMismatch
    assertBankrunTxFailed(result, "0x179f");

    // Verify the deposit failed and the entry does not exist
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 0);
  });

  it("(user 1) deposits SOL to SOL bank - happy path", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        amount: new BN(2 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.tryProcessTransaction(tx);

    // Verify the deposit worked and the account exists
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    assertKeysEqual(balances[0].bankPk, bankKeypairSol.publicKey);
  });

  it("(user 1) deposits to staked bank - should succeed (SOL co-mingle is allowed)", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(1 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    // Verify the deposit worked and the entry exists
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const depositIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(validators[0].bank)
    );
    assert.notEqual(depositIndex, -1);
  });

  it("(user 1) cannot deposit to regular banks (USDC) with staked assets - should fail", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: user.usdcAccount,
        amount: new BN(1 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // AssetTagMismatch
    assertBankrunTxFailed(result, "0x179f");

    // Verify the deposit failed and the entry does not exist
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[2].active, 0);
  });

  it("(user 2) deposits to staked bank - should succeed", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(1 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    // Verify the deposit worked and the entry exists
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const depositIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(validators[0].bank)
    );
    assert.notEqual(depositIndex, -1);
  });
});
