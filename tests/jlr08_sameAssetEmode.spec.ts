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
  bankrunContext,
  banksClient,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  riskAdmin,
  users,
} from "./rootHooks";
import {
  addBankWithSeed,
  configureBankOracle,
  configureBank,
  groupConfigure,
} from "./utils/group-instructions";
import {
  composeRemainingAccounts,
  accountInit,
  borrowIx,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endDeleverageIx,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  repayIx,
  startDeleverageIx,
  startLiquidationIx,
} from "./utils/user-instructions";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  mintToTokenAccount,
  processBankrunTransaction,
  processBankrunV0Transaction,
} from "./utils/tools";
import { assertBankrunTxFailed } from "./utils/genericTests";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import { deriveJuplendPoolKeys } from "./utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { refreshJupSimple } from "./utils/juplend/shorthand-instructions";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import { makeJuplendWithdrawSimpleIx } from "./utils/juplend/shorthand-instructions";
import { type JuplendPoolKeys } from "./utils/juplend/types";
import {
  bigNumberToWrappedI80F48,
  WrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  computeSameAssetBoundaryBorrowNative,
  computeSameValueBorrowNative,
} from "./utils/same-asset-emode";

const USER_ACCOUNT_SA_JLR = "same_asset_juplend_account";
const REGULAR_TOKEN_A_SEED = new BN(80_001);
const RECEIVERSHIP_JUP_WITHDRAW = new BN(500_000);
const SAME_ASSET_DEPOSIT = new BN(100 * 10 ** ecosystem.tokenADecimals);
const SAME_ASSET_INIT_LEVERAGE = 101;
const SAME_ASSET_MAINT_LEVERAGE = 102;
const SAME_ASSET_TIGHTENED_INIT_LEVERAGE = 99;
const SAME_ASSET_TIGHTENED_MAINT_LEVERAGE = 100;
const EXCHANGE_PRICES_PRECISION = new BN("1000000000000");
const SAME_ASSET_BORROW_ORIGINATION_FEE_RATE = 0.01;

type TestUser = (typeof users)[number];

const getNetHealth = (cache: {
  assetValue: WrappedI80F48;
  liabilityValue: WrappedI80F48;
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
}) => {
  const init = wrappedI80F48toBigNumber(cache.assetValue).minus(
    wrappedI80F48toBigNumber(cache.liabilityValue)
  );
  const maint = wrappedI80F48toBigNumber(cache.assetValueMaint).minus(
    wrappedI80F48toBigNumber(cache.liabilityValueMaint)
  );
  return { init, maint };
};

const computeJupLendSameAssetBorrow = (accountedUnderlyingNative: BN) =>
  computeSameAssetBoundaryBorrowNative({
    collateralNative: accountedUnderlyingNative,
    collateralDecimals: ecosystem.tokenADecimals,
    collateralPrice: ecosystem.tokenAPrice,
    liabilityDecimals: ecosystem.tokenADecimals,
    liabilityPrice: ecosystem.tokenAPrice,
    healthyInitLeverage: SAME_ASSET_INIT_LEVERAGE,
    tightenedRequirementLeverage: SAME_ASSET_TIGHTENED_MAINT_LEVERAGE,
    liabilityOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });

const computeSameValueUsdcBorrow = (sameAssetBorrowNative: BN) =>
  computeSameValueBorrowNative({
    sourceBorrowNative: sameAssetBorrowNative,
    sourceDecimals: ecosystem.tokenADecimals,
    sourcePrice: ecosystem.tokenAPrice,
    targetDecimals: ecosystem.usdcDecimals,
    targetPrice: ecosystem.usdcPrice,
    sourceOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
    targetOriginationFeeRate: SAME_ASSET_BORROW_ORIGINATION_FEE_RATE,
  });

describe("jlr08: JupLend same-asset emode", () => {
  let groupPk: PublicKey;
  let juplendTokenABank: PublicKey;
  let regularTokenABank: PublicKey;
  let regularUsdcBank: PublicKey;
  let tokenAPool: JuplendPoolKeys;
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let withdrawIntermediaryAta: PublicKey;

  const getSameAssetRemainingGroups = () =>
    [
      [juplendTokenABank, oracles.tokenAOracle.publicKey, tokenAPool.lending],
      [regularTokenABank, oracles.tokenAOracle.publicKey],
    ] as PublicKey[][];
  const getSameAssetWithUsdcRemainingGroups = () =>
    [
      [juplendTokenABank, oracles.tokenAOracle.publicKey, tokenAPool.lending],
      [regularUsdcBank, oracles.usdcOracle.publicKey],
    ] as PublicKey[][];

  const expectedSharesForDeposit = (
    assets: BN,
    liquidityExchangePrice: BN,
    tokenExchangePrice: BN
  ) => {
    const registeredAmountRaw = assets
      .mul(EXCHANGE_PRICES_PRECISION)
      .div(liquidityExchangePrice);
    const registeredAmount = registeredAmountRaw
      .mul(liquidityExchangePrice)
      .div(EXCHANGE_PRICES_PRECISION);
    return registeredAmount
      .mul(EXCHANGE_PRICES_PRECISION)
      .div(tokenExchangePrice);
  };

  const expectedAssetsForRedeem = (shares: BN, tokenExchangePrice: BN) =>
    shares.mul(tokenExchangePrice).div(EXCHANGE_PRICES_PRECISION);

  const getJupLendAccountedCollateralNative = async (
    marginfiAccount: PublicKey
  ) => {
    let account = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount
    );
    const accountedShares = new BN(
      wrappedI80F48toBigNumber(
        account.lendingAccount.balances[0].assetShares
      ).toString()
    );
    const lending = await juplendPrograms.lending.account.lending.fetch(
      tokenAPool.lending
    );
    const accountedUnderlying = expectedAssetsForRedeem(
      accountedShares,
      new BN(lending.tokenExchangePrice.toString())
    );

    return { accountedShares, accountedUnderlying };
  };

  const initFreshAccount = async (user: TestUser) => {
    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: groupPk,
        marginfiAccount: accountKeypair.publicKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
      accountKeypair,
    ]);
    return accountKeypair.publicKey;
  };

  const configureSameAssetLeverage = async (
    initLeverage: number,
    maintLeverage: number,
    options?: {
      newRiskAdmin?: PublicKey;
    }
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupConfigure(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: groupPk,
          newRiskAdmin: options?.newRiskAdmin,
          sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(initLeverage),
          sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(maintLeverage),
        })
      ),
      [groupAdmin.wallet]
    );
  };

  const resetSameAssetLeverage = async (options?: {
    newRiskAdmin?: PublicKey;
  }) =>
    configureSameAssetLeverage(
      SAME_ASSET_INIT_LEVERAGE,
      SAME_ASSET_MAINT_LEVERAGE,
      options
    );

  const tightenSameAssetLeverage = async (options?: {
    newRiskAdmin?: PublicKey;
  }) =>
    configureSameAssetLeverage(
      SAME_ASSET_TIGHTENED_INIT_LEVERAGE,
      SAME_ASSET_TIGHTENED_MAINT_LEVERAGE,
      options
    );

  const refreshJupLendPoolIx = () =>
    refreshJupSimple(juplendPrograms.lending, { pool: tokenAPool });

  const depositJuplendCollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount = SAME_ASSET_DEPOSIT
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupLendPoolIx(),
        await makeJuplendDepositIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          signerTokenAccount: user.tokenAAccount,
          bank: juplendTokenABank,
          pool: tokenAPool,
          amount,
        })
      ),
      [user.wallet]
    );
  };

  const borrowFromRegularTokenA = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount: BN,
    remainingGroups = getSameAssetRemainingGroups()
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupLendPoolIx(),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: regularTokenABank,
          tokenAccount: user.tokenAAccount,
          remaining: composeRemainingAccounts(remainingGroups),
          amount,
        })
      ),
      [user.wallet]
    );
  };

  const pulseJupLendSameAssetHealth = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    remainingGroups: PublicKey[][]
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupLendPoolIx(),
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount,
          remaining: composeRemainingAccounts(remainingGroups),
        })
      ),
      [user.wallet]
    );

    return bankrunProgram.account.marginfiAccount.fetch(marginfiAccount);
  };

  const depositRegularTokenACollateral = async (
    user: TestUser,
    marginfiAccount: PublicKey,
    amount: BN
  ) => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: regularTokenABank,
          tokenAccount: user.tokenAAccount,
          amount,
          depositUpToLimit: false,
        })
      ),
      [user.wallet]
    );
  };

  const setupSameAssetScenario = async () => {
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      juplendTokenABank
    );
    const expectedWithdrawIntermediaryAta = getAssociatedTokenAddressSync(
      ecosystem.tokenAMint.publicKey,
      liquidityVaultAuthority,
      true,
      tokenAPool.tokenProgram
    );
    assert.equal(
      withdrawIntermediaryAta.toBase58(),
      expectedWithdrawIntermediaryAta.toBase58()
    );

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        ecosystem.tokenAMint.publicKey,
        tokenAPool.tokenProgram
      );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [groupAdmin.wallet]
    );

    const regularTokenAAddTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: groupPk,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        config: defaultBankConfig(),
        seed: REGULAR_TOKEN_A_SEED,
      })
    );
    await processBankrunTransaction(bankrunContext, regularTokenAAddTx, [
      groupAdmin.wallet,
    ]);

    const regularTokenAOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: regularTokenABank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.tokenAOracle.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, regularTokenAOracleTx, [
      groupAdmin.wallet,
    ]);

    await resetSameAssetLeverage();

    const discounted = blankBankConfigOptRaw();
    discounted.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    discounted.assetWeightMaint = bigNumberToWrappedI80F48(0.5);

    const juplendTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: juplendTokenABank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, juplendTx, [
      groupAdmin.wallet,
    ]);

    const regularTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: regularTokenABank,
        bankConfigOpt: discounted,
      })
    );
    await processBankrunTransaction(bankrunContext, regularTx, [
      groupAdmin.wallet,
    ]);

    for (const user of users) {
      const accountKeypair = Keypair.generate();
      user.accounts.set(USER_ACCOUNT_SA_JLR, accountKeypair.publicKey);

      const tx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: groupPk,
          marginfiAccount: accountKeypair.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      );
      await processBankrunTransaction(bankrunContext, tx, [
        user.wallet,
        accountKeypair,
      ]);
    }

    for (const user of users) {
      await mintToTokenAccount(
        ecosystem.tokenAMint.publicKey,
        user.tokenAAccount,
        new BN(2_000 * 10 ** ecosystem.tokenADecimals)
      );
      await mintToTokenAccount(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        new BN(2_000 * 10 ** ecosystem.usdcDecimals)
      );
    }

    const seedUser = users[2];
    const seedMarginfiAccount = seedUser.accounts.get(USER_ACCOUNT_SA_JLR)!;
    const tx = new Transaction()
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularTokenABank,
          tokenAccount: seedUser.tokenAAccount,
          amount: new BN(200 * 10 ** ecosystem.tokenADecimals),
          depositUpToLimit: false,
        })
      )
      .add(
        await depositIx(seedUser.mrgnBankrunProgram, {
          marginfiAccount: seedMarginfiAccount,
          bank: regularUsdcBank,
          tokenAccount: seedUser.usdcAccount,
          amount: new BN(2_000 * 10 ** ecosystem.usdcDecimals),
          depositUpToLimit: false,
        })
      );
    await processBankrunTransaction(bankrunContext, tx, [seedUser.wallet]);
  };

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    groupPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01Group)!;
    juplendTokenABank = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr01BankTokenA
    )!;
    regularUsdcBank = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr01RegularBankUsdc
    )!;

    const existingTokenABankState = await bankrunProgram.account.bank.fetch(
      juplendTokenABank
    );
    tokenAPool = deriveJuplendPoolKeys({ mint: existingTokenABankState.mint });
    withdrawIntermediaryAta = existingTokenABankState.integrationAcc3;

    [regularTokenABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      groupPk,
      ecosystem.tokenAMint.publicKey,
      REGULAR_TOKEN_A_SEED
    );
    await setupSameAssetScenario();
  });

  it("(user 0) JupLend Token A collateral is healthy only because same-asset emode lifts the weight", async () => {
    const user = users[0];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    const bank = await bankrunProgram.account.bank.fetch(juplendTokenABank);
    const lendingBefore = await juplendPrograms.lending.account.lending.fetch(
      tokenAPool.lending
    );
    const liquidityExchangePrice = new BN(
      lendingBefore.liquidityExchangePrice.toString()
    );
    const tokenExchangePrice = new BN(
      lendingBefore.tokenExchangePrice.toString()
    );
    const expectedShares = expectedSharesForDeposit(
      SAME_ASSET_DEPOSIT,
      liquidityExchangePrice,
      tokenExchangePrice
    );

    // Deposit = 100 Token A at $10, so the nominal collateral value is $1,000 before confidence.
    // JupLend first normalizes the deposit through `liquidityExchangePrice`, then mints
    // `expectedShares` f-token shares through `tokenExchangePrice`. The helper below redeems the
    // recorded shares back into current underlying Token A before deriving the borrow amount, and
    // the assertions below pin that redeemed amount to the nominal deposit.
    // `computeJupLendSameAssetBorrow(accountedUnderlying)` then applies:
    // - the oracle lower/upper confidence haircut used by the risk engine
    // - a 1% origination fee on the liability side
    // - a 25%-into-the-gap position between the healthy 101x init weight = 100 / 101 ~= 0.990099
    //   and the tightened 100x maint weight = 99 / 100 = 0.99
    await depositJuplendCollateral(user, marginfiAccount);

    const { accountedShares, accountedUnderlying } =
      await getJupLendAccountedCollateralNative(marginfiAccount);
    const sameAssetBorrow = computeJupLendSameAssetBorrow(accountedUnderlying);
    assert.equal(accountedShares.toString(), expectedShares.toString());
    assert.equal(accountedUnderlying.toString(), SAME_ASSET_DEPOSIT.toString());

    const sameAssetRemaining = [
      [juplendTokenABank, oracles.tokenAOracle.publicKey, bank.integrationAcc1],
      [regularTokenABank, oracles.tokenAOracle.publicKey],
    ] as PublicKey[][];

    await borrowFromRegularTokenA(
      user,
      marginfiAccount,
      sameAssetBorrow,
      sameAssetRemaining
    );

    let account = await pulseJupLendSameAssetHealth(
      user,
      marginfiAccount,
      sameAssetRemaining
    );
    const health = getNetHealth(account.healthCache);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((account.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0);
    assert.equal(account.healthCache.internalErr, 0);
    assert.equal(account.healthCache.mrgnErr, 0);
    assert.isTrue(health.init.isGreaterThan(0));
    assert.isTrue(health.maint.isGreaterThan(0));

    await tightenSameAssetLeverage();

    account = await pulseJupLendSameAssetHealth(
      user,
      marginfiAccount,
      getSameAssetRemainingGroups()
    );
    const tightenedHealth = getNetHealth(account.healthCache);
    assert.equal(account.healthCache.flags & HEALTH_CACHE_HEALTHY, 0);
    assert.isTrue(tightenedHealth.init.isLessThan(0));
    assert.isTrue(tightenedHealth.maint.isLessThan(0));
  });

  it("(user 1) repaying the same-mint borrow and switching to equal-value USDC debt removes the lift", async () => {
    const user = users[1];
    const marginfiAccount = await initFreshAccount(user);
    await resetSameAssetLeverage();
    const bank = await bankrunProgram.account.bank.fetch(juplendTokenABank);
    const lendingBefore = await juplendPrograms.lending.account.lending.fetch(
      tokenAPool.lending
    );
    const expectedShares = expectedSharesForDeposit(
      SAME_ASSET_DEPOSIT,
      new BN(lendingBefore.liquidityExchangePrice.toString()),
      new BN(lendingBefore.tokenExchangePrice.toString())
    );

    // Deposit = 100 Token A at $10, so the nominal collateral is worth $1,000 before weighting.
    // `computeJupLendSameAssetBorrow(accountedUnderlying)` sizes the Token A borrow from the live
    // underlying-equivalent collateral amount, using the oracle confidence haircut, the 1%
    // origination fee, and a 25%-into-the-gap position inside the 101x-init vs 100x-tightened
    // boundary window.
    // `computeSameValueUsdcBorrow(sameAssetBorrow)` then converts that exact fee-adjusted debt
    // notional into USDC, so only the liability mint changes.
    // Once the liability mint changes, the account loses the same-asset lift and falls back to the
    // plain 0.5 regular weight, so the equal-value USDC debt must be rejected.
    await depositJuplendCollateral(user, marginfiAccount);

    const { accountedShares, accountedUnderlying } =
      await getJupLendAccountedCollateralNative(marginfiAccount);
    const sameAssetBorrow = computeJupLendSameAssetBorrow(accountedUnderlying);
    const differentMintSameValueBorrow =
      computeSameValueUsdcBorrow(sameAssetBorrow);
    assert.equal(accountedShares.toString(), expectedShares.toString());

    const sameAssetRemaining = [
      [juplendTokenABank, oracles.tokenAOracle.publicKey, bank.integrationAcc1],
      [regularTokenABank, oracles.tokenAOracle.publicKey],
    ] as PublicKey[][];

    await borrowFromRegularTokenA(
      user,
      marginfiAccount,
      sameAssetBorrow,
      sameAssetRemaining
    );

    const account = await pulseJupLendSameAssetHealth(
      user,
      marginfiAccount,
      sameAssetRemaining
    );
    assert.ok((account.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0);

    const repayAllTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularTokenABank,
        tokenAccount: user.tokenAAccount,
        amount: new BN(0),
        repayAll: true,
        remaining: composeRemainingAccounts(sameAssetRemaining),
      })
    );
    await processBankrunTransaction(bankrunContext, repayAllTx, [user.wallet]);

    const unrelatedBorrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount,
        bank: regularUsdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts(
          getSameAssetWithUsdcRemainingGroups()
        ),
        amount: differentMintSameValueBorrow,
      })
    );
    unrelatedBorrowTx.recentBlockhash = await getBankrunBlockhash(
      bankrunContext
    );
    unrelatedBorrowTx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(unrelatedBorrowTx);
    assertBankrunTxFailed(result, "0x1779");
  });

  it("(admin) tightening same-asset leverage makes a JupLend/P0 position liquidatable", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];
    const liquidateeAccount = await initFreshAccount(liquidatee);
    const liquidatorAccount = await initFreshAccount(liquidator);
    const sameAssetRemaining = getSameAssetRemainingGroups();
    const startRemaining =
      composeRemainingAccountsWriteableMeta(sameAssetRemaining);
    const endRemaining =
      composeRemainingAccountsMetaBanksOnly(sameAssetRemaining);

    await mintToTokenAccount(
      ecosystem.tokenAMint.publicKey,
      liquidator.tokenAAccount,
      new BN(300 * 10 ** ecosystem.tokenADecimals)
    );

    await resetSameAssetLeverage();
    await depositRegularTokenACollateral(
      liquidator,
      liquidatorAccount,
      new BN(150 * 10 ** ecosystem.tokenADecimals)
    );
    await depositJuplendCollateral(liquidatee, liquidateeAccount);

    const { accountedUnderlying: liquidateeUnderlying } =
      await getJupLendAccountedCollateralNative(liquidateeAccount);
    const sameAssetBorrow = computeJupLendSameAssetBorrow(liquidateeUnderlying);

    // The liquidatee deposit is 100 Token A at $10, so the nominal collateral is $1,000 before
    // confidence.
    // `computeJupLendSameAssetBorrow(liquidateeUnderlying)` uses the live redeemed underlying
    // amount from the JupLend shares, the oracle lower/upper confidence haircut, and the 1%
    // origination fee to place the fee-adjusted liability 25% of the way from the tightened
    // boundary back toward the healthy boundary
    await borrowFromRegularTokenA(
      liquidatee,
      liquidateeAccount,
      sameAssetBorrow
    );

    await tightenSameAssetLeverage();

    const liquidationIxs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        feePayer: liquidator.wallet.publicKey,
      }),
      await refreshJupLendPoolIx(),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: startRemaining,
      }),
      await makeJuplendWithdrawSimpleIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        destinationTokenAccount: liquidator.tokenAAccount,
        bank: juplendTokenABank,
        pool: tokenAPool,
        amount: RECEIVERSHIP_JUP_WITHDRAW,
        withdrawAll: false,
        remainingAccounts: composeRemainingAccounts(sameAssetRemaining),
      }),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: regularTokenABank,
        tokenAccount: liquidator.tokenAAccount,
        amount: RECEIVERSHIP_JUP_WITHDRAW,
        remaining: composeRemainingAccounts(sameAssetRemaining),
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: endRemaining,
      }),
    ];
    const liquidationLut = await createLookupTableForInstructions(
      liquidator.wallet,
      liquidationIxs
    );
    const liquidationBlockhash = await getBankrunBlockhash(bankrunContext);
    const liquidationMessage = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: liquidationBlockhash,
      instructions: liquidationIxs,
    }).compileToV0Message([liquidationLut]);
    const liquidationTx = new VersionedTransaction(liquidationMessage);
    await processBankrunV0Transaction(
      bankrunContext,
      liquidationTx,
      [liquidator.wallet],
      false,
      true
    );
  });

  it("(admin) same-asset deleverage can improve a tightened JupLend/P0 position", async () => {
    const deleveragee = users[3];
    const deleverageeAccount = await initFreshAccount(deleveragee);
    const sameAssetRemaining = getSameAssetRemainingGroups();
    const startRemaining =
      composeRemainingAccountsWriteableMeta(sameAssetRemaining);
    const endRemaining =
      composeRemainingAccountsMetaBanksOnly(sameAssetRemaining);

    await mintToTokenAccount(
      ecosystem.tokenAMint.publicKey,
      riskAdmin.tokenAAccount,
      new BN(300 * 10 ** ecosystem.tokenADecimals)
    );

    await resetSameAssetLeverage({ newRiskAdmin: riskAdmin.wallet.publicKey });
    await depositJuplendCollateral(deleveragee, deleverageeAccount);

    const { accountedUnderlying: deleverageeUnderlying } =
      await getJupLendAccountedCollateralNative(deleverageeAccount);
    const sameAssetBorrow = computeJupLendSameAssetBorrow(
      deleverageeUnderlying
    );

    // The deleveragee deposit is also 100 Token A at $10, so the nominal collateral is $1,000.
    // `computeJupLendSameAssetBorrow(deleverageeUnderlying)` uses the live redeemed underlying
    // amount, the oracle confidence haircut, and the 1% origination fee to place the fee-adjusted
    // Token A debt 25% of the way from the tightened 100x maint boundary back toward the healthy
    // 101x init boundary.
    await borrowFromRegularTokenA(
      deleveragee,
      deleverageeAccount,
      sameAssetBorrow
    );

    await tightenSameAssetLeverage();

    await pulseJupLendSameAssetHealth(
      deleveragee,
      deleverageeAccount,
      sameAssetRemaining
    );

    const before = await bankrunProgram.account.marginfiAccount.fetch(
      deleverageeAccount
    );
    const healthBefore = getNetHealth(before.healthCache);
    assert.isTrue(healthBefore.maint.isLessThan(0));

    const deleverageIxs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        feePayer: riskAdmin.wallet.publicKey,
      }),
      await refreshJupLendPoolIx(),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: startRemaining,
      }),
      await makeJuplendWithdrawSimpleIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        destinationTokenAccount: riskAdmin.tokenAAccount,
        bank: juplendTokenABank,
        pool: tokenAPool,
        amount: RECEIVERSHIP_JUP_WITHDRAW,
        withdrawAll: false,
        remainingAccounts: composeRemainingAccounts(sameAssetRemaining),
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        bank: regularTokenABank,
        tokenAccount: riskAdmin.tokenAAccount,
        amount: RECEIVERSHIP_JUP_WITHDRAW,
        remaining: composeRemainingAccounts(sameAssetRemaining),
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram, {
        marginfiAccount: deleverageeAccount,
        remaining: endRemaining,
      }),
    ];
    const deleverageLut = await createLookupTableForInstructions(
      riskAdmin.wallet,
      deleverageIxs
    );
    const deleverageBlockhash = await getBankrunBlockhash(bankrunContext);
    const deleverageMessage = new TransactionMessage({
      payerKey: riskAdmin.wallet.publicKey,
      recentBlockhash: deleverageBlockhash,
      instructions: deleverageIxs,
    }).compileToV0Message([deleverageLut]);
    const deleverageTx = new VersionedTransaction(deleverageMessage);
    await processBankrunV0Transaction(
      bankrunContext,
      deleverageTx,
      [riskAdmin.wallet],
      false,
      true
    );
  });
});
