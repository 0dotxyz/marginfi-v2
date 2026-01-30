import { BN, IdlAccounts } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  globalFeeWallet,
  globalProgramAdmin,
  users,
  PROGRAM_FEE_FIXED,
  PROGRAM_FEE_RATE,
  INIT_POOL_ORIGINATION_FEE,
  bankrunProgram,
  bankrunContext,
  banksClient,
  EMODE_SEED,
  ecosystem,
  emodeGroup,
  oracles,
  LIQUIDATION_MAX_FEE,
} from "./rootHooks";
import { assert } from "chai";
import { deriveBankWithSeed, deriveGlobalFeeState } from "./utils/pdas";
import {
  initGlobalFeeState,
  panicPause,
  panicUnpause,
  panicUnpausePermissionless,
  propagateFeeState,
} from "./utils/group-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  assertBankrunTxFailed,
  assertBNApproximately,
  assertBNEqual,
  waitUntil,
} from "./utils/genericTests";
import { PAUSE_DURATION_SECONDS } from "./utils/types";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { USER_ACCOUNT_E } from "./utils/mocks";
import {
  liquidateIx,
  composeRemainingAccounts,
  depositIx,
  withdrawIx,
  borrowIx,
  repayIx,
} from "./utils/user-instructions";

describe("Panic Mode state test (Bankrun)", () => {
  type FeeState = IdlAccounts<Marginfi>["feeState"];

  let feeStateKey: PublicKey;
  let feeState: FeeState;

  let firstTimestamp: BN;

  const seed = new BN(EMODE_SEED);
  let usdcBank: PublicKey;
  let lstBBank: PublicKey;
  let stableBank: PublicKey;
  let lstABank: PublicKey;
  let solBank: PublicKey;

  before(async () => {
    feeStateKey = deriveGlobalFeeState(bankrunProgram.programId)[0];

    // Initialize fee state if it doesn't exist
    try {
      feeState = await bankrunProgram.account.feeState.fetch(feeStateKey);
    } catch (err) {
      const tx = new Transaction();
      tx.add(
        await initGlobalFeeState(globalProgramAdmin.mrgnBankrunProgram, {
          payer: globalProgramAdmin.wallet.publicKey,
          admin: globalProgramAdmin.wallet.publicKey,
          wallet: globalFeeWallet,
          bankInitFlatSolFee: INIT_POOL_ORIGINATION_FEE,
          programFeeFixed: bigNumberToWrappedI80F48(PROGRAM_FEE_FIXED),
          programFeeRate: bigNumberToWrappedI80F48(PROGRAM_FEE_RATE),
          liquidationFlatSolFee: 12345,
          liquidationMaxFee: bigNumberToWrappedI80F48(LIQUIDATION_MAX_FEE),
        })
      );

      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(globalProgramAdmin.wallet);
      await banksClient.processTransaction(tx);

      feeState = await bankrunProgram.account.feeState.fetch(feeStateKey);
    }

    [usdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );
    [lstBBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed.addn(1)
    );
    [lstABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed
    );
    [solBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      seed
    );
    [stableBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed.addn(1)
    );
  });

  it("(fee admin) pause the protocol - happy path", async () => {
    const tx = new Transaction();
    tx.add(await panicPause(globalProgramAdmin.mrgnBankrunProgram, {}));
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    await banksClient.processTransaction(tx);

    const fs = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assert.equal(fs.panicState.pauseFlags, 1);

    const clock = await banksClient.getClock();
    const now = Number(clock.unixTimestamp);
    assertBNApproximately(fs.panicState.pauseStartTimestamp, now, 100);
    assertBNApproximately(fs.panicState.lastDailyResetTimestamp, now, 100);

    firstTimestamp = fs.panicState.lastDailyResetTimestamp;
    assert.equal(fs.panicState.dailyPauseCount, 1);
    assert.equal(fs.panicState.consecutivePauseCount, 1);
    // If you're getting issues having firstTimestamp in later tests, bump this. Yes it's a dumb
    // hack, but oh well.
    waitUntil(now + 2);
  });

  it("(fee admin) extends an existing pause - happy path", async () => {
    const tx = new Transaction();
    tx.add(
      await panicPause(globalProgramAdmin.mrgnBankrunProgram, {}),
      // Dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        toPubkey: users[0].wallet.publicKey,
        lamports: 54321,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    await banksClient.processTransaction(tx);

    const fs = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assert.equal(fs.panicState.pauseFlags, 1);

    // Extension applies to the previous "start" time (which can be in the future).
    assertBNApproximately(
      fs.panicState.pauseStartTimestamp,
      // firstTimestamp undefined error here? Bump the wait in the above test.
      firstTimestamp.toNumber() + PAUSE_DURATION_SECONDS,
      10
    );
    // No change on reset
    assertBNEqual(fs.panicState.lastDailyResetTimestamp, firstTimestamp);
    assert.equal(fs.panicState.dailyPauseCount, 2);
    assert.equal(fs.panicState.consecutivePauseCount, 2);
  });

  it("(fee admin) tries extends an existing pause again - fails due to pause limits", async () => {
    const tx = new Transaction();
    tx.add(
      await panicPause(globalProgramAdmin.mrgnBankrunProgram, {}),
      // Dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        toPubkey: users[0].wallet.publicKey,
        lamports: 5675679,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // PauseLimitExceeded
    assertBankrunTxFailed(result, 6082);
  });

  // Note: A pause isn't really "active" until propagated to the groups. In practice, the MS tx that
  // should init a pause should also propagate it, otherwise were will be a lag when it's actually
  // needed. Likewise to unpause, don't forget to propagate.
  it("(permissionless) propagate a pause state to a group - happy path", async () => {
    const tx = new Transaction();
    tx.add(
      await propagateFeeState(globalProgramAdmin.mrgnBankrunProgram, {
        group: emodeGroup.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    await banksClient.processTransaction(tx);

    const clock = await banksClient.getClock();
    const now = Number(clock.unixTimestamp);

    const fs = await bankrunProgram.account.feeState.fetch(feeStateKey);
    const group = await bankrunProgram.account.marginfiGroup.fetch(
      emodeGroup.publicKey
    );
    const cache = group.panicStateCache;
    assert.equal(cache.pauseFlags, 1);

    assertBNEqual(cache.pauseStartTimestamp, fs.panicState.pauseStartTimestamp);
    assertBNApproximately(cache.lastCacheUpdate, now, 100);
  });

  it("(liquidator) liquidations no longer run when paused", async () => {
    const liquidatee = users[0];
    const liquidator = users[2];

    const assetBankKey = solBank;
    const liabilityBankKey = lstABank;
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_E);
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({
        units: 260_000,
      }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey,
        liabilityBankKey,
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.wsolOracle.publicKey, // asset oracle
          oracles.pythPullLst.publicKey, // liab oracle
          ...composeRemainingAccounts([
            // liquidator accounts
            [usdcBank, oracles.usdcOracle.publicKey],
            [solBank, oracles.wsolOracle.publicKey],
            [lstABank, oracles.pythPullLst.publicKey],
          ]),
          ...composeRemainingAccounts([
            // liquidatee accounts
            [solBank, oracles.wsolOracle.publicKey],
            [lstABank, oracles.pythPullLst.publicKey],
          ]),
        ],
        amount: new BN(0.0000001 * 10 ** ecosystem.wsolDecimals),
        liquidatorAccounts: 6,
        liquidateeAccounts: 4,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(liquidator.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Protocol paused
    assertBankrunTxFailed(result, 6080);
  });

  // Note: This is an interesting edge case to consider. While liquidations are allowed to continue
  // to run, it's notable that liquidators cannot deposit fresh funds or withdraw their earnings, or
  // repay their debts. This causes them to take on delta risk during the pause. The novel
  // liquidation approach coming in 1.5 will also have to be configured to bypass a pause (or not?)
  it("(user 2 aka liquidator) tries to deposit funds - fails due to pause", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        amount: new BN(0.0001 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Protocol paused
    assertBankrunTxFailed(result, 6080);
  });

  it("(user 2 aka liquidator) tries to withdraw funds - fails due to pause", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await withdrawIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        amount: new BN(0.0001 * 10 ** ecosystem.usdcDecimals),
        remaining: [
          ...composeRemainingAccounts([
            // liquidator accounts
            [usdcBank, oracles.usdcOracle.publicKey],
            [solBank, oracles.wsolOracle.publicKey],
            [lstABank, oracles.pythPullLst.publicKey],
          ]),
        ],
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Protocol paused
    assertBankrunTxFailed(result, 6080);
  });

  it("(user 2 aka liquidator) tries to borrow funds - fails due to pause", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstBBank,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(0.0001 * 10 ** ecosystem.lstAlphaDecimals),
        remaining: [
          ...composeRemainingAccounts([
            // liquidator accounts
            [usdcBank, oracles.usdcOracle.publicKey],
            [solBank, oracles.wsolOracle.publicKey],
            [lstABank, oracles.pythPullLst.publicKey],
            // new position!
            [lstBBank, oracles.pythPullLst.publicKey],
          ]),
        ],
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Protocol paused
    assertBankrunTxFailed(result, 6080);
  });

  it("(user 2 aka liquidator) tries to repay funds - fails due to pause", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstABank,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(0.0001 * 10 ** ecosystem.lstAlphaDecimals),
        remaining: [
          ...composeRemainingAccounts([
            // liquidator accounts
            [usdcBank, oracles.usdcOracle.publicKey],
            [solBank, oracles.wsolOracle.publicKey],
            [lstABank, oracles.pythPullLst.publicKey],
          ]),
        ],
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // Protocol paused
    assertBankrunTxFailed(result, 6080);
  });

  it("(attacker) tries to pause - should fail", async () => {
    const tx = new Transaction();
    tx.add(
      await panicPause(users[0].mrgnBankrunProgram, {}),
      // Dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: users[0].wallet.publicKey,
        toPubkey: users[1].wallet.publicKey,
        lamports: 654321,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // (fee state admin doesn't match fee state)
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(fee admin) tries to pause beyond daily pause limits - should fail", async () => {
    const tx = new Transaction();
    tx.add(await panicPause(globalProgramAdmin.mrgnBankrunProgram, {}));
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // PauseLimitExceeded
    assertBankrunTxFailed(result, 6082);
  });

  it("(fee admin) admin unpause - happy path", async () => {
    const tx = new Transaction();
    tx.add(await panicUnpause(globalProgramAdmin.mrgnBankrunProgram, {}));
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    await banksClient.processTransaction(tx);

    const fs = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assert.equal(fs.panicState.pauseFlags, 0);
    assertBNEqual(fs.panicState.pauseStartTimestamp, 0);
    // No change to reset timestamp
    assertBNEqual(fs.panicState.lastDailyResetTimestamp, firstTimestamp);
    assert.equal(fs.panicState.consecutivePauseCount, 0);
  });

  it("(fee admin) admin unpause when not paused - should fail", async () => {
    const tx = new Transaction();
    tx.add(
      await panicUnpause(globalProgramAdmin.mrgnBankrunProgram, {}),
      // Dummy tx to trick bankrun
      SystemProgram.transfer({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        toPubkey: users[1].wallet.publicKey,
        lamports: 456783,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // ProtocolNotPaused
    assertBankrunTxFailed(result, 6083);
  });

  it("(attacker) non-admin tries to call admin unpause - should fail", async () => {
    const tx = new Transaction();
    tx.add(await panicUnpause(users[0].mrgnBankrunProgram, {}));
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(permissionless) permissionless unpause when not paused - should fail", async () => {
    const tx = new Transaction();
    tx.add(await panicUnpausePermissionless(users[0].mrgnBankrunProgram, {}));
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);

    const result = await banksClient.tryProcessTransaction(tx);
    // ProtocolNotPaused
    assertBankrunTxFailed(result, 6083);
  });

  it("(permissionless) propagate unpause state to a group - happy path", async () => {
    const tx = new Transaction();
    tx.add(
      await propagateFeeState(globalProgramAdmin.mrgnBankrunProgram, {
        group: emodeGroup.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(globalProgramAdmin.wallet);

    await banksClient.processTransaction(tx);

    const clock = await banksClient.getClock();
    const now = Number(clock.unixTimestamp);

    const fs = await bankrunProgram.account.feeState.fetch(feeStateKey);
    const group = await bankrunProgram.account.marginfiGroup.fetch(
      emodeGroup.publicKey
    );
    const cache = group.panicStateCache;
    assert.equal(cache.pauseFlags, 0);

    assertBNEqual(cache.pauseStartTimestamp, fs.panicState.pauseStartTimestamp);
    assertBNApproximately(cache.lastCacheUpdate, now, 100);
  });

  // TODO settings can be changed while paused
  // TODO bankrun into the future and show we can pause again
  // TODO bankrun into the future and show permissionless unpause
});
