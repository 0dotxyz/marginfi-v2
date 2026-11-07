import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { WrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  groupAdmin,
  oracles,
  stakedMarginfiGroup,
  users,
  validators,
} from "../../rootHooks";
import {
  addBankPermissionless,
  configureBank,
  editStakedSettingsFast,
  propagateStakedSettings,
} from "../../utils/group-instructions";
import { pulseBankPrice } from "../../utils/user-instructions";
import { deriveBankWithSeed, deriveStakedSettings } from "../../utils/pdas";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  assertI80F48Equal,
} from "../../utils/genericTests";
import {
  BANK_SEED_KNOWN_FLAG,
  blankBankConfigOptRaw,
  blankStakedSettingsEditFast,
  CIRCUIT_BREAKER_ENABLED,
  CLOSE_ENABLED_FLAG,
  StakedSettingsEditFast,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import {
  advanceBankrunClock,
  getBankrunTime,
  processBankrunTransaction,
} from "../../utils/tools";
import { bnToBigIntSafe, mulI80, toI80Scaled } from "../../utils/bn-utils";

describe("Staked circuit breaker settings", () => {
  let settingsKey: PublicKey;
  let newBankKey: PublicKey;

  const cbSettings: StakedSettingsEditFast = {
    ...blankStakedSettingsEditFast(),
    circuitBreakerEnabled: true,
    cbDeviationBpsTiers: [600, 1050, 1800],
    cbTierDurationsSeconds: [600, 1800, 7200],
    cbEscalationWindowMult: 2,
    cbEmaAlphaBps: 2000,
  };

  const validator0Oracles = () => [
    oracles.wsolOracle.publicKey,
    validators[0].splMint,
    validators[0].splSolPool,
    validators[0].splOnRampPool,
  ];

  /** The breaker's reference for a bank's cached price: price times the cached multiplier. */
  const cachedCbPrice = (cache: {
    lastOraclePrice: WrappedI80F48;
    priceMultiplier: WrappedI80F48;
  }) =>
    mulI80(
      toI80Scaled(cache.lastOraclePrice),
      toI80Scaled(cache.priceMultiplier)
    );

  const cbFlag = (flags: BN) =>
    bnToBigIntSafe(flags) & BigInt(CIRCUIT_BREAKER_ENABLED);

  before(async () => {
    [settingsKey] = deriveStakedSettings(
      bankrunProgram.programId,
      stakedMarginfiGroup.publicKey
    );
    [newBankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      stakedMarginfiGroup.publicKey,
      validators[0].splMint,
      new BN(1)
    );
  });

  it("(attacker) edits staked circuit breaker settings - should fail", async () => {
    const user = users[0];
    const tx = new Transaction().add(
      await editStakedSettingsFast(user.mrgnBankrunProgram, {
        settingsKey,
        settings: cbSettings,
        admin: user.wallet.publicKey,
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [user.wallet],
      true
    );
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(admin) enables the circuit breaker with a zero EMA alpha - should fail", async () => {
    const tx = new Transaction().add(
      await editStakedSettingsFast(groupAdmin.mrgnBankrunProgram, {
        settingsKey,
        settings: { ...cbSettings, cbEmaAlphaBps: 0 },
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      true
    );
    // CircuitBreakerInvalidConfig
    assertBankrunTxFailed(result, 6602);
  });

  it("(admin) enables the circuit breaker in staked settings - happy path", async () => {
    const before = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    const tx = new Transaction().add(
      await editStakedSettingsFast(groupAdmin.mrgnBankrunProgram, {
        settingsKey,
        settings: cbSettings,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assert.equal(
      bnToBigIntSafe(settingsAcc.flags),
      bnToBigIntSafe(before.flags) | BigInt(CIRCUIT_BREAKER_ENABLED)
    );
    assert.deepEqual(settingsAcc.cbDeviationBpsTiers, [600, 1050, 1800]);
    assert.deepEqual(settingsAcc.cbTierDurationsSeconds, [600, 1800, 7200]);
    assert.equal(settingsAcc.cbEscalationWindowMult, 2);
    assert.equal(settingsAcc.cbEmaAlphaBps, 2000);
    assert.equal(settingsAcc.cbWindowSeconds, 0);
    assert.equal(settingsAcc.cbWindowMaxUpBps, 0);
    assert.equal(settingsAcc.cbWindowMaxDownBps, 0);
  });

  it("(permissionless) propagates to a staked bank without a fresh price - should fail", async () => {
    await advanceBankrunClock(bankrunContext, 60);
    const tx = new Transaction().add(
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: validators[0].bank,
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [groupAdmin.wallet],
      true
    );
    // CircuitBreakerRequiresWarmCache
    assertBankrunTxFailed(result, 6603);
  });

  it("(permissionless) pulses and propagates - the staked bank's breaker turns on", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const bankBefore = await bankrunProgram.account.bank.fetch(
      validators[0].bank
    );
    assert.equal(cbFlag(bankBefore.flags), 0n);
    assertI80F48Equal(bankBefore.cbReferencePrice, 0);

    const tx = new Transaction().add(
      await pulseBankPrice(bankrunProgram, {
        bank: validators[0].bank,
        remaining: validator0Oracles(),
      }),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: validators[0].bank,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const bank = await bankrunProgram.account.bank.fetch(validators[0].bank);
    assert.equal(
      bnToBigIntSafe(bank.flags),
      bnToBigIntSafe(bankBefore.flags) | BigInt(CIRCUIT_BREAKER_ENABLED)
    );
    assert.deepEqual(bank.config.cbDeviationBpsTiers, [600, 1050, 1800]);
    assert.deepEqual(bank.config.cbTierDurationsSeconds, [600, 1800, 7200]);
    assert.equal(bank.config.cbEscalationWindowMult, 2);
    assert.equal(bank.config.cbEmaAlphaBps, 2000);
    assert.equal(toI80Scaled(bank.cbReferencePrice), cachedCbPrice(bank.cache));
    assert.equal(
      toI80Scaled(bank.cbWindowReferencePrice),
      cachedCbPrice(bank.cache)
    );
    assertBNEqual(bank.cbWindowStartedAt, await getBankrunTime(bankrunContext));
  });

  it("(admin) changes a staked breaker setting - propagation syncs it without a fresh price", async () => {
    await advanceBankrunClock(bankrunContext, 60);
    const bankBefore = await bankrunProgram.account.bank.fetch(
      validators[0].bank
    );
    const tx = new Transaction().add(
      await editStakedSettingsFast(groupAdmin.mrgnBankrunProgram, {
        settingsKey,
        settings: {
          ...blankStakedSettingsEditFast(),
          cbDeviationBpsTiers: [700, 1100, 1900],
        },
      }),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: validators[0].bank,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const bank = await bankrunProgram.account.bank.fetch(validators[0].bank);
    assert.deepEqual(bank.config.cbDeviationBpsTiers, [700, 1100, 1900]);
    assertBNEqual(bank.flags, bankBefore.flags);
    assertI80F48Equal(bank.cbReferencePrice, bankBefore.cbReferencePrice);
  });

  it("(permissionless) adds a staked bank - its breaker starts on and the first pulse seeds it", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    let tx = new Transaction().add(
      await addBankPermissionless(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: stakedMarginfiGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        pythOracle: oracles.wsolOracle.publicKey,
        stakePool: validators[0].splPool,
        validatorVoteAccount: validators[0].voteAccount,
        seed: new BN(1),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    let bank = await bankrunProgram.account.bank.fetch(newBankKey);
    // Staked settings carry only the staked oracle and circuit breaker bits, all copied here.
    assert.equal(
      bnToBigIntSafe(bank.flags),
      BigInt(CLOSE_ENABLED_FLAG | BANK_SEED_KNOWN_FLAG) |
        bnToBigIntSafe(settingsAcc.flags)
    );
    assert.deepEqual(bank.config.cbDeviationBpsTiers, [700, 1100, 1900]);
    assert.deepEqual(bank.config.cbTierDurationsSeconds, [600, 1800, 7200]);
    assert.equal(bank.config.cbEscalationWindowMult, 2);
    assert.equal(bank.config.cbEmaAlphaBps, 2000);
    assertI80F48Equal(bank.cbReferencePrice, 0);
    assertI80F48Equal(bank.cbWindowReferencePrice, 0);

    tx = new Transaction().add(
      await pulseBankPrice(bankrunProgram, {
        bank: newBankKey,
        remaining: validator0Oracles(),
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    bank = await bankrunProgram.account.bank.fetch(newBankKey);
    assert.equal(toI80Scaled(bank.cbReferencePrice), cachedCbPrice(bank.cache));
    assert.equal(
      toI80Scaled(bank.cbWindowReferencePrice),
      cachedCbPrice(bank.cache)
    );
    assertBNEqual(bank.cbWindowStartedAt, await getBankrunTime(bankrunContext));
  });

  it("(admin) disables the breaker in staked settings - propagation leaves the bank's breaker on", async () => {
    const bankBefore = await bankrunProgram.account.bank.fetch(
      validators[0].bank
    );
    const tx = new Transaction().add(
      await editStakedSettingsFast(groupAdmin.mrgnBankrunProgram, {
        settingsKey,
        settings: {
          ...blankStakedSettingsEditFast(),
          circuitBreakerEnabled: false,
        },
      }),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: validators[0].bank,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assert.equal(cbFlag(settingsAcc.flags), 0n);
    const bank = await bankrunProgram.account.bank.fetch(validators[0].bank);
    assert.equal(cbFlag(bank.flags), BigInt(CIRCUIT_BREAKER_ENABLED));
    assertBNEqual(bank.flags, bankBefore.flags);
    assert.deepEqual(bank.config.cbDeviationBpsTiers, [700, 1100, 1900]);
    assertI80F48Equal(bank.cbReferencePrice, bankBefore.cbReferencePrice);
  });

  it("(admin) turns off the breaker on each staked bank - propagation leaves it off", async () => {
    const disableCb = {
      ...blankBankConfigOptRaw(),
      circuitBreakerEnabled: false,
    };
    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: validators[0].bank,
        bankConfigOpt: disableCb,
      }),
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: newBankKey,
        bankConfigOpt: disableCb,
      }),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: validators[0].bank,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    for (const bankKey of [validators[0].bank, newBankKey]) {
      const bank = await bankrunProgram.account.bank.fetch(bankKey);
      assert.equal(cbFlag(bank.flags), 0n);
    }
  });
});
