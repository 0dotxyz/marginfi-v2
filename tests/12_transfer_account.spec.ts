import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { Program } from "@coral-xyz/anchor";
import { Marginfi } from "../target/types/marginfi";
import {
  bankrunProgram,
  marginfiGroup,
  users,
  globalFeeWallet,
} from "./rootHooks";
import {
  accountInit,
  transferAccountAuthorityIx,
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

let program: Program<Marginfi>;

describe("Transfer account authority", () => {
  before(() => {
    program = bankrunProgram;
  });

  const oldAccKeypair = Keypair.generate();
  const newAccKeypair = Keypair.generate();
  const newAuthority = Keypair.generate();

  // Here the user moves authority to some new wallet. WARN: User picks the new authority with no
  // restrictions!
  it("(user 0) migrate some account a new authority - happy path", async () => {
    const feeWalletBefore = await program.provider.connection.getAccountInfo(
      globalFeeWallet
    );

    let tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: oldAccKeypair.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [oldAccKeypair]);

    let tx2 = new Transaction().add(
      await transferAccountAuthorityIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair.publicKey,
        newAccount: newAccKeypair.publicKey,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: globalFeeWallet,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx2, [newAccKeypair]);

    const newAcc = await program.account.marginfiAccount.fetch(
      newAccKeypair.publicKey
    );
    const oldAcc = await program.account.marginfiAccount.fetch(
      oldAccKeypair.publicKey
    );
    assertBNEqual(newAcc.lastUpdate, oldAcc.lastUpdate);
    const feeWalletAfter = await program.provider.connection.getAccountInfo(
      globalFeeWallet
    );
    assertKeysEqual(newAcc.authority, newAuthority.publicKey);
    assertKeysEqual(newAcc.migratedFrom, oldAccKeypair.publicKey);
    assertKeyDefault(newAcc.migratedTo);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);
    assertKeysEqual(oldAcc.migratedTo, newAccKeypair.publicKey);
    assert.equal(
      feeWalletBefore.lamports,
      feeWalletAfter.lamports - ACCOUNT_TRANSFER_FEE
    );
  });

  it("(user 0) tries to migrate their old account again - should fail", async () => {
    const anotherNewKeypair = Keypair.generate();

    let tx = new Transaction().add(
      await transferAccountAuthorityIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair.publicKey,
        newAccount: anotherNewKeypair.publicKey,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: globalFeeWallet,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, [
        anotherNewKeypair,
      ]);
    }, "AccountAlreadyMigrated");
  });

  // Here the user wants to retain ownership but move all their positions to a new account for some
  // reason (mostly this use-case applies to integrators that use accounts for whatever use-case)
  it("(user 0) migrate an account with positions to a new account - happy path", async () => {
    const oldAccKey = users[0].accounts.get(USER_ACCOUNT);
    const oldAccBefore = await program.account.marginfiAccount.fetch(oldAccKey);
    dumpAccBalances(oldAccBefore);
    const newAccKeypair = Keypair.generate();

    let tx = new Transaction().add(
      await transferAccountAuthorityIx(users[0].mrgnProgram, {
        oldAccount: oldAccKey,
        newAccount: newAccKeypair.publicKey,
        newAuthority: users[0].wallet.publicKey,
        globalFeeWallet: globalFeeWallet,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [newAccKeypair]);
    users[0].accounts.set(USER_ACCOUNT, newAccKeypair.publicKey);

    const newAcc = await program.account.marginfiAccount.fetch(
      newAccKeypair.publicKey
    );
    const oldAcc = await program.account.marginfiAccount.fetch(oldAccKey);
    dumpAccBalances(newAcc);
    dumpAccBalances(oldAcc);
    assertKeysEqual(newAcc.authority, oldAcc.authority);
    assertKeysEqual(newAcc.migratedFrom, oldAccKey);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);
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
    const oldAccKey = users[0].accounts.get(USER_ACCOUNT);
    const newAccKeypair = Keypair.generate();
    const newAuthority = Keypair.generate();

    let tx = new Transaction().add(
      await transferAccountAuthorityIx(users[0].mrgnProgram, {
        oldAccount: oldAccKey,
        newAccount: newAccKeypair.publicKey,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: users[0].wallet.publicKey,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, [newAccKeypair]);
    }, "InvalidFeeAta");
  });

  // TODO emissions destination and/or flags?

  it("(user 0) transfer account with separate fee payer - happy path", async () => {
    const oldAccKeypair = Keypair.generate();
    const newAccKeypair = Keypair.generate();
    const newAuthority = Keypair.generate();
    const separateFeePayer = Keypair.generate();

    // Fund the separate fee payer
    let fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: users[0].wallet.publicKey,
        toPubkey: separateFeePayer.publicKey,
        lamports: 1 * LAMPORTS_PER_SOL,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(fundTx, []);

    // Create the old account
    let tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: oldAccKeypair.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [oldAccKeypair]);

    // Transfer with separate fee payer
    let tx2 = new Transaction().add(
      await transferAccountAuthorityIx(users[0].mrgnProgram, {
        oldAccount: oldAccKeypair.publicKey,
        newAccount: newAccKeypair.publicKey,
        newAuthority: newAuthority.publicKey,
        globalFeeWallet: globalFeeWallet,
        feePayer: separateFeePayer.publicKey,
      })
    );

    await users[0].mrgnProgram.provider.sendAndConfirm(tx2, [
      newAccKeypair,
      separateFeePayer,
    ]);

    const newAcc = await program.account.marginfiAccount.fetch(
      newAccKeypair.publicKey
    );
    const oldAcc = await program.account.marginfiAccount.fetch(
      oldAccKeypair.publicKey
    );

    assertKeysEqual(newAcc.authority, newAuthority.publicKey);
    assertKeysEqual(newAcc.migratedFrom, oldAccKeypair.publicKey);
    assertKeyDefault(newAcc.migratedTo);
    assertBNEqual(oldAcc.accountFlags, ACCOUNT_DISABLED);
    assertKeysEqual(oldAcc.migratedTo, newAccKeypair.publicKey);
  });

  it("(user 0) transfer account after authority wallet ownership transferred to Token Program", async () => {
    const { TOKEN_PROGRAM_ID } = await import("@solana/spl-token");
    const authorityKeypair = Keypair.generate();
    const marginfiAccountKeypair = Keypair.generate();

    // Fund the authority wallet
    let fundTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: users[0].wallet.publicKey,
        toPubkey: authorityKeypair.publicKey,
        lamports: 10 * LAMPORTS_PER_SOL,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(fundTx, []);

    // Create marginfi account with this authority
    let createTx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: marginfiAccountKeypair.publicKey,
        authority: authorityKeypair.publicKey,
        feePayer: authorityKeypair.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(createTx, [
      authorityKeypair,
      marginfiAccountKeypair,
    ]);

    // Transfer ownership of the authority wallet to Token Program (arbitrary program)
    let assignTx = new Transaction().add(
      SystemProgram.assign({
        accountPubkey: authorityKeypair.publicKey,
        programId: TOKEN_PROGRAM_ID, // Use Token Program instead of marginfi
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(assignTx, [
      authorityKeypair,
    ]);

    // Verify the authority wallet is now owned by Token Program
    const authorityAccountInfo = await program.provider.connection.getAccountInfo(
      authorityKeypair.publicKey
    );
    assertKeysEqual(authorityAccountInfo.owner, TOKEN_PROGRAM_ID);

    // Now transfer the marginfi account using the authority whose wallet ownership was transferred to Token Program
    // The signature should still work because we're signing with the private key
    const newAccKeypair = Keypair.generate();
    const newAuthority = Keypair.generate();

    let transferIx = await transferAccountAuthorityIx(users[0].mrgnProgram, {
      oldAccount: marginfiAccountKeypair.publicKey,
      newAccount: newAccKeypair.publicKey,
      newAuthority: newAuthority.publicKey,
      globalFeeWallet: globalFeeWallet,
      authority: authorityKeypair.publicKey,
      feePayer: users[0].wallet.publicKey,
    });

    // Build transaction with users[0] as fee payer (since authorityKeypair can no longer pay fees after ownership transfer)
    // But authorityKeypair must still sign as the authority
    let transferTx = new Transaction().add(transferIx);
    transferTx.feePayer = users[0].wallet.publicKey;
    const { blockhash, lastValidBlockHeight } =
      await program.provider.connection.getLatestBlockhash();
    transferTx.recentBlockhash = blockhash;

    // Sign with both the authority (whose wallet is now owned by Token Program) and the new account keypair
    // The key insight: signature verification works regardless of which program owns the authority wallet
    transferTx.partialSign(authorityKeypair, newAccKeypair);

    // Also need the fee payer to sign
    await users[0].mrgnProgram.provider.wallet.signTransaction(transferTx);

    // This should succeed - signature verification works regardless of wallet ownership
    const signature = await program.provider.connection.sendRawTransaction(
      transferTx.serialize()
    );
    await program.provider.connection.confirmTransaction({
      signature,
      blockhash,
      lastValidBlockHeight,
    });

    // Verify the transfer succeeded
    const newAcc = await program.account.marginfiAccount.fetch(
      newAccKeypair.publicKey
    );
    assertKeysEqual(newAcc.authority, newAuthority.publicKey);
    assertKeysEqual(newAcc.migratedFrom, marginfiAccountKeypair.publicKey);
  });
});
