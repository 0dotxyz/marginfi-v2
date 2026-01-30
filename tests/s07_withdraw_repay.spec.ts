import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import BN from "bn.js";
import {
  stakedMarginfiGroup,
  validators,
  oracles,
  bankrunContext,
  banksClient,
  bankrunProgram,
  users,
  ecosystem,
  stakedBankKeypairSol,
  bankRunProvider,
} from "./rootHooks";
import { deriveBankWithSeed, deriveStakedSettings } from "./utils/pdas";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import { getTokenBalance, assertI80F48Equal } from "./utils/genericTests";
import { LST_ATA, USER_ACCOUNT } from "./utils/mocks";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  repayIx,
  withdrawIx,
} from "./utils/user-instructions";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

let marginfiGroup: Keypair;
let bankKeypairSol: Keypair;

describe("Withdraw staked asset", () => {
  let settingsKey: PublicKey;
  let bankKey: PublicKey;

  before(async () => {
    marginfiGroup = stakedMarginfiGroup;
    bankKeypairSol = stakedBankKeypairSol;
    // Refresh oracles to ensure they're up to date
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    [settingsKey] = deriveStakedSettings(
      bankrunProgram.programId,
      marginfiGroup.publicKey
    );
    [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      marginfiGroup.publicKey,
      validators[0].splMint,
      new BN(0)
    );
  });

  it("(user 3) deposits some native staked and borrows SOL against it - happy path", async () => {
    const user = users[3];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    let depositTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(1 * 10 ** ecosystem.wsolDecimals),
      })
    );

    depositTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    depositTx.sign(user.wallet);
    await banksClient.tryProcessTransaction(depositTx);

    let borrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.5 * 10 ** ecosystem.wsolDecimals),
      })
    );
    borrowTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    borrowTx.sign(user.wallet);
    await banksClient.processTransaction(borrowTx);
  });

  it("(user 3) withdraws a small amount of native staked position - happy path", async () => {
    const amtNative = 0.1 * 10 ** ecosystem.wsolDecimals;
    const user = users[3];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    const lstBefore = await getTokenBalance(bankRunProvider, userLstAta);

    let tx = new Transaction().add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(amtNative),
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const lstAfter = await getTokenBalance(bankRunProvider, userLstAta);
    assert.equal(lstAfter, lstBefore + amtNative);

    const userAcc = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);

    // TODO assert other balances changes as expected...
  });

  it("(user 3) repays a small amount of SOL borrowed against stake - happy path", async () => {
    const amtNative = 0.1 * 10 ** ecosystem.wsolDecimals;
    const user = users[3];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const solBefore = await getTokenBalance(bankRunProvider, user.wsolAccount);

    let tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        amount: new BN(amtNative),
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const solAfter = await getTokenBalance(bankRunProvider, user.wsolAccount);
    assert.equal(solAfter, solBefore - amtNative);

    const userAcc = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 1);

    // TODO assert other balances changes as expected...
  });

  it("(user 3) repays the entire borrowed SOL balance - happy path", async () => {
    const amtNative = 0.1 * 10 ** ecosystem.wsolDecimals;
    const user = users[3];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const solBefore = await getTokenBalance(bankRunProvider, user.wsolAccount);
    const userAccBefore =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);
    const bankBefore = await user.mrgnBankrunProgram.account.bank.fetch(
      bankKeypairSol.publicKey
    );

    // Note: the SOL balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const solIndex = userAccBefore.lendingAccount.balances.findIndex(
      (balance) => balance.bankPk.equals(bankKeypairSol.publicKey)
    );

    const amtExpected =
      wrappedI80F48toBigNumber(
        userAccBefore.lendingAccount.balances[solIndex].liabilityShares
      ).toNumber() *
      wrappedI80F48toBigNumber(bankBefore.liabilityShareValue).toNumber();

    let tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        amount: new BN(amtNative),
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        repayAll: true,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const solAfter = await getTokenBalance(bankRunProvider, user.wsolAccount);
    assert.approximately(solAfter, solBefore - amtExpected, 2);

    const userAcc = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assertI80F48Equal(balances[1].liabilityShares, 0);
    assert.equal(balances[1].active, 0);

    // TODO assert other balances changes as expected...
  });

  it("(user 3) withdraws the entire native staked position - happy path", async () => {
    const amtNative = 0.1 * 10 ** ecosystem.wsolDecimals;
    const user = users[3];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA);

    const lstBefore = await getTokenBalance(bankRunProvider, userLstAta);
    const userAccBefore =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);
    const bankBefore = await user.mrgnBankrunProgram.account.bank.fetch(
      bankKeypairSol.publicKey
    );
    const amtExpected =
      wrappedI80F48toBigNumber(
        userAccBefore.lendingAccount.balances[0].assetShares
      ).toNumber() *
      wrappedI80F48toBigNumber(bankBefore.assetShareValue).toNumber();

    let tx = new Transaction().add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: userLstAta,
        amount: new BN(amtNative),
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        withdrawAll: true,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const lstAfter = await getTokenBalance(bankRunProvider, userLstAta);
    assert.equal(lstAfter, lstBefore + amtExpected);

    const userAcc = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const balances = userAcc.lendingAccount.balances;
    assertI80F48Equal(balances[0].assetShares, 0);
    assert.equal(balances[0].active, 0);

    // TODO assert other balances changes as expected...
  });
});
