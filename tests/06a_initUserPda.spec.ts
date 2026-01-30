import { Program } from "@coral-xyz/anchor";
import { Keypair, Transaction, PublicKey } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import { bankrunProgram, marginfiGroup, users } from "./rootHooks";
import {
  assertBNEqual,
  assertI80F48Equal,
  assertKeyDefault,
  assertKeysEqual,
  expectFailedTxWithMessage,
} from "./utils/genericTests";
import { assert } from "chai";
import { accountInitPda } from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { deriveMarginfiAccountPda } from "./utils/pdas";

let program: Program<Marginfi>;

describe("Initialize user account with PDA", () => {
  before(() => {
    program = bankrunProgram;
  });

  it("(user 0) Initialize PDA user account - happy path", async () => {
    const accountIndex = 555;
    const [accountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex
    );

    users[0].accounts.set("USER_ACCOUNT_PDA", accountPda);

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountPda,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
        accountIndex: accountIndex,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);

    const userAccount = await program.account.marginfiAccount.fetch(accountPda);
    assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
    assertKeysEqual(userAccount.authority, users[0].wallet.publicKey);
    assertKeyDefault(userAccount.migratedFrom);
    assertKeyDefault(userAccount.migratedTo);
    assert.equal(userAccount.accountIndex, accountIndex);
    assert.equal(userAccount.thirdPartyIndex, 0);
    assert.equal(userAccount.bump, bump);
  });

  it("(user 0) Initialize multiple PDA accounts with different indices", async () => {
    for (let i = 1; i < 4; i++) {
      const [accountPda, bump] = deriveMarginfiAccountPda(
        program.programId,
        marginfiGroup.publicKey,
        users[0].wallet.publicKey,
        i
      );

      let tx: Transaction = new Transaction();
      tx.add(
        await accountInitPda(program, {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: accountPda,
          authority: users[0].wallet.publicKey,
          feePayer: users[0].wallet.publicKey,
          accountIndex: i,
        })
      );
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);

      const userAccount = await program.account.marginfiAccount.fetch(
        accountPda
      );
      assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
      assertKeysEqual(userAccount.authority, users[0].wallet.publicKey);
      assert.equal(userAccount.accountIndex, i);

      users[0].accounts.set(`USER_ACCOUNT_PDA_${i}`, accountPda);
    }
  });

  it("(user 0) Initialize PDA account with third-party ID", async () => {
    const accountIndex = 0;
    const thirdPartyId = 100; // Non-restricted ID
    const [accountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex,
      thirdPartyId
    );

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountPda,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
        accountIndex: accountIndex,
        thirdPartyId: thirdPartyId,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);

    const userAccount = await program.account.marginfiAccount.fetch(accountPda);
    assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
    assertKeysEqual(userAccount.authority, users[0].wallet.publicKey);
    assert.equal(userAccount.thirdPartyIndex, thirdPartyId);

    users[0].accounts.set("USER_ACCOUNT_PDA_THIRD_PARTY", accountPda);
  });

  it("(user 1) Initialize PDA account for different user", async () => {
    const accountIndex = 0;
    const [accountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[1].wallet.publicKey,
      accountIndex
    );

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountPda,
        authority: users[1].wallet.publicKey,
        feePayer: users[1].wallet.publicKey,
        accountIndex: accountIndex,
      })
    );
    await users[1].mrgnProgram.provider.sendAndConfirm(tx, []);

    const userAccount = await program.account.marginfiAccount.fetch(accountPda);
    assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
    assertKeysEqual(userAccount.authority, users[1].wallet.publicKey);
    assert.equal(userAccount.accountIndex, accountIndex);
    assert.equal(userAccount.thirdPartyIndex, 0);
    assert.equal(userAccount.bump, bump);

    users[1].accounts.set("USER_ACCOUNT_PDA", accountPda);
  });

  it("(user 0) Try to initialize same PDA account twice - should fail", async () => {
    const accountIndex = 555;
    const [accountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex
    );

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountPda,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
        accountIndex: accountIndex,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);
    }, "already in use");
  });

  it("Verify PDA accounts have unique addresses", async () => {
    const accountIndex = 0;

    // Different authorities, same index
    const [pda1] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex
    );

    const [pda2] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[1].wallet.publicKey,
      accountIndex
    );

    // Same authority, different indices
    const [pda3] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      1
    );

    // Same authority, same index, different third-party ID
    const [pda4] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex,
      100
    );

    // All PDAs should be unique
    assert.notEqual(pda1.toBase58(), pda2.toBase58());
    assert.notEqual(pda1.toBase58(), pda3.toBase58());
    assert.notEqual(pda1.toBase58(), pda4.toBase58());
    assert.notEqual(pda2.toBase58(), pda3.toBase58());
    assert.notEqual(pda2.toBase58(), pda4.toBase58());
    assert.notEqual(pda3.toBase58(), pda4.toBase58());
  });

  it("(user 0) Try to initialize PDA account with restricted third-party ID 42 - should fail", async () => {
    const accountIndex = 10; // Use different index to avoid conflicts
    const restrictedThirdPartyId = 10_001;
    const [accountPda, bump] = deriveMarginfiAccountPda(
      program.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex,
      restrictedThirdPartyId
    );

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInitPda(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountPda,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
        accountIndex: accountIndex,
        thirdPartyId: restrictedThirdPartyId,
      })
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, []);
    }, "Unauthorized");
  });
});
