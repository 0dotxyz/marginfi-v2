import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  emodeAdmin,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  premiumGroup,
  PREMIUM_SEED,
  PREMIUM_SOL_TAG,
  PREMIUM_SOL_TO_STABLE,
  PREMIUM_STABLE_TAG,
  users,
  verbose,
} from "../../rootHooks";
import {
  addBankWithSeed,
  groupConfigure,
  groupInitialize,
} from "../../utils/group-instructions";
import {
  configBankPremium,
  configGroupPremium,
  editFeeStatePremium,
  MAX_PREMIUM_ENTRIES,
  newPremiumEntry,
  PREMIUM_ACTIVE,
  premiumRateToU32,
} from "../../utils/premium-instructions";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  assertKeysEqual,
} from "../../utils/genericTests";
import {
  BankConfig,
  defaultBankConfig,
  I80F48_ONE,
  I80F48_ZERO,
  makeRatePoints,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { deriveBankWithSeed, deriveGlobalFeeState } from "../../utils/pdas";
import { getBankrunBlockhash, processBankrunTransaction } from "../../utils/tools";

// Bank seeds for the premium suite. Re-derived (not exported) by every vb* spec.
export const USDC_SEED = new BN(PREMIUM_SEED);
export const SOL_SEED = new BN(PREMIUM_SEED + 1);
export const SOL_UNTAGGED_SEED = new BN(PREMIUM_SEED + 2);

/** USDC liability bank with all interest/fees zeroed so debt growth is purely premium. */
export const zeroInterestConfig = (): BankConfig => {
  const config = defaultBankConfig();
  config.assetWeightInit = I80F48_ONE;
  config.assetWeightMaint = I80F48_ONE;
  config.depositLimit = new BN(100_000_000_000_000);
  config.borrowLimit = new BN(100_000_000_000_000);
  const rate = config.interestRateConfig;
  rate.zeroUtilRate = 0;
  rate.hundredUtilRate = 0;
  rate.points = makeRatePoints([], []);
  rate.protocolOriginationFee = I80F48_ZERO;
  rate.insuranceFeeFixedApr = I80F48_ZERO;
  rate.insuranceIrFee = I80F48_ZERO;
  rate.protocolFixedFeeApr = I80F48_ZERO;
  rate.protocolIrFee = I80F48_ZERO;
  return config;
};

/** Collateral bank: full asset weight, large limits, no origination fee. */
export const collateralConfig = (): BankConfig => {
  const config = defaultBankConfig();
  config.assetWeightInit = I80F48_ONE;
  config.assetWeightMaint = I80F48_ONE;
  config.depositLimit = new BN(100_000_000_000_000);
  config.borrowLimit = new BN(100_000_000_000_000);
  config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
  return config;
};

/** Add a bank to the premium group and set its oracle in a single tx. Returns the bank key. */
export const addPremiumBank = async (opts: {
  mint: PublicKey;
  oracle: PublicKey;
  seed: BN;
  config: BankConfig;
}): Promise<PublicKey> => {
  const [bankKey] = deriveBankWithSeed(
    bankrunProgram.programId,
    premiumGroup.publicKey,
    opts.mint,
    opts.seed,
  );

  const configOracleIx = await groupAdmin.mrgnProgram.methods
    .lendingPoolConfigureBankOracle(ORACLE_SETUP_PYTH_PUSH, opts.oracle)
    .accountsPartial({
      group: premiumGroup.publicKey,
      bank: bankKey,
      admin: groupAdmin.wallet.publicKey,
    })
    .remainingAccounts([
      { pubkey: opts.oracle, isSigner: false, isWritable: false },
    ])
    .instruction();

  const addIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
    marginfiGroup: premiumGroup.publicKey,
    feePayer: groupAdmin.wallet.publicKey,
    bankMint: opts.mint,
    config: opts.config,
    seed: opts.seed,
  });

  const tx = new Transaction().add(addIx, configOracleIx);
  await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  return bankKey;
};

describe("vb01: Configure variable-borrow premium", () => {
  let usdcBank: PublicKey;
  let solBank: PublicKey;
  let solUntaggedBank: PublicKey;

  it("(admin) init premium group + set emode admin", async () => {
    const tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: premiumGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, premiumGroup);
    await banksClient.processTransaction(tx);

    const configTx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: premiumGroup.publicKey,
        newEmodeAdmin: emodeAdmin.wallet.publicKey,
      }),
    );
    configTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    configTx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(configTx);

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      premiumGroup.publicKey,
    );
    assertKeysEqual(group.emodeAdmin, emodeAdmin.wallet.publicKey);
    if (verbose) console.log("*init premium group: " + premiumGroup.publicKey);
  });

  it("(admin) add banks (usdc liability, sol collateral, sol untagged)", async () => {
    usdcBank = await addPremiumBank({
      mint: ecosystem.usdcMint.publicKey,
      oracle: oracles.usdcOracle.publicKey,
      seed: USDC_SEED,
      config: zeroInterestConfig(),
    });
    solBank = await addPremiumBank({
      mint: ecosystem.wsolMint.publicKey,
      oracle: oracles.wsolOracle.publicKey,
      seed: SOL_SEED,
      config: collateralConfig(),
    });
    solUntaggedBank = await addPremiumBank({
      mint: ecosystem.wsolMint.publicKey,
      oracle: oracles.wsolOracle.publicKey,
      seed: SOL_UNTAGGED_SEED,
      config: collateralConfig(),
    });
    if (verbose) {
      console.log("*usdc bank:      " + usdcBank);
      console.log("*sol bank:       " + solBank);
      console.log("*sol (untagged): " + solUntaggedBank);
    }
  });

  it("Fund premium-suite users/admin (USDC + WSOL)", async () => {
    const payer = bankrunContext.payer;
    const recipients = [groupAdmin, ...users];
    for (const u of recipients) {
      const tx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          u.usdcAccount,
          payer.publicKey,
          1_000_000 * 10 ** ecosystem.usdcDecimals,
        ),
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          u.wsolAccount,
          payer.publicKey,
          100_000 * 10 ** ecosystem.wsolDecimals,
        ),
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(payer);
      await banksClient.processTransaction(tx);
    }
  });

  it("fee state starts with no premium wallet", async () => {
    const [feeStateKey] = deriveGlobalFeeState(bankrunProgram.programId);
    const feeState = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assertKeysEqual(feeState.globalFeeAdmin, globalProgramAdmin.wallet.publicKey);
    assertKeysEqual(feeState.premiumWallet, PublicKey.default);
  });

  it("(non-admin) edit fee state premium fails", async () => {
    const tx = new Transaction().add(
      await editFeeStatePremium(users[0].mrgnBankrunProgram, {
        admin: users[0].wallet.publicKey,
        premiumWallet: PublicKey.unique(),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // 6042 Unauthorized
    assertBankrunTxFailed(result, "0x179a");
  });

  it("(global fee admin) sets premium wallet", async () => {
    const premiumWallet = PublicKey.unique();
    const tx = new Transaction().add(
      await editFeeStatePremium(globalProgramAdmin.mrgnBankrunProgram, {
        admin: globalProgramAdmin.wallet.publicKey,
        premiumWallet,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [
      globalProgramAdmin.wallet,
    ]);

    const [feeStateKey] = deriveGlobalFeeState(bankrunProgram.programId);
    const feeState = await bankrunProgram.account.feeState.fetch(feeStateKey);
    assertKeysEqual(feeState.premiumWallet, premiumWallet);
  });

  it("(non-emode-admin) configure matrix fails", async () => {
    const tx = new Transaction().add(
      await configGroupPremium(users[0].mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(1, 2, 0.01),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);
    const result = await banksClient.tryProcessTransaction(tx);
    // 6042 Unauthorized
    assertBankrunTxFailed(result, "0x179a");
  });

  it("matrix validation: zero tags and delete-missing rejected", async () => {
    const submit = async (entry: ReturnType<typeof newPremiumEntry>) => {
      const tx = new Transaction().add(
        await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
          group: premiumGroup.publicKey,
          entry,
        }),
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(emodeAdmin.wallet);
      return banksClient.tryProcessTransaction(tx);
    };

    // Zero collateral tag -> PremiumEntryInvalid (6610)
    assertBankrunTxFailed(await submit(newPremiumEntry(0, 100, 0.01)), "0x19d2");
    // Zero liability tag -> PremiumEntryInvalid (6610)
    assertBankrunTxFailed(await submit(newPremiumEntry(100, 0, 0.01)), "0x19d2");
    // Removing a pair that is not in the matrix -> PremiumEntryNotFound (6614)
    assertBankrunTxFailed(await submit(newPremiumEntry(9, 9, 0)), "0x19d6");
  });

  it("(emode admin) pairs are stored sorted regardless of add order", async () => {
    // Two pairs added out of collateral-tag order; storage must sort ascending.
    const tx = new Transaction().add(
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(PREMIUM_SOL_TAG, PREMIUM_STABLE_TAG, 0.01),
      }),
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(PREMIUM_STABLE_TAG, PREMIUM_SOL_TAG, 0.005),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      premiumGroup.publicKey,
    );
    assert.equal(group.premiumSettings.entryCount, 2);
    assert.equal(group.premiumSettings.entryCapacity, MAX_PREMIUM_ENTRIES);
    // Sorted ascending by collateral tag: STABLE(100) then SOL(200)
    assert.equal(group.premiumEntries[0].collateralTag, PREMIUM_STABLE_TAG);
    assert.equal(group.premiumEntries[1].collateralTag, PREMIUM_SOL_TAG);
  });

  it("(emode admin) re-config updates a pair in place; rate 0 removes it", async () => {
    // From the previous spec the matrix is [(STABLE, SOL, 0.5%), (SOL, STABLE, 1%)]
    const tx = new Transaction().add(
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(PREMIUM_SOL_TAG, PREMIUM_STABLE_TAG, 0.02), // update
      }),
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(PREMIUM_STABLE_TAG, PREMIUM_SOL_TAG, 0), // delete
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      premiumGroup.publicKey,
    );
    assert.equal(group.premiumSettings.entryCount, 1);
    assert.equal(group.premiumEntries[0].collateralTag, PREMIUM_SOL_TAG);
    assert.equal(group.premiumEntries[0].rate, premiumRateToU32(0.02));
    // The vacated slot behind entryCount is zeroed
    assert.equal(group.premiumEntries[1].collateralTag, 0);
  });

  it("(emode admin) removing the last pair turns the matrix off (entryCount 0)", async () => {
    const tx = new Transaction().add(
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(PREMIUM_SOL_TAG, PREMIUM_STABLE_TAG, 0),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      premiumGroup.publicKey,
    );
    assert.equal(group.premiumSettings.entryCount, 0);
  });

  it("(emode admin) set the production matrix (sol -> stable = 1%)", async () => {
    const tx = new Transaction().add(
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: premiumGroup.publicKey,
        entry: newPremiumEntry(
          PREMIUM_SOL_TAG,
          PREMIUM_STABLE_TAG,
          PREMIUM_SOL_TO_STABLE,
        ),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      premiumGroup.publicKey,
    );
    assert.equal(group.premiumSettings.entryCount, 1);
  });

  it("(emode admin) configure bank premium tags + flags", async () => {
    // USDC (liability) bank: tag + active.
    let tx = new Transaction().add(
      await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
        bank: usdcBank,
        premiumTag: PREMIUM_STABLE_TAG,
        active: true,
      }),
      await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
        bank: solBank,
        premiumTag: PREMIUM_SOL_TAG,
        active: true,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);

    let usdc = await bankrunProgram.account.bank.fetch(usdcBank);
    assert.equal(usdc.premiumTag, PREMIUM_STABLE_TAG);
    assertBNEqual(usdc.flags.and(new BN(PREMIUM_ACTIVE)), PREMIUM_ACTIVE);

    // Disabling clears the flag but keeps the tag.
    tx = new Transaction().add(
      await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
        bank: usdcBank,
        premiumTag: PREMIUM_STABLE_TAG,
        active: false,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);
    usdc = await bankrunProgram.account.bank.fetch(usdcBank);
    assert.equal(usdc.premiumTag, PREMIUM_STABLE_TAG);
    assertBNEqual(usdc.flags.and(new BN(PREMIUM_ACTIVE)), 0);

    // Re-enable for the downstream accrual specs.
    tx = new Transaction().add(
      await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
        bank: usdcBank,
        premiumTag: PREMIUM_STABLE_TAG,
        active: true,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);
  });

});
