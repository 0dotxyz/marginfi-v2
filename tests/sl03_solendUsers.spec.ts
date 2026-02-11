import { assert } from "chai";
import {
  Keypair,
  Transaction,
  SystemProgram,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import {
  bankrunContext,
  globalProgramAdmin,
  ecosystem,
  users,
  solendAccounts,
  SOLEND_MARKET,
  bankRunProvider,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import {
  makeSolendInitObligationIx,
  SOLEND_PROGRAM_ID,
} from "./utils/solend-sdk";
import { SOLEND_OBLIGATION_SIZE } from "./utils/solend-utils";
import { getTokenBalance } from "./utils/genericTests";
import { MockUser } from "./utils/mocks";

describe("sl03: Solend - Initialize Users", () => {
  let userA: MockUser;
  let userB: MockUser;

  const userAObligation = Keypair.generate();
  const userBObligation = Keypair.generate();

  before(async () => {
    userA = users[0];
    userB = users[1];
  });

  it("(user A) Initialize obligation", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);

    const createObligationAccountIx = SystemProgram.createAccount({
      fromPubkey: userA.wallet.publicKey,
      newAccountPubkey: userAObligation.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          SOLEND_OBLIGATION_SIZE
        ),
      space: SOLEND_OBLIGATION_SIZE,
      programId: SOLEND_PROGRAM_ID,
    });

    const initObligationIx = makeSolendInitObligationIx({
      obligation: userAObligation.publicKey,
      lendingMarket: solendMarket,
      obligationOwner: userA.wallet.publicKey,
    });

    const tx = new Transaction().add(
      createObligationAccountIx,
      initObligationIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet, userAObligation],
      false,
      true
    );

    solendAccounts.set(`user_a_obligation`, userAObligation.publicKey);

    const obligationAccount = await bankrunContext.banksClient.getAccount(
      userAObligation.publicKey
    );

    assert.ok(obligationAccount);
  });

  it("(user B) Initialize obligation", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);

    const createObligationAccountIx = SystemProgram.createAccount({
      fromPubkey: userB.wallet.publicKey,
      newAccountPubkey: userBObligation.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          SOLEND_OBLIGATION_SIZE
        ),
      space: SOLEND_OBLIGATION_SIZE,
      programId: SOLEND_PROGRAM_ID,
    });

    const initObligationIx = makeSolendInitObligationIx({
      obligation: userBObligation.publicKey,
      lendingMarket: solendMarket,
      obligationOwner: userB.wallet.publicKey,
    });

    const tx = new Transaction().add(
      createObligationAccountIx,
      initObligationIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userB.wallet, userBObligation],
      false,
      true
    );

    solendAccounts.set(`user_b_obligation`, userBObligation.publicKey);

    const obligationAccount = await bankrunContext.banksClient.getAccount(
      userBObligation.publicKey
    );

    assert.ok(obligationAccount);
  });

  it("(admin) Fund user A with Token A", async () => {
    const fundAmount = 50_000 * 10 ** ecosystem.tokenADecimals;

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      userA.tokenAAccount
    );

    const mintToUserIx = createMintToInstruction(
      ecosystem.tokenAMint.publicKey,
      userA.tokenAAccount,
      globalProgramAdmin.wallet.publicKey,
      fundAmount
    );

    const tx = new Transaction().add(mintToUserIx);

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      userA.tokenAAccount
    );
    assert.equal(balanceAfter - balanceBefore, fundAmount);
  });

  it("(admin) Fund user B with USDC", async () => {
    const fundAmount = 50_000 * 10 ** ecosystem.usdcDecimals;

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      userB.usdcAccount
    );

    const mintToUserIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      userB.usdcAccount,
      globalProgramAdmin.wallet.publicKey,
      fundAmount
    );

    const tx = new Transaction().add(mintToUserIx);

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      userB.usdcAccount
    );
    assert.equal(balanceAfter - balanceBefore, fundAmount);
  });
});
