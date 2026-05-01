import { Program } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";

import { Marginfi } from "../target/types/marginfi";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  users,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertKeyDefault,
  assertKeysEqual,
} from "./utils/genericTests";
import { groupInitialize } from "./utils/group-instructions";
import { deriveLiquidationRecord } from "./utils/pdas";
import { processBankrunTransaction } from "./utils/tools";
import {
  accountCloseIx,
  accountInit,
  closeLiquidationRecordIx,
  initLiquidationRecordIx,
} from "./utils/user-instructions";

let program: Program<Marginfi>;

describe("Close account requires liquidation record closed", () => {
  before(() => {
    program = bankrunProgram;
  });

  it("(user 0) fails to close account before liquidation record is closed, then succeeds after", async () => {
    const user = users[0];
    const group = Keypair.generate();
    const marginfiAccount = Keypair.generate();
    const marginfiAccountPk = marginfiAccount.publicKey;
    const [liqRecordPk] = deriveLiquidationRecord(
      program.programId,
      marginfiAccountPk,
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(user.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          admin: user.wallet.publicKey,
        }),
      ),
      [user.wallet, group],
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          marginfiAccount: marginfiAccountPk,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet, marginfiAccount],
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await initLiquidationRecordIx(user.mrgnBankrunProgram, {
          marginfiAccount: marginfiAccountPk,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet],
    );

    const accountAfterInit = await program.account.marginfiAccount.fetch(
      marginfiAccountPk,
    );
    assertKeysEqual(accountAfterInit.liquidationRecord, liqRecordPk);

    const closeBeforeRecordResult = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountCloseIx(user.mrgnBankrunProgram, {
          marginfiAccount: marginfiAccountPk,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet],
      true,
    );
    assertBankrunTxFailed(closeBeforeRecordResult, 6043); // IllegalAction

    const recordStillExists = await banksClient.getAccount(liqRecordPk);
    assert.isNotNull(recordStillExists);

    // Note: Closing the record and the account in the same tx is perfectly fine.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }),
        await closeLiquidationRecordIx(user.mrgnBankrunProgram, {
          marginfiAccount: marginfiAccountPk,
          recordPayer: user.wallet.publicKey,
          liquidationRecord: liqRecordPk,
        }),
        await accountCloseIx(user.mrgnBankrunProgram, {
          marginfiAccount: marginfiAccountPk,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet],
    );

    const accountAfterClose = await banksClient.getAccount(marginfiAccountPk);
    assert.isNull(accountAfterClose);
  });

  it("(user 1) can close account immediately when liquidation record never existed", async () => {
    const user = users[1];
    const group = Keypair.generate();
    const marginfiAccount = Keypair.generate();
    const marginfiAccountPk = marginfiAccount.publicKey;

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(user.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          admin: user.wallet.publicKey,
        }),
      ),
      [user.wallet, group],
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          marginfiAccount: marginfiAccountPk,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet, marginfiAccount],
    );

    const accountBeforeClose = await program.account.marginfiAccount.fetch(
      marginfiAccountPk,
    );
    assertKeyDefault(accountBeforeClose.liquidationRecord);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountCloseIx(user.mrgnBankrunProgram, {
          marginfiAccount: marginfiAccountPk,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      ),
      [user.wallet],
    );

    const accountAfterClose = await banksClient.getAccount(marginfiAccountPk);
    assert.isNull(accountAfterClose);
  });
});
