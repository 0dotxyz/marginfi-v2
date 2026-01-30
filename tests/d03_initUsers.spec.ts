import { assert } from "chai";
import { Transaction } from "@solana/web3.js";
import { users, bankrunContext, driftBankrunProgram } from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { assertKeysEqual } from "./utils/genericTests";
import {
  makeInitializeUserStatsIx,
  makeInitializeUserIx,
} from "./utils/drift-sdk";
import {
  getUserAccount,
  getUserStatsAccount,
  getDriftStateAccount,
  DriftUser,
} from "./utils/drift-utils";
import { MockUser } from "./utils/mocks";
import { deriveUserPDA } from "./utils/pdas";

describe("d03: Drift - Initialize User Accounts", () => {
  let userA: MockUser;
  let userB: MockUser;

  before(async () => {
    userA = users[0];
    userB = users[1];
  });

  it("Initialize User A stats and account", async () => {
    const initUserAStatsIx = await makeInitializeUserStatsIx(
      driftBankrunProgram,
      {
        authority: userA.wallet.publicKey,
        payer: userA.wallet.publicKey,
      }
    );

    const tx1 = new Transaction().add(initUserAStatsIx);
    await processBankrunTransaction(bankrunContext, tx1, [userA.wallet]);

    const userAStats = await getUserStatsAccount(
      driftBankrunProgram,
      userA.wallet.publicKey
    );
    assert.ok(userAStats);
    assertKeysEqual(userAStats.authority, userA.wallet.publicKey);
    assert.equal(userAStats.numberOfSubAccounts, 0);

    const [userPublicKey] = deriveUserPDA(
      driftBankrunProgram.programId,
      userA.wallet.publicKey,
      0 // sub_account_id is always zero in our case
    );

    const initUserAIx = await makeInitializeUserIx(
      driftBankrunProgram,
      {
        authority: userA.wallet.publicKey,
        payer: userA.wallet.publicKey,
        user: userPublicKey,
      },
      {
        subAccountId: 0,
        name: Array(32).fill(0),
      }
    );

    const tx2 = new Transaction().add(initUserAIx);
    await processBankrunTransaction(bankrunContext, tx2, [userA.wallet]);

    const userAAccount = await getUserAccount(
      driftBankrunProgram,
      userA.wallet.publicKey,
      0
    );
    assert.ok(userAAccount);
    assertKeysEqual(userAAccount.authority, userA.wallet.publicKey);
    assert.equal(userAAccount.subAccountId, 0);

    const state = await getDriftStateAccount(driftBankrunProgram);
    assert.equal(state.numberOfSubAccounts.toNumber(), 1);

    const updatedUserAStats = await getUserStatsAccount(
      driftBankrunProgram,
      userA.wallet.publicKey
    );
    assert.equal(updatedUserAStats.numberOfSubAccounts, 1);

    const driftUserA = await driftBankrunProgram.account.user.fetch(
      userPublicKey
    );
    assert.equal(driftUserA.subAccountId, 0);
  });

  it("Initialize User B stats and account", async () => {
    const initUserBStatsIx = await makeInitializeUserStatsIx(
      driftBankrunProgram,
      {
        authority: userB.wallet.publicKey,
        payer: userB.wallet.publicKey,
      }
    );

    const tx1 = new Transaction().add(initUserBStatsIx);
    await processBankrunTransaction(bankrunContext, tx1, [userB.wallet]);

    const userBStats = await getUserStatsAccount(
      driftBankrunProgram,
      userB.wallet.publicKey
    );
    assert.ok(userBStats);
    assertKeysEqual(userBStats.authority, userB.wallet.publicKey);

    const [userPublicKey] = deriveUserPDA(
      driftBankrunProgram.programId,
      userB.wallet.publicKey,
      0 // sub_account_id is always zero in our case
    );

    const initUserBIx = await makeInitializeUserIx(
      driftBankrunProgram,
      {
        authority: userB.wallet.publicKey,
        payer: userB.wallet.publicKey,
        user: userPublicKey,
      },
      {
        subAccountId: 0,
        name: Array(32).fill(0),
      }
    );

    const tx2 = new Transaction().add(initUserBIx);
    await processBankrunTransaction(bankrunContext, tx2, [userB.wallet]);

    const userBAccount = await getUserAccount(
      driftBankrunProgram,
      userB.wallet.publicKey,
      0
    );
    assert.ok(userBAccount);
    assertKeysEqual(userBAccount.authority, userB.wallet.publicKey);
    assert.equal(userBAccount.subAccountId, 0);

    const state = await getDriftStateAccount(driftBankrunProgram);
    assert.equal(state.numberOfSubAccounts.toNumber(), 2);

    const driftUserB = await driftBankrunProgram.account.user.fetch(
      userPublicKey
    );
    assert.equal(driftUserB.subAccountId, 0);
  });
});
