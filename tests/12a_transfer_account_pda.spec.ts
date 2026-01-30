import { Keypair, Transaction } from "@solana/web3.js";
import { Program } from "@coral-xyz/anchor";
import { Marginfi } from "../target/types/marginfi";
import { bankrunProgram, marginfiGroup, users, globalFeeWallet } from "./rootHooks";
import {
  accountInit,
  transferAccountAuthorityPdaIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { assert } from "chai";
import {
  assertBNEqual,
  assertI80F48Equal,
  assertKeyDefault,
  assertKeysEqual,
  expectFailedTxWithMessage,
} from "./utils/genericTests";
import {
  ACCOUNT_DISABLED,
  ACCOUNT_TRANSFER_FEE,
  I80F48_ZERO,
} from "./utils/types";
import { dumpAccBalances } from "./utils/tools";
import { deriveMarginfiAccountPda } from "./utils/pdas";

let program: Program<Marginfi>;

describe("Transfer account authority to PDA", () => {
  before(() => {
    program = bankrunProgram;
  });

  const oldAccKeypair = Keypair.generate();
  const newAuthority = Keypair.generate();

  it("(user 0) migrate keypair account to PDA account - happy path", async () => {
    const feeWalletBefore = await program.provider.connection.getAccountInfo(
      globalFeeWallet
    );

    // Create old keypair-based account
    let tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: oldAccKeypair.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [oldAccKeypair]);

    // Derive PDA for new account
    const accountIndex = 0;
    const [newAccountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      newAuthority.publicKey,
      accountIndex
    );

    // Transfer to PDA account
    let tx2 = new Transaction().add(
      await transferAccountAuthorityPdaIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair.publicKey,
        newAccount: newAccountPda,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: globalFeeWallet,
        accountIndex: accountIndex,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx2, []);

    const newAcc = await program.account.marginfiAccount.fetch(newAccountPda);
    const oldAcc = await program.account.marginfiAccount.fetch(
      oldAccKeypair.publicKey
    );
    const feeWalletAfter = await program.provider.connection.getAccountInfo(
      globalFeeWallet
    );

    assertKeysEqual(newAcc.authority, newAuthority.publicKey);
    assertKeysEqual(newAcc.migratedFrom, oldAccKeypair.publicKey);
    assertKeyDefault(newAcc.migratedTo);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);
    assertKeysEqual(oldAcc.migratedTo, newAccountPda);
    assert.equal(
      feeWalletBefore.lamports,
      feeWalletAfter.lamports - ACCOUNT_TRANSFER_FEE
    );
  });

  it("(user 0) migrate account with third-party ID", async () => {
    const oldAccKeypair2 = Keypair.generate();
    const newAuthority2 = Keypair.generate();

    // Create old keypair-based account
    let tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: oldAccKeypair2.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [oldAccKeypair2]);

    // Derive PDA for new account with third-party ID
    const accountIndex = 42;
    const thirdPartyId = 200;
    const [newAccountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      newAuthority2.publicKey,
      accountIndex,
      thirdPartyId
    );

    // Transfer to PDA account with third-party ID
    let tx2 = new Transaction().add(
      await transferAccountAuthorityPdaIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair2.publicKey,
        newAccount: newAccountPda,
        newAuthority: newAuthority2.publicKey,
        globalFeeWallet: globalFeeWallet,
        accountIndex: accountIndex,
        thirdPartyId: thirdPartyId,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx2, []);

    const newAcc = await program.account.marginfiAccount.fetch(newAccountPda);
    const oldAcc = await program.account.marginfiAccount.fetch(
      oldAccKeypair2.publicKey
    );

    assertKeysEqual(newAcc.authority, newAuthority2.publicKey);
    assertKeysEqual(newAcc.migratedFrom, oldAccKeypair2.publicKey);
    assertKeyDefault(newAcc.migratedTo);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);
    assertKeysEqual(oldAcc.migratedTo, newAccountPda);
    assert.equal(newAcc.accountIndex, accountIndex);
    assert.equal(newAcc.thirdPartyIndex, thirdPartyId)
    assert.equal(newAcc.bump, bump);
  });

  it("(user 0) tries to migrate their old account again - should fail", async () => {
    const accountIndex = 1;
    const [anotherNewPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      newAuthority.publicKey,
      accountIndex
    );

    let tx = new Transaction().add(
      await transferAccountAuthorityPdaIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair.publicKey,
        newAccount: anotherNewPda,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: globalFeeWallet,
        accountIndex: accountIndex,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);
    }, "AccountAlreadyMigrated");
  });

  it("(user 0) migrate an account with positions to a PDA - happy path", async () => {
    const oldAccKey = users[0].accounts.get(USER_ACCOUNT);
    const oldAccBefore = await program.account.marginfiAccount.fetch(oldAccKey);

    const accountIndex = 5; // Use a different index to avoid collision with 06a tests
    const [newAccountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,  // Keep same authority
      accountIndex
    );

    let tx = new Transaction().add(
      await transferAccountAuthorityPdaIx(users[0].mrgnProgram, {
        oldAccount: oldAccKey,
        newAccount: newAccountPda,
        newAuthority: users[0].wallet.publicKey,
        globalFeeWallet: globalFeeWallet,
        accountIndex: accountIndex,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);
    users[0].accounts.set(USER_ACCOUNT, newAccountPda);

    const newAcc = await program.account.marginfiAccount.fetch(newAccountPda);
    const oldAcc = await program.account.marginfiAccount.fetch(oldAccKey);

    assertKeysEqual(newAcc.authority, users[0].wallet.publicKey);
    assertKeysEqual(newAcc.migratedFrom, oldAccKey);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);

    // Verify balances were transferred correctly
    for (let i = 0; i < newAcc.lendingAccount.balances.length; i++) {
      const balOld = oldAccBefore.lendingAccount.balances[i];
      const balNew = newAcc.lendingAccount.balances[i];
      assertKeysEqual(balOld.bankPk, balNew.bankPk);
      assertI80F48Equal(balOld.assetShares, balNew.assetShares);
      assertI80F48Equal(balOld.liabilityShares, balNew.liabilityShares);
      assert.equal(balOld.active, balNew.active);
      assert.equal(balOld.bankAssetTag, balNew.bankAssetTag);
      assertI80F48Equal(
        balOld.emissionsOutstanding,
        balNew.emissionsOutstanding
      );

      // The old account is now empty
      const balEmpty = oldAcc.lendingAccount.balances[i];
      assertKeyDefault(balEmpty.bankPk);
      assertI80F48Equal(balEmpty.assetShares, I80F48_ZERO);
      assertI80F48Equal(balEmpty.liabilityShares, I80F48_ZERO);
    }
  });

  it("(user 0) migrate account with bad global fee wallet - should fail", async () => {
    const oldAccKeypair3 = Keypair.generate();
    const newAuthority3 = Keypair.generate();

    // Create old keypair-based account
    let tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: oldAccKeypair3.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [oldAccKeypair3]);

    const accountIndex = 0;
    const [newAccountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      newAuthority3.publicKey,
      accountIndex
    );

    let tx2 = new Transaction().add(
      await transferAccountAuthorityPdaIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair3.publicKey,
        newAccount: newAccountPda,
        newAuthority: newAuthority3.publicKey,
        globalFeeWallet: users[0].wallet.publicKey, // Wrong fee wallet
        accountIndex: accountIndex,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx2, []);
    }, "InvalidFeeAta");
  });

});
