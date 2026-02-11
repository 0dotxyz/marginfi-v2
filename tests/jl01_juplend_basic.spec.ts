import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { assertKeysEqual, getTokenBalance } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import { accountInit } from "./utils/user-instructions";
import { ASSET_TAG_JUPLEND } from "./utils/types";

import { ensureJuplendPoolForMint, ensureJuplendClaimAccount } from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
  makeJuplendWithdrawIx,
} from "./utils/juplend/juplend-test-env";
import { decodeJuplendLendingState, expectedSharesForDeposit, expectedSharesForWithdraw } from "./utils/juplend/juplend-state";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";

import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

/** in mockUser.accounts, key used to get/set the users's account for the juplend group */
const USER_ACCOUNT_JL = "jl_acc";

/** deterministic (32 bytes) */
const JUPLEND_GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000000");

describe("jl01: JupLend - basic deposit/withdraw (bankrun)", () => {
  const juplendGroup = Keypair.fromSeed(JUPLEND_GROUP_SEED);

  const BANK_SEED = new BN(1);
  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)
  const USER_DEPOSIT_AMOUNT = new BN(10_000_000); // 10 USDC
  const USER_WITHDRAW_AMOUNT = new BN(10_000_000); // 10 USDC

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];

  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;

  let userMarginfiAccount: Keypair;
  let juplendBank: PublicKey;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;
  let claimAccount: PublicKey;

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // Mint USDC to userA for deposit testing
    const mintAmount = 1_000_000_000; // 1000 USDC (6 decimals)
    const mintIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      userA.usdcAccount,
      globalProgramAdmin.wallet.publicKey, // mint authority is payer
      mintAmount
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(mintIx),
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // Create / cache the on-chain JupLend pool for USDC.
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });
  });

  it("(admin) Initialize juplend marginfi group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });
    const tx = new Transaction().add(ix);

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );
  });

  it("(userA) Initialize marginfi account", async () => {
    userMarginfiAccount = Keypair.generate();
    const ix = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });
    const tx = new Transaction().add(ix);

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet, userMarginfiAccount],
      false,
      true
    );

    userA.accounts.set(USER_ACCOUNT_JL, userMarginfiAccount.publicKey);
  });

  it("(admin) Add USDC JupLend bank", async () => {
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

    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    const ix = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      oracle: oracles.usdcOracle.publicKey,
      integrationAcc1: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
    });

    const tx = new Transaction().add(ix);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      true
    );

    const bank = await bankrunProgram.account.bank.fetch(juplendBank);
    assertKeysEqual(bank.mint, ecosystem.usdcMint.publicKey);
    assertKeysEqual(bank.group, juplendGroup.publicKey);
    assert.equal(bank.config.assetTag, ASSET_TAG_JUPLEND);

    assertKeysEqual(bank.integrationAcc1, pool.lending);
    assertKeysEqual(bank.config.oracleKeys[0], oracles.usdcOracle.publicKey);
    assertKeysEqual(bank.config.oracleKeys[1], pool.lending);
    assert.ok(Object.keys(bank.config.oracleSetup).includes("juplendPythPull"));

    // Newly added JupLend banks are paused until activated via init_position
    assert.ok(Object.keys(bank.config.operationalState).includes("paused"));
  });

  it("(admin) Activate bank via juplend_init_position", async () => {
    const ix = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram!, {
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

    const tx = new Transaction().add(ix);
    await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      false,
      true
    );

    // Bank should now be operational.
    const bank = await bankrunProgram.account.bank.fetch(juplendBank);
    assert.ok(Object.keys(bank.config.operationalState).includes("operational"));

    // Seed deposit should have minted some fTokens into fTokenVault
    const fTokenBal = await getTokenBalance(bankRunProvider, fTokenVault);
    assert.isAbove(fTokenBal, 0);
  });

  it("(userA) Deposit & withdraw via marginfi <-> juplend", async () => {
    // ----------------------------
    // Deposit
    // ----------------------------
    const userUsdcBefore = await getTokenBalance(bankRunProvider, userA.usdcAccount);
    const fTokenVaultBeforeDeposit = BigInt(
      await getTokenBalance(bankRunProvider, fTokenVault)
    );

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,

      bank: juplendBank,
      signerTokenAccount: userA.usdcAccount,

      liquidityVaultAuthority,
      liquidityVault,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      fTokenVault,

      amount: USER_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );

    const userUsdcAfterDeposit = await getTokenBalance(
      bankRunProvider,
      userA.usdcAccount
    );
    assert.equal(
      userUsdcBefore - userUsdcAfterDeposit,
      USER_DEPOSIT_AMOUNT.toNumber(),
      "user USDC should decrease by deposit amount"
    );

    // Compute expected shares from on-chain lending state after deposit
    const lendingAccInfoAfterDeposit = await bankRunProvider.connection.getAccountInfo(
      pool.lending
    );
    assert.ok(lendingAccInfoAfterDeposit);
    const lendingAfterDeposit = decodeJuplendLendingState(
      lendingAccInfoAfterDeposit!.data
    );

    const expectedSharesMinted = expectedSharesForDeposit(
      BigInt(USER_DEPOSIT_AMOUNT.toString()),
      lendingAfterDeposit.tokenExchangePrice
    );

    const fTokenVaultAfterDeposit = BigInt(
      await getTokenBalance(bankRunProvider, fTokenVault)
    );
    const mintedShares = fTokenVaultAfterDeposit - fTokenVaultBeforeDeposit;
    assert.equal(
      mintedShares.toString(),
      expectedSharesMinted.toString(),
      "fToken vault delta should equal expected minted shares"
    );

    const userAccAfterDeposit = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );
    const jlBalanceAfterDeposit = userAccAfterDeposit.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(juplendBank)
    );
    assert.ok(jlBalanceAfterDeposit);

    const userAssetSharesAfterDeposit = BigInt(
      wrappedI80F48toBigNumber(jlBalanceAfterDeposit!.assetShares).toFixed(0)
    );
    assert.equal(
      userAssetSharesAfterDeposit.toString(),
      expectedSharesMinted.toString(),
      "marginfi asset shares should match minted shares"
    );

    // ----------------------------
    // Withdraw
    // ----------------------------

    // Create the claim account for liquidity_vault_authority before first withdraw.
    // This is permissionless and required despite IDL marking it as optional.
    await ensureJuplendClaimAccount({
      payer: userA.wallet,
      user: liquidityVaultAuthority,
      mint: ecosystem.usdcMint.publicKey,
    });

    const fTokenVaultBeforeWithdraw = BigInt(
      await getTokenBalance(bankRunProvider, fTokenVault)
    );
    const userUsdcBeforeWithdraw = await getTokenBalance(
      bankRunProvider,
      userA.usdcAccount
    );

    const remaining = juplendHealthRemainingAccounts(
      juplendBank,
      oracles.usdcOracle.publicKey,
      pool.lending
    );

    const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
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

      amount: USER_WITHDRAW_AMOUNT,
      remainingAccounts: remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawIx),
      [userA.wallet],
      false,
      true
    );

    const userUsdcAfterWithdraw = await getTokenBalance(
      bankRunProvider,
      userA.usdcAccount
    );
    assert.equal(
      userUsdcAfterWithdraw - userUsdcBeforeWithdraw,
      USER_WITHDRAW_AMOUNT.toNumber(),
      "user USDC should increase by withdraw amount"
    );

    // Compute expected shares burned from on-chain lending state after withdraw
    const lendingAccInfoAfterWithdraw = await bankRunProvider.connection.getAccountInfo(
      pool.lending
    );
    assert.ok(lendingAccInfoAfterWithdraw);
    const lendingAfterWithdraw = decodeJuplendLendingState(
      lendingAccInfoAfterWithdraw!.data
    );

    const expectedSharesBurned = expectedSharesForWithdraw(
      BigInt(USER_WITHDRAW_AMOUNT.toString()),
      lendingAfterWithdraw.tokenExchangePrice
    );

    const fTokenVaultAfterWithdraw = BigInt(
      await getTokenBalance(bankRunProvider, fTokenVault)
    );
    const burnedShares = fTokenVaultBeforeWithdraw - fTokenVaultAfterWithdraw;

    assert.equal(
      burnedShares.toString(),
      expectedSharesBurned.toString(),
      "fToken vault delta should equal expected burned shares"
    );

    const userAccAfterWithdraw = await bankrunProgram.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );
    const jlBalanceAfterWithdraw = userAccAfterWithdraw.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(juplendBank)
    );
    assert.ok(jlBalanceAfterWithdraw);

    const userAssetSharesAfterWithdraw = BigInt(
      wrappedI80F48toBigNumber(jlBalanceAfterWithdraw!.assetShares).toFixed(0)
    );
    assert.equal(
      (userAssetSharesAfterDeposit - userAssetSharesAfterWithdraw).toString(),
      expectedSharesBurned.toString(),
      "marginfi asset shares delta should match burned shares"
    );
  });
});
