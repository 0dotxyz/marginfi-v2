import { Transaction } from "@solana/web3.js";
import { assert } from "chai";
import { resizeBankAccount } from "../../utils/group-instructions";
import {
  BANK_ACCOUNT_LEN,
  BANK_V1_ACCOUNT_LEN,
  bankrunContext,
  bankrunProgram,
  banksClient,
  bankKeypairUsdc,
  groupAdmin,
} from "../../rootHooks";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import {
  BANK_RATE_READINGS,
  RATE_READING_LEN,
} from "../../utils/rate-readings";
import { getBankrunBlockhash } from "../../utils/tools";

/** Where the reserve starts: past the rate readings that follow the v1 layout. */
const BANK_RESERVE_OFFSET =
  BANK_V1_ACCOUNT_LEN + BANK_RATE_READINGS * RATE_READING_LEN;

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
        .subarray(BANK_RESERVE_OFFSET)
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

    // The rate readings sit past the v1 struct, so the grown bank starts with none.
    const after = await bankrunProgram.account.bank.fetch(bank);
    assert.deepEqual(after, { ...before, rateReadings: after.rateReadings });
  });

  it("a bank already at the current size cannot be resized", async () => {
    const result = await resize();
    assertBankrunTxFailed(result, "0x1971"); // InvalidResize

    const account = await banksClient.getAccount(bank);
    assert.equal(account.data.length, BANK_ACCOUNT_LEN);
  });
});
