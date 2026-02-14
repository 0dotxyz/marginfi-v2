import { BN } from "@coral-xyz/anchor";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { PublicKey, Transaction } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import { assert } from "chai";

import {
  bankRunProvider,
  banksClient,
  bankrunContext,
  bankrunProgram,
  ecosystem,
  juplendAccounts,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import { assertBNEqual, getTokenBalance } from "./utils/genericTests";
import {
  depositIx,
  healthPulse,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import {
  processBankrunTransaction,
  bytesToF64,
  mintToTokenAccount,
} from "./utils/tools";
import {
  ASSET_TAG_JUPLEND,
  CONF_INTERVAL_MULTIPLE,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";
import { deriveJuplendPoolKeys } from "./utils/juplend/juplend-pdas";
import {
  makeJuplendDepositIx,
  makeJuplendNativeUpdateRateIx,
} from "./utils/juplend/user-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { EXCHANGE_PRICES_PRECISION } from "./utils/juplend/constants";

const toNative = (uiAmount: number, decimals: number) =>
  new BN(uiAmount).mul(new BN(10).pow(new BN(decimals)));

const JUP_TOKEN_A_DEPOSIT = toNative(5, ecosystem.tokenADecimals);
const JUP_WSOL_DEPOSIT = toNative(1, ecosystem.wsolDecimals);
const REG_TOKEN_B_DEPOSIT = toNative(8, ecosystem.tokenBDecimals);
const REG_LST_DEPOSIT = toNative(2, ecosystem.lstAlphaDecimals);

describe("jlr03: JupLend multi-deposit + health pulse (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let user = users[0];
  let userMarginfiAccountPk = PublicKey.default;

  let jupUsdcBankPk = PublicKey.default;
  let jupTokenABankPk = PublicKey.default;
  let jupWsolBankPk = PublicKey.default;
  let regTokenBBankPk = PublicKey.default;
  let regLstBankPk = PublicKey.default;

  type JupBankCtx = {
    bankPk: PublicKey;
    pool: ReturnType<typeof deriveJuplendPoolKeys>;
    mint: PublicKey;
    decimals: number;
  };
  let jupCtxByBank = new Map<string, JupBankCtx>();

  const requireStateKey = (key: string): PublicKey => {
    const value = juplendAccounts.get(key);
    if (!value) {
      throw new Error(`missing juplend test state key: ${key}`);
    }
    return value;
  };

  // Note: All prices in this test are for assets, so the get a confidence discount
  const adjustedOraclePriceForMint = (mint: PublicKey): number => {
    const confAdj = 1 - ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    if (mint.equals(ecosystem.usdcMint.publicKey))
      return ecosystem.usdcPrice * confAdj;
    if (mint.equals(ecosystem.tokenAMint.publicKey))
      return ecosystem.tokenAPrice * confAdj;
    if (mint.equals(ecosystem.wsolMint.publicKey))
      return ecosystem.wsolPrice * confAdj;
    if (mint.equals(ecosystem.tokenBMint.publicKey))
      return ecosystem.tokenBPrice * confAdj;
    if (mint.equals(ecosystem.lstAlphaMint.publicKey))
      return ecosystem.lstAlphaPrice * confAdj;
    throw new Error(`unsupported mint for expected price: ${mint.toBase58()}`);
  };

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    user = users[0];
    userMarginfiAccountPk = requireStateKey(
      JUPLEND_STATE_KEYS.jlr02User0MarginfiAccount,
    );

    jupUsdcBankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    jupTokenABankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01BankTokenA);
    jupWsolBankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01BankWsol);
    regTokenBBankPk = requireStateKey(
      JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
    );
    regLstBankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01RegularBankLst);

    const [jupUsdcBank, jupTokenABank, jupWsolBank] = await Promise.all([
      bankrunProgram.account.bank.fetch(jupUsdcBankPk),
      bankrunProgram.account.bank.fetch(jupTokenABankPk),
      bankrunProgram.account.bank.fetch(jupWsolBankPk),
    ]);

    const jupBanks: JupBankCtx[] = [
      {
        bankPk: jupUsdcBankPk,
        pool: deriveJuplendPoolKeys({ mint: jupUsdcBank.mint }),
        mint: jupUsdcBank.mint,
        decimals: jupUsdcBank.mintDecimals,
      },
      {
        bankPk: jupTokenABankPk,
        pool: deriveJuplendPoolKeys({ mint: jupTokenABank.mint }),
        mint: jupTokenABank.mint,
        decimals: jupTokenABank.mintDecimals,
      },
      {
        bankPk: jupWsolBankPk,
        pool: deriveJuplendPoolKeys({ mint: jupWsolBank.mint }),
        mint: jupWsolBank.mint,
        decimals: jupWsolBank.mintDecimals,
      },
    ];
    jupCtxByBank = new Map(jupBanks.map((ctx) => [ctx.bankPk.toBase58(), ctx]));

    await Promise.all([
      mintToTokenAccount(
        ecosystem.tokenAMint.publicKey,
        user.tokenAAccount,
        JUP_TOKEN_A_DEPOSIT.mul(new BN(2)),
      ),
      mintToTokenAccount(
        ecosystem.wsolMint.publicKey,
        user.wsolAccount,
        JUP_WSOL_DEPOSIT.mul(new BN(2)),
      ),
      mintToTokenAccount(
        ecosystem.tokenBMint.publicKey,
        user.tokenBAccount,
        REG_TOKEN_B_DEPOSIT.mul(new BN(2)),
      ),
      mintToTokenAccount(
        ecosystem.lstAlphaMint.publicKey,
        user.lstAlphaAccount,
        REG_LST_DEPOSIT.mul(new BN(2)),
      ),
    ]);
  });

  it("(user 0) deposits across multiple Jup + regular banks", async () => {
    const [tokenABefore, wsolBefore, tokenBBefore, lstBefore] =
      await Promise.all([
        getTokenBalance(bankRunProvider, user.tokenAAccount),
        getTokenBalance(bankRunProvider, user.wsolAccount),
        getTokenBalance(bankRunProvider, user.tokenBAccount),
        getTokenBalance(bankRunProvider, user.lstAlphaAccount),
      ]);

    const tokenAJupBank = await bankrunProgram.account.bank.fetch(
      jupTokenABankPk,
    );
    const wsolJupBank = await bankrunProgram.account.bank.fetch(jupWsolBankPk);

    const depositTokenAJupIx = await makeJuplendDepositIx(
      user.mrgnBankrunProgram!,
      {
        marginfiAccount: userMarginfiAccountPk,
        signerTokenAccount: user.tokenAAccount,
        bank: jupTokenABankPk,
        fTokenVault: tokenAJupBank.integrationAcc2,
        pool: jupCtxByBank.get(jupTokenABankPk.toBase58())!.pool,
        amount: JUP_TOKEN_A_DEPOSIT,
      },
    );

    const depositWsolJupIx = await makeJuplendDepositIx(
      user.mrgnBankrunProgram!,
      {
        marginfiAccount: userMarginfiAccountPk,
        signerTokenAccount: user.wsolAccount,
        bank: jupWsolBankPk,
        fTokenVault: wsolJupBank.integrationAcc2,
        pool: jupCtxByBank.get(jupWsolBankPk.toBase58())!.pool,
        amount: JUP_WSOL_DEPOSIT,
      },
    );

    const depositTokenBMrgnIx = await depositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccountPk,
      bank: regTokenBBankPk,
      tokenAccount: user.tokenBAccount,
      amount: REG_TOKEN_B_DEPOSIT,
    });

    const depositLstMrgnIx = await depositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccountPk,
      bank: regLstBankPk,
      tokenAccount: user.lstAlphaAccount,
      amount: REG_LST_DEPOSIT,
    });

    for (const ix of [
      depositTokenAJupIx,
      depositWsolJupIx,
      depositTokenBMrgnIx,
      depositLstMrgnIx,
    ]) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [user.wallet],
        false,
        true,
      );
    }

    const [tokenAAfter, wsolAfter, tokenBAfter, lstAfter, userAccount] =
      await Promise.all([
        getTokenBalance(bankRunProvider, user.tokenAAccount),
        getTokenBalance(bankRunProvider, user.wsolAccount),
        getTokenBalance(bankRunProvider, user.tokenBAccount),
        getTokenBalance(bankRunProvider, user.lstAlphaAccount),
        bankrunProgram.account.marginfiAccount.fetch(userMarginfiAccountPk),
      ]);

    assertBNEqual(new BN(tokenABefore - tokenAAfter), JUP_TOKEN_A_DEPOSIT);
    assertBNEqual(new BN(wsolBefore - wsolAfter), JUP_WSOL_DEPOSIT);
    assertBNEqual(new BN(tokenBBefore - tokenBAfter), REG_TOKEN_B_DEPOSIT);
    assertBNEqual(new BN(lstBefore - lstAfter), REG_LST_DEPOSIT);

    const activeBanks = userAccount.lendingAccount.balances
      .filter((b) => b.active)
      .map((b) => b.bankPk.toBase58());

    for (const bankPk of [
      jupUsdcBankPk,
      jupTokenABankPk,
      jupWsolBankPk,
      regTokenBBankPk,
      regLstBankPk,
    ]) {
      assert.include(activeBanks, bankPk.toBase58());
    }
  });

  it("(user 0) health pulse reflects accurate Jup bank prices and healthy account state", async () => {
    // jlr02 advances the clock by one hour; refresh pull oracles before pulse.
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const userAccountBefore =
      await bankrunProgram.account.marginfiAccount.fetch(userMarginfiAccountPk);

    const remainingGroups: PublicKey[][] = [];
    const refreshRateIxs = [];
    for (const balance of userAccountBefore.lendingAccount.balances) {
      if (!balance.active) continue;
      const bank = await bankrunProgram.account.bank.fetch(balance.bankPk);
      const accounts = [balance.bankPk, bank.config.oracleKeys[0]];
      if (!bank.config.oracleKeys[1].equals(PublicKey.default)) {
        accounts.push(bank.config.oracleKeys[1]);
      }
      remainingGroups.push(accounts);

      const jupCtx = jupCtxByBank.get(balance.bankPk.toBase58());
      if (jupCtx) {
        refreshRateIxs.push(
          await makeJuplendNativeUpdateRateIx(juplendPrograms.lending, {
            pool: jupCtx.pool,
          }),
        );
      }
    }

    const pulseIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccountPk,
      remaining: composeRemainingAccounts(remainingGroups),
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...refreshRateIxs, pulseIx),
      [user.wallet],
      false,
      true,
    );

    const userAccountAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccountPk,
    );
    const healthCache = userAccountAfter.healthCache;

    assert.ok((healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);
    assert.equal(healthCache.internalErr, 0);

    const pulsePrices = healthCache.prices.map((p) => bytesToF64(p));

    let expectedAssetValue = new BigNumber(0);
    for (const [
      idx,
      balance,
    ] of userAccountBefore.lendingAccount.balances.entries()) {
      if (!balance.active) continue;
      const bank = await bankrunProgram.account.bank.fetch(balance.bankPk);

      let expectedPrice = adjustedOraclePriceForMint(bank.mint);
      // we only care juplend is priced correctly in this test suite
      if (bank.config.assetTag === ASSET_TAG_JUPLEND) {
        const jupCtx = jupCtxByBank.get(balance.bankPk.toBase58());
        const lending = await juplendPrograms.lending.account.lending.fetch(
          jupCtx.pool.lending,
        );
        const exchange =
          Number(lending.tokenExchangePrice.toString()) /
          EXCHANGE_PRICES_PRECISION;
        expectedPrice *= exchange;
      }

      const pulsePrice = pulsePrices[idx];
      if (verbose) {
        console.log("price: " + pulsePrice + " exp: " + expectedPrice);
      }
      const priceTolerance = expectedPrice * 0.002;
      assert.approximately(
        pulsePrice,
        expectedPrice,
        priceTolerance,
        `pulse price mismatch for bank=${balance.bankPk.toBase58()} idx=${idx}`,
      );

      const shares = wrappedI80F48toBigNumber(balance.assetShares);
      const amountUi = shares.div(new BigNumber(10).pow(bank.mintDecimals));
      const weight = wrappedI80F48toBigNumber(bank.config.assetWeightInit);
      expectedAssetValue = expectedAssetValue.plus(
        amountUi.multipliedBy(expectedPrice).multipliedBy(weight),
      );
    }

    const actualAssetValue = wrappedI80F48toBigNumber(healthCache.assetValue);
    const assetValueTolerance = expectedAssetValue.multipliedBy(0.002);
    const absDiff = actualAssetValue.minus(expectedAssetValue).abs();
    assert.ok(
      absDiff.lte(assetValueTolerance),
      `unexpected health asset value, expected=${expectedAssetValue.toFixed(
        8,
      )} actual=${actualAssetValue.toFixed(8)}`,
    );
  });
});

// TODO repeat this at the end of the suite once interest has accrued
