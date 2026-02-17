import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";

import {
  bankRunProvider,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { accountInit, composeRemainingAccounts, healthPulse } from "./utils/user-instructions";
import { bytesToF64, processBankrunTransaction } from "./utils/tools";

import {
  ensureJuplendPoolForMint,
  getJuplendPrograms,
  makeJuplendUpdateExchangePriceIx,
  JuplendPoolKeys,
} from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/juplend-test-env";
import {
  JUPLEND_LIQUIDITY_PROGRAM_ID,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "./utils/juplend/juplend-pdas";
import { juplendUpdateRateIx } from "./utils/juplend/juplend-instructions";
import { decodeJuplendLendingState } from "./utils/juplend/juplend-state";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import {
  ASSET_TAG_JUPLEND,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  ONE_WEEK_IN_SECONDS,
} from "./utils/types";
import {
  createBankrunSwitchboardPullFeedAccount,
  refreshPullOraclesBankrun,
  refreshSwitchboardPullOracleBankrun,
  setSwitchboardPullFeedAccountData,
  SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN,
} from "./utils/bankrun-oracles";

/** Deterministic seed (32 bytes) - use 12 to avoid conflicts */
const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000012");

/** Deterministic seed (32 bytes) */
const SWB_FEED_SEED = Buffer.from("SWB_FEED_SEED_JUPLEND_USDC_00000");

/**
 * Switchboard Pull feed fixture (hex) copied from:
 * `programs/marginfi/src/state/price.rs` (unit test `swb_pull_get_price_1`)
 *
 * - Owner must be `SWITCHBOARD_PULL_PROGRAM_ID` (see bankrun-oracles.ts)
 * - Discriminator is included (first 8 bytes)
 * - Price is ~155.59, conf is ~0.47 (exactness not important; we check ratios)
 */
const SWB_FEED_HEX =
  "c41b6cc40ad7db286f5e7566ac000a9530e56b1db49585772719aeaaeeadb4d9bd8c2357b88e9e782e53d81000000000000000000000000000985f538057856308000000000000005cba953f3f15356b17703e554d3983801916531d7976aa424ad64348ec50e4224650d81000000000000000000000000000a0d5a780cc7f580800000000000000a20b742cedab55efd1faf60aef2cb872a092d24dfba8a48c8b953a5e90ac7bbf874ed81000000000000000000000000000c04958360093580800000000000000e7ef024ea756f8beec2eaa40234070da356754a8eeb2ac6a17c32d17c3e99f8ddc50d81000000000000000000000000000bc8739b45d215b0800000000000000e3e5130902c3e9c27917789769f1ae05de15cf504658beafeed2c598a949b3b7bf53d810000000000000000000000000007cec168c94d667080000000000000020e270b743473d87eff321663e267ba1c9a151f7969cef8147f625e9a2af7287ea54d81000000000000000000000000000dc65eccc174d6f0800000000000000ab605484238ac93f225c65f24d7705bb74b00cdb576555c3995e196691a4de5f484ed8100000000000000000000000000088f28dc9271d59080000000000000015196392573dc9043242716f629d4c0fb93bc0cff7a1a10ede24281b0e98fb7d5454d810000000000000000000000000000441a10ca4a268080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000048ac38271f28ab1b12e49439bddf54871094e4832a56c7a8ec57bd18d357980086807068432f186a147cf0b13a30067d386204ea9d6c8b04743ac2ef010b07524c935636f2523f6aeeb6dc7b7dab0e86a13ff2c794f7895fc78851d69fdb593bdccdb36600000000000000000000000000e40b540200000001000000534f4c2f55534400000000000000000000000000000000000000000000000000000000019e9eb66600000000fca3d11000000000000000000000000000000000000000000000000000000000000000000000000000dc65eccc174d6f0800000000000000006c9225e039550300000000000000000070d3c6ecddf76b080000000000000000d8244bc073aa060000000000000000000441a10ca4a268080000000000000000dc65eccc174d6f08000000000000000200000000000000ea54d810000000005454d81000000000ea54d81000000000fa0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

describe("jl12: JupLend Switchboard Pull oracle conversion tracks token_exchange_price", () => {
  const juplendGroup = Keypair.fromSeed(GROUP_SEED);
  const BANK_SEED = new BN(1);

  // Test amounts (USDC has 6 decimals)
  const FUND_AMOUNT = new BN(2_000 * 10 ** ecosystem.usdcDecimals);
  const SEED_DEPOSIT_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals);
  const USER_DEPOSIT_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const BORROW_AMOUNT = new BN(50 * 10 ** ecosystem.usdcDecimals);

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: (typeof users)[0];
  const borrower = Keypair.generate();

  let pool: JuplendPoolKeys;
  let juplendBank: PublicKey;
  let userMarginfiAccount: Keypair;

  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;

  let borrowerUsdcAta: PublicKey;

  // Values we compare
  let price1: number;
  let price2: number;
  let exchange1: bigint;
  let exchange2: bigint;

  // Switchboard feed (owned by Switchboard on-demand program id)
  const switchboardFeed = Keypair.fromSeed(SWB_FEED_SEED);

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // -----------------------------------------------------------------------
    // Switchboard pull feed setup (pure account data, no CPI)
    // -----------------------------------------------------------------------
    const swbBytesRaw = Buffer.from(SWB_FEED_HEX, "hex");

    // Ensure buffer is at least the minimum size expected by Marginfi
    const swbBytes =
      swbBytesRaw.length >= SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN
        ? swbBytesRaw
        : Buffer.concat([
            swbBytesRaw,
            Buffer.alloc(SWITCHBOARD_PULL_MIN_FEED_ACCOUNT_DATA_LEN - swbBytesRaw.length),
          ]);

    await createBankrunSwitchboardPullFeedAccount(
      bankrunContext,
      banksClient,
      switchboardFeed,
      swbBytes.length
    );

    await setSwitchboardPullFeedAccountData(
      bankrunContext,
      banksClient,
      switchboardFeed.publicKey,
      swbBytes
    );

    // Bring last_update_timestamp to the current bankrun clock so staleness behaves realistically.
    await refreshSwitchboardPullOracleBankrun(bankrunContext, banksClient, switchboardFeed.publicKey);

    // Ensure the protocol pool exists in bankrun.
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC_swb",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // Derive Marginfi PDAs for this bank.
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      fTokenMint: pool.fTokenMint,
      tokenProgram: pool.tokenProgram,
    });

    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;

    // Borrower ATA to create utilization in the Liquidity program.
    borrowerUsdcAta = getAssociatedTokenAddressSync(ecosystem.usdcMint.publicKey, borrower.publicKey);

    const createBorrowerAtaIx = createAssociatedTokenAccountInstruction(
      groupAdmin.wallet.publicKey,
      borrowerUsdcAta,
      borrower.publicKey,
      ecosystem.usdcMint.publicKey
    );

    // Fund user + admin for seed deposit.
    const fundAdminIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      groupAdmin.usdcAccount,
      globalProgramAdmin.wallet.publicKey,
      FUND_AMOUNT.toNumber()
    );

    const fundUserIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      userA.usdcAccount,
      globalProgramAdmin.wallet.publicKey,
      FUND_AMOUNT.toNumber()
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createBorrowerAtaIx, fundAdminIx, fundUserIx),
      [groupAdmin.wallet, globalProgramAdmin.wallet],
      false,
      true
    );
  });

  it("(admin) init marginfi group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );
  });

  it("(user) init marginfi account", async () => {
    userMarginfiAccount = Keypair.generate();

    const ix = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [userA.wallet, userMarginfiAccount],
      false,
      true
    );
  });

  it("(admin) add juplend bank (switchboard) + activate via init_position", async () => {
    // Refresh oracle to ensure it's fresh for bank creation
    await refreshSwitchboardPullOracleBankrun(bankrunContext, banksClient, switchboardFeed.publicKey);

    const config = defaultJuplendBankConfig(switchboardFeed.publicKey, ecosystem.usdcDecimals);

    // Switchboard oracle setup for this bank
    config.oracleSetup = { juplendSwitchboardPull: {} };
    // Use realistic max age (seconds) and refresh the feed timestamp in bankrun (like Pyth Pull).
    config.oracleMaxAge = 60;

    // Step 1: Add the JupLend bank
    const addBankIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      oracle: switchboardFeed.publicKey,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
      tokenProgram: pool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Step 2: Activate via init_position (bank must exist first so we can fetch its group)
    const initIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: groupAdmin.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const fetchedBank = await bankrunProgram.account.bank.fetch(juplendBank);
    assert.equal(fetchedBank.config.assetTag, ASSET_TAG_JUPLEND);
    assert.isTrue(Object.keys(fetchedBank.config.operationalState).includes("operational"));
  });

  it("(user) deposit into the juplend bank", async () => {
    // Refresh oracle before deposit
    await refreshSwitchboardPullOracleBankrun(bankrunContext, banksClient, switchboardFeed.publicKey);

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      signerTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: USER_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );
  });

  it("(setup) borrow from Liquidity to create utilization", async () => {
    const { liquidity } = getJuplendPrograms();

    const [borrowerSupplyPos] = findJuplendLiquiditySupplyPositionPda(
      ecosystem.usdcMint.publicKey,
      borrower.publicKey,
      JUPLEND_LIQUIDITY_PROGRAM_ID
    );

    const [borrowerBorrowPos] = findJuplendLiquidityBorrowPositionPda(
      ecosystem.usdcMint.publicKey,
      borrower.publicKey,
      JUPLEND_LIQUIDITY_PROGRAM_ID
    );

    const initBorrowerIx = await liquidity.methods
      .initNewProtocol(ecosystem.usdcMint.publicKey, ecosystem.usdcMint.publicKey, borrower.publicKey)
      .accounts({
        authority: groupAdmin.wallet.publicKey,
        authList: pool.authList,
        userSupplyPosition: borrowerSupplyPos,
        userBorrowPosition: borrowerBorrowPos,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const setBorrowerClassIx = await liquidity.methods
      .updateUserClass([{ addr: borrower.publicKey, value: 1 }])
      .accounts({
        authority: groupAdmin.wallet.publicKey,
        authList: pool.authList,
      })
      .instruction();

    const DEFAULT_PERCENT_PRECISION = new BN(100);
    const LAMPORTS_PER_SOL = 1e9;
    const borrowerBorrowCfg = {
      mode: 1,
      expandPercent: new BN(20).mul(DEFAULT_PERCENT_PRECISION), // 20%
      expandDuration: new BN(2 * 24 * 60 * 60), // 2 days
      baseDebtCeiling: new BN(1e4 * LAMPORTS_PER_SOL), // 10k SOL worth
      maxDebtCeiling: new BN(1e6 * LAMPORTS_PER_SOL), // 1M SOL worth
    };

    const setBorrowerBorrowCfgIx = await liquidity.methods
      .updateUserBorrowConfig(borrowerBorrowCfg)
      .accounts({
        authority: groupAdmin.wallet.publicKey,
        protocol: borrower.publicKey,
        authList: pool.authList,
        rateModel: pool.rateModel,
        mint: ecosystem.usdcMint.publicKey,
        tokenReserve: pool.tokenReserve,
        userBorrowPosition: borrowerBorrowPos,
      })
      .instruction();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initBorrowerIx, setBorrowerClassIx, setBorrowerBorrowCfgIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const preOperateIx = await liquidity.methods
      .preOperate(ecosystem.usdcMint.publicKey)
      .accounts({
        protocol: borrower.publicKey,
        liquidity: pool.liquidity,
        userSupplyPosition: borrowerSupplyPos,
        userBorrowPosition: borrowerBorrowPos,
        vault: pool.vault,
        tokenReserve: pool.tokenReserve,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: pool.tokenProgram,
      })
      .instruction();

    const operateIx = await liquidity.methods
      .operate(
        new BN(0), // supply_amount
        BORROW_AMOUNT, // borrow_amount
        borrower.publicKey, // withdraw_to (unused)
        borrower.publicKey, // borrow_to
        { direct: {} }
      )
      .accounts({
        protocol: borrower.publicKey,
        liquidity: pool.liquidity,
        tokenReserve: pool.tokenReserve,
        mint: ecosystem.usdcMint.publicKey,
        vault: pool.vault,
        userSupplyPosition: borrowerSupplyPos,
        userBorrowPosition: borrowerBorrowPos,
        rateModel: pool.rateModel,
        borrowToAccount: borrowerUsdcAta,
        borrowClaimAccount: null,
        withdrawClaimAccount: null,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: pool.tokenProgram,
      })
      .instruction();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(preOperateIx, operateIx),
      [groupAdmin.wallet, borrower],
      false,
      true
    );
  });

  it("captures price before and after interest accrual (ratio matches token_exchange_price) and enforces Switchboard staleness", async () => {
    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(juplendBank, switchboardFeed.publicKey, pool.lending),
    ]);

    // -------------------------------------------------
    // 1) First pulse (fresh oracle + fresh update_rate in same tx)
    // -------------------------------------------------
    await refreshSwitchboardPullOracleBankrun(bankrunContext, banksClient, switchboardFeed.publicKey);

    const updateRateIx1 = juplendUpdateRateIx({
      lending: pool.lending,
      mint: ecosystem.usdcMint.publicKey,
      fTokenMint: pool.fTokenMint,
      supplyTokenReservesLiquidity: pool.tokenReserve,
      rewardsRateModel: pool.lendingRewardsRateModel,
    });

    const pulseIx1 = await healthPulse(groupAdmin.mrgnBankrunProgram, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(updateRateIx1, pulseIx1),
      [groupAdmin.wallet],
      false,
      true
    );

    const accAfterPulse1 = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );

    assert.equal(accAfterPulse1.healthCache.internalErr, 0, "expected no oracle error on fresh Switchboard feed");
    assert.ok(
      (accAfterPulse1.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0,
      "expected oracle_ok on fresh Switchboard feed"
    );

    price1 = bytesToF64(accAfterPulse1.healthCache.prices[0]);

    const lendingInfo1 = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfo1);
    exchange1 = decodeJuplendLendingState(lendingInfo1!.data).tokenExchangePrice;

    // -------------------------------------------------
    // 2) Warp + refresh exchange rates
    // -------------------------------------------------
    const now = await banksClient.getClock();
    const targetTs = now.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);

    // Advance the clock timestamp (for interest accrual) and slot (for rate refresh).
    // NOTE: Use a small slot advance to avoid polluting state for subsequent tests.
    // Large slot advances break tests that warp to absolute slot numbers.
    const slotsToAdvance = BigInt(10);
    bankrunContext.setClock(
      new Clock(now.slot + slotsToAdvance, now.epochStartTimestamp, now.epoch, now.leaderScheduleEpoch, targetTs)
    );

    // CRITICAL: Refresh shared Pyth oracles to prevent state pollution for other test suites
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const refreshLiquidityIx = await makeJuplendUpdateExchangePriceIx({
      mint: ecosystem.usdcMint.publicKey,
      rateModel: pool.rateModel,
      tokenReserve: pool.tokenReserve,
    });

    const updateRateIx2 = juplendUpdateRateIx({
      lending: pool.lending,
      mint: ecosystem.usdcMint.publicKey,
      fTokenMint: pool.fTokenMint,
      supplyTokenReservesLiquidity: pool.tokenReserve,
      rewardsRateModel: pool.lendingRewardsRateModel,
    });

    const pulseIx2 = await healthPulse(groupAdmin.mrgnBankrunProgram, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    // -------------------------------------------------
    // 3) Pulse WITHOUT refreshing Switchboard oracle (should record stale error)
    // -------------------------------------------------
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshLiquidityIx, updateRateIx2, pulseIx2),
      [groupAdmin.wallet],
      false,
      true
    );

    const accAfterStalePulse = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );

    const SWITCHBOARD_STALE_ERR = 6049;
    assert.equal(
      accAfterStalePulse.healthCache.internalErr,
      SWITCHBOARD_STALE_ERR,
      "expected SwitchboardStalePrice (6049) when feed is out of date"
    );

    // health_pulse does not revert; it stores oracle errors in `internalErr`
    assert.ok(
      (accAfterStalePulse.healthCache.flags & HEALTH_CACHE_ENGINE_OK) !== 0,
      "engine_ok should remain true even when oracle is stale"
    );
    assert.equal(
      accAfterStalePulse.healthCache.flags & HEALTH_CACHE_ORACLE_OK,
      0,
      "oracle_ok should be false when oracle is stale"
    );

    // -------------------------------------------------
    // 4) Refresh Switchboard oracle bytes and pulse again (should succeed)
    // -------------------------------------------------
    await refreshSwitchboardPullOracleBankrun(bankrunContext, banksClient, switchboardFeed.publicKey);

    const updateRateIx3 = juplendUpdateRateIx({
      lending: pool.lending,
      mint: ecosystem.usdcMint.publicKey,
      fTokenMint: pool.fTokenMint,
      supplyTokenReservesLiquidity: pool.tokenReserve,
      rewardsRateModel: pool.lendingRewardsRateModel,
    });

    const pulseIx3 = await healthPulse(groupAdmin.mrgnBankrunProgram, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(updateRateIx3, pulseIx3),
      [groupAdmin.wallet],
      false,
      true
    );

    const accAfterPulse2 = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );

    assert.equal(accAfterPulse2.healthCache.internalErr, 0, "expected oracle error cleared after refresh");
    assert.ok(
      (accAfterPulse2.healthCache.flags & HEALTH_CACHE_HEALTHY) !== 0,
      "expected HEALTHY flag after refreshing Switchboard oracle"
    );
    assert.ok(
      (accAfterPulse2.healthCache.flags & HEALTH_CACHE_ORACLE_OK) !== 0,
      "expected oracle_ok flag after refreshing Switchboard oracle"
    );

    price2 = bytesToF64(accAfterPulse2.healthCache.prices[0]);

    const lendingInfo2 = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfo2);
    exchange2 = decodeJuplendLendingState(lendingInfo2!.data).tokenExchangePrice;

    // -------------------------------------------------
    // 5) Assertions
    // -------------------------------------------------
    assert.isTrue(exchange2 > exchange1, "token_exchange_price should increase");
    assert.isTrue(price2 > price1, "oracle price should increase with exchange rate");

    const observedRatio = price2 / price1;
    const expectedRatio = Number(exchange2) / Number(exchange1);
    const relErr = Math.abs(observedRatio - expectedRatio) / expectedRatio;

    // Sanity: implied underlying oracle price should be ~155.59 both times.
    const EXCHANGE_PRICES_PRECISION = 1_000_000_000_000; // 1e12
    const impliedUnderlying1 = price1 / (Number(exchange1) / EXCHANGE_PRICES_PRECISION);
    const impliedUnderlying2 = price2 / (Number(exchange2) / EXCHANGE_PRICES_PRECISION);

    assert.approximately(impliedUnderlying1, 155.59, 0.5);
    assert.approximately(impliedUnderlying2, 155.59, 0.5);

    if (verbose) {
      console.log("exchange1:", exchange1.toString());
      console.log("exchange2:", exchange2.toString());
      console.log("price1:", price1);
      console.log("price2:", price2);
      console.log("observedRatio:", observedRatio);
      console.log("expectedRatio:", expectedRatio);
      console.log("relErr:", relErr);
      console.log("impliedUnderlying1:", impliedUnderlying1);
      console.log("impliedUnderlying2:", impliedUnderlying2);
    }

    assert.isBelow(relErr, 1e-6, "oracle conversion ratio should match token_exchange_price ratio");
  });

  // NOTE: No after() hook - jl12 doesn't reset clock state.
  // jl12 only uses setClock (not warpToSlot) so it doesn't advance bankrun's internal
  // slot counter. The shared Pyth oracles are refreshed mid-test after the time warp.
});
