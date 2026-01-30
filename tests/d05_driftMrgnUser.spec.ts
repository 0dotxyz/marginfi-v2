import { Keypair, Transaction } from "@solana/web3.js";
import {
  groupAdmin,
  users,
  bankrunContext,
  bankrunProgram,
  driftGroup,
} from "./rootHooks";
import { USER_ACCOUNT_D } from "./utils/mocks";
import {
  assertBNEqual,
  assertI80F48Equal,
  assertKeyDefault,
  assertKeysEqual,
} from "./utils/genericTests";
import { assert } from "chai";
import { accountInit } from "./utils/user-instructions";
import { groupInitialize } from "./utils/group-instructions";
import { processBankrunTransaction } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";

let ctx: ProgramTestContext;

describe("d05: Initialize Marginfi-Drift integration", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  it("(admin) Initialize marginfi group", async () => {
    const tx = new Transaction().add(
      await groupInitialize(bankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet, driftGroup]);
  });

  it("(users 0/1) Initialize marginfi accounts", async () => {
    await initUserMarginfiAccount(0);
    await initUserMarginfiAccount(1);
  });

  async function initUserMarginfiAccount(userIndex: number) {
    const user = users[userIndex];
    const accountKeypair = Keypair.generate();
    const accountKey = accountKeypair.publicKey;
    user.accounts.set(USER_ACCOUNT_D, accountKey);

    let tx = new Transaction();
    tx.add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: driftGroup.publicKey,
        marginfiAccount: accountKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);

    const userAcc = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      accountKey
    );
    assertKeysEqual(userAcc.group, driftGroup.publicKey);
    assertKeysEqual(userAcc.authority, user.wallet.publicKey);
    const balances = userAcc.lendingAccount.balances;
    for (let i = 0; i < balances.length; i++) {
      assert.equal(balances[i].active, 0);
      assertKeyDefault(balances[i].bankPk);
      assertI80F48Equal(balances[i].assetShares, 0);
      assertI80F48Equal(balances[i].liabilityShares, 0);
      assertI80F48Equal(balances[i].emissionsOutstanding, 0);
      assertBNEqual(balances[i].lastUpdate, 0);
    }
    assertBNEqual(userAcc.accountFlags, 0);
  }
});
