import { BN } from "@coral-xyz/anchor";
import { Keypair, Transaction } from "@solana/web3.js";
import {
  stakedBankKeypairSol,
  stakedBankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  oracles,
  users,
  validators,
} from "./rootHooks";
import { assertBankrunTxFailed, assertKeysEqual } from "./utils/genericTests";
import { assert } from "chai";
import { borrowIx, composeRemainingAccounts } from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

let bankKeypairSol: Keypair;
let bankKeypairUsdc: Keypair;

describe("Deposit funds (included staked assets)", () => {
  // User 0 has a USDC deposit position
  // User 1 has a SOL [0] and validator 0 Staked [1] deposit position

  before(async () => {
    bankKeypairSol = stakedBankKeypairSol;
    bankKeypairUsdc = stakedBankKeypairUsdc;
    // Refresh oracles to ensure they're up to date
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("(user 0) borrows SOL against their USDC position - succeeds (SOL/regular comingle is allowed)", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const userAccBefore = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balancesBefore = userAccBefore.lendingAccount.balances;
    assert.equal(balancesBefore[1].active, 0);
    assertKeysEqual(balancesBefore[0].bankPk, bankKeypairUsdc.publicKey);

    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.01 * 10 ** ecosystem.wsolDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;

    // Find balances by bank key (order may vary due to pubkey sorting)
    const solBalanceIndex = balances.findIndex((b) =>
      b.bankPk.equals(bankKeypairSol.publicKey)
    );
    const usdcBalanceIndex = balances.findIndex((b) =>
      b.bankPk.equals(bankKeypairUsdc.publicKey)
    );

    assert.notEqual(solBalanceIndex, -1, "SOL balance not found");
    assert.notEqual(usdcBalanceIndex, -1, "USDC balance not found");
    assert.equal(balances[solBalanceIndex].active, 1);
    assert.equal(balances[usdcBalanceIndex].active, 1);
  });

  // Note: Borrowing STAKED assets is generally forbidden (their borrow cap is set to 0)
  // If we ever change this, add a test here to validate user 0 cannot borrow staked assets

  it("(user 1) tries to borrow USDC - should fail (Regular assets cannot comingle with Staked)", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairUsdc.publicKey,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey, // Note the Staked bank uses wsol oracle too
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
        ]),
        amount: new BN(0.1 * 10 ** ecosystem.usdcDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // AssetTagMismatch
    assertBankrunTxFailed(result, "0x179f");

    // Verify the deposit worked and the entry does not exist
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[2].active, 0);
  });

  // TODO withdraw user 1's SOL collateral and verify they can borrow SOL
});
