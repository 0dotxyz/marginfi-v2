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
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

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
import { ASSET_TAG_JUPLEND, ONE_WEEK_IN_SECONDS } from "./utils/types";

/** deterministic (32 bytes) */
const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000004");

describe("jl04: JupLend oracle price conversion tracks token_exchange_price", () => {
  const juplendGroup = Keypair.fromSeed(GROUP_SEED);
  const BANK_SEED = new BN(1);

  // Test amounts (USDC has 6 decimals)
  const FUND_AMOUNT = new BN(2_000 * 10 ** ecosystem.usdcDecimals);
  const SEED_DEPOSIT_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals);
  const USER_DEPOSIT_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const BORROW_AMOUNT = new BN(50 * 10 ** ecosystem.usdcDecimals);

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];
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

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // Ensure the protocol pool exists in bankrun.
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
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
    borrowerUsdcAta = getAssociatedTokenAddressSync(
      ecosystem.usdcMint.publicKey,
      borrower.publicKey
    );

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

  it("(admin) add juplend bank + activate via init_position", async () => {
    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    // Step 1: Add the JupLend bank
    const addBankIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      oracle: oracles.usdcOracle.publicKey,
      integrationAcc1: pool.lending,
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
    assert.isTrue(
      Object.keys(fetchedBank.config.operationalState).includes("operational")
    );
  });

  it("(user) deposit into the juplend bank", async () => {
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
      .initNewProtocol(
        ecosystem.usdcMint.publicKey,
        ecosystem.usdcMint.publicKey,
        borrower.publicKey
      )
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

    // Allow borrower to borrow.
    // Note: max_debt_ceiling cannot exceed 10x token total supply per JupLend program validation
    const LAMPORTS_PER_SOL = 1_000_000_000;
    const DEFAULT_PERCENT_PRECISION = new BN(100);
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

  it("captures price before and after interest accrual (ratio matches token_exchange_price)", async () => {
    // -------------------------------------------------
    // 1) First pulse (fresh update_rate in same tx)
    // -------------------------------------------------
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

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

    price1 = bytesToF64(accAfterPulse1.healthCache.prices[0]);

    const lendingInfo1 = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfo1);
    exchange1 = decodeJuplendLendingState(lendingInfo1!.data).tokenExchangePrice;

    // -------------------------------------------------
    // 2) Warp + refresh exchange rates
    // -------------------------------------------------
    const now = await banksClient.getClock();
    const targetTs = now.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);

    bankrunContext.setClock(
      new Clock(
        now.slot,
        now.epochStartTimestamp,
        now.epoch,
        now.leaderScheduleEpoch,
        targetTs
      )
    );

    // Warp slots so Pyth pull oracles can be refreshed in a newer slot.
    const slotsToAdvance = BigInt(Math.floor(ONE_WEEK_IN_SECONDS * 0.4));
    bankrunContext.warpToSlot(now.slot + slotsToAdvance);

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

    // Same-slot requirement: refresh + pulse must be in one transaction.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshLiquidityIx, updateRateIx2, pulseIx2),
      [groupAdmin.wallet],
      false,
      true
    );

    const accAfterPulse2 = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );

    price2 = bytesToF64(accAfterPulse2.healthCache.prices[0]);

    const lendingInfo2 = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfo2);
    exchange2 = decodeJuplendLendingState(lendingInfo2!.data).tokenExchangePrice;

    // -------------------------------------------------
    // 3) Assertions
    // -------------------------------------------------
    assert.isTrue(exchange2 > exchange1, "token_exchange_price should increase");
    assert.isTrue(price2 > price1, "oracle price should increase with exchange rate");

    const observedRatio = price2 / price1;
    const expectedRatio = Number(exchange2) / Number(exchange1);
    const relErr = Math.abs(observedRatio - expectedRatio) / expectedRatio;

    if (verbose) {
      console.log("exchange1:", exchange1.toString());
      console.log("exchange2:", exchange2.toString());
      console.log("price1:", price1);
      console.log("price2:", price2);
      console.log("observedRatio:", observedRatio);
      console.log("expectedRatio:", expectedRatio);
      console.log("relErr:", relErr);
    }

    assert.isBelow(
      relErr,
      1e-6,
      "oracle conversion ratio should match token_exchange_price ratio"
    );

    // -------------------------------------------------
    // 4) Verify exact dollar value of collateral
    // -------------------------------------------------
    // This proves the on-chain health calculation assigns the correct absolute
    // dollar value to the user's JupLend collateral, including accrued interest.
    //
    // The health cache price already includes:
    //   price = oracle_price × confidence_adj × token_exchange_price / 1e12
    //
    // So we can derive the confidence-adjusted oracle price from price2:
    //   oracle_conf_adj = price2 × 1e12 / exchange2
    //
    // Formula: assetValue = deposit × (E2/E1) × oracle_conf_adj × asset_weight
    const ASSET_WEIGHT_INIT = 0.8; // from defaultJuplendBankConfig
    const DEPOSIT_IN_WHOLE_TOKENS = 100; // USER_DEPOSIT_AMOUNT = 100 USDC
    const EXCHANGE_PRECISION = 1e12;

    // Derive the confidence-adjusted oracle price from health cache
    const oracleConfAdj = price2 * EXCHANGE_PRECISION / Number(exchange2);

    // After interest, the underlying value has grown by the exchange rate ratio
    const exchangeRatio = Number(exchange2) / Number(exchange1);
    const underlyingValueAfterInterest = DEPOSIT_IN_WHOLE_TOKENS * exchangeRatio;

    // Expected weighted collateral value including oracle confidence adjustment
    const expectedAssetValue =
      underlyingValueAfterInterest * oracleConfAdj * ASSET_WEIGHT_INIT;

    const actualAssetValue = wrappedI80F48toBigNumber(
      accAfterPulse2.healthCache.assetValue
    ).toNumber();

    if (verbose) {
      console.log("oracleConfAdj:", oracleConfAdj);
      console.log("exchangeRatio:", exchangeRatio);
      console.log("underlyingValueAfterInterest:", underlyingValueAfterInterest);
      console.log("expectedAssetValue:", expectedAssetValue);
      console.log("actualAssetValue:", actualAssetValue);
    }

    // Tight tolerance - should match exactly (within floating point precision)
    const tolerance = expectedAssetValue * 0.0001; // 0.01% tolerance
    assert.approximately(
      actualAssetValue,
      expectedAssetValue,
      tolerance,
      `Collateral dollar value should be ~$${expectedAssetValue.toFixed(2)} ` +
        `(100 USDC × ${exchangeRatio.toFixed(6)} yield × ${oracleConfAdj.toFixed(4)} oracle × 0.8 weight)`
    );
  });
});
