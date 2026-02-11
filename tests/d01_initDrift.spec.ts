import { assert } from "chai";
import { Transaction } from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  bankrunContext,
  driftBankrunProgram,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { assertKeysEqual } from "./utils/genericTests";
import { makeInitializeDriftIx } from "./utils/drift-sdk";
import { getDriftStateAccount } from "./utils/drift-utils";

describe("d01: Drift - Initialize State", () => {
  it("Initialize Drift program state", async () => {
    const initDriftIx = await makeInitializeDriftIx(driftBankrunProgram, {
      admin: groupAdmin.wallet.publicKey,
      usdcMint: ecosystem.usdcMint.publicKey,
    });

    const tx = new Transaction().add(initDriftIx);
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const state = await getDriftStateAccount(driftBankrunProgram);

    assert.ok(state);
    assertKeysEqual(state.admin, groupAdmin.wallet.publicKey);
    assert.equal(state.numberOfMarkets, 0);
    assert.equal(state.numberOfSubAccounts.toNumber(), 0);
  });
});
