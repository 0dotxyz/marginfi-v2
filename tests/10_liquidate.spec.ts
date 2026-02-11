import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { ComputeBudgetProgram, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertI80F48Equal,
  assertKeysEqual,
  expectFailedTxWithError,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  composeRemainingAccounts,
  liquidateIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { CONF_INTERVAL_MULTIPLE, defaultBankConfigOptRaw } from "./utils/types";
import { configureBank } from "./utils/group-instructions";
import { getBankrunTime } from "./utils/tools";

let program: Program<Marginfi>;
let provider: BankrunProvider;

describe("Liquidate user", () => {
  before(() => {
    provider = bankRunProvider;
    program = bankrunProgram;
  });

  const confidenceInterval = 0.01 * CONF_INTERVAL_MULTIPLE;
  const liquidateAmountA = 0.2;
  const liquidateAmountA_native = new BN(
    liquidateAmountA * 10 ** ecosystem.tokenADecimals
  );

  it("(user 1) tries to sneak in a bad oracle for itself - should fail", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];

    const assetBankKey = bankKeypairA.publicKey;
    const liabilityBankKey = bankKeypairUsdc.publicKey;
    console.log("asset bank key", assetBankKey.toString()); //6g
    console.log("liability bank key", liabilityBankKey.toString()); //47

    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT);
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT);

    await expectFailedTxWithError(
      async () => {
        await liquidator.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({
              units: 300_000,
            }),
            await liquidateIx(liquidator.mrgnProgram, {
              assetBankKey,
              liabilityBankKey,
              liquidatorMarginfiAccount: liquidatorAccount,
              liquidateeMarginfiAccount: liquidateeAccount,
              remaining: [
                oracles.tokenAOracle.publicKey,
                oracles.usdcOracle.publicKey,
                ...composeRemainingAccounts([
                  [liabilityBankKey, oracles.fakeUsdc], // sneaky sneaky
                  [assetBankKey, oracles.tokenAOracle.publicKey],
                ]),
                ...composeRemainingAccounts([
                  [liabilityBankKey, oracles.usdcOracle.publicKey],
                  [assetBankKey, oracles.tokenAOracle.publicKey],
                ]),
              ],
              amount: liquidateAmountA_native,
              liquidateeAccounts: 4,
              liquidatorAccounts: 4,
            })
          )
        );
        // TODO this should throw a more oracle-specific error further upstream, this is kinda dumb.
      },
      "HealthyAccount",
      6068
    );
  });

  it("(user 1) tries to sneak in a bad oracle for the liquidatee - should fail", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];

    const assetBankKey = bankKeypairA.publicKey;
    const liabilityBankKey = bankKeypairUsdc.publicKey;
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT);
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT);

    await expectFailedTxWithError(
      async () => {
        await liquidator.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(

            ComputeBudgetProgram.setComputeUnitLimit({
              units: 300_000,
            }),
            await liquidateIx(liquidator.mrgnProgram, {
              assetBankKey,
              liabilityBankKey,
              liquidatorMarginfiAccount: liquidatorAccount,
              liquidateeMarginfiAccount: liquidateeAccount,
              remaining: [
                oracles.tokenAOracle.publicKey,
                oracles.usdcOracle.publicKey,
                ...composeRemainingAccounts([
                  [liabilityBankKey, oracles.usdcOracle.publicKey],
                  [assetBankKey, oracles.tokenAOracle.publicKey],
                ]),
                ...composeRemainingAccounts([
                  [liabilityBankKey, oracles.fakeUsdc], // sneaky sneaky
                  [assetBankKey, oracles.tokenAOracle.publicKey],
                ]),
              ],
              amount: liquidateAmountA_native,
              liquidateeAccounts: 4,
              liquidatorAccounts: 4,
            })
          )
        );
      },
      "WrongOracleAccountKeys",
      6052
    );
  });

  it("(admin) vastly reduce Token A bank collateral ratio to induce liquidation", async () => {
    let config = defaultBankConfigOptRaw();
    config.assetWeightInit = bigNumberToWrappedI80F48(0.05);
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.1);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await configureBank(groupAdmin.mrgnProgram, {
          bank: bankKeypairA.publicKey,
          bankConfigOpt: config,
        })
      )
    );
  });

  /**
   * Maintenance ratio allowed = 10%
   * Liquidator fee = 2.5%
   * Insurance fee = 2.5%
   * Confidence interval = 2.12% (1% confidence * 2.12 = 2.12%)
   *
   * Token A is worth $10 with conf $0.212 (worth $9.788 low, $10.212 high)
   * USDC is worth $1 with conf $0.0212 (worth $0.9788 low, $1.0212 high)
   *
   * User has:
   * ASSETS
   *    [index 0] 200,000,000 (2) Token A (worth $20)
   * DEBTS
   *    [index 1] 5,050,000 (5.05) USDC (worth $5.05)
   * Note: $5.05 is 25.25% of $20, which is more than 10%, so liquidation is allowed
   *
   * Health calc before: (2 * 9.788 * .1) - (5.05 * 1.0212) = -3.19946
   *
   * Liquidator tries to repay .2 token A (worth $2) of liquidatee's debt, so liquidator's assets
   * increase by this value, while liquidatee's assets decrease by this value. Which also means that:
   *
   * Liquidator must pay
   *  value of A minus liquidator fee (low bias within the confidence interval): .2 * (1 - 0.025) * 9.788 = $1.90866
   *  USDC equivalent (high bias): 1.90866 / 1.0212 = $1.869036 (1,869,036 native)
   *
   * Liquidatee receives
   *  value of A minus (liquidator fee + insurance) (low bias): .2 * (1 - 0.025 - 0.025) * 9.788 = $1.8608
   *  USDC equivalent (high bias): 1.8608 / 1.0212 = $1.822457 (1,822,457 native)
   *
   * Insurance fund collects the difference
   *  USDC diff 1,869,036  - 1,822,457 = 46,579
   *
   * Health calc after: ((2-0.2) * 9.788 * .1) - ((5.05-1.8608) * 1.0212) = -1.49497104
   */

  it("(user 1) liquidate user 0 who borrowed USDC against their token A position - happy path", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];

    const assetBankKey = bankKeypairA.publicKey;
    const assetBankBefore = await program.account.bank.fetch(assetBankKey);
    const liabilityBankKey = bankKeypairUsdc.publicKey;
    const liabilityBankBefore = await program.account.bank.fetch(
      liabilityBankKey
    );

    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT);
    const liquidateeMarginfiAccount =
      await program.account.marginfiAccount.fetch(liquidateeAccount);

    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT);
    const liquidatorMarginfiAccount =
      await program.account.marginfiAccount.fetch(liquidatorAccount);

    const liquidateeBalances =
      liquidateeMarginfiAccount.lendingAccount.balances;
    const liquidatorBalances =
      liquidatorMarginfiAccount.lendingAccount.balances;
    const liabilitySharesBefore = liquidateeBalances[0].liabilityShares;
    assertI80F48Equal(liquidatorBalances[1].assetShares, 0);

    const insuranceVaultBalance = await getTokenBalance(
      provider,
      liabilityBankBefore.insuranceVault
    );
    assert.equal(insuranceVaultBalance, 0);

    const sharesA = wrappedI80F48toBigNumber(
      liquidateeBalances[1].assetShares
    ).toNumber();
    const shareValueA = wrappedI80F48toBigNumber(
      assetBankBefore.assetShareValue
    ).toNumber();
    const sharesUsdc = wrappedI80F48toBigNumber(
      liabilitySharesBefore
    ).toNumber();
    const shareValueUsdc = wrappedI80F48toBigNumber(
      liabilityBankBefore.liabilityShareValue
    ).toNumber();
    if (verbose) {
      console.log("BEFORE");
      console.log(
        "liability bank insurance vault before: " +
        insuranceVaultBalance.toLocaleString()
      );
      console.log(
        "user 0 (liquidatee) Token A asset shares: " + sharesA.toString()
      );
      console.log(
        "  value (in Token A native): " +
        (sharesA * shareValueA).toLocaleString()
      );
      console.log(
        "  value (in dollars): $" +
        (
          (sharesA * shareValueA * oracles.tokenAPrice) /
          10 ** oracles.tokenADecimals
        ).toLocaleString()
      );
      console.log(
        "user 0 (liquidatee) USDC liability shares: " + sharesUsdc.toString()
      );
      console.log(
        "  debt (in USDC native): " +
        (sharesUsdc * shareValueUsdc).toLocaleString()
      );
      console.log(
        "  debt (in dollars): $" +
        (
          (sharesUsdc * shareValueUsdc * oracles.usdcPrice) /
          10 ** oracles.usdcDecimals
        ).toLocaleString()
      );
      console.log(
        "user 1 (liquidator) USDC asset shares: " +
        wrappedI80F48toBigNumber(liquidatorBalances[0].assetShares).toString()
      );
      console.log(
        "user 1 (liquidator) USDC liability shares: " +
        wrappedI80F48toBigNumber(
          liquidatorBalances[0].liabilityShares
        ).toString()
      );
    }

    const tokenALowPrice = oracles.tokenAPrice * (1 - confidenceInterval); // see top of test
    const usdcHighPrice = oracles.usdcPrice * (1 + confidenceInterval); // see top of test
    const insuranceToBeCollected =
      ((liquidateAmountA * 0.025 * shareValueA * tokenALowPrice) /
        (shareValueUsdc * usdcHighPrice)) *
      10 ** oracles.usdcDecimals;

    let liquidatorAccounts = composeRemainingAccounts([
      [liabilityBankKey, oracles.usdcOracle.publicKey],
      [assetBankKey, oracles.tokenAOracle.publicKey],
    ]);
    let liquidateeAccounts = composeRemainingAccounts([
      [liabilityBankKey, oracles.usdcOracle.publicKey],
      [assetBankKey, oracles.tokenAOracle.publicKey],
    ]);

    await liquidator.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({
          units: 350_000,
        }),
        await liquidateIx(liquidator.mrgnProgram, {
          assetBankKey,
          liabilityBankKey,
          liquidatorMarginfiAccount: liquidatorAccount,
          liquidateeMarginfiAccount: liquidateeAccount,
          remaining: [
            oracles.tokenAOracle.publicKey,
            oracles.usdcOracle.publicKey,
            ...liquidatorAccounts,
            ...liquidateeAccounts,
          ],
          amount: liquidateAmountA_native,
          liquidateeAccounts: liquidateeAccounts.length,
          liquidatorAccounts: liquidatorAccounts.length,
        })
      )
    );

    const liquidateeMarginfiAccountAfter =
      await program.account.marginfiAccount.fetch(liquidateeAccount);
    const liquidatorMarginfiAccountAfter =
      await program.account.marginfiAccount.fetch(liquidatorAccount);

    const liquidateeBalancesAfter =
      liquidateeMarginfiAccountAfter.lendingAccount.balances;
    const liquidatorBalancesAfter =
      liquidatorMarginfiAccountAfter.lendingAccount.balances;

    const sharesAAfter = wrappedI80F48toBigNumber(
      liquidateeBalancesAfter[0].assetShares
    ).toNumber();
    const sharesUsdcAfter = wrappedI80F48toBigNumber(
      liquidateeBalancesAfter[1].liabilityShares
    ).toNumber();

    assertI80F48Equal(
      liquidateeBalancesAfter[0].assetShares,
      wrappedI80F48toBigNumber(liquidateeBalances[0].assetShares).toNumber() -
      liquidateAmountA_native.toNumber()
    );
    assertI80F48Equal(liquidateeBalancesAfter[0].liabilityShares, 0);
    assertI80F48Equal(liquidateeBalancesAfter[1].assetShares, 0);

    assert.equal(liquidatorBalancesAfter[1].active, 1);
    assertKeysEqual(liquidatorBalancesAfter[1].bankPk, liabilityBankKey);
    assertKeysEqual(liquidatorBalancesAfter[0].bankPk, assetBankKey);

    assertI80F48Equal(liquidatorBalancesAfter[1].liabilityShares, 0);
    assertI80F48Equal(
      liquidatorBalancesAfter[0].assetShares,
      liquidateAmountA_native
    );
    assertI80F48Equal(liquidatorBalancesAfter[0].liabilityShares, 0);

    const insuranceVaultBalanceAfter = await getTokenBalance(
      provider,
      liabilityBankBefore.insuranceVault
    );

    assert.approximately(
      insuranceVaultBalanceAfter,
      insuranceToBeCollected,
      insuranceToBeCollected * 0.1
    ); // see top of test

    if (verbose) {
      console.log("AFTER");
      console.log(
        "liability bank insurance vault after (usdc): " +
        insuranceVaultBalanceAfter.toLocaleString()
      );
      console.log(
        "user 0 (liquidatee) Token A asset shares after: " +
        sharesAAfter.toString()
      );
      console.log(
        "  value (in Token A native): " +
        (sharesAAfter * shareValueA).toLocaleString()
      );
      console.log(
        "  value (in dollars): $" +
        (
          (sharesAAfter * shareValueA * oracles.tokenAPrice) /
          10 ** oracles.tokenADecimals
        ).toLocaleString()
      );
      console.log(
        "user 0 (liquidatee) USDC liability shares after: " +
        sharesUsdcAfter.toString()
      );
      console.log(
        "  debt (in USDC native): " +
        (sharesUsdcAfter * shareValueUsdc).toLocaleString()
      );
      console.log(
        "  debt (in dollars): $" +
        (
          (sharesUsdcAfter * shareValueUsdc * oracles.usdcPrice) /
          10 ** oracles.usdcDecimals
        ).toLocaleString()
      );
      console.log(
        "user 1 (liquidator) USDC asset shares after: " +
        wrappedI80F48toBigNumber(
          liquidatorBalancesAfter[0].assetShares
        ).toString()
      );
      console.log(
        "user 1 (liquidator) USDC liability shares after: " +
        wrappedI80F48toBigNumber(
          liquidatorBalancesAfter[0].liabilityShares
        ).toString()
      );
      console.log(
        "user 1 (liquidator) Token A asset shares after: " +
        wrappedI80F48toBigNumber(
          liquidatorBalancesAfter[1].assetShares
        ).toString()
      );
      console.log(
        "user 1 (liquidator) Token A liability shares after: " +
        wrappedI80F48toBigNumber(
          liquidatorBalancesAfter[1].liabilityShares
        ).toString()
      );
    }

    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(liquidatorBalancesAfter[0].lastUpdate, now, 2);
    assertBNApproximately(liquidatorBalancesAfter[1].lastUpdate, now, 2);
    assertBNApproximately(liquidatorMarginfiAccountAfter.lastUpdate, now, 2);
    assertBNApproximately(liquidateeBalancesAfter[0].lastUpdate, now, 2);
    assertBNApproximately(liquidateeBalancesAfter[1].lastUpdate, now, 2);
    assertBNApproximately(liquidateeMarginfiAccountAfter.lastUpdate, now, 2);
  });
});
