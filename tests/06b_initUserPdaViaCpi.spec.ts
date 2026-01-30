import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  Transaction,
  SYSVAR_INSTRUCTIONS_PUBKEY,
} from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import { Mocks } from "../target/types/mocks";
import {
  marginfiGroup,
  users,
  bankrunProgram,
  mocksBankrunProgram,
} from "./rootHooks";
import {
  assertKeysEqual,
  assertKeyDefault,
  expectFailedTxWithMessage,
} from "./utils/genericTests";
import { assert } from "chai";
import { deriveMarginfiAccountPda } from "./utils/pdas";

let marginfiProgram: Program<Marginfi>;
let mocksProgram: Program<Mocks>;

describe("Initialize user account with PDA via CPI", () => {
  before(() => {
    marginfiProgram = bankrunProgram;
    mocksProgram = mocksBankrunProgram;
  });

  it("(user 0) Initialize PDA user account via CPI - happy path", async () => {
    const accountIndex = 50; // Use high index to avoid conflicts with other tests
    const [accountPda, bump] = deriveMarginfiAccountPda(
      marginfiProgram.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex
    );

    const callLogKeypair = Keypair.generate();

    let tx: Transaction = new Transaction();
    tx.add(
      await mocksProgram.methods
        .createMarginfiAccountPdaViaCpi(accountIndex, null)
        .accounts({
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: accountPda,
          authority: users[0].wallet.publicKey,
          feePayer: users[0].wallet.publicKey,
          instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
          marginfiProgram: marginfiProgram.programId,
          callLog: callLogKeypair.publicKey,
        })
        .instruction()
    );

    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [callLogKeypair]);

    // Verify the marginfi account was created correctly
    const userAccount = await marginfiProgram.account.marginfiAccount.fetch(
      accountPda
    );
    assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
    assertKeysEqual(userAccount.authority, users[0].wallet.publicKey);
    assertKeyDefault(userAccount.migratedFrom);
    assertKeyDefault(userAccount.migratedTo);
    assert.equal(userAccount.accountIndex, accountIndex);
    // Here the CPI caller opted to use the default (not reserved) index of 0
    assert.equal(userAccount.thirdPartyIndex, 0);
    assert.equal(userAccount.bump, bump);

    // Verify the call log was created
    const callLog = await mocksProgram.account.cpiCallLog.fetch(
      callLogKeypair.publicKey
    );
    assertKeysEqual(callLog.callerProgram, mocksProgram.programId);
    assertKeysEqual(callLog.targetProgram, marginfiProgram.programId);
    assertKeysEqual(callLog.marginfiGroup, marginfiGroup.publicKey);
    assertKeysEqual(callLog.marginfiAccount, accountPda);
    assertKeysEqual(callLog.authority, users[0].wallet.publicKey);
    assert.equal(callLog.accountIndex, accountIndex);
    assert.equal(callLog.thirdPartyId, null);

    users[0].accounts.set("USER_ACCOUNT_PDA_CPI", accountPda);
    66;
  });

  it("(user 0) Initialize PDA account via CPI with restricted third-party ID 42 - should succeed", async () => {
    const accountIndex = 51; // Use different index
    const restrictedThirdPartyId = 10_001;
    const [accountPda, bump] = deriveMarginfiAccountPda(
      marginfiProgram.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex,
      restrictedThirdPartyId
    );

    const callLogKeypair = Keypair.generate();

    let tx: Transaction = new Transaction();
    tx.add(
      await mocksProgram.methods
        .createMarginfiAccountPdaViaCpi(accountIndex, restrictedThirdPartyId)
        .accounts({
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: accountPda,
          authority: users[0].wallet.publicKey,
          feePayer: users[0].wallet.publicKey,
          instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
          marginfiProgram: marginfiProgram.programId,
          callLog: callLogKeypair.publicKey,
        })
        .instruction()
    );

    // This should succeed because it's called via CPI from the authorized mocks program
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [callLogKeypair]);

    // Verify the marginfi account was created
    const userAccount = await marginfiProgram.account.marginfiAccount.fetch(
      accountPda
    );
    assertKeysEqual(userAccount.group, marginfiGroup.publicKey);
    assertKeysEqual(userAccount.authority, users[0].wallet.publicKey);
    assert.equal(userAccount.accountIndex, accountIndex);
    // Here the CPI caller used their registed restricted ID
    assert.equal(userAccount.thirdPartyIndex, restrictedThirdPartyId);
    assert.equal(userAccount.bump, bump);

    // Verify the call log includes the third-party ID
    const callLog = await mocksProgram.account.cpiCallLog.fetch(
      callLogKeypair.publicKey
    );
    assert.equal(callLog.thirdPartyId, restrictedThirdPartyId);

    // Doing the same thing with a restricted seed fails

    const [badPda, _badBump] = deriveMarginfiAccountPda(
      marginfiProgram.programId,
      marginfiGroup.publicKey,
      users[0].wallet.publicKey,
      accountIndex,
      10_555
    );
    let anotherCallLogKeypair = Keypair.generate();
    tx = new Transaction();
    tx.add(
      await mocksProgram.methods
        .createMarginfiAccountPdaViaCpi(accountIndex, 10_555)
        .accounts({
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: badPda,
          authority: users[0].wallet.publicKey,
          feePayer: users[0].wallet.publicKey,
          instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
          marginfiProgram: marginfiProgram.programId,
          callLog: anotherCallLogKeypair.publicKey,
        })
        .instruction()
    );

    await expectFailedTxWithMessage(async () => {
      await users[0].mrgnProgram.provider.sendAndConfirm(tx, [
        anotherCallLogKeypair,
      ]);
    }, "Unauthorized");
  });
});
