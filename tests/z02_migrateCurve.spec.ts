import { SystemProgram, Transaction } from "@solana/web3.js";
import { Clock } from "solana-bankrun";
import { assert } from "chai";
import BigNumber from "bignumber.js";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  LEGACY_BANK_SAMPLE,
  bankrunContext,
  bankrunProgram,
  banksClient,
  groupAdmin,
  users,
  verbose,
} from "./rootHooks";
import {
  accrueInterest,
  backfillBankIsT22Flag,
  migrateCurve,
} from "./utils/group-instructions";
import {
  IS_T22_FLAG,
  INTEREST_CURVE_LEGACY,
  INTEREST_CURVE_SEVEN_POINT,
  aprToU32,
  utilToU32,
} from "./utils/types";
import { assertI80F48Equal } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import { getEpochAndSlot } from "./utils/bankrunConnection";

// one day
const INTERVAL_SECONDS = 24 * 60 * 60;
const SLOT_DURATION_SECONDS = 0.4;

const toBigNumber = (value: any): BigNumber => wrappedI80F48toBigNumber(value);

const advanceTime = async (seconds: number) => {
  const currentClock = await banksClient.getClock();
  const { epoch, slot } = await getEpochAndSlot(banksClient);
  const slotsToAdvance = Math.round(seconds / SLOT_DURATION_SECONDS);
  const newClock = new Clock(
    BigInt(slot + slotsToAdvance),
    0n,
    BigInt(epoch),
    0n,
    currentClock.unixTimestamp + BigInt(seconds)
  );
  bankrunContext.setClock(newClock);
};

const sendLegacyAccrual = async () => {
  const user = users[0];
  const tx = new Transaction();
  tx.add(
    await accrueInterest(user.mrgnBankrunProgram, {
      bank: LEGACY_BANK_SAMPLE,
    }),
    // dummy tx to trick bankrun
    SystemProgram.transfer({
      fromPubkey: user.wallet.publicKey,
      toPubkey: groupAdmin.wallet.publicKey,
      lamports: Math.round(Math.random() * 10000),
    })
  );
  await processBankrunTransaction(
    bankrunContext,
    tx,
    [user.wallet],
    false,
    true
  );
};

const sendMigration = async () => {
  const user = users[0];
  const tx = new Transaction();
  tx.add(
    await migrateCurve(user.mrgnBankrunProgram, {
      bank: LEGACY_BANK_SAMPLE,
    })
  );
  await processBankrunTransaction(
    bankrunContext,
    tx,
    [user.wallet],
    false,
    true
  );
};

const sendT22Backfill = async () => {
  const user = users[0];
  const tx = new Transaction();
  tx.add(
    await backfillBankIsT22Flag(user.mrgnBankrunProgram, {
      bank: LEGACY_BANK_SAMPLE,
    })
  );
  await processBankrunTransaction(
    bankrunContext,
    tx,
    [user.wallet],
    false,
    true
  );
};

describe("Legacy bank curve migration", () => {
  let initialLiabilityShareValue: BigNumber;
  let postWarmupLiabilityShareValue: BigNumber;
  let postLegacyLiabilityShareValue: BigNumber;
  let postMigrationLiabilityShareValue: BigNumber;
  let postNewLiabilityShareValue: BigNumber;

  let optimalBefore: number;
  let plateauBefore: number;
  let maxRateBefore: number;

  it("captures the legacy curve configuration", async () => {
    const bankBefore = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    const ircBefore = bankBefore.config.interestRateConfig;

    assert.equal(ircBefore.curveType, INTEREST_CURVE_LEGACY);

    optimalBefore = toBigNumber(ircBefore.optimalUtilizationRate).toNumber();
    plateauBefore = toBigNumber(ircBefore.plateauInterestRate).toNumber();
    maxRateBefore = toBigNumber(ircBefore.maxInterestRate).toNumber();

    if (verbose) {
      console.log("Rates before");
      console.log(" optimal: " + optimalBefore);
      console.log(" plat: " + plateauBefore);
      console.log(" max: " + maxRateBefore);
    }

    assert.isAbove(optimalBefore, 0);
    assert.isAbove(plateauBefore, 0);
    assert.isAbove(maxRateBefore, 0);
    assert.equal(bankBefore.flags.toNumber() & IS_T22_FLAG, 0);

    initialLiabilityShareValue = toBigNumber(bankBefore.liabilityShareValue);
  });

  it("backfill ix is a no-op for a classic bank", async () => {
    const bankBefore = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    const flagsBefore = bankBefore.flags.toNumber();
    assert.equal(flagsBefore & IS_T22_FLAG, 0);

    await sendT22Backfill();

    const bankAfter = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    const flagsAfter = bankAfter.flags.toNumber();
    assert.equal(flagsAfter & IS_T22_FLAG, 0);
    assert.equal(flagsAfter, flagsBefore);
  });

  // Note: we accrue a "warmup" first so we can have a fixed amount of time elapse in the next step
  it("accrues interest using the legacy curve", async () => {
    await sendLegacyAccrual();
    const bankAfterWarmup = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    postWarmupLiabilityShareValue = toBigNumber(
      bankAfterWarmup.liabilityShareValue
    );
    assert.isTrue(
      postWarmupLiabilityShareValue.gte(initialLiabilityShareValue)
    );
  });

  it("advances time and accrues again on the legacy curve", async () => {
    await advanceTime(INTERVAL_SECONDS);
    await sendLegacyAccrual();
    const bankAfterLegacyInterval = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    postLegacyLiabilityShareValue = toBigNumber(
      bankAfterLegacyInterval.liabilityShareValue
    );
    const legacyDelta = postLegacyLiabilityShareValue.minus(
      postWarmupLiabilityShareValue
    );
    if (verbose) {
      console.log("delta: " + legacyDelta);
    }
    assert.isTrue(legacyDelta.gt(0));
  });

  it("migrates the curve and validates the configuration", async () => {
    await sendMigration();
    const bankAfterMigration = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    postMigrationLiabilityShareValue = toBigNumber(
      bankAfterMigration.liabilityShareValue
    );

    const ircAfter = bankAfterMigration.config.interestRateConfig;

    assert.equal(ircAfter.curveType, INTEREST_CURVE_SEVEN_POINT);
    assertI80F48Equal(ircAfter.optimalUtilizationRate, 0);
    assertI80F48Equal(ircAfter.plateauInterestRate, 0);
    assertI80F48Equal(ircAfter.maxInterestRate, 0);
    assert.equal(ircAfter.zeroUtilRate, 0);

    const expectedUtil = utilToU32(optimalBefore);
    const expectedPlateauRate = aprToU32(plateauBefore);
    const expectedHundredRate = aprToU32(maxRateBefore);

    assert.approximately(ircAfter.points[0].util, expectedUtil, 5);
    assert.approximately(ircAfter.points[0].rate, expectedPlateauRate, 5);
    assert.approximately(ircAfter.hundredUtilRate, expectedHundredRate, 5);
  });

  it("accrues interest with the migrated curve at a similar rate", async () => {
    const legacyDelta = postLegacyLiabilityShareValue.minus(
      postWarmupLiabilityShareValue
    );
    await advanceTime(INTERVAL_SECONDS);
    await sendLegacyAccrual();
    const bankAfterNewInterval = await bankrunProgram.account.bank.fetch(
      LEGACY_BANK_SAMPLE
    );
    postNewLiabilityShareValue = toBigNumber(
      bankAfterNewInterval.liabilityShareValue
    );
    const migratedDelta = postNewLiabilityShareValue.minus(
      postMigrationLiabilityShareValue
    );
    if (verbose) {
      console.log("delta: " + migratedDelta);
    }

    assert.isTrue(migratedDelta.gt(0));

    const tolerance = legacyDelta
      .multipliedBy(0.01)
      .plus(new BigNumber("1e-12"));
    const deltaDifference = legacyDelta.minus(migratedDelta).abs();
    if (verbose) {
      console.log("diff between legacy/migrated: " + deltaDifference);
      // Note: Migrated is consistently higher, but the delta is <1%. Lenders will enjoy this tiny
      // windfall due to rounding magic!
      if (legacyDelta.minus(migratedDelta).isPositive()) {
        console.log("legacy was higher");
      } else {
        console.log("migrated was higher");
      }
    }

    assert.isTrue(deltaDifference.lte(tolerance));
  });
});
