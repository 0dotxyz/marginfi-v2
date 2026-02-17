import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";

import {
  bankrunContext,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  riskAdmin,
  users,
} from "./rootHooks";

import { groupConfigure, groupInitialize } from "./utils/group-instructions";
import { assertBankrunTxFailed } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import {
  accountInit,
  composeRemainingAccountsWriteableMeta,
  endDeleverageIx,
  initLiquidationRecordIx,
  startDeleverageIx,
} from "./utils/user-instructions";
import { ONE_WEEK_IN_SECONDS } from "./utils/types";

import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

import { ensureJuplendPoolForMint } from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/juplend-test-env";
import { juplendUpdateRateIx } from "./utils/juplend/juplend-instructions";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";

describe("jl09: JupLend receivership allowlist (start_deleverage + update_rate)", () => {
  /** deterministic (32 bytes) */
  const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000009");
  const BANK_SEED = new BN(1);

  const FUND_AMOUNT = new BN(100 * 10 ** ecosystem.usdcDecimals);
  const SEED_DEPOSIT_AMOUNT = new BN(1 * 10 ** ecosystem.usdcDecimals);
  const USER_DEPOSIT_AMOUNT = new BN(25 * 10 ** ecosystem.usdcDecimals);

  const juplendGroup = Keypair.fromSeed(GROUP_SEED);
  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];

  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;
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

    // Set risk admin so we can call start_deleverage.
    const configureIx = await groupConfigure(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      newRiskAdmin: riskAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(configureIx),
      [groupAdmin.wallet],
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

  it("Start deleverage fails when lending is stale, and succeeds when update_rate is prepended (and allowlisted)", async () => {
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

    const remaining = composeRemainingAccountsWriteableMeta([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

    // Initialize the liquidation record in a separate transaction first
    const initLiqRecordIx = await initLiquidationRecordIx(riskAdmin.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccount.publicKey,
      feePayer: riskAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initLiqRecordIx),
      [riskAdmin.wallet],
      false,
      true
    );

    const startIx = await startDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccount.publicKey,
      riskAdmin: riskAdmin.wallet.publicKey,
      remaining,
    });

    const endIx = await endDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccount.publicKey,
      remaining,
    });

    // (A) Without update_rate: stale health should fail.
    const staleResult = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(startIx, endIx),
      [riskAdmin.wallet],
      true,
      false
    );
    assertBankrunTxFailed(staleResult, 6504);

    // (B) With update_rate prepended: should succeed, and update_rate must be allowlisted.
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
      new Transaction().add(updateRateIx, startIx, endIx),
      [riskAdmin.wallet],
      true,
      false
    );

    // When trySend=true, result is BanksTransactionResultWithMeta with result=null on success
    assert(
      "result" in okResult && !okResult.result,
      "expected deleverage tx to succeed after update_rate"
    );
  });
});
