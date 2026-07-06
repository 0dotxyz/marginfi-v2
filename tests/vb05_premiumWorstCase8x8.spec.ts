import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import { assert } from "chai";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  emodeAdmin,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  addBankWithSeed,
  groupConfigure,
  groupInitialize,
} from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
} from "./utils/user-instructions";
import {
  configBankPremium,
  configGroupPremium,
  copyFeeStateToV2,
  deriveFeeStateV2,
  initGlobalFeeStateV2,
  newPremiumEntry,
  u32ToPremiumRate,
} from "./utils/premium-instructions";
import {
  BankConfig,
  defaultBankConfig,
  I80F48_ONE,
  I80F48_ZERO,
  makeRatePoints,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import { deriveBankWithSeed } from "./utils/pdas";
import { getBankrunBlockhash, processBankrunTransaction } from "./utils/tools";
import { assertBankrunTxFailed } from "./utils/genericTests";

// 8 collateral banks x 8 liability banks, one account with 16 active balances.
const N = 8;
const BANK_SEED_BASE = 300;
const RATE_STEP = 0.0005; // 0.05% APR increment
const COLLATERAL_TAG_BASE = 1000;
const LIABILITY_TAG_BASE = 2000;
const GROUP_SEED = Buffer.from("PREMIUM_8X8_GROUP_SEED_000000000");
const group8x8 = Keypair.fromSeed(GROUP_SEED);

/** rate(i,j) for collateral i, liability j: strictly increasing, all 64 distinct. */
const pairRate = (i: number, j: number) => (i * N + j + 1) * RATE_STEP;
/** Equal collateral USD => snapshot for liability j = average of column j. */
const expectedSnapshot = (j: number) => RATE_STEP * (j + 1 + N * ((N - 1) / 2));

const lstNative = (n: number) =>
  new BN(n * 10 ** ecosystem.lstAlphaDecimals);

const collateralConfig = (): BankConfig => {
  const config = defaultBankConfig();
  config.assetWeightInit = bigNumberToWrappedI80F48(0.9);
  config.assetWeightMaint = bigNumberToWrappedI80F48(0.95);
  config.depositLimit = new BN(100_000_000_000_000);
  config.borrowLimit = new BN(100_000_000_000_000);
  config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;
  return config;
};

const liabilityConfig = (): BankConfig => {
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
  return config;
};

describe("vb05: Premium worst case (8x8 matrix, 16 balances)", () => {
  const lstOracle = () => oracles.pythPullLst.publicKey;
  // banks[0..7] = collateral, banks[8..15] = liability
  const banks: PublicKey[] = [];
  let borrowerAccount: PublicKey;

  const addBank = async (seedOffset: number, config: BankConfig) => {
    const seed = new BN(BANK_SEED_BASE + seedOffset);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      group8x8.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed,
    );
    const oracleIx = await groupAdmin.mrgnProgram.methods
      .lendingPoolConfigureBankOracle(ORACLE_SETUP_PYTH_PUSH, lstOracle())
      .accountsPartial({
        group: group8x8.publicKey,
        bank: bankKey,
        admin: groupAdmin.wallet.publicKey,
      })
      .remainingAccounts([
        { pubkey: lstOracle(), isSigner: false, isWritable: false },
      ])
      .instruction();
    const addIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: group8x8.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.lstAlphaMint.publicKey,
      config,
      seed,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addIx, oracleIx),
      [groupAdmin.wallet],
    );
    return bankKey;
  };

  it("init group + FeeStateV2, fund LST", async () => {
    // Group + emode admin
    let tx = new Transaction().add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: group8x8.publicKey,
        admin: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, group8x8);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: group8x8.publicKey,
        newEmodeAdmin: emodeAdmin.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    // FeeStateV2 is global: init+copy only if a prior vb spec hasn't already.
    const [feeStateV2Key] = deriveFeeStateV2(bankrunProgram.programId);
    const existing = await banksClient.getAccount(feeStateV2Key);
    if (!existing) {
      tx = new Transaction().add(
        await initGlobalFeeStateV2(globalProgramAdmin.mrgnBankrunProgram, {
          payer: globalProgramAdmin.wallet.publicKey,
        }),
        await copyFeeStateToV2(globalProgramAdmin.mrgnBankrunProgram),
      );
      await processBankrunTransaction(bankrunContext, tx, [
        globalProgramAdmin.wallet,
      ]);
    }

    // Fund borrower + lender with LST.
    const payer = bankrunContext.payer;
    for (const u of [groupAdmin, users[0]]) {
      const fundTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.lstAlphaMint.publicKey,
          u.lstAlphaAccount,
          payer.publicKey,
          10_000_000 * 10 ** ecosystem.lstAlphaDecimals,
        ),
      );
      fundTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      fundTx.sign(payer);
      await banksClient.processTransaction(fundTx);
    }
  });

  it("add 16 banks (8 collateral, 8 liability)", async () => {
    for (let i = 0; i < N; i++) {
      banks[i] = await addBank(i, collateralConfig());
    }
    for (let j = 0; j < N; j++) {
      banks[N + j] = await addBank(N + j, liabilityConfig());
    }
    if (verbose) console.log("*added " + banks.length + " banks");
  });

  it("(emode admin) configure 64 distinct matrix entries + 16 bank tags", async () => {
    const entries = [];
    for (let i = 0; i < N; i++) {
      for (let j = 0; j < N; j++) {
        entries.push(
          newPremiumEntry(
            COLLATERAL_TAG_BASE + i,
            LIABILITY_TAG_BASE + j,
            pairRate(i, j),
          ),
        );
      }
    }
    assert.equal(entries.length, 64);
    // One pair per instruction; batch 32 per tx to stay under the tx size limit.
    for (let start = 0; start < entries.length; start += 32) {
      const tx = new Transaction();
      for (const entry of entries.slice(start, start + 32)) {
        tx.add(
          await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
            group: group8x8.publicKey,
            entry,
          }),
        );
      }
      await processBankrunTransaction(bankrunContext, tx, [emodeAdmin.wallet]);
    }

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      group8x8.publicKey,
    );
    assert.equal(group.premiumSettings.entryCount, 64);

    // The matrix is full: a 65th pair -> PremiumMatrixFull (6601)
    const overfullTx = new Transaction().add(
      await configGroupPremium(emodeAdmin.mrgnBankrunProgram, {
        group: group8x8.publicKey,
        entry: newPremiumEntry(999, 999, 0.01),
      }),
    );
    overfullTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    overfullTx.sign(emodeAdmin.wallet);
    assertBankrunTxFailed(
      await banksClient.tryProcessTransaction(overfullTx),
      "0x19c9",
    );

    // Tag every bank; liability banks are premium-active. Batch to keep tx sizes small.
    const tagIxs = [];
    for (let i = 0; i < N; i++) {
      tagIxs.push(
        await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
          bank: banks[i],
          premiumTag: COLLATERAL_TAG_BASE + i,
          active: true,
        }),
      );
    }
    for (let j = 0; j < N; j++) {
      tagIxs.push(
        await configBankPremium(emodeAdmin.mrgnBankrunProgram, {
          bank: banks[N + j],
          premiumTag: LIABILITY_TAG_BASE + j,
          active: true,
        }),
      );
    }
    for (let k = 0; k < tagIxs.length; k += 4) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(...tagIxs.slice(k, k + 4)),
        [emodeAdmin.wallet],
      );
    }
  });

  it("(admin) seeds liquidity in the 8 liability banks", async () => {
    // groupAdmin needs a marginfi account on this group.
    const kp = Keypair.generate();
    let tx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: group8x8.publicKey,
        marginfiAccount: kp.publicKey,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, kp);
    await banksClient.processTransaction(tx);

    // 5 deposits per tx (no LUT) as in m01.
    for (let j = 0; j < N; j += 5) {
      const chunk = banks.slice(N + j, N + Math.min(j + 5, N));
      const depositTx = new Transaction();
      for (const bank of chunk) {
        depositTx.add(
          await depositIx(groupAdmin.mrgnBankrunProgram, {
            marginfiAccount: kp.publicKey,
            bank,
            tokenAccount: groupAdmin.lstAlphaAccount,
            amount: lstNative(10_000),
          }),
        );
      }
      await processBankrunTransaction(bankrunContext, depositTx, [
        groupAdmin.wallet,
      ]);
    }
  });

  it("(user 0) deposits 8 collateral + borrows 8 liabilities (16 balances)", async () => {
    const user = users[0];
    const kp = Keypair.generate();
    borrowerAccount = kp.publicKey;
    let tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: group8x8.publicKey,
        marginfiAccount: kp.publicKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet, kp);
    await banksClient.processTransaction(tx);

    // 8 equal collateral deposits (equal USD => equal premium weighting).
    for (let i = 0; i < N; i += 5) {
      const depositTx = new Transaction();
      for (let k = i; k < Math.min(i + 5, N); k++) {
        depositTx.add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: borrowerAccount,
            bank: banks[k],
            tokenAccount: user.lstAlphaAccount,
            amount: lstNative(1_000),
          }),
        );
      }
      await processBankrunTransaction(bankrunContext, depositTx, [user.wallet]);
    }

    // 8 small borrows, one per liability bank. Remaining accounts grow with each new balance.
    for (let j = 0; j < N; j++) {
      const activeBanks: PublicKey[][] = [];
      for (let c = 0; c < N; c++) activeBanks.push([banks[c], lstOracle()]);
      for (let l = 0; l <= j; l++) activeBanks.push([banks[N + l], lstOracle()]);

      const borrowTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: borrowerAccount,
          bank: banks[N + j],
          tokenAccount: user.lstAlphaAccount,
          remaining: composeRemainingAccounts(activeBanks),
          amount: lstNative(1),
        }),
      );
      await processBankrunTransaction(bankrunContext, borrowTx, [user.wallet]);
    }

    const account =
      await bankrunProgram.account.marginfiAccount.fetch(borrowerAccount);
    const activeCount = account.lendingAccount.balances.filter(
      (b: any) => b.active,
    ).length;
    assert.equal(activeCount, 16, "expected 16 active balances");
  });

  it("pulseHealth: distinct snapshots, CU < 1.3M, tx size logged", async () => {
    const allBanks: PublicKey[][] = [];
    for (let k = 0; k < 2 * N; k++) allBanks.push([banks[k], lstOracle()]);

    const pulseIx = await healthPulse(users[0].mrgnBankrunProgram, {
      marginfiAccount: borrowerAccount,
      group: group8x8.publicKey,
      remaining: composeRemainingAccounts(allBanks),
    });
    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      pulseIx,
    );
    tx.feePayer = users[0].wallet.publicKey;
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);

    const serializedSize = tx.serialize().length;
    const uniqueKeys = tx.compileMessage().accountKeys.length;
    console.log(
      `*8x8 pulse: serialized tx = ${serializedSize} bytes, ${uniqueKeys} unique account keys`,
    );
    // Shared LST oracle keeps the deduplicated account-key set small enough for a legacy tx.
    assert.isAtMost(
      serializedSize,
      1232,
      `pulse tx is ${serializedSize} bytes, exceeds the 1232 legacy limit`,
    );

    const result = await banksClient.processTransaction(tx);
    const cu = Number(result.computeUnitsConsumed);
    console.log(`*8x8 pulse: compute units consumed = ${cu}`);
    assert.isBelow(cu, 1_300_000, "pulse exceeded the CU budget");

    // Each of the 8 liability balances carries a distinct, hand-computable snapshot.
    const account =
      await bankrunProgram.account.marginfiAccount.fetch(borrowerAccount);
    const seen = new Set<number>();
    for (let j = 0; j < N; j++) {
      const liabBank = banks[N + j];
      const balance = account.lendingAccount.balances.find(
        (b: any) => b.active && b.bankPk.equals(liabBank),
      );
      assert.ok(balance, `liability balance ${j} missing`);
      const rate = u32ToPremiumRate(balance.premiumRateSnapshot);
      seen.add(Math.round(rate * 1e6));
      if (verbose) {
        console.log(
          `  liab[${j}] snapshot ${(rate * 100).toFixed(4)}% (expected ${(
            expectedSnapshot(j) * 100
          ).toFixed(4)}%)`,
        );
      }
      assert.approximately(rate, expectedSnapshot(j), 0.00005);
    }
    assert.equal(seen.size, N, "expected 8 distinct snapshot values");
  });
});
