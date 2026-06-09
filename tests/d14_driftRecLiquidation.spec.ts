import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  AccountMeta,
  AddressLookupTableAccount,
  Keypair,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
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
  klendBankrunProgram,
  kaminoAccounts,
  farmAccounts,
  MARKET,
  TOKEN_A_RESERVE,
  A_FARM_STATE,
  FARMS_PROGRAM_ID,
} from "./rootHooks";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  createLut,
  getBankrunBlockhash,
  processBankrunTransaction,
} from "./utils/tools";
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
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import {
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import { ensureMultiSuiteIntegrationsSetup } from "./utils/multi-limits-setup";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { makeUpdateSpotMarketCumulativeInterestIx } from "./utils/drift-sdk";
import {
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  repayIx,
  startLiquidationIx,
} from "./utils/user-instructions";
import { blankBankConfigOptRaw } from "./utils/types";
import { configureBank } from "./utils/group-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assertBankrunTxFailed } from "./utils/genericTests";
import { Clock } from "./utils/litesvm";
import { getEpochAndSlot } from "./utils/bankrunConnection";

const USER_ACCOUNT_D15 = "d15_account";
const THROWAWAY_GROUP_SEED_D15 = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000015"
);
const STARTING_SEED = 150;
const TIME_TO_WAIT = 0.1 * 60 * 60;

describe("d14: Drift rec liquidation", () => {
  let throwawayGroup: Keypair;
  let driftTokenABank: PublicKey;
  let driftTokenASpotMarket: PublicKey;
  let driftTokenAPullOracle: PublicKey;
  let driftTokenAPullFeed: PublicKey;
  let liabBank: PublicKey;
  let remainingStartMeta: AccountMeta[] = [];
  let remainingEndMeta: AccountMeta[] = [];

  before(async () => {
    const result = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_D15,
      THROWAWAY_GROUP_SEED_D15,
      STARTING_SEED
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
      bankSeed
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
      }
    );

    const addBankTx = new Transaction().add(addBankIx);
    await processBankrunTransaction(
      bankrunContext,
      addBankTx,
      [groupAdmin.wallet],
      false,
      true
    );

    const initUserAmount = new BN(100);
    const fundAdminTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        initUserAmount.toNumber()
      )
    );
    await processBankrunTransaction(
      bankrunContext,
      fundAdminTx,
      [globalProgramAdmin.wallet],
      false,
      true
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
      TOKEN_A_MARKET_INDEX
    );

    const initUserTx = new Transaction().add(initUserIx);
    await processBankrunTransaction(
      bankrunContext,
      initUserTx,
      [groupAdmin.wallet],
      false,
      true
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
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      seedLiqTx,
      [groupAdmin.wallet],
      false,
      true
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
    remainingStartMeta =
      composeRemainingAccountsWriteableMeta(remainingAccounts);
    remainingEndMeta = composeRemainingAccountsMetaBanksOnly(remainingAccounts);

    const fundTokenATx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        liquidatee.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        100 * 10 ** ecosystem.tokenADecimals
      )
    );
    await processBankrunTransaction(
      bankrunContext,
      fundTokenATx,
      [globalProgramAdmin.wallet],
      false,
      true
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
      TOKEN_A_MARKET_INDEX
    );

    const depositTx = new Transaction().add(depositIx);
    await processBankrunTransaction(
      bankrunContext,
      depositTx,
      [liquidatee.wallet],
      false,
      true
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
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      borrowTx,
      [liquidatee.wallet],
      false,
      true
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
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      configTx,
      [groupAdmin.wallet],
      false,
      true
    );

    const healthTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      healthTx,
      [liquidatee.wallet],
      false,
      true
    );

    const initLiqRecordTx = new Transaction().add(
      await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        feePayer: liquidator.wallet.publicKey,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      initLiqRecordTx,
      [liquidator.wallet],
      false,
      true
    );

    const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);

    const repayAmount = new BN((1 / 5) * 10 ** ecosystem.lstAlphaDecimals);
    const liquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: remainingStartMeta,
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
          withdrawAll: false,
          remaining,
        },
        driftBankrunProgram
      ),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: liabBank,
        tokenAccount: liquidator.lstAlphaAccount,
        amount: repayAmount,
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: remainingEndMeta,
      })
    );

    await processBankrunTransaction(
      bankrunContext,
      liquidationTx,
      [liquidator.wallet],
      false,
      true
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
      targetUnix
    );
    bankrunContext.setClock(newClock);

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshDriftOracles(
      oracles,
      driftAccounts,
      bankrunContext,
      banksClient
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
        withdrawAll: false,
        remaining,
      },
      driftBankrunProgram
    );

    const repayAmount = new BN((1 / 5) * 10 ** ecosystem.lstAlphaDecimals);
    const startLiqIx = await startLiquidationIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      liquidationReceiver: liquidator.wallet.publicKey,
      remaining: remainingStartMeta,
    });
    const repayLiqIx = await repayIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      bank: liabBank,
      tokenAccount: liquidator.lstAlphaAccount,
      amount: repayAmount,
    });
    const endLiqIx = await endLiquidationIx(liquidator.mrgnBankrunProgram, {
      marginfiAccount: liquidateeAccount,
      remaining: remainingEndMeta,
    });
    const liquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      startLiqIx,
      driftWithdrawIx,
      repayLiqIx,
      endLiqIx
    );

    // Passes without refreshSpotMarketIx
    const result = await processBankrunTransaction(
      bankrunContext,
      liquidationTx,
      [liquidator.wallet],
      true,
      false
    );

    assertBankrunTxFailed(result, 6322);

    // Passes with refresh
    const refreshSpotMarketIx = await makeUpdateSpotMarketCumulativeInterestIx(
      driftBankrunProgram,
      { oracle: driftTokenAPullOracle },
      TOKEN_A_MARKET_INDEX
    );

    const refreshedLiquidationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      refreshSpotMarketIx,
      startLiqIx,
      driftWithdrawIx,
      repayLiqIx,
      endLiqIx
    );

    await processBankrunTransaction(
      bankrunContext,
      refreshedLiquidationTx,
      [liquidator.wallet],
      false,
      true
    );
  });
});

// Same flow as d14 above, but the liquidatee holds collateral in BOTH a Drift
// and a Kamino bank, and a single receivership transaction
// (start -> driftWithdraw + kaminoWithdraw -> repay -> end) unwinds both legs.
const USER_ACCOUNT_D14B = "d14b_account";
const THROWAWAY_GROUP_SEED_D14B = Buffer.from(
  "MARGINFI_GROUP_SEED_123400000016"
);
const STARTING_SEED_B = 160;

describe("d14b: Drift + Kamino mixed rec liquidation", () => {
  let throwawayGroup: Keypair;
  let liabBank: PublicKey;
  let kaminoBank: PublicKey;
  let driftBank: PublicKey;
  let lendingMarket: PublicKey;
  let tokenAReserve: PublicKey;
  let reserveFarmState: PublicKey;
  let driftSpotMarket: PublicKey;
  let driftPullOracle: PublicKey;

  // Drift bank, Kamino bank, then the regular liability bank.
  let remainingGroups: PublicKey[][] = [];
  let remaining: PublicKey[] = [];
  let remainingStartMeta: AccountMeta[] = [];
  let remainingEndMeta: AccountMeta[] = [];

  const kaminoObligationFarmUserState = (): PublicKey => {
    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      kaminoBank
    );
    const [obligation] = deriveBaseObligation(
      lendingVaultAuthority,
      lendingMarket
    );
    const [userState] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), reserveFarmState.toBuffer(), obligation.toBuffer()],
      FARMS_PROGRAM_ID
    );
    return userState;
  };

  const refreshKaminoIxs = async () => {
    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      kaminoBank
    );
    const [obligation] = deriveBaseObligation(
      lendingVaultAuthority,
      lendingMarket
    );
    return [
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        lendingMarket,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        lendingMarket,
        obligation,
        [tokenAReserve]
      ),
    ];
  };

  before(async () => {
    // The drift slice doesn't run the k* setup specs, so bootstrap the Kamino
    // market/reserve/farm (and Drift) before adding integration banks.
    await ensureMultiSuiteIntegrationsSetup();

    const result = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_D14B,
      THROWAWAY_GROUP_SEED_D14B,
      STARTING_SEED_B,
      1, // one Kamino bank
      1 // one Drift bank
    );
    throwawayGroup = result.throwawayGroup;
    liabBank = result.banks[0];
    kaminoBank = result.kaminoBanks[0];
    driftBank = result.driftBanks[0];

    lendingMarket = kaminoAccounts.get(MARKET);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    reserveFarmState = farmAccounts.get(A_FARM_STATE);
    driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
    driftPullOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);

    // Drift, Kamino, then the regular liability bank.
    remainingGroups = [
      [driftBank, oracles.tokenAOracle.publicKey, driftSpotMarket],
      [kaminoBank, oracles.tokenAOracle.publicKey, tokenAReserve],
      [liabBank, oracles.pythPullLst.publicKey],
    ];
    remaining = composeRemainingAccounts(remainingGroups);
    remainingStartMeta = composeRemainingAccountsWriteableMeta(remainingGroups);
    remainingEndMeta = composeRemainingAccountsMetaBanksOnly(remainingGroups);

    // Seed the liability bank so the liquidatee has something to borrow.
    const adminAccount = groupAdmin.accounts.get(USER_ACCOUNT_D14B);
    const seedLiqAmount = new BN(1_000 * 10 ** ecosystem.lstAlphaDecimals);
    const seedLiqTx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: liabBank,
        tokenAccount: groupAdmin.lstAlphaAccount,
        amount: seedLiqAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      seedLiqTx,
      [groupAdmin.wallet],
      false,
      true
    );
  });

  it("(user 0) Fund, deposit to drift + kamino, borrow", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_D14B);

    // Oracles may be stale after the preceding d14 suite advanced the clock.
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshDriftOracles(
      oracles,
      driftAccounts,
      bankrunContext,
      banksClient
    );

    const fundTokenATx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        liquidatee.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        100 * 10 ** ecosystem.tokenADecimals
      )
    );
    await processBankrunTransaction(
      bankrunContext,
      fundTokenATx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    const depositAmount = new BN(50 * 10 ** ecosystem.tokenADecimals);

    // Deposit to Kamino (needs a fresh reserve + obligation first).
    const kaminoDepositTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ...(await refreshKaminoIxs()),
      await makeKaminoDepositIx(
        liquidatee.mrgnBankrunProgram,
        {
          marginfiAccount: liquidateeAccount,
          bank: kaminoBank,
          signerTokenAccount: liquidatee.tokenAAccount,
          lendingMarket,
          reserve: tokenAReserve,
          obligationFarmUserState: kaminoObligationFarmUserState(),
          reserveFarmState,
        },
        depositAmount
      )
    );
    await processBankrunTransaction(
      bankrunContext,
      kaminoDepositTx,
      [liquidatee.wallet],
      false,
      true
    );

    // Deposit to Drift.
    const driftDepositTx = new Transaction().add(
      await makeDriftDepositIx(
        liquidatee.mrgnBankrunProgram,
        {
          marginfiAccount: liquidateeAccount,
          bank: driftBank,
          signerTokenAccount: liquidatee.tokenAAccount,
          driftOracle: driftPullOracle,
        },
        depositAmount,
        TOKEN_A_MARKET_INDEX
      )
    );
    await processBankrunTransaction(
      bankrunContext,
      driftDepositTx,
      [liquidatee.wallet],
      false,
      true
    );

    const borrowAmount = new BN(2 * 10 ** ecosystem.lstAlphaDecimals);
    const borrowTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ...(await refreshKaminoIxs()),
      await borrowIx(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: liabBank,
        tokenAccount: liquidatee.lstAlphaAccount,
        remaining,
        amount: borrowAmount,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      borrowTx,
      [liquidatee.wallet],
      false,
      true
    );
  });

  it("(user 1) Receivership-liquidates user 0's drift + kamino collateral", async () => {
    const liquidatee = users[0];
    const liquidator = users[1];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_D14B);

    // Crank the liability weights so user 0 is unhealthy.
    const config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(6.0);
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(5.5);
    const configTx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: liabBank,
        bankConfigOpt: config,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      configTx,
      [groupAdmin.wallet],
      false,
      true
    );

    const healthTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ...(await refreshKaminoIxs()),
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      healthTx,
      [liquidatee.wallet],
      false,
      true
    );

    const initLiqRecordTx = new Transaction().add(
      await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        feePayer: liquidator.wallet.publicKey,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      initLiqRecordTx,
      [liquidator.wallet],
      false,
      true
    );

    const driftRemaining = composeRemainingAccounts([
      [driftBank, oracles.tokenAOracle.publicKey, driftSpotMarket],
    ]);
    const kaminoRemaining = composeRemainingAccounts([
      [kaminoBank, oracles.tokenAOracle.publicKey, tokenAReserve],
    ]);

    const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
    const repayAmount = new BN((1 / 5) * 10 ** ecosystem.lstAlphaDecimals);

    const receiverInstructions = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      // Both integration legs need a fresh price before the unwind.
      await makeUpdateSpotMarketCumulativeInterestIx(
        driftBankrunProgram,
        { oracle: driftPullOracle },
        TOKEN_A_MARKET_INDEX
      ),
      ...(await refreshKaminoIxs()),
      await startLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        liquidationReceiver: liquidator.wallet.publicKey,
        remaining: remainingStartMeta,
      }),
      await makeDriftWithdrawIx(
        liquidator.mrgnBankrunProgram,
        {
          marginfiAccount: liquidateeAccount,
          bank: driftBank,
          destinationTokenAccount: liquidator.tokenAAccount,
          driftOracle: driftPullOracle,
        },
        {
          amount: withdrawAmount,
          withdrawAll: false,
          remaining: driftRemaining,
        },
        driftBankrunProgram
      ),
      await makeKaminoWithdrawIx(
        liquidator.mrgnBankrunProgram,
        {
          marginfiAccount: liquidateeAccount,
          authority: liquidator.wallet.publicKey,
          bank: kaminoBank,
          mint: ecosystem.tokenAMint.publicKey,
          destinationTokenAccount: liquidator.tokenAAccount,
          lendingMarket,
          reserve: tokenAReserve,
          obligationFarmUserState: kaminoObligationFarmUserState(),
          reserveFarmState,
        },
        {
          amount: withdrawAmount,
          isWithdrawAll: false,
          remaining: kaminoRemaining,
        }
      ),
      await repayIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        bank: liabBank,
        tokenAccount: liquidator.lstAlphaAccount,
        amount: repayAmount,
      }),
      await endLiquidationIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: remainingEndMeta,
      }),
    ];

    // Pulling from both Kamino and Drift in one tx blows the 1232-byte legacy
    // limit, so pack the accounts into a LUT and send a v0 transaction.
    const lutAddresses: PublicKey[] = [];
    const seen = new Set<string>();
    for (const ix of receiverInstructions) {
      for (const key of [ix.programId, ...ix.keys.map((k) => k.pubkey)]) {
        if (!seen.has(key.toBase58())) {
          seen.add(key.toBase58());
          lutAddresses.push(key);
        }
      }
    }
    const lut = await createLut(liquidator.wallet, lutAddresses);

    // Advance a few slots so the LUT activates, then refresh oracles in case we
    // warped into staleness.
    const { slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(slot + 24));
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshDriftOracles(
      oracles,
      driftAccounts,
      bankrunContext,
      banksClient
    );

    const lutRaw = await banksClient.getAccount(lut.key);
    const lutAccount = new AddressLookupTableAccount({
      key: lut.key,
      state: AddressLookupTableAccount.deserialize(lutRaw.data),
    });
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: receiverInstructions,
    }).compileToV0Message([lutAccount]);
    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([liquidator.wallet]);
    await banksClient.processTransaction(versionedTx);
  });
});
