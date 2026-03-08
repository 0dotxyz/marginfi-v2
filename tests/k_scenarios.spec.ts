import { BN } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  Transaction,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  KAMINO_TOKEN_A_BANK,
  MARKET,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  oracles,
  users,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  banksClient,
  farmAccounts,
  A_FARM_STATE,
  A_OBLIGATION_USER_STATE,
  FARMS_PROGRAM_ID,
} from "./rootHooks";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
  defaultKaminoBankConfig,
} from "./utils/kamino-utils";
import { USER_ACCOUNT_K } from "./utils/mocks";
import {
  processBankrunTransaction,
  getBankrunBlockhash,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  makeKaminoDepositIx,
  makeKaminoDepositWithRefreshIx,
  makeKaminoWithdrawIx,
  makeKaminoWithdrawWithRefreshIx,
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeInitObligationBatchRefreshIx,
} from "./utils/kamino-instructions";
import {
  depositIx,
  borrowIx,
  liquidateIx,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import { configureBank } from "./utils/group-instructions";
import { blankBankConfigOptRaw } from "./utils/types";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import {
  Clock,
  ProgramTestContext,
  BanksTransactionResultWithMeta,
} from "solana-bankrun";

const THROWAWAY_GROUP_SEED_KSCEN = Buffer.from(
  "MARGINFI_GROUP_SEED_12340ks00000"
);
const USER_ACCOUNT_KSCEN = "throwaway_account_k_scen";
const LIQ_STARTING_SEED = 20;

const SLOTS_ADVANCE = 200;
const MS_PER_SLOT = 400;

let ctx: ProgramTestContext;

// Groups A-C
let usdcBank: PublicKey;
let usdcReserve: PublicKey;
let usdcObligation: PublicKey;
let tokenABank: PublicKey;
let tokenAReserve: PublicKey;
let tokenAObligation: PublicKey;
let market: PublicKey;

// Group D
let liqBanks: PublicKey[] = [];
let liqThrowawayGroup: Keypair;
let liqKaminoUsdcBank: PublicKey;
let liqKaminoObligation: PublicKey;
let liqMarket: PublicKey;
let liqUsdcReserve: PublicKey;

// Group F
const THROWAWAY_GROUP_SEED_F = Buffer.from("MARGINFI_GROUP_SEED_12340ks00002");
const USER_ACCOUNT_F = "throwaway_account_k_f";
const F_STARTING_SEED = 30;
let fThrowawayGroup: Keypair;
let fBankA: PublicKey;
let fBankB: PublicKey;
let fBankC: PublicKey;
let fBankD: PublicKey;
let fMarket: PublicKey;
let fUsdcReserve: PublicKey;
let fTokenAReserve: PublicKey;
let fUserStateC: PublicKey;
let fUserStateD: PublicKey;


interface Measurement {
  label: string;
  cuConsumed: number;
  txSizeBytes: number;
  numInstructions: number;
  numAccounts: number;
  success: boolean;
}

interface ScenarioResult {
  id: string;
  name: string;
  group: string;
  a: Measurement;
  b: Measurement;
}

const results: ScenarioResult[] = [];
const CU_BUDGET = 1_400_000;


function usdc(n: number = 1): BN {
  return new BN(n * 10 ** ecosystem.usdcDecimals);
}
function tokenA(n: number = 1): BN {
  return new BN(n * 10 ** ecosystem.tokenADecimals);
}
function lstAlpha(n: number = 1): BN {
  return new BN(n * 10 ** ecosystem.lstAlphaDecimals);
}
function computeBudget(units: number = CU_BUDGET) {
  return ComputeBudgetProgram.setComputeUnitLimit({ units });
}

function bothBanksRemaining(): PublicKey[] {
  return composeRemainingAccounts([
    [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
    [tokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
  ]);
}

function tokenAFarms() {
  return {
    obligationFarmUserState: farmAccounts.get(A_OBLIGATION_USER_STATE) ?? null,
    reserveFarmState: farmAccounts.get(A_FARM_STATE) ?? null,
  };
}

async function rawDeposit(
  user: any, bank: PublicKey, signerAcc: any, mint: any, amount: BN
) {
  const isTokenA = tokenABank && bank.equals(tokenABank);
  return makeKaminoDepositIx(user.mrgnBankrunProgram, {
    marginfiAccount: user.accounts.get(USER_ACCOUNT_K),
    bank, signerTokenAccount: signerAcc, lendingMarket: market,
    reserveLiquidityMint: mint,
    ...(isTokenA ? tokenAFarms() : {}),
  }, amount);
}

async function rawWithdraw(
  user: any, bank: PublicKey, destAcc: any, mint: any, amount: BN
) {
  const isTokenA = tokenABank && bank.equals(tokenABank);
  return makeKaminoWithdrawIx(user.mrgnBankrunProgram, {
    marginfiAccount: user.accounts.get(USER_ACCOUNT_K),
    authority: user.wallet.publicKey, bank, destinationTokenAccount: destAcc,
    lendingMarket: market, reserveLiquidityMint: mint,
    ...(isTokenA ? tokenAFarms() : {}),
  }, { amount, isWithdrawAll: false, remaining: bothBanksRemaining() });
}

async function cpiDeposit(
  user: any, bank: PublicKey, mint: any,
  signerAcc: any, amount: BN
) {
  const isTokenA = tokenABank && bank.equals(tokenABank);
  return makeKaminoDepositWithRefreshIx(user.mrgnBankrunProgram, {
    marginfiAccount: user.accounts.get(USER_ACCOUNT_K),
    bank, signerTokenAccount: signerAcc, lendingMarket: market,
    reserveLiquidityMint: mint,
    ...(isTokenA ? tokenAFarms() : {}),
  }, amount);
}

async function cpiWithdraw(
  user: any, bank: PublicKey,
  destAcc: any, mint: any, amount: BN
) {
  const isTokenA = tokenABank && bank.equals(tokenABank);
  return makeKaminoWithdrawWithRefreshIx(user.mrgnBankrunProgram, {
    marginfiAccount: user.accounts.get(USER_ACCOUNT_K),
    authority: user.wallet.publicKey, bank, destinationTokenAccount: destAcc,
    lendingMarket: market, reserveLiquidityMint: mint,
    ...(isTokenA ? tokenAFarms() : {}),
  }, { amount, isWithdrawAll: false, remaining: bothBanksRemaining() });
}

const refreshUsdcReserve = () => batchRefreshReserves([usdcReserve], market);
const refreshUsdcObligation = () =>
  simpleRefreshObligation(klendBankrunProgram, market, usdcObligation, [usdcReserve]);
const refreshTokenAReserve = () => batchRefreshReserves([tokenAReserve], market);
const refreshTokenAObligation = () =>
  simpleRefreshObligation(klendBankrunProgram, market, tokenAObligation, [tokenAReserve]);

async function batchRefreshReserves(
  reserves: PublicKey[],
  market: PublicKey
): Promise<any> {
  const remaining: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean }[] = [];
  for (const r of reserves) {
    remaining.push({ pubkey: r, isSigner: false, isWritable: true });
    remaining.push({ pubkey: market, isSigner: false, isWritable: false });
  }
  return klendBankrunProgram.methods
    .refreshReservesBatch(true)
    .remainingAccounts(remaining)
    .instruction();
}


async function measure(
  instructions: any[], signer: any, label: string, cuBudget = CU_BUDGET
): Promise<Measurement> {
  const budgetIx = computeBudget(cuBudget);
  const tx = new Transaction().add(budgetIx, ...instructions);
  tx.recentBlockhash = await getBankrunBlockhash(ctx);
  tx.feePayer = signer.publicKey;

  const numInstructions = tx.instructions.length;

  const accountSet = new Set<string>();
  accountSet.add(signer.publicKey.toBase58());
  for (const ix of tx.instructions) {
    accountSet.add(ix.programId.toBase58());
    for (const key of ix.keys) accountSet.add(key.pubkey.toBase58());
  }

  const numAccounts = accountSet.size;

  let txSizeBytes = -1;
  try {
    tx.sign(signer);
    txSizeBytes = tx.serialize().length;
  } catch (err: any) {
    if (!err?.message?.includes("too large"))
      throw err;
    /* TX exceeds 1232-byte limit */
  }

  let cuConsumed = -1;
  let success = false;
  if (txSizeBytes !== -1) {
    const txExec = new Transaction().add(budgetIx, ...instructions); // Budget CU is neglible
    const result: BanksTransactionResultWithMeta =
      await processBankrunTransaction(ctx, txExec, [signer], true);
    success = result.result === null;
    cuConsumed = Number(result.meta?.computeUnitsConsumed ?? 0);
  }

  return { label, cuConsumed, txSizeBytes, numInstructions, numAccounts, success };
}

function formatComputeUnits(n: number): string {
  return n === -1 ? "       N/A" : n.toLocaleString().padStart(10);
}
function formatTxSize(n: number): string {
  return n === -1 ? " >1232B" : `${n}B`.padStart(7);
}
function printComparison(
  id: string, name: string, aResult: Measurement, bResult: Measurement
): void {
  const cuDelta = aResult.cuConsumed !== -1 && bResult.cuConsumed !== -1
    ? bResult.cuConsumed - aResult.cuConsumed : null;
  const sizeDelta = aResult.txSizeBytes !== -1 && bResult.txSizeBytes !== -1
    ? bResult.txSizeBytes - aResult.txSizeBytes : null;
  const ixDelta = bResult.numInstructions - aResult.numInstructions;
  const formatDelta = (n: number | null) =>
    n === null ? "N/A" : ((n >= 0 ? "+" : "") + n.toLocaleString());
  const cuPctStr = cuDelta !== null && aResult.cuConsumed > 0
    ? ` (${(cuDelta / aResult.cuConsumed * 100).toFixed(1)}%)` : "";

  console.log(`    ${id}: ${name}`);
  console.log(`      ${aResult.label.padEnd(14)}: ${formatComputeUnits(aResult.cuConsumed)} CU |${formatTxSize(aResult.txSizeBytes)} | ${aResult.numInstructions} ixs | ${aResult.numAccounts} accts  ${aResult.success ? "✓" : "✗"}`);
  console.log(`      ${bResult.label.padEnd(14)}: ${formatComputeUnits(bResult.cuConsumed)} CU |${formatTxSize(bResult.txSizeBytes)} | ${bResult.numInstructions} ixs | ${bResult.numAccounts} accts  ${bResult.success ? "✓" : "✗"}`);
  console.log(`      Delta:         ${formatDelta(cuDelta).padStart(10)} CU${cuPctStr} | ${formatDelta(sizeDelta)} | ${formatDelta(ixDelta)} ixs`);
}

async function warpClock() {
  const clock = await banksClient.getClock();

  const newSlot = clock.slot + BigInt(SLOTS_ADVANCE);
  const newTimestamp = clock.unixTimestamp +
    BigInt(Math.round(SLOTS_ADVANCE * MS_PER_SLOT / 1000));
  const warped = await banksClient.getClock();
  ctx.setClock(new Clock(
    newSlot,
    warped.epochStartTimestamp,
    warped.epoch,
    warped.leaderScheduleEpoch,
    newTimestamp,
  ));
}


describe("k_scenarios: CPI vs External", () => {


  before(async () => {
    ctx = bankrunContext;
    market = kaminoAccounts.get(MARKET);
    usdcBank = kaminoAccounts.get(KAMINO_USDC_BANK);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    usdcObligation = kaminoAccounts.get(`${usdcBank.toString()}_OBLIGATION`);
    tokenAObligation = kaminoAccounts.get(`${tokenABank.toString()}_OBLIGATION`);

    await refreshPullOraclesBankrun(oracles, ctx, banksClient);

    const user = users[0];

    await processBankrunTransaction(ctx, new Transaction().add(
      computeBudget(),
      await refreshUsdcReserve(), await refreshUsdcObligation(),
      await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey,
        usdc(200)),
    ), [user.wallet], false);

    await processBankrunTransaction(ctx, new Transaction().add(
      computeBudget(),
      await refreshTokenAReserve(), await refreshTokenAObligation(),
      await rawDeposit(user, tokenABank, user.tokenAAccount, ecosystem.tokenAMint.publicKey,
        tokenA(200)),
    ), [user.wallet], false);
  });

  describe("Group A: Single operation", () => {
    it("S01: Single USDC deposit", async () => {
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);
      const user = users[0];
      const amount = usdc();

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const ext = await measure([
        await refreshUsdcReserve(), await refreshUsdcObligation(),
        await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey,
          amount),
      ], user.wallet, "External");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const cpi = await measure([
        await cpiDeposit(user, usdcBank,
          ecosystem.usdcMint.publicKey, user.usdcAccount, amount),
      ], user.wallet, "CPI");

      printComparison("S01", "Single USDC deposit", ext, cpi);
      results.push({ id: "S01", name: "Single USDC deposit", group: "A", a: ext, b: cpi });
    });

    it("S02: Single USDC withdraw", async () => {
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);
      const user = users[0];
      const amount = usdc();

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const ext = await measure([
        await refreshUsdcReserve(), await refreshUsdcObligation(),
        await rawWithdraw(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey, amount),
      ], user.wallet, "External");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const cpi = await measure([
        await cpiWithdraw(user, usdcBank,
          user.usdcAccount, ecosystem.usdcMint.publicKey, amount),
      ], user.wallet, "CPI");

      printComparison("S02", "Single USDC withdraw", ext, cpi);
      results.push({ id: "S02", name: "Single USDC withdraw", group: "A", a: ext, b: cpi });
    });
  });

  describe("Group B: Same-bank staleness", () => {
    it("S03: 2x USDC deposit - must re-refresh between ops", async () => {
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);
      const user = users[0];
      const amount = usdc();

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      // Reserves go stale after any operations
      const ext = await measure([
        await refreshUsdcReserve(), await refreshUsdcObligation(),
        await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey, amount),
        await refreshUsdcReserve(), await refreshUsdcObligation(),
        await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey, amount),
      ], user.wallet, "External");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const cpi = await measure([
        await cpiDeposit(user, usdcBank,
          ecosystem.usdcMint.publicKey, user.usdcAccount, amount),
        await cpiDeposit(user, usdcBank,
          ecosystem.usdcMint.publicKey, user.usdcAccount, amount),
      ], user.wallet, "CPI");

      printComparison("S03", "2x USDC deposit (same bank - re-refresh)", ext, cpi);
      results.push({ id: "S03", name: "2x USDC deposit (same bank - re-refresh)", group: "B", a: ext, b: cpi });
    });
  });

  describe("Group C: Multi-bank", () => {
    it("S04: 2-bank deposit", async () => {
      const user = users[0];

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const ext = await measure([
        await batchRefreshReserves([usdcReserve, tokenAReserve], market),
        await refreshUsdcObligation(), await refreshTokenAObligation(),
        await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey, usdc()),
        await rawDeposit(user, tokenABank, user.tokenAAccount, ecosystem.tokenAMint.publicKey, tokenA()),
      ], user.wallet, "External");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const cpi = await measure([
        await cpiDeposit(user, usdcBank,
          ecosystem.usdcMint.publicKey, user.usdcAccount, usdc()),
        await cpiDeposit(user, tokenABank,
          ecosystem.tokenAMint.publicKey, user.tokenAAccount, tokenA()),
      ], user.wallet, "CPI");

      printComparison("S04", "2-bank deposit", ext, cpi);
      results.push({ id: "S04", name: "2-bank deposit", group: "C", a: ext, b: cpi });
    });

    it("S05: 2-bank deposit + withdraw", async () => {
      const user = users[0];

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const ext = await measure([
        await batchRefreshReserves([usdcReserve, tokenAReserve], market),
        await refreshUsdcObligation(), await refreshTokenAObligation(),
        await rawDeposit(user, usdcBank, user.usdcAccount, ecosystem.usdcMint.publicKey, usdc()),
        await rawWithdraw(user, tokenABank, user.tokenAAccount, ecosystem.tokenAMint.publicKey, tokenA()),
      ], user.wallet, "External");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const cpi = await measure([
        await cpiDeposit(user, usdcBank,
          ecosystem.usdcMint.publicKey, user.usdcAccount, usdc()),
        await cpiWithdraw(user, tokenABank,
          user.tokenAAccount, ecosystem.tokenAMint.publicKey, tokenA()),
      ], user.wallet, "CPI");

      printComparison("S05", "2-bank deposit + withdraw", ext, cpi);
      results.push({ id: "S05", name: "2-bank deposit + withdraw", group: "C", a: ext, b: cpi });
    });
  });

  describe("Group D: Liquidation scenarios", () => {
    const mrgnID = () => bankrunProgram.programId;

    before(async () => {
      liqMarket = kaminoAccounts.get(MARKET);
      liqUsdcReserve = kaminoAccounts.get(USDC_RESERVE);

      const result = await genericMultiBankTestSetup(
        1, USER_ACCOUNT_KSCEN, THROWAWAY_GROUP_SEED_KSCEN, LIQ_STARTING_SEED
      );
      liqBanks = result.banks;
      liqThrowawayGroup = result.throwawayGroup;

      const kaminoSeed = new BN(LIQ_STARTING_SEED + 1);
      [liqKaminoUsdcBank] = deriveBankWithSeed(
        mrgnID(), liqThrowawayGroup.publicKey,
        ecosystem.usdcMint.publicKey, kaminoSeed
      );

      let tx = new Transaction().add(
        await makeAddKaminoBankIx(groupAdmin.mrgnBankrunProgram, {
          group: liqThrowawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: liqUsdcReserve,
          kaminoMarket: liqMarket,
          oracle: oracles.usdcOracle.publicKey,
        }, {
          config: defaultKaminoBankConfig(oracles.usdcOracle.publicKey),
          seed: kaminoSeed,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      const [liqVaultAuth] = deriveLiquidityVaultAuthority(mrgnID(), liqKaminoUsdcBank);
      [liqKaminoObligation] = deriveBaseObligation(liqVaultAuth, liqMarket);

      tx = new Transaction().add(
        computeBudget(2_000_000),
        await makeInitObligationIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          bank: liqKaminoUsdcBank,
          signerTokenAccount: groupAdmin.usdcAccount,
          lendingMarket: liqMarket,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          pythOracle: oracles.usdcOracle.publicKey,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      const seedAmount = lstAlpha(5);
      tx = new Transaction().add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: groupAdmin.accounts.get(USER_ACCOUNT_KSCEN),
          bank: liqBanks[0],
          tokenAccount: groupAdmin.lstAlphaAccount,
          amount: seedAmount,
          depositUpToLimit: false,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      const u0 = users[0];
      const u0Acc = u0.accounts.get(USER_ACCOUNT_KSCEN);
      const depositAmount = usdc(1000);
      const borrowAmount = lstAlpha(3);

      tx = new Transaction().add(
        computeBudget(),
        await batchRefreshReserves([liqUsdcReserve], liqMarket),
        await simpleRefreshObligation(klendBankrunProgram, liqMarket, liqKaminoObligation, [liqUsdcReserve]),
        await makeKaminoDepositIx(u0.mrgnBankrunProgram, {
          marginfiAccount: u0Acc, bank: liqKaminoUsdcBank, signerTokenAccount: u0.usdcAccount,
          lendingMarket: liqMarket, reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        }, depositAmount),
      );
      await processBankrunTransaction(ctx, tx, [u0.wallet]);

      tx = new Transaction().add(
        await borrowIx(u0.mrgnBankrunProgram, {
          marginfiAccount: u0Acc,
          bank: liqBanks[0],
          tokenAccount: u0.lstAlphaAccount,
          remaining: composeRemainingAccounts([
            [liqKaminoUsdcBank, oracles.usdcOracle.publicKey, liqUsdcReserve],
            [liqBanks[0], oracles.pythPullLst.publicKey],
          ]),
          amount: borrowAmount,
        }),
      );
      await processBankrunTransaction(ctx, tx, [u0.wallet]);

      let config = blankBankConfigOptRaw();
      config.liabilityWeightInit = bigNumberToWrappedI80F48(2.1);
      config.liabilityWeightMaint = bigNumberToWrappedI80F48(2.0);
      tx = new Transaction().add(
        await configureBank(groupAdmin.mrgnBankrunProgram, {
          bank: liqBanks[0], bankConfigOpt: config,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      const u1 = users[1];
      const u1Acc = u1.accounts.get(USER_ACCOUNT_KSCEN);
      tx = new Transaction().add(
        await depositIx(u1.mrgnBankrunProgram, {
          marginfiAccount: u1Acc,
          bank: liqBanks[0],
          tokenAccount: u1.lstAlphaAccount,
          amount: lstAlpha(),
          depositUpToLimit: false,
        })
      );
      await processBankrunTransaction(ctx, tx, [u1.wallet]);
    });

    it("S06: Deposit + liquidation", async () => {
      const liquidator = users[1];
      const liquidatorAcc = liquidator.accounts.get(USER_ACCOUNT_KSCEN);
      const liquidateeAcc = users[0].accounts.get(USER_ACCOUNT_KSCEN);
      const liqAmount = usdc(5);
      const depAmount = usdc();

      const liqRemaining = [
        oracles.usdcOracle.publicKey,
        liqUsdcReserve,
        oracles.pythPullLst.publicKey,
        ...composeRemainingAccounts([
          [liqBanks[0], oracles.pythPullLst.publicKey],
          [liqKaminoUsdcBank, oracles.usdcOracle.publicKey, liqUsdcReserve],
        ]),
        ...composeRemainingAccounts([
          [liqBanks[0], oracles.pythPullLst.publicKey],
          [liqKaminoUsdcBank, oracles.usdcOracle.publicKey, liqUsdcReserve],
        ]),
      ];

      const makeLiqIx = () => liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: liqKaminoUsdcBank,
        liabilityBankKey: liqBanks[0],
        liquidatorMarginfiAccount: liquidatorAcc,
        liquidateeMarginfiAccount: liquidateeAcc,
        remaining: liqRemaining,
        amount: liqAmount,
        liquidateeAccounts: 5,
        liquidatorAccounts: 5,
      });

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const ext = await measure([
        await batchRefreshReserves([liqUsdcReserve], liqMarket),
        await simpleRefreshObligation(klendBankrunProgram, liqMarket, liqKaminoObligation, [liqUsdcReserve]),
        await makeKaminoDepositIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidatorAcc, bank: liqKaminoUsdcBank,
          signerTokenAccount: liquidator.usdcAccount, lendingMarket: liqMarket,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        }, depAmount),
        await makeLiqIx(),
      ], liquidator.wallet, "External");

      await warpClock();

      const cpi = await measure([
        await makeKaminoDepositWithRefreshIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidatorAcc, bank: liqKaminoUsdcBank,
          signerTokenAccount: liquidator.usdcAccount, lendingMarket: liqMarket,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        }, depAmount),
        await makeLiqIx(),
      ], liquidator.wallet, "CPI");

      printComparison("S06", "Deposit + liquidation", ext, cpi);
      results.push({ id: "S06", name: "Deposit + liquidation", group: "D", a: ext, b: cpi });
    });
  });

  describe("Group E: Single Reserve Refresh", () => {
    it("S07: Single: Simple Refresh Reserve - Batch Refresh Reserves", async () => {
      const user = users[0];

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const simple = await measure([
        await simpleRefreshReserve(klendBankrunProgram, usdcReserve, market, oracles.usdcOracle.publicKey),
        // await refreshUsdcObligation(),
      ], user.wallet, "Simple");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const batch = await measure([
        await batchRefreshReserves([usdcReserve], market),
        // await refreshUsdcObligation(),
      ], user.wallet, "Batch");

      printComparison("S07", "Single: Simple Refresh Reserve - Batch Refresh Reserves", simple, batch);
      results.push({ id: "S07", name: "Single: Simple Refresh Reserve - Batch Refresh Reserves", group: "E", a: simple, b: batch });
    });
  });

  describe("Group F: Init Obligation refresh comparison", () => {
    const mrgnID = () => bankrunProgram.programId;

    before(async () => {
      fMarket = kaminoAccounts.get(MARKET);
      fUsdcReserve = kaminoAccounts.get(USDC_RESERVE);

      const result = await genericMultiBankTestSetup(
        0, USER_ACCOUNT_F, THROWAWAY_GROUP_SEED_F, F_STARTING_SEED
      );
      fThrowawayGroup = result.throwawayGroup;

      const seedA = new BN(F_STARTING_SEED);
      const seedB = new BN(F_STARTING_SEED + 1);

      [fBankA] = deriveBankWithSeed(
        mrgnID(), fThrowawayGroup.publicKey, ecosystem.usdcMint.publicKey, seedA
      );
      [fBankB] = deriveBankWithSeed(
        mrgnID(), fThrowawayGroup.publicKey, ecosystem.usdcMint.publicKey, seedB
      );

      let tx = new Transaction().add(
        await makeAddKaminoBankIx(groupAdmin.mrgnBankrunProgram, {
          group: fThrowawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: fUsdcReserve,
          kaminoMarket: fMarket,
          oracle: oracles.usdcOracle.publicKey,
        }, {
          config: defaultKaminoBankConfig(oracles.usdcOracle.publicKey),
          seed: seedA,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      tx = new Transaction().add(
        await makeAddKaminoBankIx(groupAdmin.mrgnBankrunProgram, {
          group: fThrowawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: fUsdcReserve,
          kaminoMarket: fMarket,
          oracle: oracles.usdcOracle.publicKey,
        }, {
          config: defaultKaminoBankConfig(oracles.usdcOracle.publicKey),
          seed: seedB,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      // Banks C and D: Token A with farms
      fTokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
      const farmState = farmAccounts.get(A_FARM_STATE);
      const seedC = new BN(F_STARTING_SEED + 2);
      const seedD = new BN(F_STARTING_SEED + 3);

      [fBankC] = deriveBankWithSeed(
        mrgnID(), fThrowawayGroup.publicKey, ecosystem.tokenAMint.publicKey, seedC
      );
      [fBankD] = deriveBankWithSeed(
        mrgnID(), fThrowawayGroup.publicKey, ecosystem.tokenAMint.publicKey, seedD
      );

      const [vaultAuthC] = deriveLiquidityVaultAuthority(mrgnID(), fBankC);
      const [obligationC] = deriveBaseObligation(vaultAuthC, fMarket);
      [fUserStateC] = PublicKey.findProgramAddressSync(
        [Buffer.from("user"), farmState.toBuffer(), obligationC.toBuffer()],
        FARMS_PROGRAM_ID,
      );

      const [vaultAuthD] = deriveLiquidityVaultAuthority(mrgnID(), fBankD);
      const [obligationD] = deriveBaseObligation(vaultAuthD, fMarket);
      [fUserStateD] = PublicKey.findProgramAddressSync(
        [Buffer.from("user"), farmState.toBuffer(), obligationD.toBuffer()],
        FARMS_PROGRAM_ID,
      );

      tx = new Transaction().add(
        await makeAddKaminoBankIx(groupAdmin.mrgnBankrunProgram, {
          group: fThrowawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          kaminoReserve: fTokenAReserve,
          kaminoMarket: fMarket,
          oracle: oracles.tokenAOracle.publicKey,
        }, {
          config: defaultKaminoBankConfig(oracles.tokenAOracle.publicKey),
          seed: seedC,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

      tx = new Transaction().add(
        await makeAddKaminoBankIx(groupAdmin.mrgnBankrunProgram, {
          group: fThrowawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          kaminoReserve: fTokenAReserve,
          kaminoMarket: fMarket,
          oracle: oracles.tokenAOracle.publicKey,
        }, {
          config: defaultKaminoBankConfig(oracles.tokenAOracle.publicKey),
          seed: seedD,
        })
      );
      await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);
    });

    it("S08: Init obligation: Simple refresh_reserve vs Batch refresh_reserves_batch", async () => {
      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const standard = await measure([
        await makeInitObligationIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          bank: fBankA,
          signerTokenAccount: groupAdmin.usdcAccount,
          lendingMarket: fMarket,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          pythOracle: oracles.usdcOracle.publicKey,
        }),
      ], groupAdmin.wallet, "Simple");

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const batch = await measure([
        await makeInitObligationBatchRefreshIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          bank: fBankB,
          signerTokenAccount: groupAdmin.usdcAccount,
          lendingMarket: fMarket,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        }),
      ], groupAdmin.wallet, "Batch");

      printComparison("S08", "Init obligation: Simple vs Batch Refresh", standard, batch);
      results.push({ id: "S08", name: "Init obligation: Simple vs Batch Refresh", group: "F", a: standard, b: batch });
    });

    it("S09: Init obligation with farms: Simple refresh_reserve vs Batch refresh_reserves_batch", async () => {
      const farmState = farmAccounts.get(A_FARM_STATE);

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const standard = await measure([
        await makeInitObligationIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          bank: fBankC,
          signerTokenAccount: groupAdmin.tokenAAccount,
          lendingMarket: fMarket,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          pythOracle: oracles.tokenAOracle.publicKey,
          reserveFarmState: farmState,
          obligationFarmUserState: fUserStateC,
        }),
      ], groupAdmin.wallet, "Simple", 2_000_000);

      await warpClock();
      await refreshPullOraclesBankrun(oracles, ctx, banksClient);

      const batch = await measure([
        await makeInitObligationBatchRefreshIx(groupAdmin.mrgnBankrunProgram, {
          feePayer: groupAdmin.wallet.publicKey,
          bank: fBankD,
          signerTokenAccount: groupAdmin.tokenAAccount,
          lendingMarket: fMarket,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          reserveFarmState: farmState,
          obligationFarmUserState: fUserStateD,
        }),
      ], groupAdmin.wallet, "Batch", 2_000_000);

      printComparison("S09", "Init obligation (farms): Simple vs Batch Refresh", standard, batch);
      results.push({ id: "S09", name: "Init obligation (farms): Simple vs Batch Refresh", group: "F", a: standard, b: batch });
    });
  });

  after(() => {
    if (results.length === 0) return;

    const width = 140;
    const separator = "=".repeat(width);
    const divider = "─".repeat(width);

    console.log("\n\n" + separator);
    console.log("KAMINO SCENARIO BENCHMARKS - A vs B");
    console.log(separator);
    console.log("✓ = TX succeeded   ✗ = failed / too large to serialize");
    console.log("Groups A-C use the shared rootHooks Kamino group (deposit/withdraw).");
    console.log("Group D: Liquidation scenarios (throwaway group, Kamino USDC + regular LST).");
    console.log("Group E: Refresh methods comparison.");
    console.log("Group F: Init obligation refresh comparison (Simple refresh_reserve vs Batch refresh_reserves_batch).");
    console.log("");

    const headerRow = `  ${"ID".padEnd(4)} ${"Grp"} ${"A CU".padStart(10)} ${"B CU".padStart(10)} ${"Δ-CU".padStart(10)} ${"Δ-CU%".padStart(7)} ${"A size".padStart(7)} ${"B size".padStart(7)} ${"Δ-size".padStart(5)} ${"A ix count".padStart(6)} ${"B ix count".padStart(6)} ${"Δix count".padStart(4)}  A / B`;
    console.log(headerRow);
    console.log(divider);

    for (const r of results) {
      const { a, b } = r;
      const cuDelta = (a.cuConsumed !== -1 && b.cuConsumed !== -1) ? b.cuConsumed - a.cuConsumed : null;
      const cuPctStr = cuDelta !== null && a.cuConsumed > 0 ? (cuDelta / a.cuConsumed * 100).toFixed(1) + "%" : "N/A";
      const aSizeStr = a.txSizeBytes === -1 ? " >1232B" : `${a.txSizeBytes}B`.padStart(7);
      const bSizeStr = b.txSizeBytes === -1 ? " >1232B" : `${b.txSizeBytes}B`.padStart(7);
      const sizeDeltaStr = a.txSizeBytes !== -1 && b.txSizeBytes !== -1 ? `${b.txSizeBytes - a.txSizeBytes}` : "N/A";
      const ixDelta = b.numInstructions - a.numInstructions;
      const formatDelta = (n: number | null) => n === null ? "       N/A" : ((n >= 0 ? "+" : "") + n.toLocaleString()).padStart(10);

      console.log(
        `  ${r.id.padEnd(4)} ${r.group.padEnd(3)} ` +
        `${a.cuConsumed === -1 ? "       N/A" : a.cuConsumed.toLocaleString().padStart(10)} ` +
        `${b.cuConsumed === -1 ? "       N/A" : b.cuConsumed.toLocaleString().padStart(10)} ` +
        `${formatDelta(cuDelta)} ${cuPctStr.padStart(7)} ` +
        `${aSizeStr} ${bSizeStr} ${sizeDeltaStr.padStart(5)} ` +
        `${a.numInstructions.toString().padStart(6)} ${b.numInstructions.toString().padStart(6)} ` +
        `${((ixDelta >= 0 ? "+" : "") + ixDelta).padStart(4)}  ` +
        `${a.label} / ${b.label}`
      );
    }

    console.log(separator + "\n");
  });
});
