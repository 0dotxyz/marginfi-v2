import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
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
  verbose,
} from "./rootHooks";

import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
  setFixedPrice,
} from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  pulseBankPrice,
  repayIx,
} from "./utils/user-instructions";
import {
  assertBankrunTxFailed,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { logHealthCache, processBankrunTransaction } from "./utils/tools";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  ASSET_TAG_JUPLEND,
  CONF_INTERVAL_MULTIPLE_FLOAT,
  defaultBankConfig,
  ORACLE_SETUP_FIXED_JUPLEND,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";

import {
  deriveJuplendGlobalKeys,
  deriveJuplendMrgnAddresses,
} from "./utils/juplend/juplend-pdas";
import {
  DEFAULT_BORROW_CONFIG_MIN,
  DEFAULT_RATE_CONFIG,
  DEFAULT_SUPPLY_CONFIG,
  DEFAULT_TOKEN_CONFIG,
  defaultJuplendBankConfig,
  type JuplendPoolKeys,
} from "./utils/juplend/types";
import {
  configureJuplendProtocolPermissions,
  initJuplendGlobals,
  initJuplendPool,
  fetchJuplendPool,
} from "./utils/juplend/jlr-pool-setup";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/group-instructions";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { ProgramTestContext } from "solana-bankrun";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { dummyIx } from "./utils/bankrunConnection";

/** deterministic (32 bytes) */
const JUPLEND_FX_GROUP_SEED = Buffer.from("JUPLEND_FX_GROUP_SEED_0000000000");

const BANK_SEED = new BN(101);
const BORROW_SEED = new BN(102);
// Note: USDC is not worth $2, so this test is silly
const FIXED_PRICE = 2;
const BORROW_AMOUNT = new BN(10 * 10 ** ecosystem.tokenADecimals);
const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)

let ctx: ProgramTestContext;
let pool: JuplendPoolKeys;
let fixedJuplendBank: PublicKey;
let liquidityVaultAuthority: PublicKey;
let withdrawIntermediaryAta: PublicKey;
let userAccount: PublicKey;
let borrowBank: PublicKey;
let adminAccount: PublicKey;
let userUsdcStart = 0;
let juplendPrograms: ReturnType<typeof getJuplendPrograms>;

describe("jlrx: Fixed JupLend price bank", () => {
  const juplendGroup = Keypair.fromSeed(JUPLEND_FX_GROUP_SEED);

  before(async () => {
    ctx = bankrunContext;
    juplendPrograms = getJuplendPrograms();

    // Mint USDC to user 3 and admin
    const mintAmount = 10_000_000_000; // 10,000 USDC (6 decimals)
    for (const wallet of [users[3].wallet, groupAdmin.wallet]) {
      const usdcAccount =
        wallet === users[3].wallet
          ? users[3].usdcAccount
          : groupAdmin.usdcAccount;
      const mintIx = createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        mintAmount,
      );
      await processBankrunTransaction(
        ctx,
        new Transaction().add(mintIx),
        [globalProgramAdmin.wallet],
        false,
        true,
      );
    }

    pool = (
      await fetchJuplendPool({
        mint: ecosystem.usdcMint.publicKey,
        programs: juplendPrograms,
      })
    ).keys;
  });

  it("(admin) initialize juplend fixed-price group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });
    await processBankrunTransaction(
      ctx,
      new Transaction().add(ix),
      [groupAdmin.wallet, juplendGroup],
      false,
      true,
    );
  });

  it("(user 3) initialize marginfi account for juplend group", async () => {
    const user = users[3];
    const accountKeypair = Keypair.generate();
    userAccount = accountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: juplendGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet, accountKeypair]);
  });

  it("(admin) add fixed JupLend USDC bank + init position", async () => {
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      tokenProgram: pool.tokenProgram,
    });

    fixedJuplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    withdrawIntermediaryAta = derived.withdrawIntermediaryAta;

    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals,
    );

    // 1. Add bank
    const addBankTx = new Transaction().add(
      await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: juplendGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: BANK_SEED,
        oracle: oracles.usdcOracle.publicKey,
        jupLendingState: pool.lending,
        fTokenMint: pool.fTokenMint,
        config,
      }),
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    // 2. Activate bank via init position (seed deposit)
    const initPosTx = new Transaction().add(
      await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.usdcAccount,
        bank: fixedJuplendBank,
        pool,
        seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      }),
    );
    await processBankrunTransaction(ctx, initPosTx, [groupAdmin.wallet]);

    // 3. Set fixed price — remaining accounts = [lending state]
    const setFixedTx = new Transaction().add(
      await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
        bank: fixedJuplendBank,
        price: FIXED_PRICE,
        remaining: [pool.lending],
      }),
    );
    await processBankrunTransaction(ctx, setFixedTx, [groupAdmin.wallet]);

    // Verify bank config
    const bank = await bankrunProgram.account.bank.fetch(fixedJuplendBank);
    assert.equal(bank.config.assetTag, ASSET_TAG_JUPLEND);
    assert.deepEqual(bank.config.oracleSetup, { fixedJuplend: {} });
    assertKeysEqual(bank.config.oracleKeys[0], PublicKey.default);
    // Note: the lending account persists, ftoken vault and ata are stored in integration accounts.
    assertKeysEqual(bank.config.oracleKeys[1], pool.lending);
    assertKeysEqual(bank.integrationAcc2, derived.fTokenVault);
    const expectedWithdrawIntermediaryAta = getAssociatedTokenAddressSync(
      bank.mint,
      derived.liquidityVaultAuthority,
      true,
    );
    assertKeysEqual(bank.integrationAcc3, expectedWithdrawIntermediaryAta);

    const fixedPrice = wrappedI80F48toBigNumber(
      bank.config.fixedPrice,
    ).toNumber();
    assert.approximately(fixedPrice, FIXED_PRICE, 0.001);

    console.log("Fixed JupLend bank:", fixedJuplendBank.toString());
  });

  it("(admin) configure_bank_oracle rejects FixedJuplend setup - use set_fixed_oracle_price", async () => {
    const tx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: fixedJuplendBank,
        type: ORACLE_SETUP_FIXED_JUPLEND,
        oracle: oracles.usdcOracle.publicKey,
      }),
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet],
      true,
    );
    assert("result" in result && !!result.result, "tx should fail");
    const logs = (result.meta?.logMessages ?? []).join("\n");
    assert.include(logs, "Use set_fixed_oracle_price instead");
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
      }),
    );
    await processBankrunTransaction(
      ctx,
      initAdminTx,
      [groupAdmin.wallet, adminAccountKeypair],
      false,
      true,
    );

    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      juplendGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      BORROW_SEED,
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
      }),
    );
    await processBankrunTransaction(ctx, addBankTx, [groupAdmin.wallet]);

    const configOracleTx = new Transaction().add(
      await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
        bank: borrowBank,
        type: ORACLE_SETUP_PYTH_PUSH,
        oracle: oracles.tokenAOracle.publicKey,
      }),
    );
    await processBankrunTransaction(ctx, configOracleTx, [groupAdmin.wallet]);

    const seedAmount = new BN(100 * 10 ** ecosystem.tokenADecimals);
    const mintSeedLiquidityIx = createMintToInstruction(
      ecosystem.tokenAMint.publicKey,
      groupAdmin.tokenAAccount,
      globalProgramAdmin.wallet.publicKey,
      BigInt(seedAmount.toString()),
    );
    await processBankrunTransaction(
      ctx,
      new Transaction().add(mintSeedLiquidityIx),
      [globalProgramAdmin.wallet],
      false,
      true,
    );

    const seedTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: borrowBank,
        tokenAccount: groupAdmin.tokenAAccount,
        amount: seedAmount,
      }),
    );
    await processBankrunTransaction(ctx, seedTx, [groupAdmin.wallet]);
  });

  it("(attacker) pulse bank price with wrong lending state - should fail", async () => {
    const user = users[3];
    // Use a random pubkey as "wrong" lending state
    const wrongLending = Keypair.generate().publicKey;
    const tx = new Transaction().add(
      await pulseBankPrice(user.mrgnBankrunProgram, {
        group: juplendGroup.publicKey,
        bank: fixedJuplendBank,
        remaining: [wrongLending],
      }),
    );
    const result = await processBankrunTransaction(
      ctx,
      tx,
      [user.wallet],
      true,
    );
    // JuplendLendingValidationFailed = 6501
    assertBankrunTxFailed(result, 6501);
  });

  it("(user 3) deposit into fixed JupLend bank - happy path", async () => {
    const user = users[3];
    const depositAmount = new BN(1_000 * 10 ** ecosystem.usdcDecimals);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh JupLend rates to ensure lending state is fresh
    const updateRateTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
    );
    updateRateTx.add(
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
    );
    await processBankrunTransaction(ctx, updateRateTx, [user.wallet]);

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    userUsdcStart = userUsdcBefore;

    const tx = new Transaction().add(
      await makeJuplendDepositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: fixedJuplendBank,
        signerTokenAccount: user.usdcAccount,
        pool,
        amount: depositAmount,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    const diff = userUsdcBefore - userUsdcAfter;
    if (verbose) {
      console.log("deposited: " + diff.toLocaleString());
    }
    assert.equal(userUsdcBefore - userUsdcAfter, depositAmount.toNumber());
  });

  it("(user 3) borrow Token A against fixed JupLend collateral - happy path", async () => {
    const user = users[3];

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh JupLend rates
    const updateRateTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
    );
    updateRateTx.add(
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
    );
    await processBankrunTransaction(ctx, updateRateTx, [user.wallet]);

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount,
    );

    // FixedJuplend remaining: [bank, lendingState] (only 2 accounts, no oracle)
    const remaining = composeRemainingAccounts([
      [fixedJuplendBank, pool.lending],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        remaining,
        amount: BORROW_AMOUNT,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      user.tokenAAccount,
    );
    assert.equal(userTokenAAfter - userTokenABefore, BORROW_AMOUNT.toNumber());
  });

  it("(user 3) health pulse reports expected valuation", async () => {
    const user = users[3];
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh JupLend rates
    const updateRateTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
    );
    updateRateTx.add(
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
    );
    await processBankrunTransaction(ctx, updateRateTx, [user.wallet]);

    const remaining = composeRemainingAccounts([
      [fixedJuplendBank, pool.lending],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const cache = accAfter.healthCache;
    if (verbose) {
      logHealthCache("cache after deposit", cache);
    }

    const actualAssetValue = wrappedI80F48toBigNumber(
      cache.assetValue,
    ).toNumber();
    const actualLiabilityValue = wrappedI80F48toBigNumber(
      cache.liabilityValue,
    ).toNumber();

    // Fixed price applied to the deposit amount, weighted by assetWeightInit (0.8).
    // The exchange rate is ~1.0 initially, so asset value ≈ FIXED_PRICE * 1000 * 0.8
    const ASSET_WEIGHT_INIT = 0.8; // from defaultJuplendBankConfig
    const expectedAssetValue = FIXED_PRICE * 1000 * ASSET_WEIGHT_INIT;
    // 10 tokens (at high price bias)
    const expectedLiabilityValue =
      oracles.tokenAPrice * (1 + CONF_INTERVAL_MULTIPLE_FLOAT) * 10;

    const assetTolerance = Math.max(0.01, expectedAssetValue * 0.005);
    const liabTolerance = Math.max(0.01, expectedLiabilityValue * 0.005);

    assert.approximately(actualAssetValue, expectedAssetValue, assetTolerance);
    assert.approximately(
      actualLiabilityValue,
      expectedLiabilityValue,
      liabTolerance,
    );
  });

  it("(user 3) withdraw from fixed JupLend bank - happy path", async () => {
    const user = users[3];
    const withdrawAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh JupLend rates
    const updateRateTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
    );
    updateRateTx.add(
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
    );
    await processBankrunTransaction(ctx, updateRateTx, [user.wallet]);

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        ecosystem.usdcMint.publicKey,
        pool.tokenProgram,
      );
    await processBankrunTransaction(
      ctx,
      new Transaction().add(createWithdrawIntermediaryAtaIx),
      [user.wallet],
    );

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );

    const remaining = composeRemainingAccounts([
      [fixedJuplendBank, pool.lending],
      [borrowBank, oracles.tokenAOracle.publicKey],
    ]);

    const tx = new Transaction().add(
      await makeJuplendWithdrawSimpleIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: fixedJuplendBank,
        destinationTokenAccount: user.usdcAccount,
        pool,
        amount: withdrawAmount,
        remainingAccounts: remaining,
      }),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    const diff = userUsdcAfter - userUsdcBefore;
    if (verbose) {
      console.log("withdrew: " + diff.toLocaleString());
    }

    // JupLend withdraw should return approximately the requested amount
    assert.approximately(diff, withdrawAmount.toNumber(), 2);
  });

  it("(user 3) repay borrow and withdraw all - gets initial deposit back", async () => {
    const user = users[3];

    const repayTx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: borrowBank,
        tokenAccount: user.tokenAAccount,
        amount: BORROW_AMOUNT,
        repayAll: true,
        remaining: composeRemainingAccounts([
          [fixedJuplendBank, pool.lending],
          [borrowBank, oracles.tokenAOracle.publicKey],
        ]),
      }),
    );
    await processBankrunTransaction(ctx, repayTx, [user.wallet]);

    const remaining = composeRemainingAccounts(
      [
        [fixedJuplendBank, pool.lending],
        [borrowBank, oracles.tokenAOracle.publicKey],
      ].filter((group) => !group[0].equals(fixedJuplendBank))
    );

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    // Refresh JupLend rates
    const updateRateTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool }),
    );
    updateRateTx.add(
      dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
    );
    await processBankrunTransaction(ctx, updateRateTx, [user.wallet]);

    const withdrawAllTx = new Transaction().add(
      await makeJuplendWithdrawSimpleIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: fixedJuplendBank,
        destinationTokenAccount: user.usdcAccount,
        pool,
        amount: new BN(0),
        withdrawAll: true,
        remainingAccounts: remaining,
      }),
    );
    await processBankrunTransaction(ctx, withdrawAllTx, [user.wallet]);

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount,
    );
    // Note: JupLend round-trip rounding can lose a few lamports per operation
    assert.approximately(userUsdcAfter, userUsdcStart, 5);
    assert.isAtMost(userUsdcAfter, userUsdcStart);
  });
});
