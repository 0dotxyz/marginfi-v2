import { BN } from "@coral-xyz/anchor";
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
import { assert } from "chai";
import {
  BanksTransactionMeta,
  BanksTransactionResultWithMeta,
} from "solana-bankrun";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  juplendAccounts,
  users,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertBNEqual,
  assertBNGreaterThan,
  assertBankrunTxFailed,
  assertI80F48Equal,
  getTokenBalance,
  i80ToBn,
} from "./utils/genericTests";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";
import { deriveJuplendPoolKeys } from "./utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "./utils/juplend/programs";
import type { JuplendPoolKeys } from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import { makeJuplendWithdrawSimpleIx } from "./utils/juplend/shorthand-instructions";
import { accountInit } from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  createLookupTableForInstructions,
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
  processBankrunV0Transaction,
} from "./utils/tools";
import { advanceOneHour, dummyIx } from "./utils/bankrunConnection";
import { setFixedPrice } from "./utils/group-instructions";

const EXCHANGE_PRICES_PRECISION = new BN("1000000000000");
const USER1_ACCOUNT_SEED = Buffer.from("JLR04_USER1_ACCOUNT_SEED_0000000");
const user1MarginfiAccount = Keypair.fromSeed(USER1_ACCOUNT_SEED);

const usdc = (ui: number) => new BN(ui * 10 ** ecosystem.usdcDecimals);

const CLEAN_DEPOSIT_AMOUNT = usdc(50);
const INTEREST_DEPOSIT_AMOUNT = usdc(50);
const PARTIALS_WITH_INTEREST_DEPOSIT_AMOUNT = usdc(60);
const PARTIALS_WITH_INTEREST_WITHDRAW_AMOUNT = usdc(10);
const FULL_WITHOUT_FLAG_DEPOSIT_AMOUNT = usdc(40);
const PARTIALS_THEN_ALL_DEPOSIT_AMOUNT = usdc(70);
const PARTIALS_THEN_ALL_WITHDRAW_AMOUNT = usdc(10);

type Snapshot = {
  userUsdc: BN;
  withdrawIntermediaryAta: BN;
  fTokenVault: BN;
  jupReserveVault: BN;
  lendingTokenExchangePrice: BN;
  lendingLiquidityExchangePrice: BN;
  tokenReserveSupplyRaw: BN;
  tokenReserveBorrowRaw: BN;
  supplyPositionRaw: BN;
  bankTotalAssetShares: BN;
  bankAssetShareValue: any;
  bankLiabilityShareValue: any;
  userAssetShares: BN;
  hasActiveBalance: boolean;
};

describe("jlr04: JupLend withdraws (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let user = users[1];
  let activeMarginfiAccountPk = PublicKey.default;
  let user0MarginfiAccountPk = PublicKey.default;

  let groupPk = PublicKey.default;
  let usdcJupBankPk = PublicKey.default;
  let usdcJupPool: JuplendPoolKeys;
  let liquidityVaultAuthorityPk = PublicKey.default;
  let fTokenVaultPk = PublicKey.default;
  let withdrawIntermediaryAtaPk = PublicKey.default;

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    user = users[1];
    activeMarginfiAccountPk = user1MarginfiAccount.publicKey;
    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group);
    usdcJupBankPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    user0MarginfiAccountPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr02User0MarginfiAccount,
    );

    const bank = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    usdcJupPool = deriveJuplendPoolKeys({ mint: bank.mint });
    fTokenVaultPk = bank.integrationAcc2;
    withdrawIntermediaryAtaPk = bank.integrationAcc3;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      usdcJupBankPk,
    );
    liquidityVaultAuthorityPk = liquidityVaultAuthority;

    const expectedWithdrawIntermediaryAta = getAssociatedTokenAddressSync(
      bank.mint,
      liquidityVaultAuthorityPk,
      true,
      usdcJupPool.tokenProgram,
    );
    assert.equal(
      withdrawIntermediaryAtaPk.toBase58(),
      expectedWithdrawIntermediaryAta.toBase58(),
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      user.usdcAccount,
      usdc(500),
    );

    const initUserIx = await accountInit(user.mrgnBankrunProgram!, {
      marginfiGroup: groupPk,
      marginfiAccount: user1MarginfiAccount.publicKey,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initUserIx),
      [user.wallet, user1MarginfiAccount],
      false,
      true,
    );

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        withdrawIntermediaryAtaPk,
        liquidityVaultAuthorityPk,
        usdcJupPool.mint,
        usdcJupPool.tokenProgram,
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [user.wallet],
      false,
      true,
    );
  });

  const setActiveUser = (
    nextUser: (typeof users)[number],
    marginfiAccount: PublicKey,
  ) => {
    user = nextUser;
    activeMarginfiAccountPk = marginfiAccount;
  };

  const previewSharesForDeposit = (
    assets: BN,
    liquidityExchangePrice: BN,
    tokenExchangePrice: BN,
  ): BN => {
    if (liquidityExchangePrice.isZero() || tokenExchangePrice.isZero()) {
      return undefined;
    }
    const registeredRaw = assets
      .mul(EXCHANGE_PRICES_PRECISION)
      .div(liquidityExchangePrice);
    const registered = registeredRaw
      .mul(liquidityExchangePrice)
      .div(EXCHANGE_PRICES_PRECISION);
    return registered.mul(EXCHANGE_PRICES_PRECISION).div(tokenExchangePrice);
  };

  const previewSharesForWithdraw = (assets: BN, tokenExchangePrice: BN): BN => {
    if (tokenExchangePrice.isZero()) {
      return undefined;
    }
    return assets
      .mul(EXCHANGE_PRICES_PRECISION)
      .add(tokenExchangePrice.sub(new BN(1)))
      .div(tokenExchangePrice);
  };

  const previewAssetsForRedeem = (shares: BN, tokenExchangePrice: BN): BN => {
    if (tokenExchangePrice.isZero()) {
      return undefined;
    }
    return shares.mul(tokenExchangePrice).div(EXCHANGE_PRICES_PRECISION);
  };

  const getActiveUserAssetShares = (
    marginfiAccount: any,
    bankPk: PublicKey,
  ): { shares: BN; active: boolean } => {
    const userBalance = marginfiAccount.lendingAccount.balances.find(
      (b: any) => b.active && b.bankPk.equals(bankPk),
    );
    if (!userBalance) {
      return { shares: new BN(0), active: false };
    }
    return { shares: i80ToBn(userBalance.assetShares), active: true };
  };

  const fetchSnapshot = async (): Promise<Snapshot> => {
    const [
      userUsdc,
      withdrawIntermediaryAta,
      fTokenVault,
      jupReserveVault,
      lending,
      tokenReserve,
      supplyPosition,
      bank,
      userAccount,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, withdrawIntermediaryAtaPk),
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
      bankrunProgram.account.marginfiAccount.fetch(activeMarginfiAccountPk),
    ]);

    const { shares, active } = getActiveUserAssetShares(
      userAccount,
      usdcJupBankPk,
    );

    return {
      userUsdc: new BN(userUsdc),
      withdrawIntermediaryAta: new BN(withdrawIntermediaryAta),
      fTokenVault: new BN(fTokenVault),
      jupReserveVault: new BN(jupReserveVault),
      lendingTokenExchangePrice: lending.tokenExchangePrice,
      lendingLiquidityExchangePrice: lending.liquidityExchangePrice,
      tokenReserveSupplyRaw: tokenReserve.totalSupplyWithInterest,
      tokenReserveBorrowRaw: tokenReserve.totalBorrowWithInterest,
      supplyPositionRaw: supplyPosition.amount,
      bankTotalAssetShares: i80ToBn(bank.totalAssetShares),
      bankAssetShareValue: bank.assetShareValue,
      bankLiabilityShareValue: bank.liabilityShareValue,
      userAssetShares: shares,
      hasActiveBalance: active,
    };
  };

  const executeDeposit = async (
    amount: BN,
  ): Promise<{ before: Snapshot; after: Snapshot }> => {
    const before = await fetchSnapshot();
    const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
      marginfiAccount: activeMarginfiAccountPk,
      signerTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      pool: usdcJupPool,
      amount,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [user.wallet],
      false,
      true,
    );
    const after = await fetchSnapshot();
    return { before, after };
  };

  const executeWithdraw = async (
    amount: BN,
    withdrawAll: boolean,
    trySend: boolean = false,
    withDummyIx: boolean = false,
    useLut: boolean = false,
  ): Promise<BanksTransactionResultWithMeta | BanksTransactionMeta> => {
    const remainingAccounts = await buildHealthRemainingAccounts(
      activeMarginfiAccountPk,
    );

    const ix = await makeJuplendWithdrawSimpleIx(user.mrgnBankrunProgram!, {
      marginfiAccount: activeMarginfiAccountPk,
      destinationTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      pool: usdcJupPool,
      amount,
      withdrawAll,
      remainingAccounts,
    });

    const tx = new Transaction();
    if (useLut) {
      tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }));
    }
    tx.add(ix);
    if (withDummyIx) {
      tx.add(dummyIx(user.wallet.publicKey, users[0].wallet.publicKey));
    }

    if (useLut) {
      const lutAccount = await createLookupTableForInstructions(
        user.wallet,
        tx.instructions,
      );
      const blockhash = await getBankrunBlockhash(bankrunContext);
      const messageV0 = new TransactionMessage({
        payerKey: user.wallet.publicKey,
        recentBlockhash: blockhash,
        instructions: tx.instructions,
      }).compileToV0Message([lutAccount]);
      const vtx = new VersionedTransaction(messageV0);
      return processBankrunV0Transaction(
        bankrunContext,
        vtx,
        [user.wallet],
        trySend,
        true,
      );
    }

    return processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      trySend,
      false,
    );
  };

  const assertDepositDeltas = (
    before: Snapshot,
    after: Snapshot,
    amount: BN,
  ) => {
    const mintedShares = after.fTokenVault.sub(before.fTokenVault);
    const expectedMintedShares = previewSharesForDeposit(
      amount,
      before.lendingLiquidityExchangePrice,
      before.lendingTokenExchangePrice,
    );
    assertBNApproximately(mintedShares, expectedMintedShares!, 1);
    assertBNGreaterThan(mintedShares, 0);

    assertBNEqual(before.userUsdc.sub(after.userUsdc), amount);
    assertBNEqual(after.jupReserveVault.sub(before.jupReserveVault), amount);
    assertBNEqual(
      after.withdrawIntermediaryAta,
      before.withdrawIntermediaryAta,
    );

    const reserveRawSupplyDelta = after.tokenReserveSupplyRaw.sub(
      before.tokenReserveSupplyRaw,
    );
    const supplyPositionRawDelta = after.supplyPositionRaw.sub(
      before.supplyPositionRaw,
    );
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assertBNApproximately(supplyPositionRawDelta, mintedShares, 1);
    assertBNEqual(after.tokenReserveBorrowRaw, before.tokenReserveBorrowRaw);

    assertBNEqual(
      after.userAssetShares.sub(before.userAssetShares),
      mintedShares,
    );
    assertBNEqual(
      after.bankTotalAssetShares.sub(before.bankTotalAssetShares),
      mintedShares,
    );
    assertI80F48Equal(after.bankAssetShareValue, before.bankAssetShareValue);
    assertI80F48Equal(
      after.bankLiabilityShareValue,
      before.bankLiabilityShareValue,
    );
  };

  const assertWithdrawDeltas = (
    before: Snapshot,
    after: Snapshot,
    amount: BN,
  ) => {
    const burnedShares = before.fTokenVault.sub(after.fTokenVault);
    const expectedBurnedShares = previewSharesForWithdraw(
      amount,
      after.lendingTokenExchangePrice,
    );
    assertBNEqual(burnedShares, expectedBurnedShares!);
    assertBNGreaterThan(burnedShares, 0);

    assertBNEqual(after.userUsdc.sub(before.userUsdc), amount);
    assertBNEqual(before.jupReserveVault.sub(after.jupReserveVault), amount);
    assertBNEqual(
      after.withdrawIntermediaryAta,
      before.withdrawIntermediaryAta,
    );

    const reserveRawSupplyDelta = before.tokenReserveSupplyRaw.sub(
      after.tokenReserveSupplyRaw,
    );
    const supplyPositionRawDelta = before.supplyPositionRaw.sub(
      after.supplyPositionRaw,
    );
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assertBNApproximately(supplyPositionRawDelta, burnedShares, 1);
    assertBNEqual(after.tokenReserveBorrowRaw, before.tokenReserveBorrowRaw);

    const userShareDelta = before.userAssetShares.sub(after.userAssetShares);
    const bankShareDelta = before.bankTotalAssetShares.sub(
      after.bankTotalAssetShares,
    );
    assertBNEqual(userShareDelta, burnedShares);
    assertBNEqual(bankShareDelta, burnedShares);
    assertBNEqual(burnedShares, userShareDelta);

    assertI80F48Equal(after.bankAssetShareValue, before.bankAssetShareValue);
    assertI80F48Equal(
      after.bankLiabilityShareValue,
      before.bankLiabilityShareValue,
    );
  };

  const assertWithdrawAllDeltas = (before: Snapshot, after: Snapshot) => {
    const burnedShares = before.fTokenVault.sub(after.fTokenVault);
    const expectedTokenAmount = previewAssetsForRedeem(
      before.userAssetShares,
      after.lendingTokenExchangePrice,
    );
    const expectedBurnedShares = previewSharesForWithdraw(
      expectedTokenAmount!,
      after.lendingTokenExchangePrice,
    );
    assertBNEqual(burnedShares, expectedBurnedShares!);

    assertBNEqual(after.userUsdc.sub(before.userUsdc), expectedTokenAmount!);
    assertBNEqual(
      before.jupReserveVault.sub(after.jupReserveVault),
      expectedTokenAmount!,
    );
    assertBNEqual(
      after.withdrawIntermediaryAta,
      before.withdrawIntermediaryAta,
    );

    const reserveRawSupplyDelta = before.tokenReserveSupplyRaw.sub(
      after.tokenReserveSupplyRaw,
    );
    const supplyPositionRawDelta = before.supplyPositionRaw.sub(
      after.supplyPositionRaw,
    );
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assertBNApproximately(supplyPositionRawDelta, burnedShares, 1);
    assertBNEqual(after.tokenReserveBorrowRaw, before.tokenReserveBorrowRaw);

    const userShareDelta = before.userAssetShares.sub(after.userAssetShares);
    const bankShareDelta = before.bankTotalAssetShares.sub(
      after.bankTotalAssetShares,
    );
    assertBNEqual(after.userAssetShares, 0);
    assert.equal(after.hasActiveBalance, false);
    assertBNEqual(userShareDelta, before.userAssetShares);
    assertBNEqual(bankShareDelta, before.userAssetShares);
    assertBNEqual(burnedShares, userShareDelta);

    assertI80F48Equal(after.bankAssetShareValue, before.bankAssetShareValue);
    assertI80F48Equal(
      after.bankLiabilityShareValue,
      before.bankLiabilityShareValue,
    );
  };

  it("(user 0) withdraw from JupLend USDC bank - happy path", async () => {
    setActiveUser(users[0], user0MarginfiAccountPk);

    const withdrawAmount = usdc(10);
    const before = await fetchSnapshot();
    await executeWithdraw(withdrawAmount, false, false, false, true);
    const after = await fetchSnapshot();

    assertWithdrawDeltas(before, after, withdrawAmount);
    assertBNGreaterThan(after.userAssetShares, 0);
    assert.equal(after.hasActiveBalance, true);
  });

  it("(user 1) clean deposit + withdraw_all - happy path", async () => {
    setActiveUser(users[1], user1MarginfiAccount.publicKey);
    const deposited = await executeDeposit(CLEAN_DEPOSIT_AMOUNT);
    assertDepositDeltas(
      deposited.before,
      deposited.after,
      CLEAN_DEPOSIT_AMOUNT,
    );

    const beforeWithdrawAll = await fetchSnapshot();
    await executeWithdraw(new BN(0), true);
    const afterWithdrawAll = await fetchSnapshot();
    assertWithdrawAllDeltas(beforeWithdrawAll, afterWithdrawAll);
  });

  it("(user 1) deposit + withdraw_all after one hour of interest", async () => {
    setActiveUser(users[1], user1MarginfiAccount.publicKey);
    const deposited = await executeDeposit(INTEREST_DEPOSIT_AMOUNT);
    assertDepositDeltas(
      deposited.before,
      deposited.after,
      INTEREST_DEPOSIT_AMOUNT,
    );

    await advanceOneHour(banksClient, bankrunContext);

    const beforeWithdrawAll = await fetchSnapshot();
    await executeWithdraw(new BN(0), true);
    const afterWithdrawAll = await fetchSnapshot();
    assertWithdrawAllDeltas(beforeWithdrawAll, afterWithdrawAll);

    assertBNGreaterThan(
      afterWithdrawAll.lendingTokenExchangePrice,
      beforeWithdrawAll.lendingTokenExchangePrice,
    );
  });

  it("(user 1) deposit + equal partial withdraws with interest between", async () => {
    setActiveUser(users[1], user1MarginfiAccount.publicKey);
    const deposited = await executeDeposit(
      PARTIALS_WITH_INTEREST_DEPOSIT_AMOUNT,
    );
    assertDepositDeltas(
      deposited.before,
      deposited.after,
      PARTIALS_WITH_INTEREST_DEPOSIT_AMOUNT,
    );

    for (let i = 0; i < 3; i++) {
    await advanceOneHour(banksClient, bankrunContext);

      const beforeWithdraw = await fetchSnapshot();
      await executeWithdraw(
        PARTIALS_WITH_INTEREST_WITHDRAW_AMOUNT,
        false,
        false,
        true,
      );
      const afterWithdraw = await fetchSnapshot();
      assertWithdrawDeltas(
        beforeWithdraw,
        afterWithdraw,
        PARTIALS_WITH_INTEREST_WITHDRAW_AMOUNT,
      );
      assertBNGreaterThan(
        afterWithdraw.lendingTokenExchangePrice,
        beforeWithdraw.lendingTokenExchangePrice,
      );
    }

    const endState = await fetchSnapshot();
    assertBNGreaterThan(endState.userAssetShares, 0);
    assert.equal(endState.hasActiveBalance, true);
  });

  it("(user 1) withdraw full redeemable amount without withdraw_all, then withdraw_all no-op", async () => {
    setActiveUser(users[1], user1MarginfiAccount.publicKey);
    const deposited = await executeDeposit(FULL_WITHOUT_FLAG_DEPOSIT_AMOUNT);
    assertDepositDeltas(
      deposited.before,
      deposited.after,
      FULL_WITHOUT_FLAG_DEPOSIT_AMOUNT,
    );

    const beforeFullWithdraw = await fetchSnapshot();
    const fullRedeemAmount = previewAssetsForRedeem(
      beforeFullWithdraw.userAssetShares,
      beforeFullWithdraw.lendingTokenExchangePrice,
    );
    assertBNGreaterThan(fullRedeemAmount!, 0);

    await executeWithdraw(fullRedeemAmount, false);
    const afterFullWithdraw = await fetchSnapshot();
    assertWithdrawDeltas(
      beforeFullWithdraw,
      afterFullWithdraw,
      fullRedeemAmount!,
    );

    const closeAttempt = await executeWithdraw(new BN(0), true, true);
    // NoAssetFound
    assertBankrunTxFailed(closeAttempt, 6023);
  });

  it("(user 1) deposit + equal value partial withdraws ending with withdraw_all", async () => {
    setActiveUser(users[1], user1MarginfiAccount.publicKey);
    const deposited = await executeDeposit(PARTIALS_THEN_ALL_DEPOSIT_AMOUNT);
    assertDepositDeltas(
      deposited.before,
      deposited.after,
      PARTIALS_THEN_ALL_DEPOSIT_AMOUNT,
    );

    for (let i = 0; i < 3; i++) {
      const beforeWithdraw = await fetchSnapshot();
      await executeWithdraw(
        PARTIALS_THEN_ALL_WITHDRAW_AMOUNT,
        false,
        false,
        true,
      );
      const afterWithdraw = await fetchSnapshot();
      assertWithdrawDeltas(
        beforeWithdraw,
        afterWithdraw,
        PARTIALS_THEN_ALL_WITHDRAW_AMOUNT,
      );
    }

    const beforeWithdrawAll = await fetchSnapshot();
    // Note: amount doesn't matter when withdrawing all
    await executeWithdraw(new BN(0), true);
    const afterWithdrawAll = await fetchSnapshot();
    assertWithdrawAllDeltas(beforeWithdrawAll, afterWithdrawAll);
  });
});
