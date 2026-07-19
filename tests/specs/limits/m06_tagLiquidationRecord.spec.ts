import { BN, Program } from "@coral-xyz/anchor";
import {
  AccountMeta,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";

import { Marginfi } from "../../../target/types/marginfi";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import {
  assertBankrunTxFailed,
  assertKeyDefault,
  assertKeysEqual,
  getTokenBalance,
} from "../../utils/genericTests";
import {
  accrueInterest,
  addBank,
  configureBank,
} from "../../utils/group-instructions";
import {
  borrowIx,
  closeLiquidationRecordIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endLiquidationIx,
  initLiquidationRecordIx,
  liquidateIx,
  repayIx,
  startLiquidationIx,
  tagLiquidationRecordIx,
  withdrawIx,
} from "../../utils/user-instructions";
import { deriveLiquidationRecord } from "../../utils/pdas";
import { getBankrunTime, processBankrunTransaction } from "../../utils/tools";
import {
  defaultBankConfig,
  defaultBankConfigOptRaw,
  ONE_WEEK_IN_SECONDS,
  ORACLE_SETUP_PYTH_PUSH,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { getEpochAndSlot } from "../../utils/bankrunConnection";
import { Clock } from "../../utils/litesvm";
import { genericMultiBankTestSetup } from "../../genericSetups";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";

const USER_ACCOUNT_THROWAWAY = "throwaway_account_m6_tag";
const groupSeed = Buffer.from("MARGINFI_GROUP_SEED_12340000TAG0");
const startingSeed = 480;

const ERR_ACCOUNT_OWNED_BY_WRONG_PROGRAM = 3007;
const ERR_ILLEGAL_ACTION = 6043;
const ERR_HEALTHY_ACCOUNT = 6068;
const ERR_INVALID_LIQUIDATION_RECORD = 6095;
const ERR_ALREADY_TAGGED = 6605;

/** Original limits from bank creation (defaultBankConfig), reapplied on every configureBank. */
const BANK_LIMIT = new BN(100_000_000_000);

describe("m06: Tag liquidation record (liquidation premium grows over time)", () => {
  let program: Program<Marginfi>;
  /** lst alpha bank, borrowed from by the liquidatee */
  let debtBank: PublicKey;
  /** token A bank, the liquidatee's collateral */
  let collateralBank: PublicKey;
  let liquidateeAccount: PublicKey;
  let liquidationRecord: PublicKey;
  /** [[bank, oracle]] groups for the liquidatee's two balances */
  let balanceGroups: PublicKey[][] = [];
  let tagNonce = 0;

  const collateralWeightConfig = (init: number, maint: number) => {
    const config = defaultBankConfigOptRaw();
    config.assetWeightInit = bigNumberToWrappedI80F48(init);
    config.assetWeightMaint = bigNumberToWrappedI80F48(maint);
    config.depositLimit = BANK_LIMIT;
    config.borrowLimit = BANK_LIMIT;
    return config;
  };

  const sendTag = async (expectFail: boolean = false) => {
    const liquidator = users[1];
    const tx = new Transaction().add(
      // The varied CU limit makes each tag tx unique so results are never replayed
      ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 + tagNonce++ }),
      await tagLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts(balanceGroups),
      })
    );
    return processBankrunTransaction(
      bankrunContext,
      tx,
      [liquidator.wallet],
      expectFail
    );
  };

  const fetchRecord = () =>
    program.account.liquidationRecord.fetch(liquidationRecord);

  before(async () => {
    program = bankrunProgram;
    const setup = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_THROWAWAY,
      groupSeed,
      startingSeed
    );
    debtBank = setup.banks[0];
    liquidateeAccount = users[0].accounts.get(USER_ACCOUNT_THROWAWAY);
    [liquidationRecord] = deriveLiquidationRecord(
      program.programId,
      liquidateeAccount
    );

    // Add a token A collateral bank to the throwaway group
    const bankKeypair = Keypair.generate();
    collateralBank = bankKeypair.publicKey;
    const oracleMeta: AccountMeta = {
      pubkey: oracles.tokenAOracle.publicKey,
      isSigner: false,
      isWritable: false,
    };
    const oracleConfigIx = await groupAdmin.mrgnBankrunProgram.methods
      .lendingPoolConfigureBankOracle(
        ORACLE_SETUP_PYTH_PUSH,
        oracles.tokenAOracle.publicKey
      )
      .accountsPartial({
        group: setup.throwawayGroup.publicKey,
        bank: collateralBank,
        admin: groupAdmin.wallet.publicKey,
      })
      .remainingAccounts([oracleMeta])
      .instruction();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await addBank(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: setup.throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          bank: collateralBank,
          config: defaultBankConfig(),
        }),
        oracleConfigIx
      ),
      [groupAdmin.wallet, bankKeypair]
    );

    balanceGroups = [
      [collateralBank, oracles.tokenAOracle.publicKey],
      [debtBank, oracles.pythPullLst.publicKey],
    ];
  });

  it("(user 1) provides liquidity, (user 0) borrows against collateral", async () => {
    const liquidator = users[1];
    const liquidatee = users[0];
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidator.accounts.get(USER_ACCOUNT_THROWAWAY),
          bank: debtBank,
          tokenAccount: liquidator.lstAlphaAccount,
          amount: new BN(5 * 10 ** ecosystem.lstAlphaDecimals),
        })
      ),
      [liquidator.wallet]
    );

    // Deposit 2 token A ($20) and borrow 0.05 lst alpha ($8.75)
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(liquidatee.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: collateralBank,
          tokenAccount: liquidatee.tokenAAccount,
          amount: new BN(2 * 10 ** ecosystem.tokenADecimals),
        }),
        await borrowIx(liquidatee.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: debtBank,
          tokenAccount: liquidatee.lstAlphaAccount,
          amount: new BN(0.05 * 10 ** ecosystem.lstAlphaDecimals),
          remaining: composeRemainingAccounts(balanceGroups),
        })
      ),
      [liquidatee.wallet]
    );
  });

  it("(permissionless) tag before the record exists - should fail", async () => {
    const liquidator = users[1];
    const tx = new Transaction().add(
      await tagLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts(balanceGroups),
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [liquidator.wallet],
      true
    );
    // The record PDA does not exist yet, so it is still owned by the system program
    assertBankrunTxFailed(result, ERR_ACCOUNT_OWNED_BY_WRONG_PROGRAM);
  });

  it("(admin) cuts collateral weights so user 0 is unhealthy", async () => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: collateralBank,
          bankConfigOpt: collateralWeightConfig(0.05, 0.1),
        })
      ),
      [groupAdmin.wallet]
    );
  });

  it("(permissionless) init record and tag the unhealthy account - happy path", async () => {
    const liquidator = users[1];
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          feePayer: liquidator.wallet.publicKey,
        })
      ),
      [liquidator.wallet]
    );

    const timeAtTag = await getBankrunTime(bankrunContext);
    await sendTag();

    const record = await fetchRecord();
    assertKeysEqual(record.marginfiAccount, liquidateeAccount);
    assert.equal(record.taggedAt.toNumber(), timeAtTag);
  });

  it("(permissionless) tag again while already tagged - should fail", async () => {
    const recordBefore = await fetchRecord();
    const result = await sendTag(true);
    assertBankrunTxFailed(result, ERR_ALREADY_TAGGED);

    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), recordBefore.taggedAt.toNumber());
  });

  it("(user 1) liquidates at a ~37% premium after the tag matures; tag resets", async () => {
    const liquidator = users[1];

    // Warp one week ahead so the tag matures (premium cap grows to 100%)
    let clock = await banksClient.getClock();
    const targetUnix = clock.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);
    bankrunContext.setClock(
      new Clock(
        clock.slot,
        clock.epochStartTimestamp,
        clock.epoch,
        clock.leaderScheduleEpoch,
        targetUnix
      )
    );
    const { slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(slot + ONE_WEEK_IN_SECONDS * 0.4));
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accrueInterest(liquidator.mrgnBankrunProgram, {
          bank: debtBank,
        }),
        await accrueInterest(liquidator.mrgnBankrunProgram, {
          bank: collateralBank,
        })
      ),
      [liquidator.wallet]
    );

    const tokenABalanceBefore = await getTokenBalance(
      bankRunProvider,
      liquidator.tokenAAccount
    );
    const lstBalanceBefore = await getTokenBalance(
      bankRunProvider,
      liquidator.lstAlphaAccount
    );

    // Seize 0.6 token A ($6) against a 0.024 lst repayment ($4.20), a ~37% premium after
    // confidence bias, far above the 5% base cap but within the matured 100% cap
    const withdrawAmount = new BN(0.6 * 10 ** ecosystem.tokenADecimals);
    const repayAmount = new BN(0.024 * 10 ** ecosystem.lstAlphaDecimals);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await startLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          liquidationReceiver: liquidator.wallet.publicKey,
          remaining: composeRemainingAccountsWriteableMeta(balanceGroups),
        }),
        await withdrawIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: collateralBank,
          tokenAccount: liquidator.tokenAAccount,
          amount: withdrawAmount,
          remaining: composeRemainingAccounts(balanceGroups),
        }),
        await repayIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: debtBank,
          tokenAccount: liquidator.lstAlphaAccount,
          amount: repayAmount,
        }),
        await endLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          remaining: composeRemainingAccountsMetaBanksOnly(balanceGroups),
        })
      ),
      [liquidator.wallet]
    );

    // Liquidator receives exactly the seized collateral and pays exactly the repayment
    const tokenABalanceAfter = await getTokenBalance(
      bankRunProvider,
      liquidator.tokenAAccount
    );
    const lstBalanceAfter = await getTokenBalance(
      bankRunProvider,
      liquidator.lstAlphaAccount
    );
    assert.equal(
      tokenABalanceAfter - tokenABalanceBefore,
      withdrawAmount.toNumber()
    );
    assert.equal(lstBalanceBefore - lstBalanceAfter, repayAmount.toNumber());

    // The liquidation resets the tag and records the entry
    const now = await getBankrunTime(bankrunContext);
    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), 0);
    assertKeyDefault(record.liquidationReceiver);
    assert.equal(record.entries[3].timestamp.toNumber(), now);
  });

  it("(permissionless) re-tag after reset while still unhealthy", async () => {
    const timeAtTag = await getBankrunTime(bankrunContext);
    await sendTag();
    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), timeAtTag);
  });

  it("(admin) restores weights; tag clears, then tagging while healthy fails", async () => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: collateralBank,
          bankConfigOpt: collateralWeightConfig(1, 1),
        })
      ),
      [groupAdmin.wallet]
    );

    await sendTag();
    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), 0);

    // Healthy and untagged: nothing to do
    const result = await sendTag(true);
    assertBankrunTxFailed(result, ERR_HEALTHY_ACCOUNT);
  });

  it("(admin) cuts weights again; the account is re-tagged", async () => {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: collateralBank,
          bankConfigOpt: collateralWeightConfig(0.05, 0.1),
        })
      ),
      [groupAdmin.wallet]
    );

    const timeAtTag = await getBankrunTime(bankrunContext);
    await sendTag();
    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), timeAtTag);
  });

  it("(anyone) close the record while tagged - should fail", async () => {
    const liquidator = users[1];
    const tx = new Transaction().add(
      await closeLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        recordPayer: liquidator.wallet.publicKey,
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [liquidator.wallet],
      true
    );
    assertBankrunTxFailed(result, ERR_ILLEGAL_ACTION);
  });

  it("(user 1) legacy liquidate omitting the liquidation record - should fail", async () => {
    const liquidator = users[1];
    const liquidatorAccounts = composeRemainingAccounts([
      [debtBank, oracles.pythPullLst.publicKey],
      [collateralBank, oracles.tokenAOracle.publicKey],
    ]);
    const liquidateeAccounts = composeRemainingAccounts(balanceGroups);
    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: collateralBank,
        liabilityBankKey: debtBank,
        liquidatorMarginfiAccount: liquidator.accounts.get(
          USER_ACCOUNT_THROWAWAY
        ),
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.tokenAOracle.publicKey,
          oracles.pythPullLst.publicKey,
          ...liquidatorAccounts,
          ...liquidateeAccounts,
        ],
        amount: new BN(0.2 * 10 ** ecosystem.tokenADecimals),
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: liquidatorAccounts.length,
      })
    );
    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [liquidator.wallet],
      true
    );
    assertBankrunTxFailed(result, ERR_INVALID_LIQUIDATION_RECORD);
  });

  it("(user 1) legacy liquidate with the record resets the tag", async () => {
    const liquidator = users[1];
    const liquidatorAccounts = composeRemainingAccounts([
      [debtBank, oracles.pythPullLst.publicKey],
      [collateralBank, oracles.tokenAOracle.publicKey],
    ]);
    const liquidateeAccounts = composeRemainingAccounts(balanceGroups);
    // Seize 0.2 token A ($2); the ~$1.90 repaid is well above the 5% reset threshold on the
    // remaining ~$4.55 debt
    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: collateralBank,
        liabilityBankKey: debtBank,
        liquidatorMarginfiAccount: liquidator.accounts.get(
          USER_ACCOUNT_THROWAWAY
        ),
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.tokenAOracle.publicKey,
          oracles.pythPullLst.publicKey,
          ...liquidatorAccounts,
          ...liquidateeAccounts,
        ],
        amount: new BN(0.2 * 10 ** ecosystem.tokenADecimals),
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: liquidatorAccounts.length,
        liquidateeLiquidationRecord: liquidationRecord,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [liquidator.wallet]);

    const record = await fetchRecord();
    assert.equal(record.taggedAt.toNumber(), 0);
  });
});
