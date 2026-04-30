import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  ComputeBudgetProgram,
  PublicKey,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  banksClient,
  bankrunProgram,
  driftAccounts,
  driftBankrunProgram,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  ecosystem,
  farmAccounts,
  A_FARM_STATE,
  FARMS_PROGRAM_ID,
  globalProgramAdmin,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  users,
} from "./rootHooks";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  borrowIx,
  depositIx,
  composeRemainingAccounts,
  endExecuteOrderIx,
  healthPulse,
  placeOrderIx,
  repayIx,
  startExecuteOrderIx,
} from "./utils/user-instructions";
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
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import { JuplendPoolKeys } from "./utils/juplend/types";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { refreshPullOracles } from "./utils/pyth-pull-mocks";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
  processBankrunV0Transaction,
} from "./utils/tools";
import { MAX_BALANCES } from "./utils/types";
import {
  deriveBaseObligation,
  deriveExecuteOrderPda,
  deriveLiquidityVaultAuthority,
  deriveOrderPda,
} from "./utils/pdas";
import { assert } from "chai";
import { TOKEN_A_MARKET_INDEX, refreshDriftOracles } from "./utils/drift-utils";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { assertBankrunTxFailed } from "./utils/genericTests";
import type { MockUser } from "./utils/mocks";
import { ensureMultiSuiteIntegrationsSetup } from "./utils/multi-limits-setup";
import { addJuplendBanksForGroup } from "./utils/multi-limits-juplend-setup";
import { getEpochAndSlot } from "./utils/bankrunConnection";

const startingSeed: number = 77;
const U32_MAX = 2 ** 32 - 1;
const bpsToU32 = (bps: number) => Math.floor((bps / 10_000) * U32_MAX);
const maxSlippage = bpsToU32(500);
const takeProfitThreshold = 1_000;

const SCENARIOS: Array<{
  kaminoDeposits: number;
  driftDeposits: number;
  juplendDeposits: number;
}> = [
  { kaminoDeposits: 15, driftDeposits: 0, juplendDeposits: 0 },
  { kaminoDeposits: 8, driftDeposits: 7, juplendDeposits: 0 },
  { kaminoDeposits: 0, driftDeposits: 15, juplendDeposits: 0 },
  { kaminoDeposits: 0, driftDeposits: 0, juplendDeposits: 15 },
  { kaminoDeposits: 5, driftDeposits: 5, juplendDeposits: 5 },
];

function groupSeedForScenario(index: number): Buffer {
  return Buffer.from(
    `MARGINFI_GROUP_SEED_12340000M4${index.toString().padStart(2, "0")}`,
  );
}

function userAccountNameForScenario(index: number): string {
  return `throwaway_account_m4_${index}`;
}

function scenarioName(
  kaminoDeposits: number,
  driftDeposits: number,
  juplendDeposits: number,
) {
  return `m04: Order limits (Kamino=${kaminoDeposits}, Drift=${driftDeposits}, Juplend=${juplendDeposits})`;
}

SCENARIOS.forEach(
  ({ kaminoDeposits, driftDeposits, juplendDeposits }, scenarioIndex) => {
    const totalDeposits = kaminoDeposits + driftDeposits + juplendDeposits;
    if (totalDeposits !== MAX_BALANCES - 1) {
      throw new Error(
        `Invalid scenario: Kamino=${kaminoDeposits}, Drift=${driftDeposits}, Juplend=${juplendDeposits} must total ${
          MAX_BALANCES - 1
        }.`,
      );
    }
    const groupBuff = groupSeedForScenario(scenarioIndex);
    const USER_ACCOUNT_THROWAWAY = userAccountNameForScenario(scenarioIndex);

    describe(
      scenarioName(kaminoDeposits, driftDeposits, juplendDeposits),
      () => {
        let banks: PublicKey[] = [];
        let kaminoBanks: PublicKey[] = [];
        let driftBanks: PublicKey[] = [];
        let juplendBanks: PublicKey[] = [];
        let juplendPool: JuplendPoolKeys | null = null;
        let juplendPrograms: ReturnType<typeof getJuplendPrograms> | null =
          null;
        let remainingGroups: PublicKey[][] = [];
        let remainingAccounts: PublicKey[] = [];
        let lutAccount: AddressLookupTableAccount;
        let orderPk: PublicKey;
        let assetBank: PublicKey;
        let assetIntegration: "kamino" | "drift" | "juplend" = "drift";
        let throwawayGroup: PublicKey;
        let lendingMarket: PublicKey;
        let tokenAReserve: PublicKey;
        let reserveFarmState: PublicKey | undefined;
        let driftSpotMarket: PublicKey;
        let user: MockUser;
        let userAccount: PublicKey;

        before(async () => {
          await ensureMultiSuiteIntegrationsSetup();
          user = users[0];
          if (juplendDeposits > 0) {
            juplendPrograms = getJuplendPrograms();
          }
        });

        const buildRemainingGroups = (): PublicKey[][] => {
          const groups: PublicKey[][] = [];
          for (const bank of banks) {
            groups.push([bank, oracles.pythPullLst.publicKey]);
          }
          for (const bank of kaminoBanks) {
            groups.push([bank, oracles.tokenAOracle.publicKey, tokenAReserve]);
          }
          for (const bank of driftBanks) {
            groups.push([
              bank,
              oracles.tokenAOracle.publicKey,
              driftSpotMarket,
            ]);
          }
          for (const bank of juplendBanks) {
            if (!juplendPool) {
              throw new Error("Juplend banks exist without a Juplend pool");
            }
            groups.push([
              bank,
              oracles.tokenAOracle.publicKey,
              juplendPool.lending,
            ]);
          }
          return groups;
        };

        const buildExecuteOrderTx = async (
          signer: MockUser,
          startIx: TransactionInstruction,
          repayInstruction: TransactionInstruction,
          withdrawInstruction: TransactionInstruction,
          endIx: TransactionInstruction,
          preIxs: TransactionInstruction[] = [],
        ): Promise<VersionedTransaction> => {
          const blockhash = await getBankrunBlockhash(bankrunContext);
          const messageV0 = new TransactionMessage({
            payerKey: signer.wallet.publicKey,
            recentBlockhash: blockhash,
            instructions: [
              ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
              ...preIxs,
              startIx,
              repayInstruction,
              withdrawInstruction,
              endIx,
            ],
          }).compileToV0Message([lutAccount]);
          const versionedTx = new VersionedTransaction(messageV0);
          versionedTx.sign([signer.wallet]);
          return versionedTx;
        };

        const buildIntegrationRefreshIxs = async (): Promise<
          TransactionInstruction[]
        > => {
          const ixs: TransactionInstruction[] = [];
          if (kaminoBanks.length > 0) {
            ixs.push(
              await simpleRefreshReserve(
                klendBankrunProgram,
                tokenAReserve,
                lendingMarket,
                oracles.tokenAOracle.publicKey,
              ),
            );
          }
          if (juplendBanks.length > 0) {
            if (!juplendPool || !juplendPrograms) {
              throw new Error(
                "Juplend refresh requested without Juplend setup",
              );
            }
            ixs.push(
              await refreshJupSimple(juplendPrograms.lending, {
                pool: juplendPool,
              }),
            );
          }
          return ixs;
        };

        // Note: the Drift suite uses special oracles. It really shouldn't, but it's easier to just
        // refresh all of them than fix it at this point.
        const refreshAllOracleFeeds = async () => {
          const clock = await banksClient.getClock();
          await refreshPullOracles(
            oracles,
            globalProgramAdmin.wallet,
            new BN(Number(clock.slot)),
            Number(clock.unixTimestamp),
            bankrunContext,
            false,
          );
          if (driftBanks.length > 0) {
            await refreshDriftOracles(
              oracles,
              driftAccounts,
              bankrunContext,
              banksClient,
            );
          }
        };

        // Sending all together causes transaction size problems
        const pulseOrderHealth = async (
          remaining: PublicKey[],
        ): Promise<void> => {
          const refreshIxs = await buildIntegrationRefreshIxs();
          if (refreshIxs.length > 0) {
            const refreshTx = new Transaction().add(...refreshIxs);
            await processBankrunTransaction(
              bankrunContext,
              refreshTx,
              [user.wallet],
              false,
              true,
            );
          }

          const pulseIx = await healthPulse(user.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            remaining,
          });
          const pulseTx = new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
            pulseIx,
          );
          await processBankrunTransaction(
            bankrunContext,
            pulseTx,
            [user.wallet],
            false,
            true,
          );
        };

        it("init group, banks, and fund accounts", async () => {
          const result = await genericMultiBankTestSetup(
            1,
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
          userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
          throwawayGroup = result.throwawayGroup.publicKey;

          if (juplendDeposits > 0) {
            const created = await addJuplendBanksForGroup({
              group: throwawayGroup,
              numberOfBanks: juplendDeposits,
              startingSeed: 30_000 + scenarioIndex * 100,
            });
            juplendBanks = created.juplendBanks;
            juplendPool = created.pool;
          }
        });

        it("refresh oracles", async () => {
          await refreshAllOracleFeeds();
        });

        it("(admin) Seeds liquidity in all banks", async () => {
          const marginfiAccount = groupAdmin.accounts.get(
            USER_ACCOUNT_THROWAWAY,
          );
          const depositLstAmount = new BN(
            10 * 10 ** ecosystem.lstAlphaDecimals,
          );
          const depositTokenAAmount = new BN(
            100 * 10 ** ecosystem.tokenADecimals,
          );

          for (const bank of banks) {
            const tx = new Transaction().add(
              await depositIx(groupAdmin.mrgnBankrunProgram, {
                marginfiAccount,
                bank,
                tokenAccount: groupAdmin.lstAlphaAccount,
                amount: depositLstAmount,
                depositUpToLimit: false,
              }),
            );
            await processBankrunTransaction(bankrunContext, tx, [
              groupAdmin.wallet,
            ]);
          }

          for (const bank of kaminoBanks) {
            const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
              bankrunProgram.programId,
              bank,
            );
            const [obligation] = deriveBaseObligation(
              lendingVaultAuthority,
              lendingMarket,
            );
            const obligationFarmUserState = reserveFarmState
              ? PublicKey.findProgramAddressSync(
                  [
                    Buffer.from("user"),
                    reserveFarmState.toBuffer(),
                    obligation.toBuffer(),
                  ],
                  FARMS_PROGRAM_ID,
                )[0]
              : null;

            const tx = new Transaction().add(
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
                groupAdmin.mrgnBankrunProgram,
                {
                  marginfiAccount,
                  bank,
                  signerTokenAccount: groupAdmin.tokenAAccount,
                  lendingMarket,
                  reserve: tokenAReserve,
                  obligationFarmUserState,
                  reserveFarmState: reserveFarmState ?? null,
                },
                depositTokenAAmount,
              ),
            );
            await processBankrunTransaction(bankrunContext, tx, [
              groupAdmin.wallet,
            ]);
          }

          for (const bank of driftBanks) {
            const tx = new Transaction().add(
              await makeDriftDepositIx(
                groupAdmin.mrgnBankrunProgram,
                {
                  marginfiAccount,
                  bank,
                  signerTokenAccount: groupAdmin.tokenAAccount,
                  driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
                },
                depositTokenAAmount,
                TOKEN_A_MARKET_INDEX,
              ),
            );
            await processBankrunTransaction(
              bankrunContext,
              tx,
              [groupAdmin.wallet],
              false,
              true,
            );
          }

          for (const bank of juplendBanks) {
            if (!juplendPool) {
              throw new Error("Juplend banks exist without a Juplend pool");
            }
            const tx = new Transaction().add(
              await makeJuplendDepositIx(groupAdmin.mrgnBankrunProgram, {
                marginfiAccount,
                signerTokenAccount: groupAdmin.tokenAAccount,
                bank,
                pool: juplendPool,
                amount: depositTokenAAmount,
              }),
            );
            await processBankrunTransaction(bankrunContext, tx, [
              groupAdmin.wallet,
            ]);
          }
        });

        it("(user 0) Deposits to all integration banks and borrows from a regular bank", async () => {
          const depositTokenAAmount = new BN(
            10 * 10 ** ecosystem.tokenADecimals,
          );
          const borrowLstAmount = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);

          for (const bank of kaminoBanks) {
            const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
              bankrunProgram.programId,
              bank,
            );
            const [obligation] = deriveBaseObligation(
              lendingVaultAuthority,
              lendingMarket,
            );
            const obligationFarmUserState = reserveFarmState
              ? PublicKey.findProgramAddressSync(
                  [
                    Buffer.from("user"),
                    reserveFarmState.toBuffer(),
                    obligation.toBuffer(),
                  ],
                  FARMS_PROGRAM_ID,
                )[0]
              : null;

            const tx = new Transaction().add(
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
                  marginfiAccount: userAccount,
                  bank,
                  signerTokenAccount: user.tokenAAccount,
                  lendingMarket,
                  reserve: tokenAReserve,
                  obligationFarmUserState,
                  reserveFarmState: reserveFarmState ?? null,
                },
                depositTokenAAmount,
              ),
            );

            await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
          }

          for (const bank of driftBanks) {
            const tx = new Transaction().add(
              await makeDriftDepositIx(
                user.mrgnBankrunProgram,
                {
                  marginfiAccount: userAccount,
                  bank,
                  signerTokenAccount: user.tokenAAccount,
                  driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
                },
                depositTokenAAmount,
                TOKEN_A_MARKET_INDEX,
              ),
            );
            await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
          }

          for (const bank of juplendBanks) {
            if (!juplendPool) {
              throw new Error("Juplend banks exist without a Juplend pool");
            }
            const tx = new Transaction().add(
              await makeJuplendDepositIx(user.mrgnBankrunProgram, {
                marginfiAccount: userAccount,
                signerTokenAccount: user.tokenAAccount,
                bank,
                pool: juplendPool,
                amount: depositTokenAAmount,
              }),
            );
            await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
          }

          remainingGroups = buildRemainingGroups();
          remainingAccounts = composeRemainingAccounts(remainingGroups);

          const tx = new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
            await borrowIx(user.mrgnBankrunProgram, {
              marginfiAccount: userAccount,
              bank: banks[0],
              tokenAccount: user.lstAlphaAccount,
              remaining: remainingAccounts,
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

          const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
            userAccount,
          );
          const activeBalances = accAfter.lendingAccount.balances.filter(
            (b: any) => b.active === 1,
          );
          assert.equal(
            activeBalances.length,
            1 + kaminoBanks.length + driftBanks.length + juplendBanks.length,
          );
        });

        it("(user 0) Places a take-profit order", async () => {
          if (kaminoBanks.length > 0) {
            assetBank = kaminoBanks[0];
            assetIntegration = "kamino";
          } else if (driftBanks.length > 0) {
            assetBank = driftBanks[0];
            assetIntegration = "drift";
          } else {
            assetBank = juplendBanks[0];
            assetIntegration = "juplend";
          }

          const ix = await placeOrderIx(user.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
            bankKeys: [assetBank, banks[0]],
            trigger: {
              takeProfit: {
                threshold: bigNumberToWrappedI80F48(takeProfitThreshold),
                maxSlippage,
              },
            },
          });

          const tx = new Transaction().add(ix);
          await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

          [orderPk] = deriveOrderPda(
            user.mrgnBankrunProgram.programId,
            userAccount,
            [assetBank, banks[0]],
          );
        });

        it("fails to execute before oracle update, then succeeds after price changes", async () => {
          const [executeRecordPk] = deriveExecuteOrderPda(
            user.mrgnBankrunProgram.programId,
            orderPk,
          );

          const remainingGroupsPostRepay = remainingGroups.filter(
            (group) => !group[0].equals(banks[0]),
          );
          const remainingAccountsPostRepay = composeRemainingAccounts(
            remainingGroupsPostRepay,
          );

          const startIx = await startExecuteOrderIx(user.mrgnBankrunProgram, {
            group: throwawayGroup,
            marginfiAccount: userAccount,
            feePayer: user.wallet.publicKey,
            executor: user.wallet.publicKey,
            order: orderPk,
            remaining: remainingAccounts,
          });

          const repayInstruction = await repayIx(user.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            bank: banks[0],
            tokenAccount: user.lstAlphaAccount,
            amount: new BN(1 * 10 ** ecosystem.lstAlphaDecimals),
            repayAll: true,
            remaining: remainingAccounts,
          });

          const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
          const withdrawRemaining = remainingAccountsPostRepay;

          const preIxs: TransactionInstruction[] = [];
          let withdrawInstruction: TransactionInstruction;

          if (assetIntegration === "kamino") {
            const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(
              bankrunProgram.programId,
              assetBank,
            );
            const [obligation] = deriveBaseObligation(
              lendingVaultAuthority,
              lendingMarket,
            );
            const obligationFarmUserState = reserveFarmState
              ? PublicKey.findProgramAddressSync(
                  [
                    Buffer.from("user"),
                    reserveFarmState.toBuffer(),
                    obligation.toBuffer(),
                  ],
                  FARMS_PROGRAM_ID,
                )[0]
              : null;

            preIxs.push(
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

            withdrawInstruction = await makeKaminoWithdrawIx(
              user.mrgnBankrunProgram,
              {
                marginfiAccount: userAccount,
                authority: user.wallet.publicKey,
                bank: assetBank,
                mint: ecosystem.tokenAMint.publicKey,
                destinationTokenAccount: user.tokenAAccount,
                lendingMarket,
                reserve: tokenAReserve,
                obligationFarmUserState,
                reserveFarmState: reserveFarmState ?? null,
              },
              {
                amount: withdrawAmount,
                isWithdrawAll: false,
                remaining: withdrawRemaining,
              },
            );
          } else if (assetIntegration === "drift") {
            withdrawInstruction = await makeDriftWithdrawIx(
              user.mrgnBankrunProgram,
              {
                marginfiAccount: userAccount,
                bank: assetBank,
                destinationTokenAccount: user.tokenAAccount,
                driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
              },
              {
                amount: withdrawAmount,
                withdrawAll: false,
                remaining: withdrawRemaining,
              },
              driftBankrunProgram,
            );
          } else {
            if (!juplendPool || !juplendPrograms) {
              throw new Error(
                "Juplend withdraw requested without Juplend setup",
              );
            }
            preIxs.push(
              await refreshJupSimple(juplendPrograms.lending, {
                pool: juplendPool,
              }),
            );
            withdrawInstruction = await makeJuplendWithdrawSimpleIx(
              user.mrgnBankrunProgram,
              {
                marginfiAccount: userAccount,
                destinationTokenAccount: user.tokenAAccount,
                bank: assetBank,
                pool: juplendPool,
                amount: withdrawAmount,
                withdrawAll: false,
                remainingAccounts: withdrawRemaining,
              },
            );
          }

          const endIx = await endExecuteOrderIx(user.mrgnBankrunProgram, {
            group: throwawayGroup,
            marginfiAccount: userAccount,
            executor: user.wallet.publicKey,
            order: orderPk,
            executeRecord: executeRecordPk,
            feeRecipient: user.wallet.publicKey,
            remaining: remainingAccountsPostRepay,
          });

          lutAccount = await createLookupTableForInstructions(
            user.wallet,
            [startIx, repayInstruction, withdrawInstruction, endIx, ...preIxs],
          );

          // low price -> should fail trigger
          oracles.tokenAPrice = 10;
          await refreshAllOracleFeeds();
          let assetValueBefore = 0;
          let liabilityValueBefore = 0;
          let netValueBefore = 0;
          {
            await pulseOrderHealth(remainingAccounts);
            // const acc = await bankrunProgram.account.marginfiAccount.fetch(
            //   userAccount,
            // );
            // logHealthCache("m04 health cache (low price)", acc.healthCache);
          }

          // Trigger not met, fails until the price changes...
          const failTx = await buildExecuteOrderTx(
            user,
            startIx,
            repayInstruction,
            withdrawInstruction,
            endIx,
            preIxs,
          );
          const failResult = await banksClient.tryProcessTransaction(failTx);
          assertBankrunTxFailed(failResult, 6107);

          // advance time + update price
          const { slot } = await getEpochAndSlot(banksClient);
          bankrunContext.warpToSlot(BigInt(slot + 20));
          oracles.tokenAPrice = 200;
          await refreshAllOracleFeeds();
          {
            await pulseOrderHealth(remainingAccounts);
            const acc = await bankrunProgram.account.marginfiAccount.fetch(
              userAccount,
            );
            const cache = acc.healthCache;
            assetValueBefore = wrappedI80F48toBigNumber(
              cache.assetValue,
            ).toNumber();
            liabilityValueBefore = wrappedI80F48toBigNumber(
              cache.liabilityValue,
            ).toNumber();
            netValueBefore = assetValueBefore - liabilityValueBefore;
          }

          await refreshAllOracleFeeds();
          const successTx = await buildExecuteOrderTx(
            user,
            startIx,
            repayInstruction,
            withdrawInstruction,
            endIx,
            preIxs,
          );
          await processBankrunV0Transaction(
            bankrunContext,
            successTx,
            [user.wallet],
            false,
            true,
          );

          {
            await pulseOrderHealth(remainingAccountsPostRepay);
          }

          const accountAfter =
            await bankrunProgram.account.marginfiAccount.fetch(userAccount);
          const cacheAfter = accountAfter.healthCache;
          const assetValueAfter = wrappedI80F48toBigNumber(
            cacheAfter.assetValue,
          ).toNumber();
          const liabilityValueAfter = wrappedI80F48toBigNumber(
            cacheAfter.liabilityValue,
          ).toNumber();
          const netValueAfter = assetValueAfter - liabilityValueAfter;

          const liabDecrease = liabilityValueBefore - liabilityValueAfter;
          const assetDecrease = assetValueBefore - assetValueAfter;
          const netDecrease = netValueBefore - netValueAfter;

          const maxSlippageFrac = 0.05;
          const netLowerBound = netValueBefore * (1 - maxSlippageFrac) - 1;
          const netIncreaseTolerance = assetIntegration === "juplend" ? 30 : 1;

          assert.ok(
            netValueAfter <= netValueBefore + netIncreaseTolerance,
            `net value increased (b=${netValueBefore}, a=${netValueAfter}, t=${netIncreaseTolerance})`,
          );
          assert.ok(
            netValueAfter >= netLowerBound,
            "net value should not drop beyond allowed slippage",
          );
          assert.ok(
            liabilityValueAfter <= 0.01,
            "liability should be fully repaid",
          );
          if (assetIntegration !== "juplend") {
            assert.ok(
              assetDecrease >= liabDecrease - 0.01,
              "asset decrease should be at least liability decrease",
            );
            assert.ok(
              assetDecrease - liabDecrease <=
                netValueBefore * maxSlippageFrac + 1,
              "keeper profit should be bounded by slippage",
            );
            assert.ok(
              Math.abs(netDecrease - (assetDecrease - liabDecrease)) <= 1,
              "net decrease should match keeper profit (within tolerance)",
            );
          }
          // The liability and the order itself are both closed
          const liabBalance = accountAfter.lendingAccount.balances.find(
            (b: any) => b.bankPk && b.bankPk.equals(banks[0]),
          );
          assert.isUndefined(liabBalance);

          const orderInfo =
            await bankrunProgram.provider.connection.getAccountInfo(orderPk);
          assert.isNull(orderInfo);

          // Restore default price to avoid side-effects later.
          oracles.tokenAPrice = 10;
          await refreshAllOracleFeeds();
        });
      },
    );
  },
);
