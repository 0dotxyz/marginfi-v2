import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";

import {
  bankRunProvider,
  bankrunContext,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import { accountInit, healthPulse } from "./utils/user-instructions";
import { HEALTH_CACHE_ENGINE_OK, HEALTH_CACHE_HEALTHY, HEALTH_CACHE_ORACLE_OK, ONE_WEEK_IN_SECONDS } from "./utils/types";

import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import { ensureJuplendPoolForMint, JuplendPoolKeys } from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/juplend-test-env";
import { juplendUpdateRateIx } from "./utils/juplend/juplend-instructions";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

describe("Juplend integration: lending stale guard via health pulse", () => {
  const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000002");
  const BANK_SEED = new BN(1);

  const FUND_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const SEED_DEPOSIT_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals);
  const USER_DEPOSIT_AMOUNT = new BN(25 * 10 ** ecosystem.usdcDecimals);

  const juplendGroup = Keypair.fromSeed(GROUP_SEED);
  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];

  let pool: JuplendPoolKeys;
  let juplendBank: PublicKey;
  let userMarginfiAccount: Keypair;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // Stand up (or reuse) the protocol-side JupLend pool for USDC.
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // Create a fresh group for this spec.
    const initGroupIx = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initGroupIx),
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );

    // Derive Marginfi-side PDAs for the bank + vaults.
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: groupAdmin.mrgnBankrunProgram.programId,
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

    // Add a JupLend bank config to the group.
    const cfg = defaultJuplendBankConfig(
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
      config: cfg,
      tokenProgram: pool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Activate bank (init position + seed deposit).
    const initPositionIx = await makeJuplendInitPositionIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.usdcAccount,
        bank: juplendBank,
        liquidityVaultAuthority,
        liquidityVault,
        fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      }
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPositionIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Create a user marginfi account.
    userMarginfiAccount = Keypair.generate();
    const initAccountIx = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initAccountIx),
      [userA.wallet, userMarginfiAccount],
      false,
      true
    );
  });

  it("Health pulse fails when JupLend lending state is stale, and succeeds after update_rate in same tx", async () => {
    // Fund userA and deposit via marginfi.
    const mintIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      userA.usdcAccount,
      globalProgramAdmin.wallet.publicKey,
      FUND_AMOUNT.toNumber()
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(mintIx),
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // Track balance before deposit for end-of-test sanity check
    const balanceBeforeDeposit = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userA.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: USER_DEPOSIT_AMOUNT,
      tokenProgram: pool.tokenProgram,
      associatedTokenProgram: undefined,
      systemProgram: SystemProgram.programId,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );

    // Pulse health once (should succeed while fresh).
    const remaining = juplendHealthRemainingAccounts(
      juplendBank,
      oracles.usdcOracle.publicKey,
      pool.lending
    );

    const firstPulseIx = await healthPulse(userA.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(firstPulseIx),
      [userA.wallet],
      false,
      true
    );

    // Warp forward so the lending state becomes stale.
    const currentClock = await banksClient.getClock();
    const newTimestamp = currentClock.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);
    const slotsToAdvance = ONE_WEEK_IN_SECONDS * 0.4;
    const newClock = new Clock(
      currentClock.slot + BigInt(slotsToAdvance),
      0n,
      currentClock.epoch,
      currentClock.epochStartTimestamp,
      newTimestamp
    );
    bankrunContext.setClock(newClock);

    // Refresh Pyth pull oracles so oracle staleness doesn't mask Juplend staleness.
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Health pulse succeeds but records JuplendLendingStale (6504) error in health_cache.
    // Note: health_pulse never fails - it catches errors and stores them in the cache.
    const stalePulseIx = await healthPulse(userA.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(stalePulseIx),
      [userA.wallet],
      false,
      true
    );

    // Verify health_cache recorded the staleness error.
    // Note: Price/oracle errors are captured in internal_err, not mrgn_err.
    // mrgn_err is only set when check_account_init_health returns a top-level error.
    const staleAcc = await userA.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );
    assert.equal(
      staleAcc.healthCache.internalErr,
      6504,
      "health_cache.internal_err should record JuplendLendingStale error (6504)"
    );
    // engine_ok is true because the engine ran successfully (price errors are soft failures)
    assert.notEqual(
      staleAcc.healthCache.flags & HEALTH_CACHE_ENGINE_OK,
      0,
      "engine_ok should be true even with price errors"
    );
    // oracle_ok should be false because internal_err != 0
    assert.equal(
      staleAcc.healthCache.flags & HEALTH_CACHE_ORACLE_OK,
      0,
      "oracle_ok should be false when lending is stale"
    );

    // Now prepend update_rate in the SAME tx and pulse again.
    const updateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    const okResult = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(updateRateIx, stalePulseIx),
      [userA.wallet],
      true,
      false
    );

    // When trySend=true, result is BanksTransactionResultWithMeta with result=null on success
    assert("result" in okResult && !okResult.result, "expected tx to succeed after update_rate");

    // Sanity: health cache should now be marked healthy.
    const acc = await userA.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      userMarginfiAccount.publicKey
    );
    assert.ok(
      acc.healthCache.flags & HEALTH_CACHE_HEALTHY,
      "health cache should be healthy after successful pulse"
    );

    // Sanity: confirm deposit still exists (user should have less USDC than before deposit).
    const usdcBal = await getTokenBalance(bankRunProvider, userA.usdcAccount);
    assert.isBelow(usdcBal, balanceBeforeDeposit);
  });
});
