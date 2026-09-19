import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { OracleSetupRaw } from "@mrgnlabs/marginfi-client-v2";
import { Reserve } from "@kamino-finance/klend-sdk";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  kaminoGroup,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  users,
} from "../../rootHooks";
import {
  defaultKaminoBankConfig,
  getLiquidityExchangeRate,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "../../utils/kamino-utils";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "../../utils/kamino-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  pulseBankPrice,
  repayIx,
} from "../../utils/user-instructions";
import {
  addBankWithSeed,
  configureBankOracle,
  configureBankOracleScope,
} from "../../utils/group-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "../../utils/pdas";
import {
  assertBankrunTxFailed,
  assertI80F48Approx,
  assertKeysEqual,
  getTokenBalance,
} from "../../utils/genericTests";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { getBankrunTime, processBankrunTransaction } from "../../utils/tools";
import { ProgramTestContext } from "../../utils/litesvm";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import {
  ASSET_TAG_KAMINO,
  CONF_INTERVAL_MULTIPLE_FLOAT,
  defaultBankConfig,
  ORACLE_SETUP_PYTH_PUSH,
  ORACLE_SETUP_SCOPE_KAMINO,
} from "../../utils/types";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";

let ctx: ProgramTestContext;
let market: PublicKey;
let usdcReserve: PublicKey;
let tokenAReserve: PublicKey;
let scopeKaminoBank: PublicKey;
let scopeKaminoObligation: PublicKey;
let userAccount: PublicKey;
let borrowBank: PublicKey;
let adminAccount: PublicKey;
let userUsdcStart = 0;

const SCOPE_SEED = new BN(7779);
const BORROW_SEED = new BN(8889);
/** entry 11 = 2.5 (value / 10^exp). USDC is not worth $2.50; the point is a price only this feed carries. */
const ENTRY = { index: 11, value: 2_500_000_000n, exp: 9n };
const SCOPE_PRICE = 2.5;
/** never written in any fixture below */
const UNREFRESHED_ENTRY = 300;
const DEPOSIT_AMOUNT = new BN(1_000 * 10 ** ecosystem.usdcDecimals);
const BORROW_AMOUNT = new BN(10 * 10 ** ecosystem.tokenADecimals);
const feed = Keypair.generate().publicKey;

describe("k21: Scope-priced Kamino bank", () => {
  /** Re-stamps the feed at the current clock, optionally `ageSeconds` in the past. */
  const freshenFeed = async (ageSeconds = 0) => {
    const now = await getBankrunTime(bankrunContext);
    setScopeFeed(
      bankrunContext,
      feed,
      makeScopePrices([{ ...ENTRY, timestamp: now - ageSeconds }])
    );
  };

  /** Refreshes the reserve in the same tx, as every pricing path must, then pulses the bank. */
  const pulse = async (remaining: PublicKey[], trySend = false) => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await pulseBankPrice(user.mrgnBankrunProgram, {
        bank: scopeKaminoBank,
        remaining,
      })
    );
    return processBankrunTransaction(ctx, tx, [user.wallet], trySend);
  };

  const scopeConfigure = async (
    entryIndex: number,
    remaining: PublicKey[],
    trySend = false
  ) => {
    const tx = new Transaction().add(
      await configureBankOracleScope(groupAdmin.mrgnBankrunProgram, {
        bank: scopeKaminoBank,
        oracle: feed,
        entryIndex,
        remaining,
      })
    );
    return processBankrunTransaction(ctx, tx, [groupAdmin.wallet], trySend);
  };

  before(async () => {
    ctx = bankrunContext;
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    await freshenFeed();
  });

  it("(user 3) initialize marginfi account for the Kamino group", async () => {
    const user = users[3];
    const accountKeypair = Keypair.generate();
    userAccount = accountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);
  });

  it("(admin) add Kamino bank with ScopeKamino at creation - fails, KaminoInvalidOracleSetup", async () => {
    // The compact config carries no entry index, so a Scope setup is only reachable through
    // configure_bank_oracle_scope once the bank exists.
    const config = defaultKaminoBankConfig(feed);
    config.oracleSetup = { scopeKamino: {} } as unknown as OracleSetupRaw;
    const tx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: usdcReserve,
          kaminoMarket: market,
          oracle: feed,
        },
        { config, seed: SCOPE_SEED }
      )
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true
    );
    assertBankrunTxFailed(result, 6211);
  });

  it("(admin) add Kamino USDC bank on Pyth + init obligation", async () => {
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      SCOPE_SEED
    );
    scopeKaminoBank = bankKey;

    const addBankTx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: usdcReserve,
          kaminoMarket: market,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultKaminoBankConfig(oracles.usdcOracle.publicKey),
          seed: SCOPE_SEED,
        }
      )
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const [authority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      scopeKaminoBank
    );
    const [obligation] = deriveBaseObligation(authority, market);
    scopeKaminoObligation = obligation;

    const initObligationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: users[3].wallet.publicKey,
          bank: scopeKaminoBank,
          signerTokenAccount: users[3].usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        new BN(100)
      )
    );
    await processBankrunTransaction(ctx, initObligationTx, [users[3].wallet]);
  });

  it("(admin) configure_bank_oracle rejects ScopeKamino - use configure_bank_oracle_scope", async () => {
    const tx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: scopeKaminoBank,
        type: ORACLE_SETUP_SCOPE_KAMINO,
        oracle: feed,
        remaining: [usdcReserve],
      })
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true
    );
    // UseConfigureBankOracleScope
    assertBankrunTxFailed(result, 6803);
  });

  it("(admin) configure scope with the feed only - fails, WrongNumberOfOracleAccounts", async () => {
    const result = await scopeConfigure(ENTRY.index, [], true);
    assertBankrunTxFailed(result, 6051);
  });

  it("(admin) configure scope with the wrong reserve - fails, KaminoReserveValidationFailed", async () => {
    const result = await scopeConfigure(ENTRY.index, [tokenAReserve], true);
    assertBankrunTxFailed(result, 6210);
  });

  it("(admin) configure scope against an unrefreshed entry - fails, ScopeInvalidEntry", async () => {
    const result = await scopeConfigure(UNREFRESHED_ENTRY, [usdcReserve], true);
    assertBankrunTxFailed(result, 6801);
  });

  it("(admin) configure scope [feed, reserve] - happy path", async () => {
    await scopeConfigure(ENTRY.index, [usdcReserve]);

    const bank = await bankrunProgram.account.bank.fetch(scopeKaminoBank);
    assert.deepEqual(bank.config.oracleSetup, { scopeKamino: {} });
    assert.equal(bank.config.scopeEntryIndex, ENTRY.index);
    assert.equal(bank.config.assetTag, ASSET_TAG_KAMINO);
    assertKeysEqual(bank.config.oracleKeys[0], feed);
    // The reserve pinned at bank creation is untouched.
    assertKeysEqual(bank.config.oracleKeys[1], usdcReserve);
    assertI80F48Approx(bank.config.fixedPrice, 0);
  });

  it("(attacker) pulse with the wrong reserve - fails, KaminoReserveValidationFailed", async () => {
    await freshenFeed();
    const result = await pulse([feed, tokenAReserve], true);
    assertBankrunTxFailed(result, 6210);
  });

  it("(attacker) pulse with the feed only - fails, WrongNumberOfOracleAccounts", async () => {
    await freshenFeed();
    const result = await pulse([feed], true);
    assertBankrunTxFailed(result, 6051);
  });

  it("(attacker) pulse with an impostor feed - fails, WrongOracleAccountKeys", async () => {
    const impostor = Keypair.generate().publicKey;
    const now = await getBankrunTime(bankrunContext);
    setScopeFeed(
      bankrunContext,
      impostor,
      makeScopePrices([{ ...ENTRY, timestamp: now }])
    );
    const result = await pulse([impostor, usdcReserve], true);
    assertBankrunTxFailed(result, 6052);
  });

  it("pulse caches the scope price and the reserve exchange rate separately", async () => {
    await freshenFeed();
    await pulse([feed, usdcReserve]);

    const bank = await bankrunProgram.account.bank.fetch(scopeKaminoBank);
    const reserveRaw = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve
    );
    const reserve = { ...reserveRaw } as Reserve;

    assertI80F48Approx(bank.cache.lastOraclePrice, SCOPE_PRICE, 0.000001);
    // Scope carries no confidence interval.
    assertI80F48Approx(bank.cache.lastOraclePriceConfidence, 0);
    assertI80F48Approx(
      bank.cache.priceMultiplier,
      getLiquidityExchangeRate(reserve).toNumber(),
      0.000001
    );
  });

  it("rejects a stale scope price", async () => {
    const maxAge = defaultKaminoBankConfig(feed).oracleMaxAge;
    await freshenFeed(maxAge + 60);
    const result = await pulse([feed, usdcReserve], true);
    // ScopeStalePrice
    assertBankrunTxFailed(result, 6802);
    await freshenFeed();
  });

  it("(admin) add throwaway regular Token A bank + seed liquidity", async () => {
    const adminAccountKeypair = Keypair.generate();
    adminAccount = adminAccountKeypair.publicKey;

    const initAdminTx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: adminAccount,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(ctx, initAdminTx, [
      groupAdmin.wallet,
      adminAccountKeypair,
    ]);

    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      BORROW_SEED
    );
    borrowBank = bankKey;

    const config = defaultBankConfig();
    config.interestRateConfig.protocolOriginationFee =
      bigNumberToWrappedI80F48(0);

    const addBankTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        config,
        seed: BORROW_SEED,
      })
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const configOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: borrowBank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.tokenAOracle.publicKey,
      })
    );
    await processBankrunTransaction(ctx, configOracleTx, [groupAdmin.wallet]);

    const seedTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: borrowBank,
        tokenAccount: groupAdmin.tokenAAccount,
        amount: new BN(100 * 10 ** ecosystem.tokenADecimals),
      })
    );
    await processBankrunTransaction(ctx, seedTx, [groupAdmin.wallet]);
  });

  it("(user 3) deposit into the scope-priced Kamino bank", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    userUsdcStart = userUsdcBefore;

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        scopeKaminoObligation,
        [usdcReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: scopeKaminoBank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        DEPOSIT_AMOUNT
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    assert.equal(userUsdcBefore - userUsdcAfter, DEPOSIT_AMOUNT.toNumber());
  });

  it("(user 3) borrow Token A against scope-priced Kamino collateral", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await freshenFeed();

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        remaining: composeRemainingAccounts([
          [scopeKaminoBank, feed, usdcReserve],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
        amount: BORROW_AMOUNT,
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );
    assert.equal(userTokenAAfter - userTokenABefore, BORROW_AMOUNT.toNumber());
  });

  it("(user 3) health pulse values the collateral at scope price x exchange rate", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await freshenFeed();

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [scopeKaminoBank, feed, usdcReserve],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const cache = (
      await bankrunProgram.account.marginfiAccount.fetch(userAccount)
    ).healthCache;
    const actualAssetValue = wrappedI80F48toBigNumber(
      cache.assetValue
    ).toNumber();
    const actualLiabilityValue = wrappedI80F48toBigNumber(
      cache.liabilityValue
    ).toNumber();

    // The collateral balance is stored in cTokens (liquidity / (liq/col rate)); the
    // ScopeKamino multiplier is that same rate, so the position is valued at the deposited
    // liquidity times the scope price. Asset weight is 1 for the default Kamino bank.
    const expectedAssetValue = SCOPE_PRICE * 1000;
    // 10 tokens at the high price bias
    const expectedLiabilityValue =
      oracles.tokenAPrice * (1 + CONF_INTERVAL_MULTIPLE_FLOAT) * 10;

    assert.approximately(
      actualAssetValue,
      expectedAssetValue,
      expectedAssetValue * 0.005
    );
    assert.approximately(
      actualLiabilityValue,
      expectedLiabilityValue,
      expectedLiabilityValue * 0.005
    );
  });

  it("(user 3) withdraw from the scope-priced Kamino bank", async () => {
    const user = users[3];
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await freshenFeed();

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        scopeKaminoObligation,
        [usdcReserve]
      ),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          bank: scopeKaminoBank,
          mint: ecosystem.usdcMint.publicKey,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        {
          amount: withdrawAmount,
          isWithdrawAll: false,
          remaining: composeRemainingAccounts([
            [scopeKaminoBank, feed, usdcReserve],
            [borrowBank, oracles.tokenAOracle.publicKey],
          ]),
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    // withdraw amount is in cTokens; the liquidity out follows the refreshed reserve's rate
    const reserveRaw = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve
    );
    const expectedWithdraw = getLiquidityExchangeRate({ ...reserveRaw } as Reserve)
      .mul(withdrawAmount.toNumber())
      .toNumber();
    assert.approximately(userUsdcAfter - userUsdcBefore, expectedWithdraw, 2);
  });

  it("(user 3) repay and withdraw all - gets the initial deposit back", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await freshenFeed();

    const repayTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        amount: BORROW_AMOUNT,
        repayAll: true,
        remaining: composeRemainingAccounts([
          [scopeKaminoBank, feed, usdcReserve],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      })
    );
    await processBankrunTransaction(ctx, repayTx, [user.wallet]);

    const withdrawAllTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        scopeKaminoObligation,
        [usdcReserve]
      ),
      await makeKaminoWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          bank: scopeKaminoBank,
          mint: ecosystem.usdcMint.publicKey,
          destinationTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserve: usdcReserve,
        },
        {
          amount: new BN(0),
          isWithdrawAll: true,
          remaining: composeRemainingAccounts([
            [borrowBank, oracles.tokenAOracle.publicKey],
          ]),
        }
      )
    );
    await processBankrunTransaction(ctx, withdrawAllTx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    // Note: you lose 1-2 lamports for Kamino withdraws
    assert.approximately(userUsdcAfter, userUsdcStart, 2);
    assert.isAtMost(userUsdcAfter, userUsdcStart);
  });
});
