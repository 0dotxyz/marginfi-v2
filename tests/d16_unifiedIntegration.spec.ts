import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  driftAccounts,
  driftBankrunProgram,
  DRIFT_USDC_BANK,
  DRIFT_USDC_SPOT_MARKET,
  ecosystem,
  oracles,
  users,
} from "./rootHooks";
import { MockUser } from "./utils/mocks";
import { accountInit } from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  getTokenBalance,
  parseMarginfiEvents,
} from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { tokenAmountToScaledBalance } from "./utils/drift-utils";
import {
  driftIntegrationProtocolMetas,
  makeIntegrationDepositIx,
  makeIntegrationWithdrawIx,
} from "./utils/integration-instructions";

describe("d16: Unified integration deposit/withdraw (Drift)", () => {
  const marginfiAccountKeypair = Keypair.generate();
  const marginfiAccount = marginfiAccountKeypair.publicKey;
  let depositAmount: BN;
  let user: MockUser;
  let bank: PublicKey;
  let spotMarket: PublicKey;
  let liquidityVault: PublicKey;

  before(async () => {
    user = users[1];
    bank = driftAccounts.get(DRIFT_USDC_BANK);
    spotMarket = driftAccounts.get(DRIFT_USDC_SPOT_MARKET);
    depositAmount = new BN(10 * 10 ** ecosystem.usdcDecimals);

    const bankAcc = await bankrunProgram.account.bank.fetch(bank);
    liquidityVault = bankAcc.liquidityVault;
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      user.usdcAccount,
      depositAmount,
    );
    const initIx = await accountInit(user.mrgnBankrunProgram, {
      marginfiGroup: bankAcc.group,
      marginfiAccount,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [user.wallet, marginfiAccountKeypair],
    );
  });

  it("(user 1) deposits to the Drift bank via integration_deposit - happy path", async () => {
    const [userUsdcBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);

    const tx = new Transaction().add(
      await makeIntegrationDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.usdcAccount,
        },
        depositAmount,
        { drift: {} },
        await driftIntegrationProtocolMetas(
          driftBankrunProgram,
          spotMarket,
          "deposit",
        ),
      ),
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);

    const depositEvent = parseMarginfiEvents(
      bankrunProgram,
      result.logMessages,
    ).find((e) => e.name === "lendingAccountDepositEvent");
    assert.isDefined(depositEvent, "Expected lendingAccountDepositEvent");
    assertBNEqual(depositEvent!.data.amount, depositAmount);

    const [userUsdcAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    assert.equal(userUsdcBefore - userUsdcAfter, depositAmount.toNumber());
    // Funds pass through the liquidity vault into the venue within the instruction
    assert.equal(vaultAfter, vaultBefore);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount,
    );
    const balance = acc.lendingAccount.balances.find(
      (b: any) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isDefined(balance, "Expected an active balance for the bank");
    // Shares are Drift scaled balance units (9-decimal precision, discounted by cumulative
    // interest); tolerance covers rounding of the conversion
    const spotMarketAcc =
      await driftBankrunProgram.account.spotMarket.fetch(spotMarket);
    const expectedShares = tokenAmountToScaledBalance(
      depositAmount,
      spotMarketAcc,
    );
    const shares = wrappedI80F48toBigNumber(balance.assetShares).toNumber();
    assert.approximately(shares, expectedShares.toNumber(), 10);
  });

  it("(user 1) deposit with wrong integration_acc_1 - should fail", async () => {
    const ix = await makeIntegrationDepositIx(
      user.mrgnBankrunProgram,
      {
        marginfiAccount,
        bank,
        signerTokenAccount: user.usdcAccount,
        integrationAccOverrides: { integrationAcc1: Keypair.generate().publicKey },
      },
      depositAmount,
      { drift: {} },
      await driftIntegrationProtocolMetas(
        driftBankrunProgram,
        spotMarket,
        "deposit",
      ),
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [user.wallet],
      true,
    );
    // IntegrationAccountKeyMismatch (has_one on the bank)
    assertBankrunTxFailed(result, 6602);
  });

  it("(user 1) withdraws all from the Drift bank via integration_withdraw - happy path", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const [userUsdcBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    const oracleKeys = await buildHealthRemainingAccounts(marginfiAccount);

    const tx = new Transaction().add(
      await makeIntegrationWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          destinationTokenAccount: user.usdcAccount,
        },
        { amount: new BN(0), withdrawAll: true, oracleKeys },
        { drift: {} },
        await driftIntegrationProtocolMetas(
          driftBankrunProgram,
          spotMarket,
          "withdraw",
        ),
      ),
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);

    const withdrawEvent = parseMarginfiEvents(
      bankrunProgram,
      result.logMessages,
    ).find((e) => e.name === "lendingAccountWithdrawEvent");
    assert.isDefined(withdrawEvent, "Expected lendingAccountWithdrawEvent");
    assert.isTrue(withdrawEvent!.data.closeBalance);

    const [userUsdcAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    // Tolerance covers venue rounding and interest accrued over the few slots since deposit
    assert.approximately(
      userUsdcAfter - userUsdcBefore,
      depositAmount.toNumber(),
      100,
    );
    // Funds pass through the liquidity vault to the destination within the instruction
    assert.equal(vaultAfter, vaultBefore);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount,
    );
    const balance = acc.lendingAccount.balances.find(
      (b: any) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isUndefined(balance, "Expected the balance to be closed");
  });
});
