import { BN } from "@coral-xyz/anchor";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  getMint,
} from "@solana/spl-token";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import { Clock } from "solana-bankrun";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  assertBNGreaterThan,
  assertI80F48Approx,
  assertI80F48Equal,
  getTokenBalance,
} from "./utils/genericTests";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { accountInit, pulseBankPrice } from "./utils/user-instructions";
import { dumpBankrunLogs, mintToTokenAccount, processBankrunTransaction } from "./utils/tools";
import {
  makeJuplendDepositIx,
  makeJuplendNativeBorrowIx,
  makeJuplendNativeLendingDepositIx,
  makeJuplendNativePreOperateIx,
} from "./utils/juplend/user-instructions";
import {
  deriveJuplendPoolKeys,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "./utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "./utils/juplend/programs";
import {
  DEFAULT_BORROW_CONFIG,
  type JuplendPoolKeys,
} from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  initJuplendProtocolPositionsIx,
  updateJuplendUserBorrowConfigIx,
  updateJuplendUserClassIx,
} from "./utils/juplend/admin-instructions";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

const USER0_ACCOUNT_SEED = Buffer.from("JLR02_USER0_ACCOUNT_SEED_0000000");
const user0MarginfiAccount = Keypair.fromSeed(USER0_ACCOUNT_SEED);
const USER_DEPOSIT_AMOUNT = new BN(50 * 10 ** ecosystem.usdcDecimals); // 50 USDC
const EXPECTED_SHARES_FOR_50_USDC = new BN(50 * 10 ** ecosystem.usdcDecimals);

const NATIVE_TOKEN_A_DEPOSIT_AMOUNT = new BN(
  25 * 10 ** ecosystem.tokenADecimals,
);
const NATIVE_USDC_BORROW_AMOUNT = new BN(45 * 10 ** ecosystem.usdcDecimals);
const ADMIN_USDC_BORROW_DEBT_CEILING = new BN(
  1_000 * 10 ** ecosystem.usdcDecimals,
);
const ONE_HOUR_IN_SECONDS = 60 * 60;
const EXCHANGE_PRICES_PRECISION = new BN("1000000000000");

describe("jlr02: JupLend deposits (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let user = users[0];
  let groupPk = PublicKey.default;
  let usdcJupBankPk = PublicKey.default;
  let usdcJupPool: JuplendPoolKeys;
  let tokenAPool: JuplendPoolKeys;
  let liquidityVaultPk = PublicKey.default;
  let fTokenVaultPk = PublicKey.default;
  let adminTokenAFTokenAta = PublicKey.default;

  let firstDepositMintedShares = new BN(0);
  let firstDepositTokenExchangePrice = new BN(0);
  let postInterestMintedShares = new BN(0);
  let postInterestTokenExchangePrice = new BN(0);
  let postInterestLiquidityExchangePrice = new BN(0);
  let firstDepositCacheMultiplier = 0;

  before(async () => {
    user = users[0];
    juplendPrograms = getJuplendPrograms();
    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);

    const usdcBankPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    const tokenABankPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr01BankTokenA,
    );

    const [usdcBank, tokenABank] = await Promise.all([
      bankrunProgram.account.bank.fetch(usdcBankPk),
      bankrunProgram.account.bank.fetch(tokenABankPk),
    ]);

    usdcJupBankPk = usdcBankPk;
    usdcJupPool = deriveJuplendPoolKeys({
      mint: usdcBank.mint,
    });
    tokenAPool = deriveJuplendPoolKeys({
      mint: tokenABank.mint,
    });
    liquidityVaultPk = usdcBank.liquidityVault;
    fTokenVaultPk = usdcBank.integrationAcc2;

    await mintToTokenAccount(
      usdcBank.mint,
      user.usdcAccount,
      USER_DEPOSIT_AMOUNT.mul(new BN(3)),
    );

    const initUserIx = await accountInit(user.mrgnBankrunProgram, {
      marginfiGroup: groupPk,
      marginfiAccount: user0MarginfiAccount.publicKey,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initUserIx),
      [user.wallet, user0MarginfiAccount],
      false,
      true,
    );

    juplendAccounts.set(
      JUPLEND_STATE_KEYS.jlr02User0MarginfiAccount,
      user0MarginfiAccount.publicKey,
    );
  });

  it("(user 0) deposit into JupLend USDC bank - happy path", async () => {
    const [
      userUsdcBefore,
      liquidityVaultBefore,
      fTokenVaultBefore,
      jupReserveVaultBefore,
      fTokenMintBefore,
      bankBefore,
      userAccountBefore,
      lendingBefore,
      tokenReserveBefore,
      supplyPositionBefore,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVaultPk),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      getMint(
        bankRunProvider.connection,
        usdcJupPool.fTokenMint,
        undefined,
        usdcJupPool.tokenProgram,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(
        user0MarginfiAccount.publicKey,
      ),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
    ]);

    const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: user0MarginfiAccount.publicKey,
      signerTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      pool: usdcJupPool,
      amount: USER_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [user.wallet],
      false,
      true,
    );

    const [
      userUsdcAfter,
      liquidityVaultAfter,
      fTokenVaultAfter,
      jupReserveVaultAfter,
      fTokenMintAfter,
      bankAfter,
      userAccountAfter,
      lendingAfter,
      tokenReserveAfter,
      supplyPositionAfter,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVaultPk),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      getMint(
        bankRunProvider.connection,
        usdcJupPool.fTokenMint,
        undefined,
        usdcJupPool.tokenProgram,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(
        user0MarginfiAccount.publicKey,
      ),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
    ]);

    assert.equal(
      userUsdcBefore - userUsdcAfter,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );

    const mintedShares = new BN(fTokenVaultAfter - fTokenVaultBefore);
    assertBNEqual(mintedShares, EXPECTED_SHARES_FOR_50_USDC);
    firstDepositMintedShares = mintedShares;
    firstDepositTokenExchangePrice = lendingAfter.tokenExchangePrice;

    assert.equal(liquidityVaultAfter, liquidityVaultBefore);
    assert.equal(
      jupReserveVaultAfter - jupReserveVaultBefore,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );
    assert.equal(
      fTokenMintAfter.supply - fTokenMintBefore.supply,
      BigInt(EXPECTED_SHARES_FOR_50_USDC.toString()),
    );

    const userBalanceAfter = userAccountAfter.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(usdcJupBankPk),
    );
    assert.ok(userBalanceAfter, "missing active bank balance for user");

    assertI80F48Equal(
      userBalanceAfter.assetShares,
      EXPECTED_SHARES_FOR_50_USDC,
    );
    assertI80F48Equal(bankBefore.totalAssetShares, 0);
    assertI80F48Equal(bankAfter.totalAssetShares, EXPECTED_SHARES_FOR_50_USDC);

    const bankSharesDelta = wrappedI80F48toBigNumber(
      bankAfter.totalAssetShares,
    ).minus(wrappedI80F48toBigNumber(bankBefore.totalAssetShares));
    assert.equal(
      bankSharesDelta.toFixed(0),
      EXPECTED_SHARES_FOR_50_USDC.toString(),
    );

    assertI80F48Equal(userBalanceAfter.liabilityShares, 0);
    assertI80F48Equal(userBalanceAfter.emissionsOutstanding, 0);
    assert.equal(
      bankAfter.lendingPositionCount,
      bankBefore.lendingPositionCount + 1,
    );

    assertBNEqual(
      tokenReserveAfter.totalSupplyWithInterest.sub(
        tokenReserveBefore.totalSupplyWithInterest,
      ),
      USER_DEPOSIT_AMOUNT,
    );
    assertBNEqual(
      supplyPositionAfter.amount.sub(supplyPositionBefore.amount),
      USER_DEPOSIT_AMOUNT,
    );
    assertBNEqual(
      lendingAfter.tokenExchangePrice,
      lendingBefore.tokenExchangePrice,
    );
    assertBNEqual(
      lendingAfter.liquidityExchangePrice,
      lendingBefore.liquidityExchangePrice,
    );
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const pulseInitialCacheIx = await pulseBankPrice(user.mrgnBankrunProgram!, {
      group: groupPk,
      bank: usdcJupBankPk,
      remaining: bankAfter.config.oracleKeys.filter(
        (key) => !key.equals(PublicKey.default),
      ),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseInitialCacheIx),
      [user.wallet],
      false,
      true,
    );
    const bankAfterPulse = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    firstDepositCacheMultiplier = Number(
      wrappedI80F48toBigNumber(bankAfterPulse.cache.priceMultiplier).toString(),
    );
    assertI80F48Approx(bankAfterPulse.cache.lastOraclePrice, oracles.usdcPrice, 0.000001);
    assertI80F48Approx(bankAfterPulse.cache.priceMultiplier, 1, 0.01);
  });

  it("(user 0) deposit 0 into JupLend USDC bank - should fail", async () => {
    const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: user0MarginfiAccount.publicKey,
      signerTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      pool: usdcJupPool,
      amount: new BN(0),
    });

    const result = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [user.wallet],
      true,
      true,
    );

    // Juplend's OperateAmountsNearlyZero.
    assertBankrunTxFailed(result, 6030);
  });

  it("(admin) deposits tokenA and borrows USDC from native jup to generate interest", async () => {
    const [adminUsdcSupplyPosition] = findJuplendLiquiditySupplyPositionPda(
      usdcJupPool.mint,
      groupAdmin.wallet.publicKey,
    );
    const [adminUsdcBorrowPosition] = findJuplendLiquidityBorrowPositionPda(
      usdcJupPool.mint,
      groupAdmin.wallet.publicKey,
    );

    adminTokenAFTokenAta = getAssociatedTokenAddressSync(
      tokenAPool.fTokenMint,
      groupAdmin.wallet.publicKey,
      false,
      tokenAPool.tokenProgram,
    );

    const createAdminTokenAFTokenAtaIx =
      createAssociatedTokenAccountInstruction(
        groupAdmin.wallet.publicKey,
        adminTokenAFTokenAta,
        groupAdmin.wallet.publicKey,
        tokenAPool.fTokenMint,
        tokenAPool.tokenProgram,
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createAdminTokenAFTokenAtaIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const adminBorrowConfig = {
      ...DEFAULT_BORROW_CONFIG,
      baseDebtCeiling: ADMIN_USDC_BORROW_DEBT_CEILING,
      maxDebtCeiling: ADMIN_USDC_BORROW_DEBT_CEILING,
    };

    const [initProtocolIx, updateUserClassIx, updateBorrowConfigIx] =
      await Promise.all([
        initJuplendProtocolPositionsIx(juplendPrograms, {
          authority: groupAdmin.wallet.publicKey,
          authList: usdcJupPool.authList,
          supplyMint: usdcJupPool.mint,
          borrowMint: usdcJupPool.mint,
          protocol: groupAdmin.wallet.publicKey,
        }),
        updateJuplendUserClassIx(juplendPrograms, {
          authority: groupAdmin.wallet.publicKey,
          authList: usdcJupPool.authList,
          entries: [{ addr: groupAdmin.wallet.publicKey, value: 1 }],
        }),
        updateJuplendUserBorrowConfigIx(juplendPrograms, {
          authority: groupAdmin.wallet.publicKey,
          protocol: groupAdmin.wallet.publicKey,
          authList: usdcJupPool.authList,
          rateModel: usdcJupPool.rateModel,
          mint: usdcJupPool.mint,
          tokenReserve: usdcJupPool.tokenReserve,
          userBorrowPosition: adminUsdcBorrowPosition,
          config: adminBorrowConfig,
        }),
      ]);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        initProtocolIx,
        updateUserClassIx,
        updateBorrowConfigIx,
      ),
      [groupAdmin.wallet],
      false,
      true,
    );

    await mintToTokenAccount(
      ecosystem.tokenAMint.publicKey,
      groupAdmin.tokenAAccount,
      NATIVE_TOKEN_A_DEPOSIT_AMOUNT,
    );

    const [
      adminTokenABefore,
      adminTokenAFTokenBefore,
      adminUsdcBefore,
      usdcReserveBefore,
      adminBorrowPosBefore,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, groupAdmin.tokenAAccount),
      getTokenBalance(bankRunProvider, adminTokenAFTokenAta),
      getTokenBalance(bankRunProvider, groupAdmin.usdcAccount),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userBorrowPosition.fetch(
        adminUsdcBorrowPosition,
      ),
    ]);

    const tokenADepositIx = await makeJuplendNativeLendingDepositIx(
      juplendPrograms.lending,
      {
        signer: groupAdmin.wallet.publicKey,
        depositorTokenAccount: groupAdmin.tokenAAccount,
        recipientTokenAccount: adminTokenAFTokenAta,
        pool: tokenAPool,
        assets: NATIVE_TOKEN_A_DEPOSIT_AMOUNT,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(tokenADepositIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const [preOperateIx, borrowIx] = await Promise.all([
      makeJuplendNativePreOperateIx(juplendPrograms.liquidity, {
        protocol: groupAdmin.wallet.publicKey,
        mint: usdcJupPool.mint,
        pool: usdcJupPool,
        userSupplyPosition: adminUsdcSupplyPosition,
        userBorrowPosition: adminUsdcBorrowPosition,
      }),
      makeJuplendNativeBorrowIx(juplendPrograms.liquidity, {
        protocol: groupAdmin.wallet.publicKey,
        pool: usdcJupPool,
        userSupplyPosition: adminUsdcSupplyPosition,
        userBorrowPosition: adminUsdcBorrowPosition,
        borrowTo: groupAdmin.wallet.publicKey,
        borrowAmount: NATIVE_USDC_BORROW_AMOUNT,
      }),
    ]);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(preOperateIx, borrowIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const [
      adminTokenAAfter,
      adminTokenAFTokenAfter,
      adminUsdcAfter,
      usdcReserveAfter,
      adminBorrowPosAfter,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, groupAdmin.tokenAAccount),
      getTokenBalance(bankRunProvider, adminTokenAFTokenAta),
      getTokenBalance(bankRunProvider, groupAdmin.usdcAccount),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userBorrowPosition.fetch(
        adminUsdcBorrowPosition,
      ),
    ]);

    assert.equal(
      adminTokenABefore - adminTokenAAfter,
      NATIVE_TOKEN_A_DEPOSIT_AMOUNT.toNumber(),
    );
    assert.equal(
      adminTokenAFTokenAfter - adminTokenAFTokenBefore,
      NATIVE_TOKEN_A_DEPOSIT_AMOUNT.toNumber(),
    );
    assert.equal(
      adminUsdcAfter - adminUsdcBefore,
      NATIVE_USDC_BORROW_AMOUNT.toNumber(),
    );
    assertBNEqual(
      usdcReserveAfter.totalBorrowWithInterest.sub(
        usdcReserveBefore.totalBorrowWithInterest,
      ),
      NATIVE_USDC_BORROW_AMOUNT,
    );
    assertBNEqual(
      adminBorrowPosAfter.amount.sub(adminBorrowPosBefore.amount),
      NATIVE_USDC_BORROW_AMOUNT,
    );
    assert.isAbove(
      usdcReserveAfter.lastUtilization,
      usdcReserveBefore.lastUtilization,
    );
    assert.isAbove(usdcReserveAfter.borrowRate, usdcReserveBefore.borrowRate);
  });

  it("One hour elapses", async () => {
    const clockBefore = await banksClient.getClock();
    bankrunContext.setClock(
      new Clock(
        clockBefore.slot,
        clockBefore.epochStartTimestamp,
        clockBefore.epoch,
        clockBefore.leaderScheduleEpoch,
        clockBefore.unixTimestamp + BigInt(ONE_HOUR_IN_SECONDS),
      ),
    );

    const clockAfter = await banksClient.getClock();
    assert.equal(
      clockAfter.unixTimestamp.toString(),
      (clockBefore.unixTimestamp + BigInt(ONE_HOUR_IN_SECONDS)).toString(),
    );
  });

  let nativeSupplyPositionRawDelta = new BN(0);
  // We deposit natively here to measure post-interest share issuance directly from JupLend.
  it("(user 0) deposits into native JupLend USDC bank after interest - happy path", async () => {
    const userUsdcFTokenAta = getAssociatedTokenAddressSync(
      usdcJupPool.fTokenMint,
      user.wallet.publicKey,
      false,
      usdcJupPool.tokenProgram,
    );

    const createUserUsdcFTokenAtaIx = createAssociatedTokenAccountInstruction(
      user.wallet.publicKey,
      userUsdcFTokenAta,
      user.wallet.publicKey,
      usdcJupPool.fTokenMint,
      usdcJupPool.tokenProgram,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createUserUsdcFTokenAtaIx),
      [user.wallet],
      false,
      true,
    );

    const [
      userUsdcBefore,
      userUsdcFTokenBefore,
      jupReserveVaultBefore,
      lendingBefore,
      tokenReserveBefore,
      supplyPositionBefore,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, userUsdcFTokenAta),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
    ]);

    const nativeDepositIx = await makeJuplendNativeLendingDepositIx(
      juplendPrograms.lending,
      {
        signer: user.wallet.publicKey,
        depositorTokenAccount: user.usdcAccount,
        recipientTokenAccount: userUsdcFTokenAta,
        pool: usdcJupPool,
        assets: USER_DEPOSIT_AMOUNT,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(nativeDepositIx),
      [user.wallet],
      false,
      true,
    );

    const [
      userUsdcAfter,
      userUsdcFTokenAfter,
      jupReserveVaultAfter,
      lendingAfter,
      tokenReserveAfter,
      supplyPositionAfter,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, userUsdcFTokenAta),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
    ]);

    const mintedSharesAfterInterest = new BN(
      userUsdcFTokenAfter - userUsdcFTokenBefore,
    );
    postInterestMintedShares = mintedSharesAfterInterest;
    postInterestTokenExchangePrice = lendingAfter.tokenExchangePrice;
    postInterestLiquidityExchangePrice = lendingAfter.liquidityExchangePrice;

    assert.equal(
      userUsdcBefore - userUsdcAfter,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );
    assert.isTrue(
      mintedSharesAfterInterest.lt(firstDepositMintedShares),
      `before=${firstDepositMintedShares.toString()} after=${mintedSharesAfterInterest.toString()}`,
    );

    assertBNGreaterThan(
      lendingAfter.tokenExchangePrice,
      firstDepositTokenExchangePrice,
    );
    assertBNGreaterThan(
      lendingAfter.tokenExchangePrice,
      lendingBefore.tokenExchangePrice,
    );
    assertBNGreaterThan(
      lendingAfter.liquidityExchangePrice,
      lendingBefore.liquidityExchangePrice,
    );

    assert.equal(
      jupReserveVaultAfter - jupReserveVaultBefore,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );

    const reserveRawSupplyDelta = tokenReserveAfter.totalSupplyWithInterest.sub(
      tokenReserveBefore.totalSupplyWithInterest,
    );
    const supplyPositionRawDelta = supplyPositionAfter.amount.sub(
      supplyPositionBefore.amount,
    );
    nativeSupplyPositionRawDelta = supplyPositionRawDelta;
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assert.isTrue(
      reserveRawSupplyDelta.lt(USER_DEPOSIT_AMOUNT),
      `raw=${reserveRawSupplyDelta.toString()} assets=${USER_DEPOSIT_AMOUNT.toString()}`,
    );

    assertBNEqual(
      tokenReserveAfter.totalBorrowWithInterest,
      tokenReserveBefore.totalBorrowWithInterest,
    );
  });

  it("(user 0) deposits into (mrgn) Juplend USDC bank after interest - happy path", async () => {
    const [
      userUsdcBefore,
      fTokenVaultBefore,
      jupReserveVaultBefore,
      lendingBefore,
      tokenReserveBefore,
      supplyPositionBefore,
      bankBefore,
      userAccountBefore,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(
        user0MarginfiAccount.publicKey,
      ),
    ]);

    assertBNEqual(
      lendingBefore.tokenExchangePrice,
      postInterestTokenExchangePrice,
    );
    assertBNEqual(
      lendingBefore.liquidityExchangePrice,
      postInterestLiquidityExchangePrice,
    );

    const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: user0MarginfiAccount.publicKey,
      signerTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      pool: usdcJupPool,
      amount: USER_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [user.wallet],
      false,
      true,
    );

    const [
      userUsdcAfter,
      fTokenVaultAfter,
      jupReserveVaultAfter,
      lendingAfter,
      tokenReserveAfter,
      supplyPositionAfter,
      bankAfter,
      userAccountAfter,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.supplyPositionOnLiquidity,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(
        user0MarginfiAccount.publicKey,
      ),
    ]);

    const userBalanceBefore = userAccountBefore.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(usdcJupBankPk),
    );
    const userBalanceAfter = userAccountAfter.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(usdcJupBankPk),
    );

    assert.ok(userBalanceBefore, "missing user balance before mrgn deposit");
    assert.ok(userBalanceAfter, "missing user balance after mrgn deposit");

    assert.equal(
      userUsdcBefore - userUsdcAfter,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );
    const mintedShares = new BN(fTokenVaultAfter - fTokenVaultBefore);
    assertBNEqual(mintedShares, postInterestMintedShares);
    assertBNGreaterThan(mintedShares, 0);

    assertBNEqual(
      lendingAfter.tokenExchangePrice,
      lendingBefore.tokenExchangePrice,
    );
    assertBNEqual(
      lendingAfter.liquidityExchangePrice,
      lendingBefore.liquidityExchangePrice,
    );
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const pulsePostInterestCacheIx = await pulseBankPrice(user.mrgnBankrunProgram!, {
      group: groupPk,
      bank: usdcJupBankPk,
      remaining: bankAfter.config.oracleKeys.filter(
        (key) => !key.equals(PublicKey.default),
      ),
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulsePostInterestCacheIx),
      [user.wallet],
      false,
      true,
    );
    const bankAfterPulse = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    const expectedCacheMultiplier =
      Number(lendingAfter.tokenExchangePrice.toString()) /
      Number(EXCHANGE_PRICES_PRECISION.toString());
    const observedCacheMultiplier = Number(
      wrappedI80F48toBigNumber(bankAfterPulse.cache.priceMultiplier).toString(),
    );
    assertI80F48Approx(
      bankAfterPulse.cache.priceMultiplier,
      expectedCacheMultiplier,
      expectedCacheMultiplier / 1000, // .01%
    );
    assertI80F48Approx(bankAfterPulse.cache.lastOraclePrice, oracles.usdcPrice, 0.000001);
    assert.isAtLeast(observedCacheMultiplier, firstDepositCacheMultiplier);
    assert.equal(
      jupReserveVaultAfter - jupReserveVaultBefore,
      USER_DEPOSIT_AMOUNT.toNumber(),
    );

    const reserveRawSupplyDelta = tokenReserveAfter.totalSupplyWithInterest.sub(
      tokenReserveBefore.totalSupplyWithInterest,
    );
    const supplyPositionRawDelta = supplyPositionAfter.amount.sub(
      supplyPositionBefore.amount,
    );
    // Here we confirm that a $50 into jup natively is equivalent to a $50 through p0
    assertBNEqual(nativeSupplyPositionRawDelta, supplyPositionRawDelta);
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assert.isTrue(
      reserveRawSupplyDelta.lt(USER_DEPOSIT_AMOUNT),
      `raw=${reserveRawSupplyDelta.toString()} assets=${USER_DEPOSIT_AMOUNT.toString()}`,
    );
    assertBNEqual(
      tokenReserveAfter.totalBorrowWithInterest,
      tokenReserveBefore.totalBorrowWithInterest,
    );

    const userAssetShareDelta = wrappedI80F48toBigNumber(
      userBalanceAfter.assetShares,
    ).minus(wrappedI80F48toBigNumber(userBalanceBefore.assetShares));
    assert.equal(userAssetShareDelta.toFixed(0), mintedShares.toString());

    const bankTotalAssetSharesDelta = wrappedI80F48toBigNumber(
      bankAfter.totalAssetShares,
    ).minus(wrappedI80F48toBigNumber(bankBefore.totalAssetShares));
    assert.equal(bankTotalAssetSharesDelta.toFixed(0), mintedShares.toString());

    assertI80F48Equal(bankAfter.assetShareValue, bankBefore.assetShareValue);
    assertI80F48Equal(
      bankAfter.liabilityShareValue,
      bankBefore.liabilityShareValue,
    );
  });
});
