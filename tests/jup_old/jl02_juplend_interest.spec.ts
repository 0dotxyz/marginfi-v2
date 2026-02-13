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
import { processBankrunTransaction } from "./utils/tools";
import { accountInit } from "./utils/user-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { ASSET_TAG_JUPLEND, ONE_WEEK_IN_SECONDS } from "./utils/types";

import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import {
  ensureJuplendPoolForMint,
  ensureJuplendClaimAccount,
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
  makeJuplendWithdrawIx,
} from "./utils/juplend/juplend-test-env";
import {
  JUPLEND_LIQUIDITY_PROGRAM_ID,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "./utils/juplend/juplend-pdas";
import {
  decodeJuplendLendingState,
  expectedSharesForDeposit,
  expectedSharesForWithdraw,
} from "./utils/juplend/juplend-state";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

describe("Juplend integration: interest accrual + exact share maths", () => {
  // Deterministic group so we can rerun tests without collisions.
  const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000001"); // 32 bytes
  const BANK_SEED = new BN(1);

  // Test amounts (USDC uses 6 decimals in our test harness).
  const FUND_AMOUNT = new BN(1_000 * 10 ** ecosystem.usdcDecimals);
  const SEED_DEPOSIT_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals);
  const DEPOSIT_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const BORROW_AMOUNT = new BN(50 * 10 ** ecosystem.usdcDecimals);
  const WITHDRAW_AMOUNT = new BN(10 * 10 ** ecosystem.usdcDecimals);

  const juplendGroup = Keypair.fromSeed(GROUP_SEED);

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];
  const borrower = Keypair.generate();

  let pool: JuplendPoolKeys;
  let juplendBank: PublicKey;
  let userMarginfiAccount: Keypair;

  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;
  let claimAccount: PublicKey;

  let borrowerUsdcAta: PublicKey;

  let mintedShares1 = 0n;
  let mintedShares2 = 0n;

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // 1) Ensure the underlying JupLend pool exists in bankrun (liquidity + rewards + lending).
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // Derive all Marginfi-side PDAs for this juplend bank.
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      fTokenMint: pool.fTokenMint,
    });

    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;
    claimAccount = derived.claimAccount;

    // 2) Create a borrower ATA (we will borrow from Liquidity to create utilization).
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

    // 3) Fund admin + user with USDC for deposits.
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

    userA.accounts.set("jl_acc", userMarginfiAccount.publicKey);
  });

  it("(admin) add juplend bank", async () => {
    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    const addBankIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      oracle: oracles.usdcOracle.publicKey,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const fetchedBank = await bankrunProgram.account.bank.fetch(juplendBank);
    assert.equal(fetchedBank.config.assetTag, ASSET_TAG_JUPLEND);
    assert.isTrue(
      Object.keys(fetchedBank.config.oracleSetup).includes("juplendPythPull")
    );

    if (verbose) {
      console.log("*juplend bank:", juplendBank.toBase58());
      console.log("  lending:", pool.lending.toBase58());
    }
  });

  it("(admin) activate bank via init-position (seed deposit)", async () => {

    const initIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: groupAdmin.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      fTokenVault,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const bank = await bankrunProgram.account.bank.fetch(juplendBank);
    assert.isTrue(Object.keys(bank.config.operationalState).includes("operational"));
  });

  it("(user) deposit (before interest accrual)", async () => {
    const acc = userA.accounts.get("jl_acc");
    assert.isTrue(acc !== undefined);

    const fTokenBefore = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: acc,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      signerTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      fTokenVault,
      amount: DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfter = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    mintedShares1 = fTokenAfter - fTokenBefore;

    // Sanity: minted shares match the current exchange rate (floor division).
    const lendingAccInfo = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingAccInfo);
    const lendingState = decodeJuplendLendingState(lendingAccInfo!.data);

    const expected = expectedSharesForDeposit(
      BigInt(DEPOSIT_AMOUNT.toString()),
      lendingState.tokenExchangePrice
    );

    assert.equal(mintedShares1.toString(), expected.toString());
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

    // Create borrower protocol positions.
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

    // Make borrower an active user class (1).
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

    // Borrow directly from liquidity (transfer_type = direct).
    const borrowerBalanceBefore = await getTokenBalance(bankRunProvider, borrowerUsdcAta);

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
        { direct: {} } // transfer_type
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

    const borrowerBalanceAfter = await getTokenBalance(bankRunProvider, borrowerUsdcAta);
    assert.equal(
      borrowerBalanceAfter - borrowerBalanceBefore,
      BORROW_AMOUNT.toNumber()
    );
  });

  it("(warp) accrue interest and refresh exchange rates", async () => {
    const { lending } = getJuplendPrograms();

    const lendingInfoBefore = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfoBefore);
    const lendingBefore = decodeJuplendLendingState(lendingInfoBefore!.data);

    // Advance time by ~1 week.
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

    // Also advance the slot so Pyth pull oracles get a new posted slot.
    const slotsToAdvance = BigInt(Math.floor(ONE_WEEK_IN_SECONDS * 0.4));
    bankrunContext.warpToSlot(now.slot + slotsToAdvance);

    // Refresh oracle timestamps for any health-check paths that run after the warp.
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Refresh liquidity exchange price (permissionless). This should apply time-delta interest.
    const refreshLiquidityIx = await makeJuplendUpdateExchangePriceIx({
      mint: ecosystem.usdcMint.publicKey,
      rateModel: pool.rateModel,
      tokenReserve: pool.tokenReserve,
    });

    // Refresh lending exchange price (permissionless). This should update token_exchange_price.
    const refreshLendingIx = await lending.methods
      .updateRate()
      .accounts({
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      })
      .instruction();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshLiquidityIx, refreshLendingIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const lendingInfoAfter = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingInfoAfter);
    const lendingAfter = decodeJuplendLendingState(lendingInfoAfter!.data);

    // We expect the exchange price to increase with time+utilization.
    assert.isTrue(lendingAfter.tokenExchangePrice > lendingBefore.tokenExchangePrice);
  });

  it("(user) deposit again (after interest accrual) and see fewer shares minted", async () => {
    const acc = userA.accounts.get("jl_acc");
    assert.isTrue(acc !== undefined);

    const fTokenBefore = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: acc,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      signerTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      fTokenVault,
      amount: DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfter = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    mintedShares2 = fTokenAfter - fTokenBefore;

    // Minted shares should be lower for the same deposit amount after interest accrual.
    assert.isTrue(mintedShares2 < mintedShares1);

    // Sanity: minted shares match the current exchange rate.
    const lendingAccInfo = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingAccInfo);
    const lendingState = decodeJuplendLendingState(lendingAccInfo!.data);

    const expected = expectedSharesForDeposit(
      BigInt(DEPOSIT_AMOUNT.toString()),
      lendingState.tokenExchangePrice
    );

    assert.equal(mintedShares2.toString(), expected.toString());
  });

  it("(user) withdraw works with post-interest exchange rate maths", async () => {
    const acc = userA.accounts.get("jl_acc");
    assert.isTrue(acc !== undefined);

    // Create the claim account for liquidity_vault_authority before first withdraw.
    await ensureJuplendClaimAccount({
      payer: userA.wallet,
      user: liquidityVaultAuthority,
      mint: ecosystem.usdcMint.publicKey,
    });

    const fTokenBefore = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const userBalanceBefore = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const remainingAccounts = juplendHealthRemainingAccounts(
      juplendBank,
      oracles.usdcOracle.publicKey,
      pool.lending
    );

    const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: acc,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      destinationTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      underlyingOracle: oracles.usdcOracle.publicKey,
      pool,
      fTokenVault,
      claimAccount,
      amount: WITHDRAW_AMOUNT,
      remainingAccounts,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawIx),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfter = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const userBalanceAfter = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const burnedShares = fTokenBefore - fTokenAfter;
    assert.equal(userBalanceAfter - userBalanceBefore, WITHDRAW_AMOUNT.toNumber());

    // Verify burned shares are what we expect for the *current* exchange rate.
    const lendingAccInfo = await bankRunProvider.connection.getAccountInfo(pool.lending);
    assert.ok(lendingAccInfo);
    const lendingState = decodeJuplendLendingState(lendingAccInfo!.data);

    const expectedBurn = expectedSharesForWithdraw(
      BigInt(WITHDRAW_AMOUNT.toString()),
      lendingState.tokenExchangePrice
    );

    assert.equal(burnedShares.toString(), expectedBurn.toString());
  });
});
