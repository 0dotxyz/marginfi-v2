import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
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
  updateEmissionsDestination,
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
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { refreshPullOracles } from "./utils/pyth-pull-mocks";
import {
  dumpBankrunLogs,
  logHealthCache,
  processBankrunTransaction,
} from "./utils/tools";
import {
  deriveBaseObligation,
  deriveExecuteOrderPda,
  deriveLiquidityVaultAuthority,
  deriveOrderPda,
} from "./utils/pdas";
import { getEpochAndSlot } from "./utils/stake-utils";
import { assert } from "chai";
import { TOKEN_A_MARKET_INDEX } from "./utils/drift-utils";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { assertBankrunTxFailed } from "./utils/genericTests";
import type { MockUser } from "./utils/mocks";

const startingSeed: number = 77;
const U32_MAX = 2 ** 32 - 1;
const bpsToU32 = (bps: number) => Math.floor((bps / 10_000) * U32_MAX);
const maxSlippage = bpsToU32(500);
const takeProfitThreshold = 1_000;

const SCENARIOS: Array<{ kaminoDeposits: number; driftDeposits: number }> = [
  { kaminoDeposits: 15, driftDeposits: 0 },
  { kaminoDeposits: 8, driftDeposits: 7 },
  { kaminoDeposits: 0, driftDeposits: 15 },
];

function groupSeedForScenario(index: number): Buffer {
  return Buffer.from(
    `MARGINFI_GROUP_SEED_12340000M4${index.toString().padStart(2, "0")}`,
  );
}

function userAccountNameForScenario(index: number): string {
  return `throwaway_account_m4_${index}`;
}

function scenarioName(kaminoDeposits: number, driftDeposits: number) {
  return `m04: Order limits (Kamino=${kaminoDeposits}, Drift=${driftDeposits})`;
}

SCENARIOS.forEach(({ kaminoDeposits, driftDeposits }, scenarioIndex) => {
  const groupBuff = groupSeedForScenario(scenarioIndex);
  const USER_ACCOUNT_THROWAWAY = userAccountNameForScenario(scenarioIndex);

  describe(scenarioName(kaminoDeposits, driftDeposits), () => {
    let banks: PublicKey[] = [];
    let kaminoBanks: PublicKey[] = [];
    let driftBanks: PublicKey[] = [];
    let remainingGroups: PublicKey[][] = [];
    let remainingAccounts: PublicKey[] = [];
    let lutAccount: AddressLookupTableAccount;
    let orderPk: PublicKey;
    let assetBank: PublicKey;
    let useKamino = false;
    let throwawayGroup: PublicKey;
    let lendingMarket: PublicKey;
    let tokenAReserve: PublicKey;
    let reserveFarmState: PublicKey | undefined;
    let driftSpotMarket: PublicKey;
    let user: MockUser;
    let userAccount: PublicKey;

    before(() => {
      user = users[0];
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
        groups.push([bank, oracles.tokenAOracle.publicKey, driftSpotMarket]);
      }
      return groups;
    };

    const createLut = async (
      signer: MockUser,
      addresses: PublicKey[],
    ): Promise<AddressLookupTableAccount> => {
      const recentSlot = Number(await banksClient.getSlot());
      const [createLutIx, lutAddress] =
        AddressLookupTableProgram.createLookupTable({
          authority: signer.wallet.publicKey,
          payer: signer.wallet.publicKey,
          recentSlot: recentSlot - 1,
        });

      const createLutTx = new Transaction().add(createLutIx);
      createLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      createLutTx.sign(signer.wallet);
      await banksClient.processTransaction(createLutTx);

      const CHUNK = 20;
      for (let i = 0; i < addresses.length; i += CHUNK) {
        const extendTx = new Transaction().add(
          AddressLookupTableProgram.extendLookupTable({
            authority: signer.wallet.publicKey,
            payer: signer.wallet.publicKey,
            lookupTable: lutAddress,
            addresses: addresses.slice(i, i + CHUNK),
          }),
        );
        extendTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
        extendTx.sign(signer.wallet);
        await banksClient.processTransaction(extendTx);
      }

      // allow LUT to activate
      const { slot } = await getEpochAndSlot(banksClient);
      bankrunContext.warpToSlot(BigInt(slot + 25));

      const lutRaw = await banksClient.getAccount(lutAddress);
      const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
      return new AddressLookupTableAccount({
        key: lutAddress,
        state: lutState,
      });
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

    const collectLutAddresses = (
      instructions: TransactionInstruction[],
    ): PublicKey[] => {
      const seen = new Set<string>();
      const out: PublicKey[] = [];
      const push = (key: PublicKey) => {
        const k = key.toBase58();
        if (seen.has(k)) return;
        seen.add(k);
        out.push(key);
      };
      for (const ix of instructions) {
        push(ix.programId);
        for (const meta of ix.keys) {
          push(meta.pubkey);
        }
      }
      return out;
    };

    const buildKaminoReserveRefreshIxs = async (): Promise<
      TransactionInstruction[]
    > => {
      if (kaminoBanks.length === 0) return [];
      return [
        await simpleRefreshReserve(
          klendBankrunProgram,
          tokenAReserve,
          lendingMarket,
          oracles.tokenAOracle.publicKey,
        ),
      ];
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
    });

    it("refresh oracles", async () => {
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

    it("(admin) Seeds liquidity in all banks", async () => {
      const marginfiAccount = groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY);
      const depositLstAmount = new BN(10 * 10 ** ecosystem.lstAlphaDecimals);
      const depositTokenAAmount = new BN(100 * 10 ** ecosystem.tokenADecimals);

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
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
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
    });

    it("(user 0) Deposits to all integration banks and borrows from a regular bank", async () => {
      const depositTokenAAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);
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
              reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
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
        1 + kaminoBanks.length + driftBanks.length,
      );
    });

    it("(user 0) Places a take-profit order", async () => {
      assetBank = kaminoBanks.length > 0 ? kaminoBanks[0] : driftBanks[0];
      useKamino = kaminoBanks.length > 0;

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

      const updateEmissionsIx = await updateEmissionsDestination(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          destinationAccount: user.wallet.publicKey,
        },
      );

      const tx = new Transaction().add(updateEmissionsIx).add(ix);
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
      });

      const withdrawAmount = new BN(1 * 10 ** ecosystem.tokenADecimals);
      const withdrawRemaining = remainingAccountsPostRepay;

      const preIxs: TransactionInstruction[] = [];
      let withdrawInstruction: TransactionInstruction;

      if (useKamino) {
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
            destinationTokenAccount: user.tokenAAccount,
            lendingMarket,
            reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
            obligationFarmUserState,
            reserveFarmState: reserveFarmState ?? null,
          },
          {
            amount: withdrawAmount,
            isWithdrawAll: false,
            remaining: withdrawRemaining,
          },
        );
      } else {
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

      const lutAddresses = collectLutAddresses([
        startIx,
        repayInstruction,
        withdrawInstruction,
        endIx,
        ...preIxs,
      ]);
      // console.log(
      //   `m04 LUT addresses: ${lutAddresses.length} (remaining: ${remainingAccounts.length}, preIxs: ${preIxs.length})`,
      // );
      lutAccount = await createLut(user, lutAddresses);

      // low price -> should fail trigger
      oracles.tokenAPrice = 10;
      let clock = await banksClient.getClock();
      await refreshPullOracles(
        oracles,
        globalProgramAdmin.wallet,
        new BN(Number(clock.slot)),
        Number(clock.unixTimestamp),
        bankrunContext,
        false,
      );
      let assetValueBefore = 0;
      let liabilityValueBefore = 0;
      let netValueBefore = 0;
      {
        const refreshIxs = await buildKaminoReserveRefreshIxs();
        const pulseIx = await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          remaining: remainingAccounts,
        });
        const pulseTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          ...refreshIxs,
          pulseIx,
        );
        await processBankrunTransaction(
          bankrunContext,
          pulseTx,
          [user.wallet],
          false,
          true,
        );
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
      clock = await banksClient.getClock();
      await refreshPullOracles(
        oracles,
        globalProgramAdmin.wallet,
        new BN(Number(clock.slot)),
        Number(clock.unixTimestamp),
        bankrunContext,
        false,
      );
      {
        const refreshIxs = await buildKaminoReserveRefreshIxs();
        const pulseIx = await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          remaining: remainingAccounts,
        });
        const pulseTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          ...refreshIxs,
          pulseIx,
        );
        await processBankrunTransaction(
          bankrunContext,
          pulseTx,
          [user.wallet],
          false,
          true,
        );
        const acc = await bankrunProgram.account.marginfiAccount.fetch(
          userAccount,
        );
        const cache = acc.healthCache;
        assetValueBefore = wrappedI80F48toBigNumber(cache.assetValue).toNumber();
        liabilityValueBefore = wrappedI80F48toBigNumber(
          cache.liabilityValue,
        ).toNumber();
        netValueBefore = assetValueBefore - liabilityValueBefore;
      }

      const successTx = await buildExecuteOrderTx(
        user,
        startIx,
        repayInstruction,
        withdrawInstruction,
        endIx,
        preIxs,
      );
      await banksClient.processTransaction(successTx);

      {
        const refreshIxs = await buildKaminoReserveRefreshIxs();
        const pulseIx = await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          remaining: remainingAccountsPostRepay,
        });
        const pulseTx = new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
          ...refreshIxs,
          pulseIx,
        );
        await processBankrunTransaction(
          bankrunContext,
          pulseTx,
          [user.wallet],
          false,
          true,
        );
      }

      const accountAfter = await bankrunProgram.account.marginfiAccount.fetch(
        userAccount,
      );
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

      assert.ok(
        netValueAfter <= netValueBefore + 1,
        "net value should not increase after execution",
      );
      assert.ok(
        netValueAfter >= netLowerBound,
        "net value should not drop beyond allowed slippage",
      );
      assert.ok(
        liabilityValueAfter <= 0.01,
        "liability should be fully repaid",
      );
      assert.ok(
        assetDecrease >= liabDecrease - 0.01,
        "asset decrease should be at least liability decrease",
      );
      assert.ok(
        assetDecrease - liabDecrease <= netValueBefore * maxSlippageFrac + 1,
        "keeper profit should be bounded by slippage",
      );
      assert.ok(
        Math.abs(netDecrease - (assetDecrease - liabDecrease)) <= 1,
        "net decrease should match keeper profit (within tolerance)",
      );
      const liabBalance = accountAfter.lendingAccount.balances.find(
        (b: any) => b.bankPk && b.bankPk.equals(banks[0]),
      );
      assert.isUndefined(liabBalance);

      const orderInfo = await bankrunProgram.provider.connection.getAccountInfo(
        orderPk,
      );
      assert.isNull(orderInfo);

      oracles.tokenAPrice = 10;
      clock = await banksClient.getClock();
      await refreshPullOracles(
        oracles,
        globalProgramAdmin.wallet,
        new BN(Number(clock.slot)),
        Number(clock.unixTimestamp),
        bankrunContext,
        false,
      );
    });
  });
});
