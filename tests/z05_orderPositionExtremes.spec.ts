import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  ComputeBudgetProgram,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  A_FARM_STATE,
  bankrunContext,
  bankrunProgram,
  banksClient,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  driftAccounts,
  ecosystem,
  farmAccounts,
  FARMS_PROGRAM_ID,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  users,
} from "./rootHooks";
import { genericMultiBankTestSetup } from "./genericSetups";
import { ensureMultiSuiteIntegrationsSetup } from "./utils/multi-limits-setup";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  endExecuteOrderIx,
  placeOrderIx,
  startExecuteOrderIx,
  OrderTriggerArgs,
} from "./utils/user-instructions";
import { deriveExecuteOrderPda, deriveOrderPda } from "./utils/pdas";
import { makeKaminoDepositIx } from "./utils/kamino-instructions";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import {
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { makeDriftDepositIx } from "./utils/drift-instructions";
import { TOKEN_A_MARKET_INDEX, refreshDriftOracles } from "./utils/drift-utils";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  createLut,
  processBankrunTransaction,
  getBankrunBlockhash,
} from "./utils/tools";
import { getEpochAndSlot } from "./utils/bankrunConnection";
import {
  bigNumberToWrappedI80F48,
  TOKEN_PROGRAM_ID,
} from "@mrgnlabs/mrgn-common";
import { assert } from "chai";

/**
 * Position-extremes coverage for orders: place an order on an account filled to the MAX of 16
 * active balances, mixing normal AND integration (Kamino + Drift)
 * banks, each with a different balance. The order's two legs (one asset, one liability) are normal
 * banks; the other 14 balances are untouched collateral. This exactly hits the order execute-record
 * limit (MAX_EXECUTE_RECORD_BALANCES = MAX_LENDING_ACCOUNT_BALANCES - 2 = 14 non-order balances).
 */
describe("Order position extremes (16 balances, mixed normal + integration)", () => {
  const startingSeed = 950;
  // Keypair.fromSeed requires exactly 32 bytes.
  const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_ORDER_XTREME");
  const USER_ACCOUNT_THROWAWAY = "throwaway_order_extremes";

  const NUM_NORMAL = 14;
  const NUM_KAMINO = 1;
  const NUM_DRIFT = 1; // total = 16 banks => 16 active balances

  let normalBanks: PublicKey[] = [];
  let kaminoBank: PublicKey;
  let driftBank: PublicKey;
  let throwawayGroupPk: PublicKey;

  let market: PublicKey;
  let tokenAReserve: PublicKey;
  let obligation: PublicKey;
  let reserveFarmState: PublicKey | null;
  let obligationFarmUserState: PublicKey | null;
  let driftSpotMarket: PublicKey;
  let driftPullOracle: PublicKey;

  let assetBank: PublicKey; // normalBanks[0] - user deposits (collateral)
  let liabBank: PublicKey; // normalBanks[1] - user borrows

  const normDecimals = ecosystem.lstAlphaDecimals;
  const tokenADecimals = ecosystem.tokenADecimals;
  const norm = (n: number) => new BN(Math.floor(n * 10 ** normDecimals));
  const tokA = (n: number) => new BN(Math.floor(n * 10 ** tokenADecimals));

  before(async () => {
    // The z* (bankruptcy) slice does not run the k*/d* setup specs, so bootstrap Kamino + Drift.
    await ensureMultiSuiteIntegrationsSetup();

    const result = await genericMultiBankTestSetup(
      NUM_NORMAL,
      USER_ACCOUNT_THROWAWAY,
      groupBuff,
      startingSeed,
      NUM_KAMINO,
      NUM_DRIFT
    );
    normalBanks = result.banks;
    kaminoBank = result.kaminoBanks[0];
    driftBank = result.driftBanks[0];
    throwawayGroupPk = result.throwawayGroup.publicKey;

    assetBank = normalBanks[0];
    liabBank = normalBanks[1];

    market = kaminoAccounts.get(MARKET);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const [lva] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      kaminoBank
    );
    [obligation] = deriveBaseObligation(lva, market);
    const farmState = farmAccounts.get(A_FARM_STATE);
    reserveFarmState = farmState ?? null;
    obligationFarmUserState = farmState
      ? PublicKey.findProgramAddressSync(
          [Buffer.from("user"), farmState.toBuffer(), obligation.toBuffer()],
          FARMS_PROGRAM_ID
        )[0]
      : null;

    driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
    driftPullOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  const kaminoRefreshIxs = async () => [
    await simpleRefreshReserve(
      klendBankrunProgram,
      tokenAReserve,
      market,
      oracles.tokenAOracle.publicKey
    ),
    await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
      tokenAReserve,
    ]),
  ];

  it("fills an account to 16 mixed balances and places an order on it", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const adminAccount = groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY);

    // groupAdmin seeds the liability bank so user 0 can borrow from it.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: adminAccount,
          bank: liabBank,
          tokenAccount: groupAdmin.lstAlphaAccount,
          amount: norm(500),
          depositUpToLimit: false,
        })
      ),
      [groupAdmin.wallet]
    );

    // user 0 deposits collateral into the order asset bank.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: assetBank,
          tokenAccount: user.lstAlphaAccount,
          amount: norm(200),
          depositUpToLimit: false,
        })
      ),
      [user.wallet]
    );

    // user 0 borrows from the liability bank (only 2 balances active, so the tx stays small).
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: liabBank,
          tokenAccount: user.lstAlphaAccount,
          remaining: composeRemainingAccounts([
            [assetBank, oracles.pythPullLst.publicKey],
            [liabBank, oracles.pythPullLst.publicKey],
          ]),
          amount: norm(1),
        })
      ),
      [user.wallet]
    );

    // Fill the remaining normal banks (distinct amounts). Deposits skip the health check, so each is
    // its own small standalone tx.
    for (let i = 2; i < NUM_NORMAL; i++) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            bank: normalBanks[i],
            tokenAccount: user.lstAlphaAccount,
            amount: norm(1 + i * 0.5), // distinct amounts
            depositUpToLimit: false,
          })
        ),
        [user.wallet]
      );
    }

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        ...(await kaminoRefreshIxs()),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoBank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserve: tokenAReserve,
            reserveFarmState,
            obligationFarmUserState,
          },
          tokA(7)
        )
      ),
      [user.wallet]
    );

    await refreshDriftOracles(oracles, driftAccounts, bankrunContext, banksClient);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await makeDriftDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: driftBank,
            signerTokenAccount: user.tokenAAccount,
            driftOracle: driftPullOracle,
          },
          tokA(9),
          TOKEN_A_MARKET_INDEX
        )
      ),
      [user.wallet]
    );

    // Account is now maxed: 16 active balances.
    const acc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const active = acc.lendingAccount.balances.filter((b) => b.active === 1);
    assert.equal(active.length, 16, "expected 16 active balances");

    const bankKeys = [assetBank, liabBank];
    const trigger: OrderTriggerArgs = {
      takeProfit: {
        threshold: bigNumberToWrappedI80F48(50),
        maxSlippage: Math.floor((100 / 10_000) * 0xffff_ffff),
      },
    };
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await placeOrderIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
          bankKeys,
          trigger,
        })
      ),
      [user.wallet]
    );

    const [orderPk] = deriveOrderPda(
      bankrunProgram.programId,
      userAccount,
      bankKeys
    );
    const order = await bankrunProgram.account.order.fetch(orderPk);
    assert.ok(order, "order should exist");
    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    assert.equal(accAfter.activeOrders, 1, "expected 1 active order");
  });

  it("executes the order on the maxed 16-balance account (via LUT) - happy path", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const keeper = users[1];
    const bankKeys = [assetBank, liabBank];
    const [orderPk] = deriveOrderPda(
      bankrunProgram.programId,
      userAccount,
      bankKeys
    );
    const [executeRecordPk] = deriveExecuteOrderPda(
      bankrunProgram.programId,
      orderPk
    );

    // Observation groups for all 16 balances. The order's asset/liability legs (normalBanks[0,1]) are
    // tag-skipped by the execute record; the other 14 fill it exactly (MAX_EXECUTE_RECORD_BALANCES).
    const allGroups: PublicKey[][] = [
      ...normalBanks.map((b) => [b, oracles.pythPullLst.publicKey]),
      [kaminoBank, oracles.tokenAOracle.publicKey, tokenAReserve],
      [driftBank, oracles.tokenAOracle.publicKey, driftSpotMarket],
    ];
    const startRemaining = composeRemainingAccounts(allGroups);
    // After repayAll the liability balance is closed, so end health omits it.
    const endRemaining = composeRemainingAccounts(
      allGroups.filter((g) => !g[0].equals(liabBank))
    );
    const asMeta = (pubkey: PublicKey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    });

    const startIx = await startExecuteOrderIx(keeper.mrgnBankrunProgram, {
      group: throwawayGroupPk,
      marginfiAccount: userAccount,
      feePayer: keeper.wallet.publicKey,
      executor: keeper.wallet.publicKey,
      order: orderPk,
      remaining: startRemaining,
    });

    // Keeper repays the user's liability in full, then withdraws an equivalent amount of the asset
    // (same lstAlpha mint), leaving the account net ~unchanged (fee ~0, well within max-fee).
    const repayInstruction = await bankrunProgram.methods
      .lendingAccountRepay(norm(1), true)
      .accountsPartial({
        marginfiAccount: userAccount,
        authority: keeper.wallet.publicKey,
        bank: liabBank,
        signerTokenAccount: keeper.lstAlphaAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(startRemaining.map(asMeta))
      .instruction();

    const withdrawInstruction = await bankrunProgram.methods
      .lendingAccountWithdraw(norm(1), false)
      .accountsPartial({
        marginfiAccount: userAccount,
        authority: keeper.wallet.publicKey,
        bank: assetBank,
        destinationTokenAccount: keeper.lstAlphaAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(
        composeRemainingAccounts([
          [assetBank, oracles.pythPullLst.publicKey],
        ]).map(asMeta)
      )
      .instruction();

    const endIx = await endExecuteOrderIx(keeper.mrgnBankrunProgram, {
      group: throwawayGroupPk,
      marginfiAccount: userAccount,
      executor: keeper.wallet.publicKey,
      order: orderPk,
      executeRecord: executeRecordPk,
      feeRecipient: keeper.wallet.publicKey,
      remaining: endRemaining,
    });

    const execIxs = [
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      startIx,
      repayInstruction,
      withdrawInstruction,
      endIx,
    ];

    // The 16-balance observation list blows the 1232-byte legacy limit; pack into a LUT + v0 tx.
    const lutAddresses: PublicKey[] = [];
    const seen = new Set<string>();
    for (const ix of execIxs) {
      for (const key of [ix.programId, ...ix.keys.map((k) => k.pubkey)]) {
        if (!seen.has(key.toBase58())) {
          seen.add(key.toBase58());
          lutAddresses.push(key);
        }
      }
    }
    const lut = await createLut(keeper.wallet, lutAddresses);

    // Advance to activate the LUT, then refresh every oracle/reserve into the fresh slot so the
    // 16-balance health reads don't see stale prices.
    const { slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(slot + 24));
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await refreshDriftOracles(oracles, driftAccounts, bankrunContext, banksClient);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        ...(await kaminoRefreshIxs())
      ),
      [user.wallet]
    );

    const lutRaw = await banksClient.getAccount(lut.key);
    const lutAccount = new AddressLookupTableAccount({
      key: lut.key,
      state: AddressLookupTableAccount.deserialize(lutRaw.data),
    });
    const messageV0 = new TransactionMessage({
      payerKey: keeper.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: execIxs,
    }).compileToV0Message([lutAccount]);
    const vtx = new VersionedTransaction(messageV0);
    vtx.sign([keeper.wallet]);
    await banksClient.processTransaction(vtx);

    // Order executed & closed; the 14 untouched balances + asset remain, liability is gone.
    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    assert.equal(accAfter.activeOrders, 0, "order should be closed post-execution");
    const active = accAfter.lendingAccount.balances.filter(
      (b) => b.active === 1
    );
    assert.equal(active.length, 15, "liability closed => 15 active balances");
    assert.ok(
      !active.some((b) => b.bankPk.equals(liabBank)),
      "liability balance should be closed"
    );
  });
});
