import { Program } from "@coral-xyz/anchor";
import { Keypair, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankrunContext,
  bankrunProgram,
  marginfiGroup,
  users,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertBNEqual,
  assertI80F48Equal,
  assertKeyDefault,
  assertKeysEqual,
} from "./utils/genericTests";
import { assert } from "chai";
import { accountInit } from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { getBankrunTime } from "./utils/tools";

let program: Program<Marginfi>;

describe("Initialize user account", () => {
  before(() => {
    program = bankrunProgram;
  });

  it("(user 0) Initialize user account - happy path", async () => {
    const accountKeypair = Keypair.generate();
    const accountKey = accountKeypair.publicKey;
    users[0].accounts.set(USER_ACCOUNT, accountKey);

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInit(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(tx, [accountKeypair]);

    const userAcc = await program.account.marginfiAccount.fetch(accountKey);
    assertKeysEqual(userAcc.group, marginfiGroup.publicKey);
    assertKeysEqual(userAcc.authority, users[0].wallet.publicKey);
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
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(userAcc.lastUpdate, now, 2);
  });

  it("(user 1) Initialize user account - happy path", async () => {
    const accountKeypair = Keypair.generate();
    const accountKey = accountKeypair.publicKey;
    users[1].accounts.set(USER_ACCOUNT, accountKey);

    let tx: Transaction = new Transaction();
    tx.add(
      await accountInit(program, {
        marginfiGroup: marginfiGroup.publicKey,
        marginfiAccount: accountKey,
        authority: users[1].wallet.publicKey,
        feePayer: users[1].wallet.publicKey,
      })
    );
    await users[1].mrgnProgram.provider.sendAndConfirm(tx, [accountKeypair]);
  });
});
