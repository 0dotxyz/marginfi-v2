import { BN, IdlAccounts } from "@coral-xyz/anchor";
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
import { createAssociatedTokenAccountIdempotentInstruction } from "@solana/spl-token";
import { assert } from "chai";
import { Marginfi } from "../target/types/marginfi";

import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import { configureBank } from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  depositIx,
  healthPulse,
  liquidateIx,
  repayIx,
} from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  bytesToF64,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { refreshSwitchboardPullOracleBankrun } from "./utils/bankrun-oracles";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/group-instructions";
import { initJuplendClaimAccountIx } from "./utils/juplend/admin-instructions";
import {
  deriveJuplendMrgnAddresses,
  deriveJuplendPoolKeys,
} from "./utils/juplend/juplend-pdas";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import { defaultJuplendBankConfig } from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  CONF_INTERVAL_MULTIPLE,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_CONF_INTERVAL,
  blankBankConfigOptRaw,
} from "./utils/types";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { EXCHANGE_PRICES_PRECISION } from "./utils/juplend/constants";
import { dummyIx } from "./utils/bankrunConnection";

const SWITCHBOARD_BANK_SEED = new BN(80_008);
const BORROWER_ACCOUNT_SEED = Buffer.from("JLR08_BORROWER_ACCOUNT_SEED_0000");
const LIQUIDATOR_ACCOUNT_SEED = Buffer.from("JLR08_LIQUIDATOR_ACCOUNT_SEED_00");

const borrowerMarginfiAccount = Keypair.fromSeed(BORROWER_ACCOUNT_SEED);
const liquidatorMarginfiAccount = Keypair.fromSeed(LIQUIDATOR_ACCOUNT_SEED);
const REWEIGHTED_LIAB_WEIGHT = 3;

const usdc = (ui: number) =>
  new BN(Math.round(ui * 10 ** ecosystem.usdcDecimals));
const tokenB = (ui: number) =>
  new BN(Math.round(ui * 10 ** ecosystem.tokenBDecimals));

const USER_DEPOSIT_USDC = usdc(2);
const USER_WITHDRAW_USDC = usdc(0.2);
const USER_BORROW_TOKEN_B = tokenB(5.2);
const USER_REPAY_TOKEN_B = tokenB(1);
const LIQUIDATOR_DEPOSIT_TOKEN_B = tokenB(5);
const LIQUIDATION_USDC = usdc(0.01);

type MarginfiAccount = IdlAccounts<Marginfi>["marginfiAccount"];
type MarginfiBalance = MarginfiAccount["lendingAccount"]["balances"][number];
type MarginfiHealthCache = MarginfiAccount["healthCache"];

describe("jlr08: Switchboard JupLend flow (bankrun)", () => {
  let borrower: (typeof users)[number];
  let liquidator: (typeof users)[number];

  let groupPk = PublicKey.default;
  let regularTokenBBankPk = PublicKey.default;
  let switchboardJupBankPk = PublicKey.default;
  let jupUsdcPool = deriveJuplendPoolKeys({
    mint: ecosystem.usdcMint.publicKey,
  });
  let preReweightAssetValue = 0;
  let preReweightLiabilityValue = 0;
  let preReweightNetValue = 0;

  /**
   * Refresh swb and pyth oracles
   */
  const refreshAllOracles = async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshSwitchboardPullOracleBankrun(
      bankrunContext,
      banksClient,
      oracles.wsolOracleSwb.publicKey,
    );
  };

  const pulseHealth = async (
    user: (typeof users)[number],
    marginfiAccountPk: PublicKey,
  ) => {
    await refreshAllOracles();
    const remaining = await buildHealthRemainingAccounts(marginfiAccountPk);
    const pulseIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: marginfiAccountPk,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, {
          pool: jupUsdcPool,
        }),
        pulseIx,
        dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [user.wallet],
      false,
      true,
    );

    return bankrunProgram.account.marginfiAccount.fetch(marginfiAccountPk);
  };

  const netHealth = (healthCache: MarginfiHealthCache) =>
    wrappedI80F48toBigNumber(healthCache.assetValue).minus(
      wrappedI80F48toBigNumber(healthCache.liabilityValue),
    );

  const uiAmount = (native: BN, decimals: number): number =>
    Number(native.toString()) / 10 ** decimals;

  const confAdjLiability = 1 + ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;

  const priceForBank = (
    marginfiAccount: MarginfiAccount,
    bankPk: PublicKey,
    errLabel: string,
  ): number => {
    const idx = marginfiAccount.lendingAccount.balances.findIndex(
      (b: MarginfiBalance) => b.active && b.bankPk.equals(bankPk),
    );
    assert.isAtLeast(idx, 0, `${errLabel}: missing active bank balance`);
    return bytesToF64(marginfiAccount.healthCache.prices[idx]);
  };

  before(async () => {
    borrower = users[2];
    liquidator = users[3];

    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);
    regularTokenBBankPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
    );

    const switchboardAddresses = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: groupPk,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: SWITCHBOARD_BANK_SEED,
    });
    switchboardJupBankPk = switchboardAddresses.bank;

    const switchboardConfig = defaultJuplendBankConfig(
      oracles.wsolOracleSwb.publicKey,
      ecosystem.usdcDecimals,
    );
    switchboardConfig.oracleSetup = { juplendSwitchboardPull: {} };
    switchboardConfig.oracleMaxAge = 300;

    const addBankIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram!, {
      group: groupPk,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: SWITCHBOARD_BANK_SEED,
      oracle: oracles.wsolOracleSwb.publicKey,
      jupLendingState: jupUsdcPool.lending,
      fTokenMint: jupUsdcPool.fTokenMint,
      config: switchboardConfig,
      tokenProgram: jupUsdcPool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const initPositionIx = await makeJuplendInitPositionIx(
      groupAdmin.mrgnBankrunProgram!,
      {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.usdcAccount,
        bank: switchboardJupBankPk,
        pool: jupUsdcPool,
        seedDepositAmount: usdc(1),
      },
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPositionIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        switchboardAddresses.withdrawIntermediaryAta,
        switchboardAddresses.liquidityVaultAuthority,
        jupUsdcPool.mint,
        jupUsdcPool.tokenProgram,
      );
    const initClaimIx = await initJuplendClaimAccountIx(getJuplendPrograms(), {
      signer: groupAdmin.wallet.publicKey,
      mint: jupUsdcPool.mint,
      accountFor: switchboardAddresses.liquidityVaultAuthority,
      claimAccount: switchboardAddresses.claimAccount,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx, initClaimIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    juplendAccounts.set(
      JUPLEND_STATE_KEYS.jlr08BankUsdcSwitchboard,
      switchboardJupBankPk,
    );

    const [initBorrowerIx, initLiquidatorIx] = await Promise.all([
      accountInit(borrower.mrgnBankrunProgram!, {
        marginfiGroup: groupPk,
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        authority: borrower.wallet.publicKey,
        feePayer: borrower.wallet.publicKey,
      }),
      accountInit(liquidator.mrgnBankrunProgram!, {
        marginfiGroup: groupPk,
        marginfiAccount: liquidatorMarginfiAccount.publicKey,
        authority: liquidator.wallet.publicKey,
        feePayer: liquidator.wallet.publicKey,
      }),
    ]);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initBorrowerIx),
      [borrower.wallet, borrowerMarginfiAccount],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initLiquidatorIx),
      [liquidator.wallet, liquidatorMarginfiAccount],
      false,
      true,
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      borrower.usdcAccount,
      USER_DEPOSIT_USDC.mul(new BN(2)),
    );
    await mintToTokenAccount(
      ecosystem.tokenBMint.publicKey,
      liquidator.tokenBAccount,
      LIQUIDATOR_DEPOSIT_TOKEN_B.mul(new BN(2)),
    );
  });

  it("(borrower) deposits into switchboard JupLend bank and pulse reports expected price/value", async () => {
    await refreshAllOracles();

    const depositIx = await makeJuplendDepositIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      signerTokenAccount: borrower.usdcAccount,
      bank: switchboardJupBankPk,
      pool: jupUsdcPool,
      amount: USER_DEPOSIT_USDC,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [borrower.wallet],
      false,
      true,
    );

    const accountAfterPulse = await pulseHealth(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const healthCache = accountAfterPulse.healthCache;
    assert.ok((healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);
    const balanceIdx = accountAfterPulse.lendingAccount.balances.findIndex(
      (b: MarginfiBalance) => b.active && b.bankPk.equals(switchboardJupBankPk),
    );
    assert.isAtLeast(balanceIdx, 0, "missing active switchboard jup balance");

    const pulsePrice = bytesToF64(healthCache.prices[balanceIdx]);
    const [lending, switchboardBank] = await Promise.all([
      getJuplendPrograms().lending.account.lending.fetch(jupUsdcPool.lending),
      bankrunProgram.account.bank.fetch(switchboardJupBankPk),
    ]);
    const exchange =
      Number(lending.tokenExchangePrice.toString()) / EXCHANGE_PRICES_PRECISION;
    const expectedSwitchboardPrice = oracles.wsolPriceSwb * exchange;
    const assetWeight = wrappedI80F48toBigNumber(
      switchboardBank.config.assetWeightInit,
    ).toNumber();

    assert.approximately(
      pulsePrice,
      expectedSwitchboardPrice,
      expectedSwitchboardPrice * 0.005,
    );

    const expectedAssetValue =
      uiAmount(USER_DEPOSIT_USDC, ecosystem.usdcDecimals) *
      expectedSwitchboardPrice *
      assetWeight;
    const actualAssetValue = wrappedI80F48toBigNumber(
      healthCache.assetValue,
    ).toNumber();
    assert.approximately(
      actualAssetValue,
      expectedAssetValue,
      expectedAssetValue * 0.005,
    );
  });

  it("(liquidator) deposits TokenB to prepare for liquidation", async () => {
    const depositTokenBIx = await depositIx(liquidator.mrgnBankrunProgram!, {
      marginfiAccount: liquidatorMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: liquidator.tokenBAccount,
      amount: LIQUIDATOR_DEPOSIT_TOKEN_B,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositTokenBIx),
      [liquidator.wallet],
      false,
      true,
    );
  });

  it("(borrower) withdraws, borrows TokenB, repays partially, and remains healthy pre-reweight", async () => {
    await refreshAllOracles();

    const withdrawRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );
    const withdrawIx = await makeJuplendWithdrawSimpleIx(
      borrower.mrgnBankrunProgram!,
      {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        destinationTokenAccount: borrower.usdcAccount,
        bank: switchboardJupBankPk,
        pool: jupUsdcPool,
        amount: USER_WITHDRAW_USDC,
        remainingAccounts: withdrawRemaining,
      },
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        withdrawIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const borrowRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
      {
        includedBankPks: [regularTokenBBankPk],
      },
    );
    const borrowTokenBIx = await borrowIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: borrower.tokenBAccount,
      remaining: borrowRemaining,
      amount: USER_BORROW_TOKEN_B,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, {
          pool: jupUsdcPool,
        }),
        borrowTokenBIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const repayRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );
    const repayTokenBIx = await repayIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: borrower.tokenBAccount,
      amount: USER_REPAY_TOKEN_B,
      remaining: repayRemaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, {
          pool: jupUsdcPool,
        }),
        repayTokenBIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const accountAfterPulse = await pulseHealth(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const net = netHealth(accountAfterPulse.healthCache);
    const switchboardPrice = priceForBank(
      accountAfterPulse,
      switchboardJupBankPk,
      "pre-reweight switchboard price",
    );
    const tokenBPrice = priceForBank(
      accountAfterPulse,
      regularTokenBBankPk,
      "pre-reweight tokenB price",
    );

    const [switchboardBank, tokenBBank, lending] = await Promise.all([
      bankrunProgram.account.bank.fetch(switchboardJupBankPk),
      bankrunProgram.account.bank.fetch(regularTokenBBankPk),
      getJuplendPrograms().lending.account.lending.fetch(jupUsdcPool.lending),
    ]);

    const expectedSwitchboardPrice =
      oracles.wsolPriceSwb *
      (Number(lending.tokenExchangePrice.toString()) /
        EXCHANGE_PRICES_PRECISION);
    const expectedTokenBPrice = ecosystem.tokenBPrice * confAdjLiability;

    assert.approximately(
      switchboardPrice,
      expectedSwitchboardPrice,
      expectedSwitchboardPrice * 0.005,
    );
    assert.approximately(
      tokenBPrice,
      expectedTokenBPrice,
      expectedTokenBPrice * 0.002,
    );

    const assetWeight = wrappedI80F48toBigNumber(
      switchboardBank.config.assetWeightInit,
    ).toNumber();
    const liabilityWeight = wrappedI80F48toBigNumber(
      tokenBBank.config.liabilityWeightInit,
    ).toNumber();
    const originationFee = wrappedI80F48toBigNumber(
      tokenBBank.config.interestRateConfig.protocolOriginationFee,
    ).toNumber();

    const expectedAssetValue =
      (uiAmount(USER_DEPOSIT_USDC, ecosystem.usdcDecimals) -
        uiAmount(USER_WITHDRAW_USDC, ecosystem.usdcDecimals)) *
      switchboardPrice *
      assetWeight;
    const expectedLiabilityValue =
      (uiAmount(USER_BORROW_TOKEN_B, ecosystem.tokenBDecimals) *
        (1 + originationFee) -
        uiAmount(USER_REPAY_TOKEN_B, ecosystem.tokenBDecimals)) *
      tokenBPrice *
      liabilityWeight;

    const actualAssetValue = wrappedI80F48toBigNumber(
      accountAfterPulse.healthCache.assetValue,
    ).toNumber();
    const actualLiabilityValue = wrappedI80F48toBigNumber(
      accountAfterPulse.healthCache.liabilityValue,
    ).toNumber();

    assert.approximately(
      actualAssetValue,
      expectedAssetValue,
      expectedAssetValue * 0.002,
    );
    assert.approximately(
      actualLiabilityValue,
      expectedLiabilityValue,
      expectedLiabilityValue * 0.002,
    );
    assert.approximately(
      net.toNumber(),
      expectedAssetValue - expectedLiabilityValue,
      Math.abs(expectedAssetValue - expectedLiabilityValue) * 0.003,
    );
    assert.ok(net.gt(0), "borrower should still be healthy before reweight");

    preReweightAssetValue = actualAssetValue;
    preReweightLiabilityValue = actualLiabilityValue;
    preReweightNetValue = net.toNumber();
  });

  it("(liquidator) liquidates borrower after TokenB liability reweight", async () => {
    const liabBankBefore = await bankrunProgram.account.bank.fetch(
      regularTokenBBankPk,
    );

    const reweightConfig = blankBankConfigOptRaw();
    reweightConfig.liabilityWeightInit = bigNumberToWrappedI80F48(
      REWEIGHTED_LIAB_WEIGHT,
    );
    reweightConfig.liabilityWeightMaint = bigNumberToWrappedI80F48(
      REWEIGHTED_LIAB_WEIGHT,
    );

    const reweightIx = await configureBank(groupAdmin.mrgnBankrunProgram!, {
      bank: regularTokenBBankPk,
      bankConfigOpt: reweightConfig,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(reweightIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const accountBeforeLiq = await pulseHealth(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const netBeforeLiq = netHealth(accountBeforeLiq.healthCache);
    const assetBeforeLiqValue = wrappedI80F48toBigNumber(
      accountBeforeLiq.healthCache.assetValue,
    ).toNumber();
    const liabilityBeforeLiqValue = wrappedI80F48toBigNumber(
      accountBeforeLiq.healthCache.liabilityValue,
    ).toNumber();
    const liabWeightBefore = wrappedI80F48toBigNumber(
      liabBankBefore.config.liabilityWeightInit,
    ).toNumber();
    const expectedLiabilityAfterReweight =
      preReweightLiabilityValue * (REWEIGHTED_LIAB_WEIGHT / liabWeightBefore);

    assert.approximately(
      assetBeforeLiqValue,
      preReweightAssetValue,
      preReweightAssetValue * 0.002,
    );
    assert.approximately(
      liabilityBeforeLiqValue,
      expectedLiabilityAfterReweight,
      expectedLiabilityAfterReweight * 0.003,
    );
    assert.approximately(
      netBeforeLiq.toNumber(),
      preReweightAssetValue - expectedLiabilityAfterReweight,
      Math.abs(preReweightAssetValue - expectedLiabilityAfterReweight) * 0.004,
    );
    assert.ok(
      preReweightNetValue > 0,
      "borrower must be healthy before reweight-induced liquidation",
    );
    assert.ok(
      netBeforeLiq.lt(0),
      "borrower should be unhealthy after reweight",
    );

    const [assetBank, liabBank] = await Promise.all([
      bankrunProgram.account.bank.fetch(switchboardJupBankPk),
      bankrunProgram.account.bank.fetch(regularTokenBBankPk),
    ]);

    const liquidatorRemaining = await buildHealthRemainingAccounts(
      liquidatorMarginfiAccount.publicKey,
      {
        includedBankPks: [switchboardJupBankPk],
      },
    );
    const liquidateeRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );

    const liqOracleAccounts: PublicKey[] = [
      assetBank.config.oracleKeys[0],
      assetBank.config.oracleKeys[1],
      liabBank.config.oracleKeys[0],
    ];

    const liqIx = await liquidateIx(liquidator.mrgnBankrunProgram!, {
      assetBankKey: switchboardJupBankPk,
      liabilityBankKey: regularTokenBBankPk,
      liquidatorMarginfiAccount: liquidatorMarginfiAccount.publicKey,
      liquidateeMarginfiAccount: borrowerMarginfiAccount.publicKey,
      remaining: [
        ...liqOracleAccounts,
        ...liquidatorRemaining,
        ...liquidateeRemaining,
      ],
      amount: LIQUIDATION_USDC,
      liquidateeAccounts: liquidateeRemaining.length,
      liquidatorAccounts: liquidatorRemaining.length,
    });

    await refreshAllOracles();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 }),
        await refreshJupSimple(getJuplendPrograms().lending, {
          pool: jupUsdcPool,
        }),
        liqIx,
        dummyIx(liquidator.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [liquidator.wallet],
      false,
      true,
    );

    const accountAfterLiq = await pulseHealth(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const assetAfterLiqValue = wrappedI80F48toBigNumber(
      accountAfterLiq.healthCache.assetValue,
    ).toNumber();
    const liabilityAfterLiqValue = wrappedI80F48toBigNumber(
      accountAfterLiq.healthCache.liabilityValue,
    ).toNumber();
    const netAfterLiq = netHealth(accountAfterLiq.healthCache);
    assert.ok(
      assetAfterLiqValue < assetBeforeLiqValue,
      "liquidation should seize some collateral from borrower",
    );
    assert.ok(
      liabilityAfterLiqValue < liabilityBeforeLiqValue,
      "liquidation should repay some borrower liability",
    );
    assert.ok(
      netAfterLiq.gt(netBeforeLiq),
      "liquidation should improve net health",
    );

    const restoreConfig = blankBankConfigOptRaw();
    restoreConfig.liabilityWeightInit =
      liabBankBefore.config.liabilityWeightInit;
    restoreConfig.liabilityWeightMaint =
      liabBankBefore.config.liabilityWeightMaint;
    const restoreIx = await configureBank(groupAdmin.mrgnBankrunProgram!, {
      bank: regularTokenBBankPk,
      bankConfigOpt: restoreConfig,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(restoreIx),
      [groupAdmin.wallet],
      false,
      true,
    );
  });
});
