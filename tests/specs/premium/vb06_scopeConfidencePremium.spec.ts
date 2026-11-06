import { BN } from "@coral-xyz/anchor";
import { createMintToInstruction } from "@solana/spl-token";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import {
  addBankWithSeed,
  configureBank,
  configureBankOracle,
  configureBankOracleScope,
  groupInitialize,
} from "../../utils/group-instructions";
import { deriveBankWithSeed } from "../../utils/pdas";
import {
  configBankPremium,
  configGroupPremium,
  newPremiumEntry,
  u32ToPremiumRate,
} from "../../utils/premium-instructions";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";
import {
  advanceBankrunClock,
  getBankrunTime,
  processBankrunTransaction,
} from "../../utils/tools";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  I80F48_ONE,
  I80F48_ZERO,
  makeRatePoints,
  ORACLE_SETUP_PYTH_PUSH,
  u32_MAX,
} from "../../utils/types";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
} from "../../utils/user-instructions";

describe("vb06: Scope confidence and premium accrual", () => {
  const group = Keypair.generate();
  const scopeFeed = Keypair.generate().publicKey;
  let borrower: (typeof users)[number];
  const borrowerAccount = Keypair.generate();
  const lenderAccount = Keypair.generate();
  const year = 365 * 24 * 60 * 60;
  const usdcUnit = 10 ** ecosystem.usdcDecimals;
  const solUnit = 10 ** ecosystem.wsolDecimals;
  const liabilityTag = 101;
  const highRateTag = 201;
  const lowRateTag = 202;
  const highRate = 0.1;
  const lowRate = 0.02;
  const expectedRate = (highRate + lowRate) / 2;
  const debt = 1000;
  const seed = [new BN(610), new BN(611), new BN(612)];

  let highRateBank: PublicKey;
  let lowRateBank: PublicKey;
  let debtBank: PublicKey;

  const remaining = () =>
    composeRemainingAccounts([
      [highRateBank, scopeFeed],
      [lowRateBank, scopeFeed],
      [debtBank, oracles.usdcOracle.publicKey],
    ]);

  const refreshOracles = async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    setScopeFeed(
      bankrunContext,
      scopeFeed,
      makeScopePrices([
        {
          index: 17,
          value: 100n,
          exp: 0n,
          timestamp: await getBankrunTime(bankrunContext),
        },
      ]),
    );
  };

  const pulse = async () => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await healthPulse(borrower.mrgnBankrunProgram, {
          marginfiAccount: borrowerAccount.publicKey,
          group: group.publicKey,
          remaining: remaining(),
        }),
      ),
      [borrower.wallet],
    );
    const account = await bankrunProgram.account.marginfiAccount.fetch(
      borrowerAccount.publicKey,
    );
    const liability = account.lendingAccount.balances.find(
      (balance) => balance.active === 1 && balance.bankPk.equals(debtBank),
    );
    assert.isDefined(liability, "expected the USDC liability");
    return {
      rate: u32ToPremiumRate(liability!.premiumRateSnapshot),
      outstanding:
        wrappedI80F48toBigNumber(liability!.premiumOutstanding).toNumber() /
        usdcUnit,
      assets: wrappedI80F48toBigNumber(
        account.healthCache.assetValue,
      ).toNumber(),
    };
  };

  before(async () => {
    borrower = users[0];
    await refreshOracles();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          admin: groupAdmin.wallet.publicKey,
        }),
      ),
      [groupAdmin.wallet, group],
    );

    // Zero base interest so that the only debt growth measured below is the premium.
    const config = defaultBankConfig();
    config.assetWeightInit = I80F48_ONE;
    config.assetWeightMaint = I80F48_ONE;
    config.depositLimit = new BN(100_000_000_000_000);
    config.borrowLimit = new BN(100_000_000_000_000);
    const interest = config.interestRateConfig;
    interest.zeroUtilRate = 0;
    interest.hundredUtilRate = 0;
    interest.points = makeRatePoints([], []);
    interest.protocolOriginationFee = I80F48_ZERO;
    interest.insuranceFeeFixedApr = I80F48_ZERO;
    interest.insuranceIrFee = I80F48_ZERO;
    interest.protocolFixedFeeApr = I80F48_ZERO;
    interest.protocolIrFee = I80F48_ZERO;

    const mints = [
      ecosystem.wsolMint.publicKey,
      ecosystem.wsolMint.publicKey,
      ecosystem.usdcMint.publicKey,
    ];
    const banks: PublicKey[] = [];
    for (let i = 0; i < seed.length; i++) {
      const [bank] = deriveBankWithSeed(
        bankrunProgram.programId,
        group.publicKey,
        mints[i],
        seed[i],
      );
      banks.push(bank);
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
            marginfiGroup: group.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mints[i],
            config,
            seed: seed[i],
          }),
        ),
        [groupAdmin.wallet],
      );
    }
    [highRateBank, lowRateBank, debtBank] = banks;

    for (const bank of [highRateBank, lowRateBank]) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await configureBankOracleScope(groupAdmin.mrgnBankrunProgram, {
            bank,
            oracle: scopeFeed,
            entryIndex: 17,
          }),
        ),
        [groupAdmin.wallet],
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
          bank: debtBank,
          oracle: oracles.usdcOracle.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
        }),
        await configBankPremium(groupAdmin.mrgnBankrunProgram, {
          bank: debtBank,
          premiumTag: liabilityTag,
          active: true,
        }),
        await configBankPremium(groupAdmin.mrgnBankrunProgram, {
          bank: highRateBank,
          premiumTag: highRateTag,
          active: false,
        }),
        await configBankPremium(groupAdmin.mrgnBankrunProgram, {
          bank: lowRateBank,
          premiumTag: lowRateTag,
          active: false,
        }),
        await configGroupPremium(groupAdmin.mrgnBankrunProgram, {
          group: group.publicKey,
          entry: newPremiumEntry(highRateTag, liabilityTag, highRate),
        }),
        await configGroupPremium(groupAdmin.mrgnBankrunProgram, {
          group: group.publicKey,
          entry: newPremiumEntry(lowRateTag, liabilityTag, lowRate),
        }),
      ),
      [groupAdmin.wallet],
    );

    const payer = bankrunContext.payer;
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          groupAdmin.usdcAccount,
          payer.publicKey,
          5000 * usdcUnit,
        ),
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          borrower.wsolAccount,
          payer.publicKey,
          20 * solUnit,
        ),
      ),
      [payer],
    );
    for (const [owner, account] of [
      [groupAdmin, lenderAccount],
      [borrower, borrowerAccount],
    ] as const) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await accountInit(owner.mrgnBankrunProgram, {
            marginfiGroup: group.publicKey,
            marginfiAccount: account.publicKey,
            authority: owner.wallet.publicKey,
            feePayer: owner.wallet.publicKey,
          }),
        ),
        [owner.wallet, account],
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: lenderAccount.publicKey,
          bank: debtBank,
          tokenAccount: groupAdmin.usdcAccount,
          amount: new BN(5000 * usdcUnit),
        }),
      ),
      [groupAdmin.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(borrower.mrgnBankrunProgram, {
          marginfiAccount: borrowerAccount.publicKey,
          bank: highRateBank,
          tokenAccount: borrower.wsolAccount,
          amount: new BN(10 * solUnit),
        }),
        await depositIx(borrower.mrgnBankrunProgram, {
          marginfiAccount: borrowerAccount.publicKey,
          bank: lowRateBank,
          tokenAccount: borrower.wsolAccount,
          amount: new BN(10 * solUnit),
        }),
      ),
      [borrower.wallet],
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await borrowIx(borrower.mrgnBankrunProgram, {
          marginfiAccount: borrowerAccount.publicKey,
          bank: debtBank,
          tokenAccount: borrower.usdcAccount,
          amount: new BN(debt * usdcUnit),
          remaining: remaining(),
        }),
      ),
      [borrower.wallet],
    );
  });

  it("haircuts health without changing the rate or premium accrued over time", async () => {
    // Both positions are worth $1,000, so their 10% and 2% pair rates average to 6%.
    const initial = await pulse();
    assert.approximately(initial.rate, expectedRate, 0.000001);
    assert.approximately(initial.assets, 2000, 0.001);
    assert.approximately(initial.outstanding, 0, 0.001);

    await advanceBankrunClock(bankrunContext, year);
    await refreshOracles();
    const unbanded = await pulse();
    assert.approximately(unbanded.outstanding, debt * expectedRate, 0.001);

    // The high-rate leg now contributes $950 to health, but $1,000 to the premium mix.
    const config = blankBankConfigOptRaw();
    config.oracleMaxConfidence = u32_MAX;
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: highRateBank,
          bankConfigOpt: config,
        }),
      ),
      [groupAdmin.wallet],
    );
    const banded = await pulse();
    assert.approximately(banded.assets, 1950, 0.001);
    assert.approximately(banded.outstanding, unbanded.outstanding, 0.001);

    await advanceBankrunClock(bankrunContext, year);
    await refreshOracles();
    const after = await pulse();
    const firstYearPremium = unbanded.outstanding - initial.outstanding;
    const secondYearPremium = after.outstanding - banded.outstanding;
    assert.approximately(secondYearPremium, debt * expectedRate, 0.001);
    assert.approximately(secondYearPremium, firstYearPremium, 0.001);
    assert.approximately(banded.rate, expectedRate, 0.000001);
    assert.approximately(after.rate, expectedRate, 0.000001);
  });
});
