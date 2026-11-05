import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import BN from "bn.js";
import { Program } from "@coral-xyz/anchor";
import { Marginfi } from "../../../target/types/marginfi";
import {
  stakedMarginfiGroup,
  validators,
  groupAdmin,
  oracles,
  bankrunContext,
  banksClient,
  bankrunProgram,
  stakedBankKeypairSol,
  users,
} from "../../rootHooks";
import {
  initSameAssetEmodeRegistry,
  propagateStakedSettings,
  setBankSameAssetEmodeEligibility,
} from "../../utils/group-instructions";
import {
  composeRemainingAccounts,
  healthPulse,
  pulseBankPrice,
} from "../../utils/user-instructions";
import { USER_ACCOUNT } from "../../utils/mocks";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";
import { deriveBankWithSeed, deriveStakedSettings } from "../../utils/pdas";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import {
  assertKeysEqual,
  assertI80F48Approx,
  assertBNEqual,
  assertBankrunTxFailed,
} from "../../utils/genericTests";
import {
  defaultStakedInterestSettings,
  ORACLE_SETUP_STAKED_WITH_PYTH_PUSH,
  ORACLE_SETUP_STAKED_WITH_SCOPE,
  PYTH_PULL_MIGRATED,
  StakedSettingsEdit,
} from "../../utils/types";
import {
  bytesToF64,
  getBankrunBlockhash,
  getBankrunTime,
} from "../../utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

let program: Program<Marginfi>;
let marginfiGroup: Keypair;

describe("Edit and propagate staked settings", () => {
  let settingsKey: PublicKey;
  let bankKey: PublicKey;

  before(async () => {
    program = bankrunProgram;
    marginfiGroup = stakedMarginfiGroup;
    [settingsKey] = deriveStakedSettings(
      program.programId,
      marginfiGroup.publicKey
    );
    [bankKey] = deriveBankWithSeed(
      program.programId,
      marginfiGroup.publicKey,
      validators[0].splMint,
      new BN(0)
    );
  });

  it("(governance admin) edits some settings - happy path", async () => {
    const settings: StakedSettingsEdit = {
      oracle: oracles.usdcOracle.publicKey,
      assetWeightInit: bigNumberToWrappedI80F48(0.2),
      assetWeightMaint: bigNumberToWrappedI80F48(0.3),
      depositLimit: new BN(42),
      totalAssetValueInitLimit: new BN(43),
      oracleMaxAge: 44,
      riskTier: {
        collateral: undefined,
      },
      baseFeed: null,
      smoothedEntry: null,
    };
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          governanceAdmin: groupAdmin.wallet.publicKey,
          stakedSettings: settingsKey,
        })
        .instruction()
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    let settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assertKeysEqual(settingsAcc.key, settingsKey);
    assertKeysEqual(settingsAcc.oracle, oracles.usdcOracle.publicKey);
    assertI80F48Approx(settingsAcc.assetWeightInit, 0.2);
    assertI80F48Approx(settingsAcc.assetWeightMaint, 0.3);
    assertBNEqual(settingsAcc.depositLimit, 42);
    assertBNEqual(settingsAcc.totalAssetValueInitLimit, 43);
    assert.equal(settingsAcc.oracleMaxAge, 44);
    assert.deepEqual(settingsAcc.riskTier, { collateral: {} });
  });

  it("(permissionless) Propagate staked settings to a bank - happy path", async () => {
    let tx = new Transaction();
    tx.add(
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: bankKey,
        oracle: oracles.usdcOracle.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet); // just to the pay the fee
    await banksClient.tryProcessTransaction(tx);

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    const config = bank.config;
    assertKeysEqual(config.oracleKeys[0], oracles.usdcOracle.publicKey);
    assertI80F48Approx(config.assetWeightInit, 0.2);
    assertI80F48Approx(config.assetWeightMaint, 0.3);
    assertBNEqual(config.depositLimit, 42);
    assertBNEqual(config.totalAssetValueInitLimit, 43);
    assert.equal(config.oracleMaxAge, 44);
    assert.deepEqual(config.riskTier, { collateral: {} });
    // Propagation always set the pyth migration flag on the first call
    assert.equal(config.configFlags, PYTH_PULL_MIGRATED);
  });

  it("(governance admin) sets a bad oracle - fails at propagation", async () => {
    const settings: StakedSettingsEdit = {
      oracle: oracles.wsolOracle.publicKey,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: null,
      riskTier: null,
      baseFeed: null,
      smoothedEntry: null,
    };
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          governanceAdmin: groupAdmin.wallet.publicKey,
          stakedSettings: settingsKey,
        })
        .instruction()
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    let settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assertKeysEqual(settingsAcc.oracle, oracles.wsolOracle.publicKey);

    tx = new Transaction();
    tx.add(
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: bankKey,
        oracle: PublicKey.default,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet); // just to the pay the fee
    let result = await banksClient.tryProcessTransaction(tx);

    // (WrongOracleAccountKeys)
    assertBankrunTxFailed(result, 6052);
  });

  it("(governance admin) restores default settings - happy path", async () => {
    const defaultSettings = defaultStakedInterestSettings(
      oracles.wsolOracle.publicKey
    );
    const settings: StakedSettingsEdit = {
      oracle: defaultSettings.oracle,
      assetWeightInit: defaultSettings.assetWeightInit,
      assetWeightMaint: defaultSettings.assetWeightMaint,
      depositLimit: defaultSettings.depositLimit,
      totalAssetValueInitLimit: defaultSettings.totalAssetValueInitLimit,
      oracleMaxAge: defaultSettings.oracleMaxAge,
      riskTier: defaultSettings.riskTier,
      baseFeed: null,
      smoothedEntry: null,
    };
    // Note you can pack propagates into the edit tx, so with a LUT you can easily propagate
    // hundreds of banks in the same ts as edit
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          governanceAdmin: groupAdmin.wallet.publicKey,
          stakedSettings: settingsKey,
        })
        .instruction(),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: bankKey,
        oracle: oracles.wsolOracle.publicKey,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  // --- SOL/USD from a Scope entry: the pool rate is unchanged, only the base feed moves ---
  const scopeFeed = Keypair.generate().publicKey;
  /** Entry 13 = 175.0, distinct from the Pyth SOL price so the source is visible. */
  const SOL_ENTRY = { index: 13, value: 17_500_000_000n, exp: 8n };
  /** Entry 113 = 140.0, the SOL EMA (a PythLazerEMA of entry 13). */
  const SOL_EMA_ENTRY = { index: 113, value: 14_000_000_000n, exp: 8n };
  let pythMultiplier: number;

  const freshenScopeFeed = async () => {
    const now = await getBankrunTime(bankrunContext);
    setScopeFeed(
      bankrunContext,
      scopeFeed,
      makeScopePrices([
        { ...SOL_ENTRY, timestamp: now },
        { ...SOL_EMA_ENTRY, timestamp: now },
      ])
    );
  };

  /** User 1 (staked + SOL deposits): health pulse, returning the staked position's initial price. */
  const stakedInitPrice = async (feed: PublicKey) => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            bankKey,
            feed,
            validators[0].splMint,
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [stakedBankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const index = acc.lendingAccount.balances.findIndex(
      (b) => b.active === 1 && b.bankPk.equals(bankKey)
    );
    assert.isAtLeast(index, 0, "user 1 has a staked position");
    assert.equal(acc.healthCache.mrgnErr, 0);
    return bytesToF64(acc.healthCache.prices[index]);
  };

  /** Edit only, returning the result so failures can be asserted. */
  const tryEdit = async (
    edit: Partial<StakedSettingsEdit>,
    remaining: PublicKey[] = []
  ) => {
    const settings: StakedSettingsEdit = {
      oracle: null,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: null,
      riskTier: null,
      baseFeed: null,
      smoothedEntry: null,
      ...edit,
    };
    const tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          governanceAdmin: groupAdmin.wallet.publicKey,
          stakedSettings: settingsKey,
        })
        .remainingAccounts(
          remaining.map((pubkey) => ({
            pubkey,
            isSigner: false,
            isWritable: false,
          }))
        )
        .instruction()
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    return banksClient.tryProcessTransaction(tx);
  };

  const pulseStaked = async (feed: PublicKey) => {
    const tx = new Transaction().add(
      await pulseBankPrice(groupAdmin.mrgnBankrunProgram, {
        bank: bankKey,
        remaining: [
          feed,
          validators[0].splMint,
          validators[0].splSolPool,
          validators[0].splOnRampPool,
        ],
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
    return (await bankrunProgram.account.bank.fetch(bankKey)).cache;
  };

  const editAndPropagate = async (
    edit: Partial<StakedSettingsEdit>,
    feed: PublicKey,
    editRemaining: PublicKey[] = []
  ) => {
    const settings: StakedSettingsEdit = {
      oracle: null,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: null,
      riskTier: null,
      baseFeed: null,
      smoothedEntry: null,
      ...edit,
    };
    const tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          governanceAdmin: groupAdmin.wallet.publicKey,
          stakedSettings: settingsKey,
        })
        .remainingAccounts(
          editRemaining.map((pubkey) => ({
            pubkey,
            isSigner: false,
            isWritable: false,
          }))
        )
        .instruction(),
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: bankKey,
        oracle: feed,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  };

  it("(permissionless) prices the staked bank from Pyth - baseline", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const cache = await pulseStaked(oracles.wsolOracle.publicKey);
    assertI80F48Approx(cache.lastOraclePrice, oracles.wsolPrice);
    pythMultiplier = wrappedI80F48toBigNumber(cache.priceMultiplier).toNumber();
  });

  it("(governance admin) a smoothed entry needs a Scope base feed - fails on Pyth", async () => {
    // InvalidOracleSetup: the settings still read Pyth.
    const result = await tryEdit({
      smoothedEntry: { scope: { entryIndex: SOL_EMA_ENTRY.index } },
    });
    assertBankrunTxFailed(result, 6025);
  });

  it("(governance admin) moves staked banks to a Scope base feed and propagates", async () => {
    await freshenScopeFeed();
    await editAndPropagate(
      {
        oracle: scopeFeed,
        baseFeed: { scope: { entryIndex: SOL_ENTRY.index } },
      },
      scopeFeed
    );

    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assertKeysEqual(settingsAcc.oracle, scopeFeed);
    assert.equal(settingsAcc.scopeEntryIndex, SOL_ENTRY.index);
    assert.equal(settingsAcc.oracleSetup, ORACLE_SETUP_STAKED_WITH_SCOPE);
    assert.equal(settingsAcc.scopeSmoothedEntryIndex, 0);

    const { config } = await bankrunProgram.account.bank.fetch(bankKey);
    assert.deepEqual(config.oracleSetup, { stakedWithScope: {} });
    assertKeysEqual(config.oracleKeys[0], scopeFeed);
    assert.equal(config.scopeEntryIndex, SOL_ENTRY.index);
  });

  it("(permissionless) prices the staked bank from the Scope entry with the same pool rate", async () => {
    await freshenScopeFeed();
    const cache = await pulseStaked(scopeFeed);
    assertI80F48Approx(cache.lastOraclePrice, 175);
    assertI80F48Approx(cache.priceMultiplier, pythMultiplier);
  });

  it("(governance admin) rejects a smoothed entry of 0 or the base entry itself", async () => {
    for (const entryIndex of [0, SOL_ENTRY.index]) {
      // ScopeInvalidSmoothedEntry
      const result = await tryEdit({ smoothedEntry: { scope: { entryIndex } } });
      assertBankrunTxFailed(result, 6804);
    }
  });

  it("(governance admin) sets the SOL EMA as the smoothed entry - init margin uses it", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await freshenScopeFeed();
    const spotInitPrice = await stakedInitPrice(scopeFeed);

    await editAndPropagate(
      { smoothedEntry: { scope: { entryIndex: SOL_EMA_ENTRY.index } } },
      scopeFeed
    );

    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assert.equal(settingsAcc.scopeSmoothedEntryIndex, SOL_EMA_ENTRY.index);
    const { config } = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(config.scopeSmoothedEntryIndex, SOL_EMA_ENTRY.index);

    // Initial margin now prices at EMA x pool rate; the cached (liquidation) price stays spot.
    const emaInitPrice = await stakedInitPrice(scopeFeed);
    assert.approximately(emaInitPrice / spotInitPrice, 140 / 175, 1e-9);
    const cache = await pulseStaked(scopeFeed);
    assertI80F48Approx(cache.lastOraclePrice, 175);
  });

  it("(governance admin) a smoothed entry change is refused for a same-asset eligible bank", async () => {
    // The staked bank's feed family makes it eligible; the group has no registry yet.
    const admin = groupAdmin.mrgnBankrunProgram;
    const send = async (
      ...ixs: Awaited<ReturnType<typeof propagateStakedSettings>>[]
    ) => {
      const tx = new Transaction().add(...ixs);
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      return banksClient.tryProcessTransaction(tx);
    };
    const assertSucceeded = (
      result: Awaited<ReturnType<typeof send>>,
      what: string
    ) =>
      assert.isTrue(
        !("result" in result) || result.result == null,
        what + ": " + (result.meta?.logMessages ?? []).slice(-3).join("\n")
      );
    const eligibility = (enabled: boolean) =>
      setBankSameAssetEmodeEligibility(admin, {
        governanceAdmin: groupAdmin.wallet.publicKey,
        bank: bankKey,
        enabled,
      });
    let result = await send(
      await initSameAssetEmodeRegistry(admin, {
        group: marginfiGroup.publicKey,
        governanceAdmin: groupAdmin.wallet.publicKey,
      }),
      await eligibility(true)
    );
    assertSucceeded(result, "eligibility should be granted");

    // The settings edit lands, but propagating the changed entry to an eligible bank is refused
    // (BadEmodeConfig): the smoothed entry is part of the same-asset price identity.
    result = await tryEdit({ smoothedEntry: { disabled: {} } });
    assertSucceeded(result, "edit should succeed");
    result = await send(
      await propagateStakedSettings(bankrunProgram, {
        settings: settingsKey,
        bank: bankKey,
        oracle: scopeFeed,
      })
    );
    assertBankrunTxFailed(result, 6075);
    const { config } = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(config.scopeSmoothedEntryIndex, SOL_EMA_ENTRY.index);

    // With eligibility off the entry can move again; restore it for the tests below.
    result = await send(await eligibility(false));
    assertSucceeded(result, "eligibility should be revoked");
    await editAndPropagate(
      { smoothedEntry: { scope: { entryIndex: SOL_EMA_ENTRY.index } } },
      scopeFeed
    );
  });

  it("(governance admin) changing the Scope entry requires setting or disabling the smoothed entry", async () => {
    // InvalidOracleSetup: the entry was validated against the current base entry.
    let result = await tryEdit({
      baseFeed: { scope: { entryIndex: SOL_ENTRY.index + 1 } },
    });
    assertBankrunTxFailed(result, 6025);
    // Same for the Scope oracle account itself.
    result = await tryEdit({ oracle: Keypair.generate().publicKey });
    assertBankrunTxFailed(result, 6025);
  });

  it("(governance admin) moves staked banks back to Pyth - clears the base feed and the smoothed entry", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    // No explicit `smoothedEntry`: switching to Pyth clears it.
    await editAndPropagate(
      {
        oracle: oracles.wsolOracle.publicKey,
        baseFeed: { pyth: {} },
      },
      oracles.wsolOracle.publicKey
    );

    const settingsAcc = await bankrunProgram.account.stakedSettings.fetch(
      settingsKey
    );
    assert.equal(settingsAcc.scopeEntryIndex, 0);
    assert.equal(settingsAcc.oracleSetup, ORACLE_SETUP_STAKED_WITH_PYTH_PUSH);
    assert.equal(settingsAcc.scopeSmoothedEntryIndex, 0);

    const { config } = await bankrunProgram.account.bank.fetch(bankKey);
    assert.deepEqual(config.oracleSetup, { stakedWithPythPush: {} });
    assertKeysEqual(config.oracleKeys[0], oracles.wsolOracle.publicKey);
    assert.equal(config.scopeEntryIndex, 0);
    assert.equal(config.scopeSmoothedEntryIndex, 0);

    const cache = await pulseStaked(oracles.wsolOracle.publicKey);
    assertI80F48Approx(cache.lastOraclePrice, oracles.wsolPrice);
  });
});
