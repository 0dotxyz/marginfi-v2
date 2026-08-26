import { Keypair, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  groupConfigure,
  resizeGlobalFeeState,
  resizeGroupAccount,
} from "../../utils/group-instructions";
import { deriveGlobalFeeState } from "../../utils/pdas";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  groupAdmin,
  marginfiGroup,
  users,
} from "../../rootHooks";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import { getBankrunBlockhash, processBankrunTransaction } from "../../utils/tools";

/** Current group layout size (8-byte discriminator + MarginfiGroup::LEN) */
const GROUP_ACCOUNT_LEN = 8 + 9248;
/** v1 group layout size, as on mainnet before this release */
const GROUP_V1_ACCOUNT_LEN = 8 + 1056;
/** Current fee-state layout size (8-byte discriminator + FeeState::LEN) */
const FEE_STATE_ACCOUNT_LEN = 8 + 512;
/** v1 fee-state layout size, as on mainnet before this release */
const FEE_STATE_V1_ACCOUNT_LEN = 8 + 256;

describe("02a: Account resize (v1 -> current layout migration)", () => {
  it("new groups and the fee state are allocated at the current size", async () => {
    const group = await banksClient.getAccount(marginfiGroup.publicKey);
    assert.equal(group.data.length, GROUP_ACCOUNT_LEN);
    const [feeStateKey] = deriveGlobalFeeState(bankrunProgram.programId);
    const feeState = await banksClient.getAccount(feeStateKey);
    assert.equal(feeState.data.length, FEE_STATE_ACCOUNT_LEN);
  });

  it("a v1-sized group is bricked until the permissionless resize; state survives", async () => {
    const adminBefore = (
      await bankrunProgram.account.marginfiGroup.fetch(marginfiGroup.publicKey)
    ).admin;

    // Simulate the mainnet group as it exists BEFORE this deploy: truncate to the v1 size
    const account = await banksClient.getAccount(marginfiGroup.publicKey);
    const v1Data = Buffer.from(account.data).subarray(0, GROUP_V1_ACCOUNT_LEN);
    bankrunContext.setAccount(marginfiGroup.publicKey, {
      ...account,
      data: v1Data,
    });

    // Undersized, the PROGRAM cannot load the account — this is the (brief) window between
    // the program upgrade and the resize transaction. (The TS coder itself is lenient and
    // still decodes the prefix, so probe with a real instruction.)
    const brickedTx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        newAdmin: null,
        marginfiGroup: marginfiGroup.publicKey,
      }),
    );
    brickedTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    brickedTx.sign(groupAdmin.wallet);
    const brickedResult = await banksClient.tryProcessTransaction(brickedTx);
    assert.isNotNull(brickedResult.result);

    // Anyone can resize; any wallet pays the rent delta
    const tx = new Transaction().add(
      await resizeGroupAccount(users[0].mrgnBankrunProgram, {
        group: marginfiGroup.publicKey,
        payer: users[0].wallet.publicKey,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [users[0].wallet]);

    const after = await banksClient.getAccount(marginfiGroup.publicKey);
    assert.equal(after.data.length, GROUP_ACCOUNT_LEN);
    // v1 prefix byte-identical, growth zero-filled
    assert.deepEqual(
      Buffer.from(after.data).subarray(0, GROUP_V1_ACCOUNT_LEN),
      v1Data,
    );
    assert.isTrue(
      Buffer.from(after.data)
        .subarray(GROUP_V1_ACCOUNT_LEN)
        .every((b) => b === 0),
    );
    const groupAfter = await bankrunProgram.account.marginfiGroup.fetch(
      marginfiGroup.publicKey,
    );
    assert.equal(groupAfter.admin.toString(), adminBefore.toString());
  });

  it("the fee state migrates the same way", async () => {
    const [feeStateKey] = deriveGlobalFeeState(bankrunProgram.programId);
    const account = await banksClient.getAccount(feeStateKey);
    const v1Data = Buffer.from(account.data).subarray(
      0,
      FEE_STATE_V1_ACCOUNT_LEN,
    );
    bankrunContext.setAccount(feeStateKey, { ...account, data: v1Data });

    const tx = new Transaction().add(
      await resizeGlobalFeeState(users[0].mrgnBankrunProgram, {
        payer: users[0].wallet.publicKey,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [users[0].wallet]);

    const after = await banksClient.getAccount(feeStateKey);
    assert.equal(after.data.length, FEE_STATE_ACCOUNT_LEN);
    assert.deepEqual(
      Buffer.from(after.data).subarray(0, FEE_STATE_V1_ACCOUNT_LEN),
      v1Data,
    );
    assert.isTrue(
      Buffer.from(after.data)
        .subarray(FEE_STATE_V1_ACCOUNT_LEN)
        .every((b) => b === 0),
    );
    // The anchor client decodes it again post-resize
    const feeState = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assert.equal(feeState.key.toString(), feeStateKey.toString());
  });

  it("resizing an already-current account fails (6513 InvalidResize)", async () => {
    const tx = new Transaction().add(
      await resizeGroupAccount(groupAdmin.mrgnBankrunProgram, {
        group: marginfiGroup.publicKey,
        payer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    assertBankrunTxFailed(
      await banksClient.tryProcessTransaction(tx),
      "0x1971",
    );
  });

  it("resizing a non-marginfi account fails", async () => {
    const bogus = Keypair.generate();
    const tx = new Transaction().add(
      await resizeGroupAccount(groupAdmin.mrgnBankrunProgram, {
        group: bogus.publicKey,
        payer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    assert.isNotNull(result.result);
  });
});
