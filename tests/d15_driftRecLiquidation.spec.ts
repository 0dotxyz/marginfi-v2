import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import {
  ecosystem,
  driftAccounts,
  driftBankrunProgram,
  banksClient,
  bankrunContext,
  bankrunProgram,
  users,
  groupAdmin,
  globalProgramAdmin,
  oracles,
  DRIFT_TOKEN_A_PULL_FEED,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
} from "./rootHooks";
import { genericMultiBankTestSetup } from "./genericSetups";
import { processBankrunTransaction } from "./utils/tools";
import {
  makeAddDriftBankIx,
  makeInitDriftUserIx,
  makeDriftDepositIx,
  makeDriftWithdrawIx,
} from "./utils/drift-instructions";
import {
  defaultDriftBankConfig,
  TOKEN_A_MARKET_INDEX,
  refreshDriftOracles,
} from "./utils/drift-utils";
import { deriveBankWithSeed } from "./utils/pdas";
import {
  refreshPullOraclesBankrun,
  setPythPullOraclePrice,
} from "./utils/bankrun-oracles";
import { makeUpdateSpotMarketCumulativeInterestIx } from "./utils/drift-sdk";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  repayIx,
  startLiquidationIx,
} from "./utils/user-instructions";
import { blankBankConfigOptRaw, ORACLE_CONF_INTERVAL } from "./utils/types";
import { configureBank } from "./utils/group-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assertBankrunTxFailed } from "./utils/genericTests";
import { Clock } from "solana-bankrun";
import { getEpochAndSlot } from "./utils/stake-utils";

const USER_ACCOUNT_D15 = "d15_account";
const THROWAWAY_GROUP_SEED_D15 = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000015",
);
const STARTING_SEED = 150;
const TIME_TO_WAIT = 0.1 * 60 * 60;

describe("d15: Drift rec liquidation", () => {
  let throwawayGroup: Keypair;
  let driftTokenABank: PublicKey;
  let driftTokenASpotMarket: PublicKey;
  let driftTokenAPullOracle: PublicKey;
  let driftTokenAPullFeed: PublicKey;
  let liabBank: PublicKey;

  before(async () => {
    const result = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_D15,
      THROWAWAY_GROUP_SEED_D15,
      STARTING_SEED,
    );
    throwawayGroup = result.throwawayGroup;
    liabBank = result.banks[0];

    driftTokenASpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
    driftTokenAPullOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);
    driftTokenAPullFeed = driftAccounts.get(DRIFT_TOKEN_A_PULL_FEED);

    const bankSeed = new BN(STARTING_SEED + 1);
    [driftTokenABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      bankSeed,
    );

    const driftConfig = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

    const addBankIx = await makeAddDriftBankIx(
      groupAdmin.mrgnBankrunProgram,
      {
        group: throwawayGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.tokenAMint.publicKey,
        integrationAcc1: driftTokenASpotMarket,
        oracle: oracles.tokenAOracle.publicKey,
      },
      {
        seed: bankSeed,
        config: driftConfig,
      },
    );

    const addBankTx = new Transaction().add(addBankIx);
    await processBankrunTransaction(
      bankrunContext,
      addBankTx,
      [groupAdmin.wallet],
      false,
      true,
    );

    const initUserAmount = new BN(100);
    const fundAdminTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        initUserAmount.toNumber(),
      ),
    );
    await processBankrunTransaction(
      bankrunContext,
      fundAdminTx,
      [globalProgramAdmin.wallet],
      false,
      true,
    );

    const initUserIx = await makeInitDriftUserIx(
      groupAdmin.mrgnBankrunProgram,
      {
        feePayer: groupAdmin.wallet.publicKey,
        bank: driftTokenABank,
        signerTokenAccount: groupAdmin.tokenAAccount,
        driftOracle: driftTokenAPullOracle,
      },
      {
        amount: initUserAmount,
      },
      TOKEN_A_MARKET_INDEX,
    );

    const initUserTx = new Transaction().add(initUserIx);
    await processBankrunTransaction(
      bankrunContext,
      initUserTx,
      [groupAdmin.wallet],
      false,
      true,
    );

    const adminAccount = groupAdmin.accounts.get(USER_ACCOUNT_D15);
    const seedLiqAmount = new BN(1_000 * 10 ** ecosystem.lstAlphaDecimals);
    const seedLiqTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: liabBank,
        tokenAccount: groupAdmin.lstAlphaAccount,
        amount: seedLiqAmount,
        depositUpToLimit: false,
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      seedLiqTx,
      [groupAdmin.wallet],
      false,
      true,
    );
  });

  it("(user 0) Fund, deposit to drift, borrow", async () => {
    const liquidatee = users[0];

    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_D15);

    const remainingAccounts = [
      [driftTokenABank, oracles.tokenAOracle.publicKey, driftTokenASpotMarket],
      [liabBank, oracles.pythPullLst.publicKey],
    ];
    const remaining = composeRemainingAccounts(remainingAccounts);

    const fundTokenATx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        liquidatee.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        100 * 10 ** ecosystem.tokenADecimals,
      ),
    );
    await processBankrunTransaction(
      bankrunContext,
      fundTokenATx,
      [globalProgramAdmin.wallet],
      false,
      true,
    );

    const depositAmount = new BN(50 * 10 ** ecosystem.tokenADecimals);
    const depositIx = await makeDriftDepositIx(
      liquidatee.mrgnBankrunProgram,
      {
        marginfiAccount: liquidateeAccount,
        bank: driftTokenABank,
        signerTokenAccount: liquidatee.tokenAAccount,
        driftOracle: driftTokenAPullOracle,
      },
      depositAmount,
      TOKEN_A_MARKET_INDEX,
    );

    const depositTx = new Transaction().add(depositIx);
    await processBankrunTransaction(
      bankrunContext,
      depositTx,
      [liquidatee.wallet],
      false,
      true,
    );

    const borrowAmount = new BN(2 * 10 ** ecosystem.lstAlphaDecimals);
    const borrowTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
      await borrowIx(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: liabBank,
        tokenAccount: liquidatee.lstAlphaAccount,
        remaining,
        amount: borrowAmount,
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      borrowTx,
      [liquidatee.wallet],
      false,
      true,
    );
  });

  it("(user 1) Liquidates user 0 drift deposit with start/end", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];

    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_D15);

    const remainingAccounts = [
      [driftTokenABank, oracles.tokenAOracle.publicKey, driftTokenASpotMarket],
      [liabBank, oracles.pythPullLst.publicKey],
    ];
    const remaining = composeRemainingAccounts(remainingAccounts);

    const config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(6.0);
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(5.5);

    const configTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: liabBank,
        bankConfigOpt: config,
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      configTx,
      [groupAdmin.wallet],
      false,
      true,
    );

    const healthTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining,
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      healthTx,
      [liquidatee.wallet],
      false,
      true,
    );

    const initLiqRecordTx = new Transaction().add(
      await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        feePayer: liquidator.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(
      bankrunContext,
      initLiqRecordTx,
      [liquidator.wallet],
      false,
      true,
    );

    const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);

    const repayAmount = new BN((1 / 5) * 10 ** ecosystem.lstAlphaDecimals);
    const liquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining,
      }),
      await makeDriftWithdrawIx(
        liquidator.mrgnBankrunProgram,
        {
          marginfiAccount: liquidateeAccount,
          bank: driftTokenABank,
          destinationTokenAccount: liquidator.tokenAAccount,
          driftOracle: driftTokenAPullOracle,
        },
        {
          amount: withdrawAmount,
          withdraw_all: false,
          remaining,
        },
        driftBankrunProgram,
      ),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: liabBank,
        tokenAccount: liquidator.lstAlphaAccount,
        amount: repayAmount,
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining,
      }),
    );

    await processBankrunTransaction(
      bankrunContext,
      liquidationTx,
      [liquidator.wallet],
      false,
      true,
    );
  });

  it("Some time elapses", async () => {
    const slotsToAdvance = TIME_TO_WAIT * 0.4;
    const clock = await banksClient.getClock();
    const { epoch, slot } = await getEpochAndSlot(banksClient);
    const timeTarget = clock.unixTimestamp + BigInt(TIME_TO_WAIT);
    const targetUnix = BigInt(timeTarget);
    const newClock = new Clock(
      BigInt(slot + slotsToAdvance),
      0n,
      BigInt(epoch),
      0n,
      targetUnix,
    );
    bankrunContext.setClock(newClock);

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshDriftOracles(
      oracles,
      driftAccounts,
      bankrunContext,
      banksClient,
    );
  });

  it("(user 1) Liquidates user 0 drift deposit with start/end (stale drift)", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];

    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_D15);

    const remainingAccounts = [
      [driftTokenABank, oracles.tokenAOracle.publicKey, driftTokenASpotMarket],
      [liabBank, oracles.pythPullLst.publicKey],
    ];
    const remaining = composeRemainingAccounts(remainingAccounts);

    const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
    const driftWithdrawIx = await makeDriftWithdrawIx(
      liquidator.mrgnBankrunProgram,
      {
        marginfiAccount: liquidateeAccount,
        bank: driftTokenABank,
        destinationTokenAccount: liquidator.tokenAAccount,
        driftOracle: driftTokenAPullOracle,
      },
      {
        amount: withdrawAmount,
        withdraw_all: false,
        remaining,
      },
      driftBankrunProgram,
    );

    const repayAmount = new BN((1 / 5) * 10 ** ecosystem.lstAlphaDecimals);
    const startLiqIx = await startLiquidationIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      liquidationReceiver: liquidator.wallet.publicKey,
      remaining,
    });
    const repayLiqIx = await repayIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      bank: liabBank,
      tokenAccount: liquidator.lstAlphaAccount,
      amount: repayAmount,
    });
    const endLiqIx = await endLiquidationIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      remaining,
    });
    const liquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      startLiqIx,
      driftWithdrawIx,
      repayLiqIx,
      endLiqIx,
    );

    // Passes without refreshSpotMarketIx
    const result = await processBankrunTransaction(
      bankrunContext,
      liquidationTx,
      [liquidator.wallet],
      true,
      false,
    );

    assertBankrunTxFailed(result, 6322);

    // Passes with refresh
    const refreshSpotMarketIx = await makeUpdateSpotMarketCumulativeInterestIx(
      driftBankrunProgram,
      { oracle: driftTokenAPullOracle },
      TOKEN_A_MARKET_INDEX,
    );

    const refreshedLiquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      refreshSpotMarketIx,
      startLiqIx,
      driftWithdrawIx,
      repayLiqIx,
      endLiqIx,
    );

    await processBankrunTransaction(
      bankrunContext,
      refreshedLiquidationTx,
      [liquidator.wallet],
      false,
      true,
    );
  });
});

// TODO same for mixed-balances including Kamino
