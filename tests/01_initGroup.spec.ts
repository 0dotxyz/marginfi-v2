import { BN, Program } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  editStakedSettings,
  groupInitialize,
  initStakedSettings,
} from "./utils/group-instructions";
import { Marginfi } from "../target/types/marginfi";
import {
  bankrunProgram,
  globalFeeWallet,
  groupAdmin,
  marginfiGroup,
  oracles,
  PROGRAM_FEE_FIXED,
  PROGRAM_FEE_RATE,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNEqual,
  assertI80F48Approx,
  assertKeysEqual,
} from "./utils/genericTests";
import { assert } from "chai";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { deriveStakedSettings } from "./utils/pdas";
import {
  defaultStakedInterestSettings,
  StakedSettingsEdit,
} from "./utils/types";

let program: Program<Marginfi>;

describe("Init group", () => {
  before(() => {
    program = bankrunProgram;
  });

  it("(admin) Init group - happy path", async () => {
    let tx = new Transaction();

    tx.add(
      await groupInitialize(program, {
        marginfiGroup: marginfiGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      })
    );

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(tx, [marginfiGroup]);

    let group = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
    if (verbose) {
      console.log("*init group: " + marginfiGroup.publicKey);
      console.log(" group admin: " + group.admin);
    }

    const feeCache = group.feeStateCache;
    const tolerance = 0.00001;
    assertI80F48Approx(feeCache.programFeeFixed, PROGRAM_FEE_FIXED, tolerance);
    assertI80F48Approx(feeCache.programFeeRate, PROGRAM_FEE_RATE, tolerance);
    assertKeysEqual(feeCache.globalFeeWallet, globalFeeWallet);
  });

  it("(attacker) Tries to init staked settings - should fail", async () => {
    const settings = defaultStakedInterestSettings(
      oracles.wsolOracle.publicKey
    );
    let failed = false;
    try {
      await users[0].mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await initStakedSettings(users[0].mrgnProgram, {
            group: marginfiGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            settings: settings,
          })
        )
      );
    } catch (err) {
      // generic signature error
      failed = true;
    }

    assert.ok(failed, "Transaction succeeded when it should have failed");
  });

  it("(admin) Init staked settings for group - opts in to use staked collateral", async () => {
    const settings = defaultStakedInterestSettings(
      oracles.wsolOracle.publicKey
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await initStakedSettings(groupAdmin.mrgnProgram, {
          group: marginfiGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          settings: settings,
        })
      )
    );

    const [settingsKey] = deriveStakedSettings(
      program.programId,
      marginfiGroup.publicKey
    );
    if (verbose) {
      console.log("*init staked settings: " + settingsKey);
    }

    let settingsAcc = await program.account.stakedSettings.fetch(settingsKey);
    assertKeysEqual(settingsAcc.key, settingsKey);
    assertKeysEqual(settingsAcc.oracle, oracles.wsolOracle.publicKey);
    assertI80F48Approx(settingsAcc.assetWeightInit, 0.8);
    assertI80F48Approx(settingsAcc.assetWeightMaint, 0.9);
    assertBNEqual(settingsAcc.depositLimit, 1_000_000_000_000);
    assertBNEqual(settingsAcc.totalAssetValueInitLimit, 150_000_000);
    assert.equal(settingsAcc.oracleMaxAge, 60);
    assert.deepEqual(settingsAcc.riskTier, { collateral: {} });
  });

  it("(attacker) Tries to edit staked settings - should fail", async () => {
    const settings: StakedSettingsEdit = {
      oracle: PublicKey.default,
      assetWeightInit: bigNumberToWrappedI80F48(0.2),
      assetWeightMaint: bigNumberToWrappedI80F48(0.3),
      depositLimit: new BN(42),
      totalAssetValueInitLimit: new BN(43),
      oracleMaxAge: 44,
      riskTier: {
        isolated: undefined,
      },
    };
    let failed = false;
    try {
      const [settingsKey] = deriveStakedSettings(
        program.programId,
        marginfiGroup.publicKey
      );

      await users[0].mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await editStakedSettings(users[0].mrgnProgram, {
            settingsKey: settingsKey,
            settings: settings,
          })
        )
      );
    } catch (err) {
      // generic signature error
      failed = true;
    }
    assert.ok(failed, "Transaction succeeded when it should have failed");
  });

  // Note: there are no Staked Collateral positions in the end to end test suite (those are in the
  // BankRun suite e.g. s01) so these settings do nothing.

  it("(admin) Edit staked settings for group", async () => {
    const settings: StakedSettingsEdit = {
      oracle: PublicKey.default,
      assetWeightInit: bigNumberToWrappedI80F48(0.2),
      assetWeightMaint: bigNumberToWrappedI80F48(0.3),
      depositLimit: new BN(42),
      totalAssetValueInitLimit: new BN(43),
      oracleMaxAge: 44,
      riskTier: {
        collateral: undefined,
      },
    };
    const [settingsKey] = deriveStakedSettings(
      program.programId,
      marginfiGroup.publicKey
    );

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await editStakedSettings(groupAdmin.mrgnProgram, {
          settingsKey: settingsKey,
          settings: settings,
        })
      )
    );

    if (verbose) {
      console.log("*edit staked settings: " + settingsKey);
    }

    let settingsAcc = await program.account.stakedSettings.fetch(settingsKey);
    assertKeysEqual(settingsAcc.key, settingsKey);
    assertKeysEqual(settingsAcc.oracle, PublicKey.default);
    assertI80F48Approx(settingsAcc.assetWeightInit, 0.2);
    assertI80F48Approx(settingsAcc.assetWeightMaint, 0.3);
    assertBNEqual(settingsAcc.depositLimit, 42);
    assertBNEqual(settingsAcc.totalAssetValueInitLimit, 43);
    assert.equal(settingsAcc.oracleMaxAge, 44);
    assert.deepEqual(settingsAcc.riskTier, { collateral: {} }); // no change
  });

  it("(admin) Partial settings update", async () => {
    const settings: StakedSettingsEdit = {
      oracle: null,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: 60,
      riskTier: null,
    };
    const [settingsKey] = deriveStakedSettings(
      program.programId,
      marginfiGroup.publicKey
    );

    await groupAdmin.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await editStakedSettings(groupAdmin.mrgnProgram, {
          settingsKey: settingsKey,
          settings: settings,
        })
      )
    );

    let settingsAcc = await program.account.stakedSettings.fetch(settingsKey);
    // No change
    assertKeysEqual(settingsAcc.key, settingsKey);
    assertKeysEqual(settingsAcc.oracle, PublicKey.default);
    assertI80F48Approx(settingsAcc.assetWeightInit, 0.2);
    assertI80F48Approx(settingsAcc.assetWeightMaint, 0.3);
    assertBNEqual(settingsAcc.depositLimit, 42);
    assertBNEqual(settingsAcc.totalAssetValueInitLimit, 43);
    assert.deepEqual(settingsAcc.riskTier, { collateral: {} });

    assert.equal(settingsAcc.oracleMaxAge, 60);
  });

  // Note: Isolated riskTier requires the weights to be zero, so this is invalid...
  it("(admin) Bad settings update - should fail", async () => {
    const settings: StakedSettingsEdit = {
      oracle: null,
      assetWeightInit: null,
      assetWeightMaint: null,
      depositLimit: null,
      totalAssetValueInitLimit: null,
      oracleMaxAge: 60,
      riskTier: {
        isolated: undefined,
      },
    };
    const [settingsKey] = deriveStakedSettings(
      program.programId,
      marginfiGroup.publicKey
    );

    let failed = false;
    try {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm(
        new Transaction().add(
          await editStakedSettings(groupAdmin.mrgnProgram, {
            settingsKey: settingsKey,
            settings: settings,
          })
        )
      );
    } catch (err) {
      // TODO create a util for this that fails with more detail
      assert.ok(
        err.logs.some((log: string) =>
          log.includes("Error Code: InvalidConfig")
        )
      );
      failed = true;
    }
    assert.ok(failed, "Transaction succeeded when it should have failed");
  });
});
