import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMintToInstruction,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";

import {
  addBankWithSeed,
  configureBankOracle,
  configureBankOracleScope,
  groupInitialize,
} from "../../utils/group-instructions";
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
  assertBankrunTxFailed,
  assertI80F48Approx,
  assertKeysEqual,
  getTokenBalance,
} from "../../utils/genericTests";
import { getBankrunTime, processBankrunTransaction } from "../../utils/tools";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { bnToBigIntSafe } from "../../utils/bn-utils";
import {
  ASSET_TAG_JUPLEND,
  CONF_INTERVAL_MULTIPLE_FLOAT,
  defaultBankConfig,
  ORACLE_SETUP_PYTH_PUSH,
  ORACLE_SETUP_SCOPE_JUPLEND,
} from "../../utils/types";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";

import { deriveJuplendMrgnAddresses } from "../../utils/juplend/juplend-pdas";
import {
  defaultJuplendBankConfig,
  type JuplendConfigCompact,
  type JuplendPoolKeys,
} from "../../utils/juplend/types";
import { fetchJuplendPool } from "../../utils/juplend/jlr-pool-setup";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "../../utils/juplend/group-instructions";
import { makeJuplendDepositIx } from "../../utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "../../utils/juplend/shorthand-instructions";
import { deriveBankWithSeed } from "../../utils/pdas";
import { ProgramTestContext } from "../../utils/litesvm";
import { getJuplendPrograms } from "../../utils/juplend/programs";
import { dummyIx } from "../../utils/bankrunConnection";

/** deterministic (32 bytes) */
const JUPLEND_SC_GROUP_SEED = Buffer.from("JUPLEND_SC_GROUP_SEED_0000000000");

const BANK_SEED = new BN(111);
const BORROW_SEED = new BN(112);
/** entry 11 = 2.5 (value / 10^exp). USDC is not worth $2.50; the point is a price only this feed carries. */
const ENTRY = { index: 11, value: 2_500_000_000n, exp: 9n };
const SCOPE_PRICE = 2.5;
/** never written in any fixture below */
const UNREFRESHED_ENTRY = 300;
/** from defaultJuplendBankConfig */
const ASSET_WEIGHT_INIT = 0.8;
const EXCHANGE_PRICES_PRECISION = 1e12;
const DEPOSIT_AMOUNT = new BN(1_000 * 10 ** ecosystem.usdcDecimals);
const BORROW_AMOUNT = new BN(10 * 10 ** ecosystem.tokenADecimals);
const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)
const feed = Keypair.generate().publicKey;

let ctx: ProgramTestContext;
let pool: JuplendPoolKeys;
let scopeJuplendBank: PublicKey;
let liquidityVaultAuthority: PublicKey;
let withdrawIntermediaryAta: PublicKey;
let userAccount: PublicKey;
let borrowBank: PublicKey;
let adminAccount: PublicKey;
let userUsdcStart = 0;
let juplendPrograms: ReturnType<typeof getJuplendPrograms>;

describe("jlr13: Scope-priced JupLend bank", () => {
  const juplendGroup = Keypair.fromSeed(JUPLEND_SC_GROUP_SEED);

  /** Re-stamps the feed at the current clock, optionally `ageSeconds` in the past. */
  const freshenFeed = async (ageSeconds = 0) => {
    const now = await getBankrunTime(bankrunContext);
    setScopeFeed(
      bankrunContext,
      feed,
      makeScopePrices([{ ...ENTRY, timestamp: now - ageSeconds }])
    );
  };

  /** JupLend's lending state must be fresh for any pricing path. */
  const refreshLending = async () => {
    const user = users[3];
    const tx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey)
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
  };

  const pulse = async (remaining: PublicKey[], trySend = false) => {
    const user = users[3];
    await refreshLending();
    const tx = new Transaction().add(
      await pulseBankPrice(user.mrgnBankrunProgram, {
        bank: scopeJuplendBank,
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
        bank: scopeJuplendBank,
        oracle: feed,
        entryIndex,
        remaining,
      })
    );
    return processBankrunTransaction(ctx, tx, [groupAdmin.wallet], trySend);
  };

  before(async () => {
    ctx = bankrunContext;
    juplendPrograms = getJuplendPrograms();

    // Mint USDC to user 3 and admin
    const mintAmount = 10_000_000_000; // 10,000 USDC (6 decimals)
    for (const usdcAccount of [users[3].usdcAccount, groupAdmin.usdcAccount]) {
      const mintIx = createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        mintAmount
      );
      await processBankrunTransaction(
        ctx,
        new Transaction().add(mintIx),
        [globalProgramAdmin.wallet],
        false,
        true
      );
    }

    pool = (
      await fetchJuplendPool({
        mint: ecosystem.usdcMint.publicKey,
        programs: juplendPrograms,
      })
    ).keys;

    await freshenFeed();
  });

  it("(admin) initialize juplend scope group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });
    await processBankrunTransaction(
      ctx,
      new Transaction().add(ix),
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );
  });

  it("(user 3) initialize marginfi account for the juplend group", async () => {
    const user = users[3];
    const accountKeypair = Keypair.generate();
    userAccount = accountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: juplendGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);
  });

  it("(admin) add JupLend bank with ScopeJuplend at creation - fails, JuplendInvalidOracleSetup", async () => {
    // The compact config carries no entry index, so a Scope setup is only reachable through
    // configure_bank_oracle_scope once the bank exists.
    const config: JuplendConfigCompact = {
      ...defaultJuplendBankConfig(feed, ecosystem.usdcDecimals),
      oracleSetup: {
        scopeJuplend: {},
      } as unknown as JuplendConfigCompact["oracleSetup"],
    };
    const tx = new Transaction().add(
      await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: juplendGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: BANK_SEED,
        oracle: feed,
        jupLendingState: pool.lending,
        fTokenMint: pool.fTokenMint,
        config,
      })
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true
    );
    assertBankrunTxFailed(result, 6500);
  });

  it("(admin) add JupLend USDC bank on Pyth + init position", async () => {
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      tokenProgram: pool.tokenProgram,
    });
    scopeJuplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    withdrawIntermediaryAta = derived.withdrawIntermediaryAta;

    const addBankTx = new Transaction().add(
      await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: juplendGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: BANK_SEED,
        oracle: oracles.usdcOracle.publicKey,
        jupLendingState: pool.lending,
        fTokenMint: pool.fTokenMint,
        config: defaultJuplendBankConfig(
          oracles.usdcOracle.publicKey,
          ecosystem.usdcDecimals
        ),
      })
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const initPosTx = new Transaction().add(
      await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.usdcAccount,
        bank: scopeJuplendBank,
        pool,
        seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      })
    );
    await processBankrunTransaction(ctx, initPosTx, [groupAdmin.wallet]);
  });

  it("(admin) configure_bank_oracle rejects ScopeJuplend - use configure_bank_oracle_scope", async () => {
    const tx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: scopeJuplendBank,
        type: ORACLE_SETUP_SCOPE_JUPLEND,
        oracle: feed,
        remaining: [pool.lending],
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

  it("(admin) configure scope with the wrong lending state - fails, JuplendLendingValidationFailed", async () => {
    const wrongLending = Keypair.generate().publicKey;
    const result = await scopeConfigure(ENTRY.index, [wrongLending], true);
    assertBankrunTxFailed(result, 6501);
  });

  it("(admin) configure scope against an unrefreshed entry - fails, ScopeInvalidEntry", async () => {
    const result = await scopeConfigure(
      UNREFRESHED_ENTRY,
      [pool.lending],
      true
    );
    assertBankrunTxFailed(result, 6801);
  });

  it("(admin) configure scope [feed, lending] - happy path", async () => {
    await scopeConfigure(ENTRY.index, [pool.lending]);

    const bank = await bankrunProgram.account.bank.fetch(scopeJuplendBank);
    assert.deepEqual(bank.config.oracleSetup, { scopeJuplend: {} });
    assert.equal(bank.config.scopeEntryIndex, ENTRY.index);
    assert.equal(bank.config.assetTag, ASSET_TAG_JUPLEND);
    assertKeysEqual(bank.config.oracleKeys[0], feed);
    // The lending state pinned at bank creation is untouched.
    assertKeysEqual(bank.config.oracleKeys[1], pool.lending);
    assertI80F48Approx(bank.config.fixedPrice, 0);
  });

  it("(attacker) pulse with the wrong lending state - fails, JuplendLendingValidationFailed", async () => {
    await freshenFeed();
    const result = await pulse([feed, Keypair.generate().publicKey], true);
    assertBankrunTxFailed(result, 6501);
  });

  it("(attacker) pulse with the feed only - fails, WrongNumberOfOracleAccounts", async () => {
    await freshenFeed();
    const result = await pulse([feed], true);
    assertBankrunTxFailed(result, 6051);
  });

  it("pulse caches the scope price and the token exchange price separately", async () => {
    await freshenFeed();
    await pulse([feed, pool.lending]);

    const bank = await bankrunProgram.account.bank.fetch(scopeJuplendBank);
    const lending = await juplendPrograms.lending.account.lending.fetch(
      pool.lending
    );
    const expectedMultiplier =
      Number(bnToBigIntSafe(lending.tokenExchangePrice)) /
      EXCHANGE_PRICES_PRECISION;

    assertI80F48Approx(bank.cache.lastOraclePrice, SCOPE_PRICE, 0.000001);
    // Scope carries no confidence interval.
    assertI80F48Approx(bank.cache.lastOraclePriceConfidence, 0);
    assertI80F48Approx(bank.cache.priceMultiplier, expectedMultiplier, 1e-9);
  });

  it("rejects a stale scope price", async () => {
    const maxAge = defaultJuplendBankConfig(
      feed,
      ecosystem.usdcDecimals
    ).oracleMaxAge;
    await freshenFeed(maxAge + 60);
    const result = await pulse([feed, pool.lending], true);
    // ScopeStalePrice
    assertBankrunTxFailed(result, 6802);
    await freshenFeed();
  });

  it("(admin) add throwaway regular Token A bank + seed liquidity", async () => {
    const adminAccountKeypair = Keypair.generate();
    adminAccount = adminAccountKeypair.publicKey;

    const initAdminTx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: juplendGroup.publicKey,
        marginfiAccount: adminAccount,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );
    await processBankrunTransaction(
      ctx,
      initAdminTx,
      [groupAdmin.wallet, adminAccountKeypair],
      false,
      true
    );

    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      juplendGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      BORROW_SEED
    );
    borrowBank = bankKey;

    const config = defaultBankConfig();
    config.interestRateConfig.protocolOriginationFee =
      bigNumberToWrappedI80F48(0);

    const addBankTx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: juplendGroup.publicKey,
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

    const seedAmount = new BN(100 * 10 ** ecosystem.tokenADecimals);
    const mintSeedLiquidityIx = createMintToInstruction(
      ecosystem.tokenAMint.publicKey,
      groupAdmin.tokenAAccount,
      globalProgramAdmin.wallet.publicKey,
      BigInt(seedAmount.toString())
    );
    await processBankrunTransaction(
      ctx,
      new Transaction().add(mintSeedLiquidityIx),
      [globalProgramAdmin.wallet],
      false,
      true
    );

    const seedTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: borrowBank,
        tokenAccount: groupAdmin.tokenAAccount,
        amount: seedAmount,
      })
    );
    await processBankrunTransaction(ctx, seedTx, [groupAdmin.wallet]);
  });

  it("(user 3) deposit into the scope-priced JupLend bank", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await refreshLending();

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    userUsdcStart = userUsdcBefore;

    const tx = new Transaction().add(
      await makeJuplendDepositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: scopeJuplendBank,
        signerTokenAccount: user.usdcAccount,
        pool,
        amount: DEPOSIT_AMOUNT,
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    assert.equal(userUsdcBefore - userUsdcAfter, DEPOSIT_AMOUNT.toNumber());
  });

  it("(user 3) borrow Token A against scope-priced JupLend collateral", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await refreshLending();
    await freshenFeed();

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount
    );

    const tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        remaining: composeRemainingAccounts([
          [scopeJuplendBank, feed, pool.lending],
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

  it("(user 3) health pulse values the collateral at scope price x exchange price", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await refreshLending();
    await freshenFeed();

    const tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [scopeJuplendBank, feed, pool.lending],
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

    // The token exchange price is ~1.0 this early, so the deposit is valued at the scope price,
    // weighted by assetWeightInit.
    const expectedAssetValue = SCOPE_PRICE * 1000 * ASSET_WEIGHT_INIT;
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

  it("(user 3) withdraw from the scope-priced JupLend bank", async () => {
    const user = users[3];
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await refreshLending();
    await freshenFeed();

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        ecosystem.usdcMint.publicKey,
        pool.tokenProgram
      );
    await processBankrunTransaction(
      ctx,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [user.wallet]
    );

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );

    const tx = new Transaction().add(
      await makeJuplendWithdrawSimpleIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: scopeJuplendBank,
        destinationTokenAccount: user.usdcAccount,
        pool,
        amount: withdrawAmount,
        remainingAccounts: composeRemainingAccounts([
          [scopeJuplendBank, feed, pool.lending],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      })
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    assert.approximately(
      userUsdcAfter - userUsdcBefore,
      withdrawAmount.toNumber(),
      2
    );
  });

  it("(user 3) repay and withdraw all - gets the initial deposit back", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
    await refreshLending();
    await freshenFeed();

    const repayTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        amount: BORROW_AMOUNT,
        repayAll: true,
        remaining: composeRemainingAccounts([
          [scopeJuplendBank, feed, pool.lending],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      })
    );
    await processBankrunTransaction(ctx, repayTx, [user.wallet]);

    const withdrawAllTx = new Transaction().add(
      await makeJuplendWithdrawSimpleIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: scopeJuplendBank,
        destinationTokenAccount: user.usdcAccount,
        pool,
        amount: new BN(0),
        withdrawAll: true,
        remainingAccounts: composeRemainingAccounts([
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      })
    );
    await processBankrunTransaction(ctx, withdrawAllTx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );
    // Note: JupLend round-trip rounding can lose a few lamports per operation
    assert.approximately(userUsdcAfter, userUsdcStart, 5);
    assert.isAtMost(userUsdcAfter, userUsdcStart);
  });
});
