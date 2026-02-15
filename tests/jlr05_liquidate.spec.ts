import { BN } from "@coral-xyz/anchor";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import BigNumber from "bignumber.js";
import { assert } from "chai";

import {
  bankRunProvider,
  banksClient,
  bankrunContext,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import {
  assertBNEqual,
  assertBNGreaterThan,
  getTokenBalance,
} from "./utils/genericTests";
import { deriveJuplendPoolKeys } from "./utils/juplend/juplend-pdas";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  accountInit,
  borrowIx,
  healthPulse,
  liquidateIx,
} from "./utils/user-instructions";
import { configureBank } from "./utils/group-instructions";
import {
  CONF_INTERVAL_MULTIPLE,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_CONF_INTERVAL,
  defaultBankConfigOptRaw,
} from "./utils/types";
import {
  buildHealthRemainingAccounts,
  logHealthCache,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

const USER1_ACCOUNT_SEED = Buffer.from("JLR05_USER1_ACCOUNT_SEED_0000000");
const user1MarginfiAccount = Keypair.fromSeed(USER1_ACCOUNT_SEED);

const JUP_USDC_DEPOSIT_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
const TOKEN_B_BORROW_AMOUNT = new BN(2.5 * 10 ** ecosystem.tokenBDecimals); // 2.5 TOKEN_B (~$50 nominal)
const JUP_USDC_LIQUIDATION_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals); // 1 USDC
const LIAB_WEIGHT_INDUCED = 2;

describe("jlr05: Juplend collateral + mrgn borrow + health pulse (bankrun)", () => {
  let user = users[1];
  let groupPk = PublicKey.default;
  let jupUsdcBankPk = PublicKey.default;
  let regTokenBBankPk = PublicKey.default;
  let user0MarginfiAccountPk = PublicKey.default;

  before(async () => {
    user = users[1];
    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);
    jupUsdcBankPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    regTokenBBankPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
    );
    user0MarginfiAccountPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr02User0MarginfiAccount,
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      user.usdcAccount,
      JUP_USDC_DEPOSIT_AMOUNT.mul(new BN(2)),
    );

    const initIx = await accountInit(user.mrgnBankrunProgram!, {
      marginfiGroup: groupPk,
      marginfiAccount: user1MarginfiAccount.publicKey,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [user.wallet, user1MarginfiAccount],
      false,
      true,
    );

    juplendAccounts.set(
      JUPLEND_STATE_KEYS.jlr05User1MarginfiAccount,
      user1MarginfiAccount.publicKey,
    );
  });

  it("(user 1) borrows regular TokenB against Juplend USDC collateral and health declines as expected", async () => {
    const jupUsdcBank = await bankrunProgram.account.bank.fetch(jupUsdcBankPk);
    const usdcPool = deriveJuplendPoolKeys({ mint: jupUsdcBank.mint });

    const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: user1MarginfiAccount.publicKey,
      signerTokenAccount: user.usdcAccount,
      bank: jupUsdcBankPk,
      fTokenVault: jupUsdcBank.integrationAcc2,
      pool: usdcPool,
      amount: JUP_USDC_DEPOSIT_AMOUNT,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [user.wallet],
      false,
      true,
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const pulseBeforeIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: user1MarginfiAccount.publicKey,
      remaining: await buildHealthRemainingAccounts(
        user1MarginfiAccount.publicKey,
      ),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseBeforeIx),
      [user.wallet],
      false,
      true,
    );

    const accountBeforeBorrow =
      await bankrunProgram.account.marginfiAccount.fetch(
        user1MarginfiAccount.publicKey,
      );
    const healthBefore = accountBeforeBorrow.healthCache;
    const netHealthBefore = wrappedI80F48toBigNumber(
      healthBefore.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthBefore.liabilityValue));

    const borrowBank = await bankrunProgram.account.bank.fetch(regTokenBBankPk);
    const tokenBBalanceBefore = await getTokenBalance(
      bankRunProvider,
      user.tokenBAccount,
    );

    const borrowInstruction = await borrowIx(user.mrgnBankrunProgram!, {
      marginfiAccount: user1MarginfiAccount.publicKey,
      bank: regTokenBBankPk,
      tokenAccount: user.tokenBAccount,
      remaining: await buildHealthRemainingAccounts(
        user1MarginfiAccount.publicKey,
        {
          includedBankPks: [regTokenBBankPk],
        },
      ),
      amount: TOKEN_B_BORROW_AMOUNT,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(borrowInstruction),
      [user.wallet],
      false,
      true,
    );

    const tokenBBalanceAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenBAccount,
    );
    assertBNEqual(
      new BN(tokenBBalanceAfter - tokenBBalanceBefore),
      TOKEN_B_BORROW_AMOUNT,
    );

    const pulseAfterIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: user1MarginfiAccount.publicKey,
      remaining: await buildHealthRemainingAccounts(
        user1MarginfiAccount.publicKey,
      ),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseAfterIx),
      [user.wallet],
      false,
      true,
    );

    const accountAfterBorrow =
      await bankrunProgram.account.marginfiAccount.fetch(
        user1MarginfiAccount.publicKey,
      );
    const healthAfter = accountAfterBorrow.healthCache;

    assert.ok((healthAfter.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((healthAfter.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((healthAfter.flags & HEALTH_CACHE_ORACLE_OK) !== 0);

    const netHealthAfter = wrappedI80F48toBigNumber(
      healthAfter.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthAfter.liabilityValue));
    const actualDecline = netHealthBefore.minus(netHealthAfter);
    assertBNGreaterThan(
      new BN(actualDecline.integerValue(BigNumber.ROUND_FLOOR).toFixed(0)),
      0,
    );

    const borrowUi = new BigNumber(TOKEN_B_BORROW_AMOUNT.toString()).div(
      new BigNumber(10).pow(ecosystem.tokenBDecimals),
    );
    const originationFeeRate = wrappedI80F48toBigNumber(
      borrowBank.config.interestRateConfig.protocolOriginationFee,
    );
    const liabilityWeight = wrappedI80F48toBigNumber(
      borrowBank.config.liabilityWeightInit,
    );
    const tokenBPriceHigh =
      ecosystem.tokenBPrice *
      (1 + ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE);

    const expectedDecline = borrowUi
      .multipliedBy(originationFeeRate.plus(1))
      .multipliedBy(liabilityWeight)
      .multipliedBy(tokenBPriceHigh);
    const declineTolerance = expectedDecline.multipliedBy(0.002);
    const declineDiff = actualDecline.minus(expectedDecline).abs();
    assert.ok(declineDiff.lte(declineTolerance));

    logHealthCache("jlr05 user 1 health after borrow", healthAfter);
  });

  /**
   * Before reweight:
   * - Collateral is Jup USDC (haircut by oracle confidence + bank weight), around ~$100
   * - Liability is TokenB debt, around ~$50
   * - Net health > 0
   */
  it("(user 0) partially liquidates user 1 after TokenB liability reweight - happy path", async () => {
    const reweightConfig = defaultBankConfigOptRaw();
    reweightConfig.liabilityWeightInit =
      bigNumberToWrappedI80F48(LIAB_WEIGHT_INDUCED);
    reweightConfig.liabilityWeightMaint =
      bigNumberToWrappedI80F48(LIAB_WEIGHT_INDUCED);

    const reweightIx = await configureBank(groupAdmin.mrgnBankrunProgram!, {
      bank: regTokenBBankPk,
      bankConfigOpt: reweightConfig,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(reweightIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const pulseBeforeLiquidationIx = await healthPulse(
      user.mrgnBankrunProgram!,
      {
        marginfiAccount: user1MarginfiAccount.publicKey,
        remaining: await buildHealthRemainingAccounts(
          user1MarginfiAccount.publicKey,
        ),
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseBeforeLiquidationIx),
      [user.wallet],
      false,
      true,
    );

    const liquidateeBefore = await bankrunProgram.account.marginfiAccount.fetch(
      user1MarginfiAccount.publicKey,
    );
    const healthBefore = liquidateeBefore.healthCache;
    logHealthCache(
      "jlr05 user 1 health before partial liquidation",
      healthBefore,
    );
    const netHealthBefore = wrappedI80F48toBigNumber(
      healthBefore.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthBefore.liabilityValue));
    const netHealthBeforeMaint = wrappedI80F48toBigNumber(
      healthBefore.assetValueMaint,
    ).minus(wrappedI80F48toBigNumber(healthBefore.liabilityValueMaint));
    // Unhealthy...
    assert.ok(netHealthBefore.lt(0));
    assert.ok(netHealthBeforeMaint.lt(0));

    const [assetBank, liabBank] = await Promise.all([
      bankrunProgram.account.bank.fetch(jupUsdcBankPk),
      bankrunProgram.account.bank.fetch(regTokenBBankPk),
    ]);

    const liquidatorRemaining = await buildHealthRemainingAccounts(
      user0MarginfiAccountPk,
    );
    const liquidateeRemaining = await buildHealthRemainingAccounts(
      user1MarginfiAccount.publicKey,
    );

    const liqIx = await liquidateIx(users[0].mrgnBankrunProgram!, {
      assetBankKey: jupUsdcBankPk,
      liabilityBankKey: regTokenBBankPk,
      liquidatorMarginfiAccount: user0MarginfiAccountPk,
      liquidateeMarginfiAccount: user1MarginfiAccount.publicKey,
      remaining: [
        assetBank.config.oracleKeys[0],
        assetBank.config.oracleKeys[1],
        liabBank.config.oracleKeys[0],
        ...liquidatorRemaining,
        ...liquidateeRemaining,
      ],
      amount: JUP_USDC_LIQUIDATION_AMOUNT,
      liquidateeAccounts: liquidateeRemaining.length,
      liquidatorAccounts: liquidatorRemaining.length,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 450_000 }),
        liqIx,
      ),
      [users[0].wallet],
      false,
      true,
    );

    const pulseAfterLiquidationIx = await healthPulse(
      user.mrgnBankrunProgram!,
      {
        marginfiAccount: user1MarginfiAccount.publicKey,
        remaining: await buildHealthRemainingAccounts(
          user1MarginfiAccount.publicKey,
        ),
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseAfterLiquidationIx),
      [user.wallet],
      false,
      true,
    );

    const liquidateeAfter = await bankrunProgram.account.marginfiAccount.fetch(
      user1MarginfiAccount.publicKey,
    );
    const healthAfter = liquidateeAfter.healthCache;
    const netHealthAfter = wrappedI80F48toBigNumber(
      healthAfter.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthAfter.liabilityValue));

    // Healthier!
    assert.ok(netHealthAfter.gt(netHealthBefore));
    // Still unheathly
    assert.ok(netHealthAfter.lt(0));

    logHealthCache(
      "jlr05 user 1 health after partial liquidation",
      healthAfter,
    );
  });
});
