import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "./utils/litesvm";
import { PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertI80F48Equal,
  assertKeysEqual,
  getTokenBalance,
  parseMarginfiEvents,
  assertI80F48Approx,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  repayIx,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { u64MAX_BN } from "./utils/types";
import {
  divI80,
  fromI80Scaled,
  getBankrunTime,
  mulI80,
  nativeToI80Scaled,
  processBankrunTransaction,
  toI80Scaled,
} from "./utils/tools";

let program: Program<Marginfi>;
let provider: BankrunProvider;

describe("Withdraw funds", () => {
  let balanceAccountGroups: PublicKey[][] = [];

  before(() => {
    provider = bankRunProvider;
    program = bankrunProgram;
    balanceAccountGroups = [
      [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
      [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
      [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
    ];
  });

  const withdrawAmountTokenA = 0.1;
  const withdrawAmountTokenA_native = new BN(
    withdrawAmountTokenA * 10 ** ecosystem.tokenADecimals,
  );

  const repayAmountUsdc = 0.1;
  const repayAmountUsdc_native = new BN(
    repayAmountUsdc * 10 ** ecosystem.usdcDecimals,
  );

  it("(user 0) withdraws some token A - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairA.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userTokenABefore, vaultTokenABefore] =
      await Promise.all([
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankBefore.liquidityVault),
      ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;
    const userTokenABalanceBefore = balancesBefore.find((b) =>
      b.bankPk.equals(bank),
    );
    assert(userTokenABalanceBefore, "missing token-A balance before withdraw");

    const tx = new Transaction().add(
      await withdrawIx(user.mrgnProgram, {
        marginfiAccount: userAccKey,
        bank: bank,
        tokenAccount: user.tokenAAccount,
        remaining: composeRemainingAccounts([
          [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
        ]),
        amount: withdrawAmountTokenA_native,
      })
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);
    const events = parseMarginfiEvents(program, result.logMessages);
    const withdrawEvent = events.find(
      (e) => e.name === "lendingAccountWithdrawEvent"
    );
    assert.isDefined(withdrawEvent, "Expected lendingAccountWithdrawEvent");
    assertI80F48Approx(
      withdrawEvent!.data.shareAmount,
      withdrawAmountTokenA_native
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userTokenAAfter, vaultTokenAAfter] = await Promise.all(
      [
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankAfter.liquidityVault),
      ],
    );
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const balancesAfter = userAccAfter.lendingAccount.balances;
    const userTokenABalanceAfter = balancesAfter.find((b) =>
      b.bankPk.equals(bank),
    );
    assert(userTokenABalanceAfter, "missing token-A balance after withdraw");
    // Partial withdraw only, position is still open
    assert.equal(bankAfter.lendingPositionCount, 1);

    const withdrawExpected = withdrawAmountTokenA_native.toNumber();
    if (verbose) {
      console.log(
        "User 0 withdrew " +
          withdrawAmountTokenA +
          " token A (" +
          withdrawExpected.toString() +
          ") native",
      );
    }

    // user gains the token A, the liquidity vault loses it....
    assert.equal(userTokenAAfter, userTokenABefore + withdrawExpected);
    assert.equal(vaultTokenAAfter, vaultTokenABefore - withdrawExpected);

    // Note: here we do all the math in I80 space to show they are exactly equal. You can usually do
    // this in JS math by simplying going toNumber() but in some OSes (linux) float rounding will
    // break this.
    const shareValueScaled = toI80Scaled(bankBefore.assetShareValue);
    const withdrawnShareScaled = divI80(
      nativeToI80Scaled(withdrawAmountTokenA_native),
      shareValueScaled,
    );
    const userSharesBeforeScaled = toI80Scaled(
      userTokenABalanceBefore.assetShares,
    );
    const bankSharesBeforeScaled = toI80Scaled(bankBefore.totalAssetShares);

    // User loses token-A shares exactly by amount / asset_share_value in I80F48 arithmetic.
    assertI80F48Equal(
      userTokenABalanceAfter.assetShares,
      fromI80Scaled(userSharesBeforeScaled - withdrawnShareScaled),
    );

    // Bank-wide shares must decrease by the same exact amount.
    assertI80F48Equal(
      bankAfter.totalAssetShares,
      fromI80Scaled(bankSharesBeforeScaled - withdrawnShareScaled),
    );
  });

  it("(user 0) repays some USDC debt - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairUsdc.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userUsdcBefore, vaultUsdcBefore] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankBefore.liquidityVault),
    ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    const tx = new Transaction().add(
      await repayIx(user.mrgnProgram, {
        marginfiAccount: userAccKey,
        bank: bank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
        ]),
        amount: repayAmountUsdc_native,
      })
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);
    const events = parseMarginfiEvents(program, result.logMessages);
    const repayEvent = events.find(
      (e) => e.name === "lendingAccountRepayEvent"
    );
    assert.isDefined(repayEvent, "Expected lendingAccountRepayEvent");
    // Repay shares delta can be slightly less than native due to interest, approximate check
    assertI80F48Approx(
      repayEvent!.data.shareAmount,
      repayAmountUsdc_native,
      0.01
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userUsdcAfter, vaultUsdcAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankAfter.liquidityVault),
    ]);
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    // Partial repay only, still has debt
    assert.equal(bankAfter.borrowingPositionCount, 1);
    // Still has deposit in token A
    assert.equal(bankAfter.lendingPositionCount, 1);
    const balancesAfter = userAccAfter.lendingAccount.balances;

    const repayExpected = repayAmountUsdc_native.toNumber();
    if (verbose) {
      console.log(
        "User 0 repaid " +
          repayAmountUsdc +
          " usdc (" +
          repayExpected.toString() +
          ") native",
      );
    }

    // user loses the USDC, the liquidity vault gains it....
    assert.equal(userUsdcAfter, userUsdcBefore - repayExpected);
    assert.equal(vaultUsdcAfter, vaultUsdcBefore + repayExpected);

    // User loses the liability shares of USDC...
    // USDC has some borrows, so there is trivial interest here that affects accounting
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[1].liabilityShares,
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[1].liabilityShares,
    ).toNumber();
    assert.approximately(sharesAfter, sharesBefore - repayExpected, 1);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalLiabilityShares,
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalLiabilityShares,
    ).toNumber();
    assert.approximately(bankSharesAfter, bankSharesBefore - repayExpected, 1);
  });

  it("(user 0) repays all of their USDC debt - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairUsdc.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userUsdcBefore, vaultUsdcBefore] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankBefore.liquidityVault),
    ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;

    const actualOwed =
      wrappedI80F48toBigNumber(balancesBefore[1].liabilityShares).toNumber() *
      wrappedI80F48toBigNumber(bankBefore.liabilityShareValue).toNumber();

    // For repayAll, pass remaining accounts excluding the closing bank.
    const remaining = composeRemainingAccounts(
      balanceAccountGroups.filter((group) => !group[0].equals(bank)),
    );
    const tx = new Transaction().add(
      await repayIx(user.mrgnProgram, {
        marginfiAccount: userAccKey,
        bank: bank,
        tokenAccount: user.usdcAccount,
        remaining,
        amount: u64MAX_BN,
        repayAll: true,
      })
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);
    const events = parseMarginfiEvents(program, result.logMessages);
    const repayEvent = events.find(
      (e) => e.name === "lendingAccountRepayEvent"
    );
    assert.isDefined(repayEvent, "Expected lendingAccountRepayEvent");
    // All shares burned
    assertI80F48Approx(
      repayEvent!.data.shareAmount,
      balancesBefore[1].liabilityShares
    );

    const bankAfter = await program.account.bank.fetch(bank);
    const [userAccAfter, userUsdcAfter, vaultUsdcAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, user.usdcAccount),
      getTokenBalance(provider, bankAfter.liquidityVault),
    ]);

    let now = await getBankrunTime(bankrunContext);
    assert(userAccBefore.lastUpdate != userAccAfter.lastUpdate);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    const balancesAfter = userAccAfter.lendingAccount.balances;

    if (verbose) {
      console.log(
        "User 0 repaid entire USDC balance: ~" + actualOwed.toLocaleString(),
      );
    }

    // USDC has some borrows, so there is trivial interest here that affects accounting

    // user loses the USDC, the liquidity vault gains it....
    assert.approximately(userUsdcAfter, userUsdcBefore - actualOwed, 2);
    assert.approximately(vaultUsdcAfter, vaultUsdcBefore + actualOwed, 2);

    // User loses the liability shares of USDC...
    const sharesBefore = wrappedI80F48toBigNumber(
      balancesBefore[1].liabilityShares,
    ).toNumber();
    const sharesAfter = wrappedI80F48toBigNumber(
      balancesAfter[1].liabilityShares,
    ).toNumber();
    // repayAll should burn *all* liability shares (the amount paid is shares * shareValue).
    assert.approximately(sharesAfter, 0, 0.000001);
    // This balance is now inactive
    assert.equal(balancesAfter[1].active, 0);
    // After repaying all debt, account is lending-only again
    assert.equal(userAccAfter.indexerFlags.isLendingOnly, 1);
    assert.equal(userAccAfter.indexerFlags.isSingleBorrower, 0);
    assertKeysEqual(balancesAfter[0].bankPk, bankKeypairA.publicKey);

    // The bank has also lost the same amount of shares...
    const bankSharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalLiabilityShares,
    ).toNumber();
    const bankSharesAfter = wrappedI80F48toBigNumber(
      bankAfter.totalLiabilityShares,
    ).toNumber();
    assert.approximately(bankSharesAfter, bankSharesBefore - sharesBefore, 2);
  });

  it("(user 0) withdraws all token A balance - happy path", async () => {
    const user = users[0];
    const userAccKey = user.accounts.get(USER_ACCOUNT);

    const bank = bankKeypairA.publicKey;
    const bankBefore = await program.account.bank.fetch(bank);
    const [userAccBefore, userTokenABefore, vaultUsdcBefore] =
      await Promise.all([
        program.account.marginfiAccount.fetch(userAccKey),
        getTokenBalance(provider, user.tokenAAccount),
        getTokenBalance(provider, bankBefore.liquidityVault),
      ]);
    const balancesBefore = userAccBefore.lendingAccount.balances;
    const userTokenABalanceBefore = balancesBefore.find((b) =>
      b.bankPk.equals(bank),
    );
    assert(
      userTokenABalanceBefore,
      "missing token-A balance before withdrawAll",
    );

    const userSharesBeforeScaled = toI80Scaled(
      userTokenABalanceBefore.assetShares,
    );
    const shareValueScaled = toI80Scaled(bankBefore.assetShareValue);
    const currentAssetAmountScaled = mulI80(
      userSharesBeforeScaled,
      shareValueScaled,
    );
    const withdrawExpected = Number(currentAssetAmountScaled >> 48n);

    // After repaying USDC, user 0 has Token A and SOL. Exclude the closing
    // bank (Token A) so the health check alignment is correct.
    const remaining = composeRemainingAccounts([
      [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
    ]);
    const tx = new Transaction().add(
      await withdrawIx(user.mrgnProgram, {
        marginfiAccount: userAccKey,
        bank: bank,
        tokenAccount: user.tokenAAccount,
        remaining,
        amount: withdrawAmountTokenA_native,
        withdrawAll: true,
      })
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);
    const events = parseMarginfiEvents(program, result.logMessages);
    const withdrawEvent = events.find(
      (e) => e.name === "lendingAccountWithdrawEvent"
    );
    assert.isDefined(withdrawEvent, "Expected lendingAccountWithdrawEvent");
    // withdrawAll closes the balance, so share_amount is the full pre-close asset shares
    assertI80F48Approx(
      withdrawEvent!.data.shareAmount,
      balancesBefore[0].assetShares
    );

    const bankAfter = await program.account.bank.fetch(bank);

    const [userAccAfter, vaultUsdcAfter, userTokenAAfter] = await Promise.all([
      program.account.marginfiAccount.fetch(userAccKey),
      getTokenBalance(provider, bankAfter.liquidityVault),
      getTokenBalance(provider, user.tokenAAccount),
    ]);
    const balancesAfter = userAccAfter.lendingAccount.balances;

    let now = await getBankrunTime(bankrunContext);
    assert(userAccBefore.lastUpdate != userAccAfter.lastUpdate);
    assertBNApproximately(userAccAfter.lastUpdate, now, 2);

    if (verbose) {
      console.log(
        "User 0 withdrew all Token A: " + withdrawExpected.toLocaleString(),
      );
    }

    // user gains the token A, the liquidity vault loses it....
    assert.equal(userTokenAAfter, userTokenABefore + withdrawExpected);
    assert.equal(vaultUsdcAfter, vaultUsdcBefore - withdrawExpected);

    const userTokenABalanceAfter = balancesAfter.find((b) =>
      b.bankPk.equals(bank),
    );
    assert.isUndefined(
      userTokenABalanceAfter,
      "token-A balance should be removed after withdrawAll",
    );

    // Bank-wide asset shares should drop exactly by the user's prior share balance.
    assertI80F48Equal(
      bankAfter.totalAssetShares,
      fromI80Scaled(
        toI80Scaled(bankBefore.totalAssetShares) - userSharesBeforeScaled,
      ),
    );
  });

  it("(user 1) withdraws all SOL balance - happy path", async () => {
    // Restore user 1 to only have USDC deposit by withdrawing all SOL.
    const user = users[1];
    const userAccKey = user.accounts.get(USER_ACCOUNT);
    const bank = bankKeypairSol.publicKey;

    // User 1 only has USDC and SOL. Exclude the closing bank (SOL) from
    // remaining accounts so the health check alignment is correct.
    const remaining = composeRemainingAccounts([
      [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
    ]);
    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(user.mrgnProgram, {
          marginfiAccount: userAccKey,
          bank: bank,
          tokenAccount: user.wsolAccount,
          remaining,
          amount: new BN(0),
          withdrawAll: true,
        }),
      ),
    );
    const bankAfter = await program.account.bank.fetch(bank);
    const userAccAfter = await program.account.marginfiAccount.fetch(
      userAccKey,
    );
    const balancesAfter = userAccAfter.lendingAccount.balances;
    assert.equal(bankAfter.lendingPositionCount, 0);

    // This balance is now inactive
    assert.equal(balancesAfter[1].active, 0);
  });

  it("(user 0) restores previous Token A deposits and USDC borrows", async () => {
    const user = users[0];
    const userAcc = user.accounts.get(USER_ACCOUNT);

    const depositAmountA = 2;
    const depositAmountA_native = new BN(
      depositAmountA * 10 ** ecosystem.tokenADecimals,
    );

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKeypairA.publicKey,
          tokenAccount: user.tokenAAccount,
          amount: depositAmountA_native,
        }),
      ),
    );

    const borrowAmountUsdc = 5;
    const borrowAmountUsdc_native = new BN(
      borrowAmountUsdc * 10 ** ecosystem.usdcDecimals,
    );

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(user.mrgnProgram, {
          marginfiAccount: userAcc,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: user.usdcAccount,
          remaining: composeRemainingAccounts([
            [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
            [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
          ]),
          amount: borrowAmountUsdc_native,
        }),
      ),
    );

    const userAccAfter = await program.account.marginfiAccount.fetch(userAcc);
    let balances = userAccAfter.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    assertKeysEqual(balances[0].bankPk, bankKeypairA.publicKey);
    assert.equal(balances[1].active, 1);
    assertKeysEqual(balances[1].bankPk, bankKeypairUsdc.publicKey);
  });
});
