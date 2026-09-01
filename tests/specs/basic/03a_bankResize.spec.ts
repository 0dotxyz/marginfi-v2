import { Transaction } from "@solana/web3.js";
import { assert } from "chai";
import { resizeBankAccount } from "../../utils/group-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  bankKeypairSol,
  bankKeypairUsdc,
  ecosystem,
  groupAdmin,
} from "../../rootHooks";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import { getBankrunBlockhash } from "../../utils/tools";
import { toI80Scaled } from "../../utils/bn-utils";

/** v1 bank layout size (8-byte discriminator + Bank::V1_LEN), as on mainnet today */
const BANK_V1_ACCOUNT_LEN = 8 + 1856;
/** Reserve added for fields later releases will claim */
const BANK_RESERVED_BYTES = 1024;
/** What the resize grows a bank to */
const BANK_ACCOUNT_LEN = BANK_V1_ACCOUNT_LEN + BANK_RESERVED_BYTES;

describe("03a: Bank resize (reserve space for later layouts)", () => {
  const resize = async (bank: typeof bankKeypairUsdc) => {
    const tx = new Transaction().add(
      await resizeBankAccount(bankrunProgram, {
        bank: bank.publicKey,
        payer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    return banksClient.tryProcessTransaction(tx);
  };

  it("banks are created at the v1 size, before any reserve", async () => {
    const bank = await banksClient.getAccount(bankKeypairUsdc.publicKey);
    assert.equal(bank.data.length, BANK_V1_ACCOUNT_LEN);
  });

  it("the permissionless resize grows a bank and zero-fills the reserve", async () => {
    const before = await bankrunProgram.account.bank.fetch(
      bankKeypairUsdc.publicKey,
    );

    const result = await resize(bankKeypairUsdc);
    assert.isNull(result.result);

    const account = await banksClient.getAccount(bankKeypairUsdc.publicKey);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
    // A later layout claiming the reserve reads zero, the same value a fresh bank carries.
    assert.isTrue(
      Buffer.from(account.data)
        .subarray(BANK_V1_ACCOUNT_LEN)
        .every((b) => b === 0),
    );

    // The struct is a byte-identical prefix, so the bank still decodes to the same state.
    const after = await bankrunProgram.account.bank.fetch(
      bankKeypairUsdc.publicKey,
    );
    assert.deepEqual(after.mint, before.mint);
    assert.deepEqual(after.group, before.group);
    assert.equal(
      toI80Scaled(after.assetShareValue),
      toI80Scaled(before.assetShareValue),
    );
  });

  it("a bank can only be resized once", async () => {
    const result = await resize(bankKeypairUsdc);
    assertBankrunTxFailed(result, "0x1971"); // InvalidResize

    const account = await banksClient.getAccount(bankKeypairUsdc.publicKey);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
  });

  it("an oversized bank stays fully operable", async () => {
    // The whole point of resizing ahead of the struct: nothing may break between this release and
    // the one that claims the reserve.
    const result = await resize(bankKeypairSol);
    assert.isNull(result.result);

    const sol = await bankrunProgram.account.bank.fetch(
      bankKeypairSol.publicKey,
    );
    assert.deepEqual(sol.mint, ecosystem.wsolMint.publicKey);
    const account = await banksClient.getAccount(bankKeypairSol.publicKey);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
  });
});
