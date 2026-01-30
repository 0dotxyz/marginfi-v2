import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import BN from "bn.js";
import { Program } from "@coral-xyz/anchor";
import { Marginfi } from "../target/types/marginfi";
import {
  stakedMarginfiGroup,
  validators,
  groupAdmin,
  oracles,
  bankrunContext,
  banksClient,
  bankrunProgram,
} from "./rootHooks";
import { propagateStakedSettings } from "./utils/group-instructions";
import { deriveBankWithSeed, deriveStakedSettings } from "./utils/pdas";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import {
  assertKeysEqual,
  assertI80F48Approx,
  assertBNEqual,
  assertBankrunTxFailed,
} from "./utils/genericTests";
import {
  defaultStakedInterestSettings,
  PYTH_PULL_MIGRATED,
  StakedSettingsEdit,
} from "./utils/types";

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

  it("(admin) edits some settings - happy path", async () => {
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
    };
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
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

  it("(admin) sets a bad oracle - fails at propagation", async () => {
    const settings: StakedSettingsEdit = {
      oracle: oracles.wsolOracle.publicKey,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: null,
      riskTier: null,
    };
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
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

  it("(admin) restores default settings - happy path", async () => {
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
    };
    // Note you can pack propagates into the edit tx, so with a LUT you can easily propagate
    // hundreds of banks in the same ts as edit
    let tx = new Transaction().add(
      await bankrunProgram.methods
        .editStakedSettings(settings)
        .accountsPartial({
          marginfiGroup: marginfiGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
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
});
