import { Transaction } from "@solana/web3.js";
import { assert } from "chai";
import { resizeBankAccount } from "../../utils/group-instructions";
import {
  BANK_ACCOUNT_LEN,
  bankrunContext,
  bankrunProgram,
  banksClient,
  bankKeypairUsdc,
  groupAdmin,
} from "../../rootHooks";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import { getBankrunBlockhash } from "../../utils/tools";

/** 
 * v1 bank layout size (8-byte discriminator + Bank::V1_LEN), as on mainnet in 0.1.11 and earlier 
 * */
const BANK_V1_ACCOUNT_LEN = 8 + 1856;

describe("25: Bank resize (v1 accounts grow to the current layout)", () => {
  const bank = bankKeypairUsdc.publicKey;

  const resize = async () => {
    const tx = new Transaction().add(
      await resizeBankAccount(bankrunProgram, {
        bank,
        payer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    return banksClient.tryProcessTransaction(tx);
  };

  it("new banks are created at the current size with a zeroed reserve", async () => {
    const account = await banksClient.getAccount(bank);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
    assert.isTrue(
      Buffer.from(account.data)
        .subarray(BANK_V1_ACCOUNT_LEN)
        .every((b) => b === 0),
    );
  });

  it("the permissionless resize grows a v1 bank and preserves its state", async () => {
    const before = await bankrunProgram.account.bank.fetch(bank);

    const fresh = await banksClient.getAccount(bank);
    const v1Data = Buffer.from(fresh.data).subarray(0, BANK_V1_ACCOUNT_LEN);
    bankrunContext.setAccount(bank, { ...fresh, data: v1Data });

    const result = await resize();
    assert.isNull(result.result);

    const account = await banksClient.getAccount(bank);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
    const data = Buffer.from(account.data);
    assert.isTrue(data.subarray(0, BANK_V1_ACCOUNT_LEN).equals(v1Data));
    assert.isTrue(data.subarray(BANK_V1_ACCOUNT_LEN).every((b) => b === 0));

    const after = await bankrunProgram.account.bank.fetch(bank);
    assert.deepEqual(after, before);
  });

  it("a bank already at the current size cannot be resized", async () => {
    const result = await resize();
    assertBankrunTxFailed(result, "0x1971"); // InvalidResize

    const account = await banksClient.getAccount(bank);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
  });
});
