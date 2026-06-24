import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  stakedBankKeypairSol,
  stakedBankKeypairUsdc,
  stakedMarginfiGroup,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  oracles,
  users,
  validators,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  repayIx,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { getBankrunBlockhash } from "./utils/tools";

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
      userAccount,
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
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const balances = userAcc.lendingAccount.balances;

    // Find balances by bank key (order may vary due to pubkey sorting)
    const solBalanceIndex = balances.findIndex((b) =>
      b.bankPk.equals(bankKeypairSol.publicKey),
    );
    const usdcBalanceIndex = balances.findIndex((b) =>
      b.bankPk.equals(bankKeypairUsdc.publicKey),
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
            validators[0].splOnRampPool,
          ],
          [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
        ]),
        amount: new BN(0.1 * 10 ** ecosystem.usdcDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // AssetTagMismatch
    assertBankrunTxFailed(result, "0x179f");

    // Verify the deposit worked and the entry does not exist
    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[2].active, 0);
  });

  it("(user 1) withdraws their SOL collateral and borrows SOL against staked, then restores", async () => {
    // With only a staked position as collateral, user 1 can still borrow SOL (SOL co-mingles with
    // staked). A funder account adds SOL liquidity first, since user 1's own SOL backs the bank's
    // borrows and can't otherwise be fully withdrawn. Each ix's `remaining` is its POST-ix active
    // balance set (the order the on-chain health check expects); the round-trip restores user 1
    // byte-exact, so later specs (incl. s08, where user 1 is the liquidator) are unaffected.
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const snapshotUser1 = async () =>
      (await bankrunProgram.account.marginfiAccount.fetch(userAccount)).lendingAccount.balances
        .filter((b) => b.active === 1)
        .map((b) => ({
          bank: b.bankPk.toBase58(),
          asset: wrappedI80F48toBigNumber(b.assetShares).toString(),
          liability: wrappedI80F48toBigNumber(b.liabilityShares).toString(),
        }))
        .sort((a, b) => a.bank.localeCompare(b.bank));
    const before = await snapshotUser1();

    const stakedObs: [PublicKey, ...PublicKey[]] = [
      validators[0].bank,
      oracles.wsolOracle.publicKey, // the Staked bank uses the wsol oracle too
      validators[0].splMint,
      validators[0].splSolPool,
    ];
    const solObs: [PublicKey, PublicKey] = [
      bankKeypairSol.publicKey,
      oracles.wsolOracle.publicKey,
    ];
    const send = async (ix: any, signers: any[] = [user.wallet]) => {
      const tx = new Transaction().add(ix);
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(...signers);
      await banksClient.processTransaction(tx);
    };

    // Funder adds SOL liquidity so user 1's full withdraw isn't blocked by the utilization cap.
    const funderKp = Keypair.generate();
    const funderAccount = funderKp.publicKey;
    await send(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: stakedMarginfiGroup.publicKey,
        marginfiAccount: funderAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
      [user.wallet, funderKp]
    );
    await send(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: funderAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        amount: new BN(2 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );

    // 1. Withdraw all of user 1's SOL collateral, leaving only the staked position.
    const wsolBefore = await getTokenBalance(bankRunProvider, user.wsolAccount);
    await send(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([stakedObs]),
        amount: new BN(0),
        withdrawAll: true,
      })
    );
    const withdrawnSol = new BN(
      (await getTokenBalance(bankRunProvider, user.wsolAccount)) - wsolBefore
    );

    // 2. Borrow SOL against the staked-only collateral — the behavior under test.
    await send(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([stakedObs, solObs]),
        amount: new BN(0.01 * 10 ** ecosystem.wsolDecimals),
      })
    );
    const borrowed = (
      await bankrunProgram.account.marginfiAccount.fetch(userAccount)
    ).lendingAccount.balances.find(
      (b) => b.bankPk.equals(bankKeypairSol.publicKey) && b.active === 1
    );
    assert.ok(borrowed, "SOL borrow position should be active");
    assert.isAbove(
      wrappedI80F48toBigNumber(borrowed.liabilityShares).toNumber(),
      0
    );

    // 3. Restore user 1: repay the borrow, then re-deposit the exact SOL withdrawn.
    await send(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([stakedObs]),
        amount: new BN(0),
        repayAll: true,
      })
    );
    await send(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        amount: withdrawnSol,
        depositUpToLimit: false,
      })
    );

    // 4. Funder withdraws its liquidity back out.
    await send(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: funderAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: [],
        amount: new BN(0),
        withdrawAll: true,
      })
    );

    // 5. User 1's active balances are byte-identical to before.
    const after = await snapshotUser1();
    assert.deepEqual(after, before);
  });
});
