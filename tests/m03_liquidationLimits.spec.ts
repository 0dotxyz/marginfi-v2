import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  groupAdmin,
  bankrunContext,
  banksClient,
  bankrunProgram,
  ecosystem,
  oracles,
  users,
  globalProgramAdmin,
  klendBankrunProgram,
  MARKET,
  TOKEN_A_RESERVE,
  kaminoAccounts,
  farmAccounts,
  A_FARM_STATE,
  FARMS_PROGRAM_ID,
  driftAccounts,
  driftBankrunProgram,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
} from "./rootHooks";
import { configureBank } from "./utils/group-instructions";
import { defaultBankConfigOptRaw, MAX_BALANCES } from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  liquidateIx,
  initLiquidationRecordIx,
  startLiquidationIx,
  endLiquidationIx,
  repayIx,
} from "./utils/user-instructions";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { dumpBankrunLogs, processBankrunTransaction } from "./utils/tools";
import { genericMultiBankTestSetup } from "./genericSetups";
import { refreshPullOracles } from "./utils/pyth-pull-mocks";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import {
  makeKaminoDepositIx,
  makeKaminoWithdrawIx,
} from "./utils/kamino-instructions";
import {
  makeDriftDepositIx,
  makeDriftWithdrawIx,
} from "./utils/drift-instructions";
import { TOKEN_A_MARKET_INDEX } from "./utils/drift-utils";
import { makeUpdateSpotMarketCumulativeInterestIx } from "./utils/drift-sdk";
import {
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { getEpochAndSlot } from "./utils/stake-utils";

const startingSeed: number = 42;

/** Always one P0 (regular) debt bank). */
const P0_BORROWS = 1;

/**
 * Define scenarios by only choosing KAMINO_DEPOSITS.
 * DRIFT_DEPOSITS is computed to fill the rest of MAX_BALANCES (minus the 1 debt bank).
 *
 * Note: KAMINO_DEPOSITS must be within [0, MAX_BALANCES - 1].
 */
const SCENARIOS: Array<{ kaminoDeposits: number }> = [
  { kaminoDeposits: 0 },
  { kaminoDeposits: 1 },
  { kaminoDeposits: 8 },
  { kaminoDeposits: 15 },
];

function groupSeedForScenario(index: number): Buffer {
  return Buffer.from(
    `MARGINFI_GROUP_SEED_12340000M3${index.toString().padStart(2, "0")}`,
  );
}

function userAccountNameForScenario(index: number): string {
  return `throwaway_account_m3${index}`;
}

function scenarioName(kaminoDeposits: number, driftDeposits: number) {
  return `m03: Limits (Kamino=${kaminoDeposits}, Drift=${driftDeposits}, RegularDebt=${P0_BORROWS})`;
}

SCENARIOS.forEach(({ kaminoDeposits }, scenarioIndex) => {
  const driftDeposits = MAX_BALANCES - P0_BORROWS - kaminoDeposits;

  if (driftDeposits < 0) {
    throw new Error(
      `Invalid scenario: kaminoDeposits=${kaminoDeposits} implies driftDeposits=${driftDeposits} (must be >= 0).`,
    );
  }

  const groupBuff = groupSeedForScenario(scenarioIndex);
  const USER_ACCOUNT_THROWAWAY = userAccountNameForScenario(scenarioIndex);

  describe(scenarioName(kaminoDeposits, driftDeposits), () => {
    let banks: PublicKey[] = [];
    let kaminoBanks: PublicKey[] = [];
    let driftBanks: PublicKey[] = [];
    let lendingMarket: PublicKey;
    let reserveFarmState: PublicKey;
    let tokenAReserve: PublicKey;
    let liquidateeRemainingAccounts: PublicKey[] = [];
    let liquidatorRemainingAccounts: PublicKey[] = [];
    let driftSpotMarket: PublicKey;
    let lookupTable: PublicKey;

    const buildReceivershipInstructions = async (
      liquidator: any,
      liquidateeAccount: PublicKey,
    ) => {
      const remaining = liquidateeRemainingAccounts;
      const withdrawTokenAAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
      const repayLstAmount = new BN(0.1 * 10 ** ecosystem.lstAlphaDecimals);

      const preInstructions = [
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      ];
      const withdrawInstructions = [];

      if (kaminoBanks.length > 0) {
        const bank = kaminoBanks[0];
        const kaminoRemaining = composeRemainingAccounts([
          [bank, oracles.tokenAOracle.publicKey, tokenAReserve],
        ]);
        const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
          bankrunProgram.programId,
          bank,
        );
        const [obligation] = deriveBaseObligation(
          lendingVaultAuthority,
          lendingMarket,
        );
        const [obligationFarmUserState] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("user"),
            reserveFarmState.toBuffer(),
            obligation.toBuffer(),
          ],
          FARMS_PROGRAM_ID,
        );

        preInstructions.push(
          await simpleRefreshReserve(
            klendBankrunProgram,
            tokenAReserve,
            lendingMarket,
            oracles.tokenAOracle.publicKey,
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            lendingMarket,
            obligation,
            [tokenAReserve],
          ),
        );

        withdrawInstructions.push(
          await makeKaminoWithdrawIx(
            liquidator.mrgnBankrunProgram,
            {
              marginfiAccount: liquidateeAccount,
              authority: liquidator.wallet.publicKey,
              bank,
              destinationTokenAccount: liquidator.tokenAAccount,
              lendingMarket,
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
              obligationFarmUserState,
              reserveFarmState,
            },
            {
              amount: withdrawTokenAAmount,
              isFinalWithdrawal: false,
              remaining: kaminoRemaining,
            },
          ),
        );
      }

      if (driftBanks.length > 0) {
        const bank = driftBanks[0];
        const driftRemaining = composeRemainingAccounts([
          [bank, oracles.tokenAOracle.publicKey, driftSpotMarket],
        ]);
        preInstructions.push(
          await makeUpdateSpotMarketCumulativeInterestIx(
            driftBankrunProgram,
            { oracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE) },
            TOKEN_A_MARKET_INDEX,
          ),
        );
        withdrawInstructions.push(
          await makeDriftWithdrawIx(
            liquidator.mrgnBankrunProgram,
            {
              marginfiAccount: liquidateeAccount,
              bank,
              destinationTokenAccount: liquidator.tokenAAccount,
              driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
            },
            {
              amount: withdrawTokenAAmount,
              withdraw_all: false,
              remaining: driftRemaining,
            },
            driftBankrunProgram,
          ),
        );
      }

      const instructions = [
        ...preInstructions,
        await startLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          liquidationReceiver: liquidator.wallet.publicKey,
          remaining,
        }),
        ...withdrawInstructions,
        await repayIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          bank: banks[0], // regular debt bank
          tokenAccount: liquidator.lstAlphaAccount,
          amount: repayLstAmount,
        }),
        await endLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          remaining,
        }),
      ];

      return instructions;
    };

    before(() => {
      console.log(
        `Running the scenario with ${kaminoDeposits} Kamino banks, ${driftDeposits} Drift banks, ${P0_BORROWS} regular debt bank`,
      );
    });

    it("init group, init banks, and fund banks", async () => {
      const result = await genericMultiBankTestSetup(
        P0_BORROWS,
        USER_ACCOUNT_THROWAWAY,
        groupBuff,
        startingSeed,
        kaminoDeposits,
        driftDeposits,
      );
      banks = result.banks;
      kaminoBanks = result.kaminoBanks;
      driftBanks = result.driftBanks;
      lendingMarket = kaminoAccounts.get(MARKET);
      tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
      reserveFarmState = farmAccounts.get(A_FARM_STATE);
      driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
    });

    it("Refresh oracles", async () => {
      const clock = await banksClient.getClock();
      await refreshPullOracles(
        oracles,
        globalProgramAdmin.wallet,
        new BN(Number(clock.slot)),
        Number(clock.unixTimestamp),
        bankrunContext,
        false,
      );
    });

    it("(admin) Seeds liquidity in all banks - happy path", async () => {
      const user = groupAdmin;
      const marginfiAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
      const depositLstAmount = new BN(10 * 10 ** ecosystem.lstAlphaDecimals);
      const depositTokenAAmount = new BN(100 * 10 ** ecosystem.tokenADecimals);

      const remainingAccounts: PublicKey[][] = [];

      // regular banks
      for (let i = 0; i < banks.length; i += 1) {
        const bank = banks[i];
        const tx = new Transaction();
        tx.add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount,
            bank,
            tokenAccount: user.lstAlphaAccount,
            amount: depositLstAmount,
            depositUpToLimit: false,
          }),
        );
        await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
        remainingAccounts.push([bank, oracles.pythPullLst.publicKey]);
      }

      // kamino banks
      for (let i = 0; i < kaminoBanks.length; i += 1) {
        const bank = kaminoBanks[i];
        const tx = new Transaction();
        const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
          bankrunProgram.programId,
          bank,
        );
        const [obligation] = deriveBaseObligation(
          lendingVaultAuthority,
          lendingMarket,
        );
        const [obligationFarmUserState] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("user"),
            reserveFarmState.toBuffer(),
            obligation.toBuffer(),
          ],
          FARMS_PROGRAM_ID,
        );

        tx.add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            tokenAReserve,
            lendingMarket,
            oracles.tokenAOracle.publicKey,
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            lendingMarket,
            obligation,
            [tokenAReserve],
          ),
          await makeKaminoDepositIx(
            user.mrgnBankrunProgram,
            {
              marginfiAccount,
              bank,
              signerTokenAccount: user.tokenAAccount,
              lendingMarket,
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
              obligationFarmUserState,
              reserveFarmState,
            },
            depositTokenAAmount,
          ),
        );

        await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
        remainingAccounts.push([
          bank,
          oracles.tokenAOracle.publicKey,
          tokenAReserve,
        ]);
      }

      // drift banks
      for (let i = 0; i < driftBanks.length; i += 1) {
        const bank = driftBanks[i];
        const tx = new Transaction();
        tx.add(
          await makeDriftDepositIx(
            user.mrgnBankrunProgram,
            {
              marginfiAccount,
              bank,
              signerTokenAccount: user.tokenAAccount,
              driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
            },
            depositTokenAAmount,
            TOKEN_A_MARKET_INDEX,
          ),
        );

        await processBankrunTransaction(
          bankrunContext,
          tx,
          [user.wallet],
          false,
          true,
        );

        remainingAccounts.push([
          bank,
          oracles.tokenAOracle.publicKey,
          driftSpotMarket,
        ]);
      }

      liquidatorRemainingAccounts = composeRemainingAccounts(remainingAccounts);
    });

    it("(user 0) Deposits to all Kamino and Drift banks and borrows from a regular one - happy path", async () => {
      const user = users[0];
      const marginfiAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
      const depositTokenAAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);
      const borrowLstAmount = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);

      const remainingAccounts: PublicKey[][] = [];

      for (let i = 0; i < kaminoBanks.length; i += 1) {
        const bank = kaminoBanks[i];
        const tx = new Transaction();

        const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
          bankrunProgram.programId,
          bank,
        );
        const [obligation] = deriveBaseObligation(
          lendingVaultAuthority,
          lendingMarket,
        );
        const [obligationFarmUserState] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("user"),
            reserveFarmState.toBuffer(),
            obligation.toBuffer(),
          ],
          FARMS_PROGRAM_ID,
        );

        tx.add(
          await simpleRefreshReserve(
            klendBankrunProgram,
            tokenAReserve,
            lendingMarket,
            oracles.tokenAOracle.publicKey,
          ),
          await simpleRefreshObligation(
            klendBankrunProgram,
            lendingMarket,
            obligation,
            [tokenAReserve],
          ),
          await makeKaminoDepositIx(
            user.mrgnBankrunProgram,
            {
              marginfiAccount,
              bank,
              signerTokenAccount: user.tokenAAccount,
              lendingMarket,
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
              obligationFarmUserState,
              reserveFarmState,
            },
            depositTokenAAmount,
          ),
        );

        remainingAccounts.push([
          bank,
          oracles.tokenAOracle.publicKey,
          tokenAReserve,
        ]);

        await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
      }

      for (let i = 0; i < driftBanks.length; i += 1) {
        const bank = driftBanks[i];
        const tx = new Transaction();

        tx.add(
          await makeDriftDepositIx(
            user.mrgnBankrunProgram,
            {
              marginfiAccount,
              bank,
              signerTokenAccount: user.tokenAAccount,
              driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
            },
            depositTokenAAmount,
            TOKEN_A_MARKET_INDEX,
          ),
        );

        remainingAccounts.push([
          bank,
          oracles.tokenAOracle.publicKey,
          driftSpotMarket,
        ]);

        await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
      }

      remainingAccounts.push([banks[0], oracles.pythPullLst.publicKey]);
      liquidateeRemainingAccounts = composeRemainingAccounts(remainingAccounts);

      const tx = new Transaction();
      tx.add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount,
          bank: banks[0], // there is only one regular bank
          tokenAccount: user.lstAlphaAccount,
          remaining: liquidateeRemainingAccounts,
          amount: borrowLstAmount,
        }),
      );

      await processBankrunTransaction(
        bankrunContext,
        tx,
        [user.wallet],
        false,
        true,
      );
    });

    it("(admin) Vastly increases regular bank liability ratio to make user 0 unhealthy", async () => {
      const config = defaultBankConfigOptRaw();
      config.liabilityWeightInit = bigNumberToWrappedI80F48(210); // 21000%
      config.liabilityWeightMaint = bigNumberToWrappedI80F48(200); // 20000%

      const tx = new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: banks[0],
          bankConfigOpt: config,
        }),
      );

      await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
    });

    it("(admin) Liquidates user 0", async () => {
      const liquidatee = users[0];
      const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
      const liquidator = groupAdmin;
      const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY);
      const liquidateAmount = new BN(0.1 * 10 ** ecosystem.lstAlphaDecimals);

      if (kaminoBanks.length > 0) {
        const kaminoTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          await liquidateIx(liquidator.mrgnBankrunProgram, {
            assetBankKey: kaminoBanks[0],
            liabilityBankKey: banks[0],
            liquidatorMarginfiAccount: liquidatorAccount,
            liquidateeMarginfiAccount: liquidateeAccount,
            remaining: [
              oracles.tokenAOracle.publicKey, // asset oracle
              tokenAReserve, // Kamino-specific "oracle"
              oracles.pythPullLst.publicKey, // liab oracle
              ...liquidatorRemainingAccounts,
              ...liquidateeRemainingAccounts,
            ],
            amount: liquidateAmount,
            liquidateeAccounts: liquidateeRemainingAccounts.length,
            liquidatorAccounts: liquidatorRemainingAccounts.length,
          }),
        );

        await processBankrunTransaction(bankrunContext, kaminoTx, [
          groupAdmin.wallet,
        ]);
      }

      if (driftBanks.length > 0) {
        const driftTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          await liquidateIx(liquidator.mrgnBankrunProgram, {
            assetBankKey: driftBanks[0],
            liabilityBankKey: banks[0],
            liquidatorMarginfiAccount: liquidatorAccount,
            liquidateeMarginfiAccount: liquidateeAccount,
            remaining: [
              oracles.tokenAOracle.publicKey, // asset oracle
              driftSpotMarket, // Drift-specific "oracle"
              oracles.pythPullLst.publicKey, // liab oracle
              ...liquidatorRemainingAccounts,
              ...liquidateeRemainingAccounts,
            ],
            amount: liquidateAmount,
            liquidateeAccounts: liquidateeRemainingAccounts.length,
            liquidatorAccounts: liquidatorRemainingAccounts.length,
          }),
        );

        await processBankrunTransaction(bankrunContext, driftTx, [
          groupAdmin.wallet,
        ]);
      }
    });

    it("(admin) Creates LUT", async () => {
      const liquidator = groupAdmin;
      const liquidateeAccount = users[0].accounts.get(USER_ACCOUNT_THROWAWAY);
      const receiverInstructions = await buildReceivershipInstructions(
        liquidator,
        liquidateeAccount,
      );
      const lutAddresses: PublicKey[] = [];
      const seen = new Set<string>();
      const addAddress = (address: PublicKey) => {
        const key = address.toBase58();
        if (!seen.has(key)) {
          seen.add(key);
          lutAddresses.push(address);
        }
      };

      for (const ix of receiverInstructions) {
        addAddress(ix.programId);
        for (const keyMeta of ix.keys) {
          addAddress(keyMeta.pubkey);
        }
      }

      const recentSlot = Number(await banksClient.getSlot());
      const [createLutIx, lutAddress] =
        AddressLookupTableProgram.createLookupTable({
          authority: liquidator.wallet.publicKey,
          payer: liquidator.wallet.publicKey,
          recentSlot: recentSlot - 1,
        });
      lookupTable = lutAddress;

      const createLutTx = new Transaction().add(createLutIx);
      createLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      createLutTx.sign(liquidator.wallet);
      await banksClient.processTransaction(createLutTx);

      const LUT_CHUNK_SIZE = 20;
      const LUT_MAX_ADDRESSES = 256;
      const addressesToLoad = lutAddresses.slice(0, LUT_MAX_ADDRESSES);
      for (let i = 0; i < addressesToLoad.length; i += LUT_CHUNK_SIZE) {
        const extendIx = AddressLookupTableProgram.extendLookupTable({
          authority: liquidator.wallet.publicKey,
          payer: liquidator.wallet.publicKey,
          lookupTable,
          addresses: addressesToLoad.slice(i, i + LUT_CHUNK_SIZE),
        });
        const extendTx = new Transaction().add(extendIx);
        extendTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
        extendTx.sign(liquidator.wallet);
        await banksClient.processTransaction(extendTx);
      }

      // We must advance the bankrun slot to allow the lut to activate
      const ONE_MINUTE = 60;
      const slotsToAdvance = ONE_MINUTE * 0.4;
      let { epoch: _, slot } = await getEpochAndSlot(banksClient);
      bankrunContext.warpToSlot(BigInt(slot + slotsToAdvance));
    });

    it("(admin) Receivership liquidates user 0 with start/end (Kamino/Drift)", async () => {
      const liquidatee = users[0];
      const liquidator = groupAdmin;
      const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);

      if (kaminoBanks.length === 0 && driftBanks.length === 0) {
        return;
      }

      const initTx = new Transaction().add(
        await initLiquidationRecordIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidateeAccount,
          feePayer: liquidator.wallet.publicKey,
        }),
      );
      await processBankrunTransaction(bankrunContext, initTx, [
        liquidator.wallet,
      ]);

      const receiverInstructions = await buildReceivershipInstructions(
        liquidator,
        liquidateeAccount,
      );
      const tx = new Transaction().add(...receiverInstructions);

      const blockhash = await getBankrunBlockhash(bankrunContext);
      const lutRaw = await banksClient.getAccount(lookupTable);
      const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
      const lutAccount = new AddressLookupTableAccount({
        key: lookupTable,
        state: lutState,
      });
      const messageV0 = new TransactionMessage({
        payerKey: liquidator.wallet.publicKey,
        recentBlockhash: blockhash,
        instructions: [...tx.instructions],
      }).compileToV0Message([lutAccount]);
      const versionedTx = new VersionedTransaction(messageV0);
      versionedTx.sign([liquidator.wallet]);
      await banksClient.processTransaction(versionedTx);
      // let result = await banksClient.tryProcessTransaction(versionedTx);
      // dumpBankrunLogs(result);
    });
  });
});
