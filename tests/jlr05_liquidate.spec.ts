import { BN } from "@coral-xyz/anchor";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
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
  assertKeyDefault,
  assertKeysEqual,
  assertBNEqual,
  assertBNGreaterThan,
  getTokenBalance,
} from "./utils/genericTests";
import {
  deriveLiquidityVaultAuthority,
  deriveLiquidationRecord,
} from "./utils/pdas";
import { deriveJuplendPoolKeys } from "./utils/juplend/juplend-pdas";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  liquidateIx,
  repayIx,
  startLiquidationIx,
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
  bytesToF64,
  buildHealthRemainingAccounts,
  createLookupTableForInstructions,
  logHealthCache,
  mintToTokenAccount,
  processBankrunTransaction,
  processBankrunV0Transaction,
  getBankrunBlockhash,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { advanceFiveMinutes, dummyIx } from "./utils/bankrunConnection";

const USER1_ACCOUNT_SEED = Buffer.from("JLR05_USER1_ACCOUNT_SEED_0000000");
const user1MarginfiAccount = Keypair.fromSeed(USER1_ACCOUNT_SEED);

const JUP_USDC_DEPOSIT_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
const TOKEN_B_BORROW_AMOUNT = new BN(2.5 * 10 ** ecosystem.tokenBDecimals); // 2.5 TOKEN_B (~$50 nominal)
const JUP_USDC_LIQUIDATION_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals); // 1 USDC
const RECEIVERSHIP_WITHDRAW_USDC = new BN(1 * 10 ** ecosystem.usdcDecimals); // 1 USDC
const RECEIVERSHIP_REPAY_TOKEN_B = new BN(
  0.05 * 10 ** ecosystem.tokenBDecimals,
); // 0.05 TOKEN_B ($1)
const LIAB_WEIGHT_INDUCED = 2;

describe("jlr05: Juplend collateral + mrgn borrow + health pulse (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let user = users[1];
  let groupPk = PublicKey.default;
  let jupUsdcBankPk = PublicKey.default;
  let regTokenBBankPk = PublicKey.default;
  let user0MarginfiAccountPk = PublicKey.default;

  before(async () => {
    juplendPrograms = getJuplendPrograms();
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
   * - Collateral is 100 Jup USDC, ~$100
   * - Liability is 2.5 TokenB debt, ~$50
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
      new Transaction().add(
        dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
        pulseBeforeLiquidationIx,
      ),
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
    const jupPool = deriveJuplendPoolKeys({ mint: assetBank.mint });

    // Force oracles to be stale so we must refresh within the tx.
    await advanceFiveMinutes(banksClient, bankrunContext);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

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
        await refreshJupSimple(juplendPrograms.lending, { pool: jupPool }),
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

  it("(user 0) liquidates user 1 with receivership - happy path", async () => {
    // Force oracles to be stale so we must refresh within the tx.
    await advanceFiveMinutes(banksClient, bankrunContext);

    const liquidator = users[0];
    const liquidateeAccountPk = user1MarginfiAccount.publicKey;
    const [liqRecordKey] = deriveLiquidationRecord(
      bankrunProgram.programId,
      liquidateeAccountPk,
    );

    await mintToTokenAccount(
      ecosystem.tokenBMint.publicKey,
      liquidator.tokenBAccount,
      RECEIVERSHIP_REPAY_TOKEN_B.mul(new BN(4)),
    );
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const pulseBeforeRxIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: liquidateeAccountPk,
      remaining: await buildHealthRemainingAccounts(liquidateeAccountPk),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
        pulseBeforeRxIx,
      ),
      [user.wallet],
      false,
      true,
    );

    const liquidateeBefore = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccountPk,
    );
    assertKeyDefault(liquidateeBefore.liquidationRecord);
    const healthBefore = liquidateeBefore.healthCache;
    const netHealthBefore = wrappedI80F48toBigNumber(
      healthBefore.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthBefore.liabilityValue));

    const initLiqRecordIx = await initLiquidationRecordIx(
      liquidator.mrgnBankrunProgram!,
      {
        marginfiAccount: liquidateeAccountPk,
        feePayer: liquidator.wallet.publicKey,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initLiqRecordIx),
      [liquidator.wallet],
      false,
      true,
    );

    const recordBefore = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey,
    );
    assertKeysEqual(recordBefore.key, liqRecordKey);
    assertKeysEqual(recordBefore.marginfiAccount, liquidateeAccountPk);
    assertKeysEqual(recordBefore.recordPayer, liquidator.wallet.publicKey);

    const [assetBank, liabBank] = await Promise.all([
      bankrunProgram.account.bank.fetch(jupUsdcBankPk),
      bankrunProgram.account.bank.fetch(regTokenBBankPk),
    ]);
    const jupPool = deriveJuplendPoolKeys({ mint: assetBank.mint });
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      jupUsdcBankPk,
    );
    const withdrawIntermediaryAta = assetBank.integrationAcc3;
    const expectedIntermediaryAta = getAssociatedTokenAddressSync(
      assetBank.mint,
      liquidityVaultAuthority,
      true,
      jupPool.tokenProgram,
    );
    assertKeysEqual(withdrawIntermediaryAta, expectedIntermediaryAta);

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        liquidator.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        assetBank.mint,
        jupPool.tokenProgram,
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [liquidator.wallet],
      false,
      true,
    );

    const assetGroup: PublicKey[] = [
      jupUsdcBankPk,
      assetBank.config.oracleKeys[0],
    ];
    if (!assetBank.config.oracleKeys[1].equals(PublicKey.default)) {
      assetGroup.push(assetBank.config.oracleKeys[1]);
    }
    const liabGroup: PublicKey[] = [
      regTokenBBankPk,
      liabBank.config.oracleKeys[0],
    ];
    if (!liabBank.config.oracleKeys[1].equals(PublicKey.default)) {
      liabGroup.push(liabBank.config.oracleKeys[1]);
    }
    const remainingGroups: PublicKey[][] = [assetGroup, liabGroup];
    const remaining = composeRemainingAccounts(remainingGroups);
    const remainingStart =
      composeRemainingAccountsWriteableMeta(remainingGroups);
    const remainingEnd = composeRemainingAccountsMetaBanksOnly(remainingGroups);

    const liquidatorUsdcBefore = await getTokenBalance(
      bankRunProvider,
      liquidator.usdcAccount,
    );
    const liquidatorTokenBBefore = await getTokenBalance(
      bankRunProvider,
      liquidator.tokenBAccount,
    );

    const rxLiquidationIxs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_300_000 }),
      await refreshJupSimple(juplendPrograms.lending, { pool: jupPool }),
      await startLiquidationIx(liquidator.mrgnBankrunProgram!, {
        marginfiAccount: liquidateeAccountPk,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: remainingStart,
      }),
      await makeJuplendWithdrawSimpleIx(liquidator.mrgnBankrunProgram!, {
        marginfiAccount: liquidateeAccountPk,
        destinationTokenAccount: liquidator.usdcAccount,
        bank: jupUsdcBankPk,
        pool: jupPool,
        amount: RECEIVERSHIP_WITHDRAW_USDC,
        remainingAccounts: remaining,
      }),
      await repayIx(liquidator.mrgnBankrunProgram!, {
        marginfiAccount: liquidateeAccountPk,
        bank: regTokenBBankPk,
        tokenAccount: liquidator.tokenBAccount,
        amount: RECEIVERSHIP_REPAY_TOKEN_B,
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram!, {
        marginfiAccount: liquidateeAccountPk,
        remaining: remainingEnd,
      }),
    ];

    const rxLutAccount = await createLookupTableForInstructions(
      liquidator.wallet,
      rxLiquidationIxs,
    );
    const blockhash = await getBankrunBlockhash(bankrunContext);
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: rxLiquidationIxs,
    }).compileToV0Message([rxLutAccount]);
    const rxLiquidationTx = new VersionedTransaction(messageV0);
    await processBankrunV0Transaction(
      bankrunContext,
      rxLiquidationTx,
      [liquidator.wallet],
      false,
      true,
    );

    const liquidatorUsdcAfter = await getTokenBalance(
      bankRunProvider,
      liquidator.usdcAccount,
    );
    const liquidatorTokenBAfter = await getTokenBalance(
      bankRunProvider,
      liquidator.tokenBAccount,
    );
    assertBNEqual(
      new BN(liquidatorUsdcAfter - liquidatorUsdcBefore),
      RECEIVERSHIP_WITHDRAW_USDC,
    );
    assertBNEqual(
      new BN(liquidatorTokenBBefore - liquidatorTokenBAfter),
      RECEIVERSHIP_REPAY_TOKEN_B,
    );

    const recordAfter = await bankrunProgram.account.liquidationRecord.fetch(
      liqRecordKey,
    );
    const rxEntry = recordAfter.entries[3];
    assert(rxEntry.timestamp.toNumber() > 0);

    const seizedUsd = bytesToF64(rxEntry.assetAmountSeized);
    const repaidUsd = bytesToF64(rxEntry.liabAmountRepaid);
    const confBps = ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    const expectedSeizedUsd =
      (RECEIVERSHIP_WITHDRAW_USDC.toNumber() / 10 ** ecosystem.usdcDecimals) *
      ecosystem.usdcPrice *
      (1 - confBps);
    const expectedRepaidUsd =
      (RECEIVERSHIP_REPAY_TOKEN_B.toNumber() / 10 ** ecosystem.tokenBDecimals) *
      ecosystem.tokenBPrice *
      (1 + confBps);

    assert.approximately(seizedUsd, expectedSeizedUsd, 0.001);
    assert.approximately(repaidUsd, expectedRepaidUsd, 0.001);

    const pulseAfterRxIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: liquidateeAccountPk,
      remaining: await buildHealthRemainingAccounts(liquidateeAccountPk),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseAfterRxIx),
      [user.wallet],
      false,
      true,
    );

    const liquidateeAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccountPk,
    );
    const healthAfter = liquidateeAfter.healthCache;
    const netHealthAfter = wrappedI80F48toBigNumber(
      healthAfter.assetValue,
    ).minus(wrappedI80F48toBigNumber(healthAfter.liabilityValue));
    assert.ok(netHealthAfter.gt(netHealthBefore));

    logHealthCache(
      "jlr05 user 1 health after receivership liquidation",
      healthAfter,
    );
  });
});
