import { BN } from "@coral-xyz/anchor";
import {
  wrappedI80F48toBigNumber,
  bigNumberToWrappedI80F48,
  type WrappedI80F48,
} from "@mrgnlabs/mrgn-common";
import { PublicKey, Transaction } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import { assert } from "chai";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  A_FARM_STATE,
  bankrunContext,
  bankrunProgram,
  banksClient,
  DRIFT_TOKEN_A_PULL_ORACLE,
  DRIFT_TOKEN_A_SPOT_MARKET,
  driftAccounts,
  ecosystem,
  FARMS_PROGRAM_ID,
  farmAccounts,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  users,
} from "./rootHooks";
import {
  addBankWithSeed,
  configureBank,
  configureBankOracle,
  groupConfigure,
} from "./utils/group-instructions";
import { makeDriftDepositIx } from "./utils/drift-instructions";
import { makeKaminoDepositIx } from "./utils/kamino-instructions";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { refreshDriftOracles, TOKEN_A_MARKET_INDEX } from "./utils/drift-utils";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import { refreshJupSimple } from "./utils/juplend/shorthand-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { type JuplendPoolKeys } from "./utils/juplend/types";
import { addJuplendBanksForGroup } from "./utils/multi-limits-juplend-setup";
import { ensureMultiSuiteIntegrationsSetup } from "./utils/multi-limits-setup";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { simpleRefreshObligation, simpleRefreshReserve } from "./utils/kamino-utils";
import { processBankrunTransaction } from "./utils/tools";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  I80F48_ZERO,
  ORACLE_SETUP_PYTH_PUSH,
} from "./utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
} from "./utils/user-instructions";
import { assertI80F48Approx } from "./utils/genericTests";

const SAME_ASSET_DISABLED = 1;
const SAME_ASSET_ENABLED_INIT_LEVERAGE = 10;
const SAME_ASSET_ENABLED_MAINT_LEVERAGE = 100;

const COLLATERAL_UI = 10;
const BORROW_UI_BEFORE_ENABLE = 3.5;
const BORROW_UI_AFTER_ENABLE = 1.5;

const NATIVE_ASSET_WEIGHT_INIT = 0.5;
const NATIVE_ASSET_WEIGHT_MAINT = 0.6;
const NATIVE_LIAB_WEIGHT_INIT = 1.2;
const NATIVE_LIAB_WEIGHT_MAINT = 1.1;

type IntegrationKind = "kamino" | "drift" | "juplend";

type HealthPair = {
  init: BigNumber;
  maint: BigNumber;
};

function fixedSeed(label: string): Buffer {
  const seed = Buffer.alloc(32);
  Buffer.from(label).copy(seed, 0, 0, Math.min(label.length, 32));
  return seed;
}

function uiToNative(ui: number, decimals: number): BN {
  return new BN(new BigNumber(ui).times(new BigNumber(10).pow(decimals)).toFixed(0));
}

function healthFromCache(cache: {
  assetValue: WrappedI80F48;
  liabilityValue: WrappedI80F48;
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
}): HealthPair {
  const init = wrappedI80F48toBigNumber(cache.assetValue).minus(
    wrappedI80F48toBigNumber(cache.liabilityValue),
  );
  const maint = wrappedI80F48toBigNumber(cache.assetValueMaint).minus(
    wrappedI80F48toBigNumber(cache.liabilityValueMaint),
  );

  return { init, maint };
}

async function createNativeAlphaBank(group: PublicKey, seed: BN): Promise<PublicKey> {
  const config = defaultBankConfig();
  config.assetWeightInit = bigNumberToWrappedI80F48(NATIVE_ASSET_WEIGHT_INIT);
  config.assetWeightMaint = bigNumberToWrappedI80F48(NATIVE_ASSET_WEIGHT_MAINT);
  config.liabilityWeightInit = bigNumberToWrappedI80F48(NATIVE_LIAB_WEIGHT_INIT);
  config.liabilityWeightMain = bigNumberToWrappedI80F48(NATIVE_LIAB_WEIGHT_MAINT);
  config.interestRateConfig.protocolOriginationFee = I80F48_ZERO;

  const addIx = addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
    marginfiGroup: group,
    feePayer: groupAdmin.wallet.publicKey,
    bankMint: ecosystem.tokenAMint.publicKey,
    config,
    seed,
  });

  const [bank] = deriveBankWithSeed(bankrunProgram.programId, group, ecosystem.tokenAMint.publicKey, seed);

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(await addIx),
    [groupAdmin.wallet],
  );

  const oracleIx = configureBankOracle(groupAdmin.mrgnBankrunProgram, {
    bank,
    type: ORACLE_SETUP_PYTH_PUSH,
    oracle: oracles.tokenAOracle.publicKey,
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(await oracleIx),
    [groupAdmin.wallet],
  );

  return bank;
}

async function setAssetWeights(bank: PublicKey, initWeight: number, maintWeight: number) {
  const config = blankBankConfigOptRaw();
  config.assetWeightInit = bigNumberToWrappedI80F48(initWeight);
  config.assetWeightMaint = bigNumberToWrappedI80F48(maintWeight);

  const ix = configureBank(groupAdmin.mrgnBankrunProgram, {
    bank,
    bankConfigOpt: config,
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(await ix),
    [groupAdmin.wallet],
  );
}

async function setSameAssetLeverage(group: PublicKey, initLeverage: number, maintLeverage: number) {
  const ix = await groupConfigure(groupAdmin.mrgnBankrunProgram, {
    marginfiGroup: group,
    sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(initLeverage),
    sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(maintLeverage),
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [groupAdmin.wallet],
  );
}

async function refreshScenarioOracles(kind: IntegrationKind, juplendPool: JuplendPoolKeys | null) {
  await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

  if (kind === "drift") {
    await refreshDriftOracles(oracles, driftAccounts, bankrunContext, banksClient);
  }

  if (kind === "juplend") {
    const juplendPrograms = getJuplendPrograms();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(juplendPrograms.lending, {
          pool: juplendPool!,
        }),
      ),
      [groupAdmin.wallet],
    );
  }
}

async function runIntegrationScenario(kind: IntegrationKind, scenarioIndex: number) {
  const userAccountName = `e08_same_asset_${kind}_${scenarioIndex}`;
  const groupSeed = fixedSeed(`e08_${kind}_${scenarioIndex}`);
  const startingSeed = 8_800 + scenarioIndex * 100;

  const { throwawayGroup, kaminoBanks, driftBanks } = await genericMultiBankTestSetup(
    0,
    userAccountName,
    groupSeed,
    startingSeed,
    kind === "kamino" ? 1 : 0,
    kind === "drift" ? 1 : 0,
  );

  let integrationBank: PublicKey;
  let integrationRemainingTail: PublicKey | undefined;
  let juplendPool: JuplendPoolKeys | null = null;

  if (kind === "juplend") {
    const { juplendBanks, pool } = await addJuplendBanksForGroup({
      group: throwawayGroup.publicKey,
      numberOfBanks: 1,
      startingSeed: startingSeed + 50,
    });
    integrationBank = juplendBanks[0];
    juplendPool = pool;
    integrationRemainingTail = pool.lending;
  } else if (kind === "kamino") {
    integrationBank = kaminoBanks[0];
    integrationRemainingTail = kaminoAccounts.get(TOKEN_A_RESERVE)!;
  } else {
    integrationBank = driftBanks[0];
    integrationRemainingTail = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET)!;
  }

  const nativeBank = await createNativeAlphaBank(throwawayGroup.publicKey, new BN(startingSeed + 90));

  await setAssetWeights(integrationBank, NATIVE_ASSET_WEIGHT_INIT, NATIVE_ASSET_WEIGHT_MAINT);
  await setSameAssetLeverage(throwawayGroup.publicKey, SAME_ASSET_DISABLED, SAME_ASSET_DISABLED);

  await refreshScenarioOracles(kind, juplendPool);

  const user = users[0];
  const userAccount = user.accounts.get(userAccountName)!;
  const adminAccount = groupAdmin.accounts.get(userAccountName)!;

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: adminAccount,
        bank: nativeBank,
        tokenAccount: groupAdmin.tokenAAccount,
        amount: uiToNative(200, ecosystem.tokenADecimals),
        depositUpToLimit: false,
      }),
    ),
    [groupAdmin.wallet],
  );

  if (kind === "kamino") {
    const lendingMarket = kaminoAccounts.get(MARKET)!;
    const tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE)!;
    const reserveFarmState = farmAccounts.get(A_FARM_STATE)!;

    const [lendingVaultAuthority] = deriveLiquidityVaultAuthority(bankrunProgram.programId, integrationBank);
    const [obligation] = deriveBaseObligation(lendingVaultAuthority, lendingMarket);
    const [obligationFarmUserState] = PublicKey.findProgramAddressSync(
      [Buffer.from("user"), reserveFarmState.toBuffer(), obligation.toBuffer()],
      FARMS_PROGRAM_ID,
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
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
            bank: integrationBank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket,
            reserve: tokenAReserve,
            reserveFarmState,
            obligationFarmUserState,
          },
          uiToNative(COLLATERAL_UI, ecosystem.tokenADecimals),
        ),
      ),
      [user.wallet],
    );
  } else if (kind === "drift") {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeDriftDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: integrationBank,
            signerTokenAccount: user.tokenAAccount,
            driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE)!,
          },
          uiToNative(COLLATERAL_UI, ecosystem.tokenADecimals),
          TOKEN_A_MARKET_INDEX,
        ),
      ),
      [user.wallet],
    );
  } else {
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeJuplendDepositIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          signerTokenAccount: user.tokenAAccount,
          bank: integrationBank,
          pool: juplendPool!,
          amount: uiToNative(COLLATERAL_UI, ecosystem.tokenADecimals),
        }),
      ),
      [user.wallet],
    );
  }

  const remaining = composeRemainingAccounts([
    [integrationBank, oracles.tokenAOracle.publicKey, integrationRemainingTail!],
    [nativeBank, oracles.tokenAOracle.publicKey],
  ]);

  await refreshScenarioOracles(kind, juplendPool);
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: nativeBank,
        tokenAccount: user.tokenAAccount,
        remaining,
        amount: uiToNative(BORROW_UI_BEFORE_ENABLE, ecosystem.tokenADecimals),
      }),
    ),
    [user.wallet],
  );

  await refreshScenarioOracles(kind, juplendPool);
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining,
      }),
    ),
    [user.wallet],
  );

  const beforeAcc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
  const before = healthFromCache(beforeAcc.healthCache);

  await setSameAssetLeverage(
    throwawayGroup.publicKey,
    SAME_ASSET_ENABLED_INIT_LEVERAGE,
    SAME_ASSET_ENABLED_MAINT_LEVERAGE,
  );

  await refreshScenarioOracles(kind, juplendPool);
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: nativeBank,
        tokenAccount: user.tokenAAccount,
        remaining,
        amount: uiToNative(BORROW_UI_AFTER_ENABLE, ecosystem.tokenADecimals),
      }),
    ),
    [user.wallet],
  );

  await refreshScenarioOracles(kind, juplendPool);
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining,
      }),
    ),
    [user.wallet],
  );

  const afterAcc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
  const after = healthFromCache(afterAcc.healthCache);
  const price = ecosystem.tokenAPrice;
  const totalBorrowUi = BORROW_UI_BEFORE_ENABLE + BORROW_UI_AFTER_ENABLE;
  const sameAssetInitWeight =
    NATIVE_LIAB_WEIGHT_INIT * (1 - 1 / SAME_ASSET_ENABLED_INIT_LEVERAGE);
  const sameAssetMaintWeight =
    NATIVE_LIAB_WEIGHT_MAINT * (1 - 1 / SAME_ASSET_ENABLED_MAINT_LEVERAGE);

  const expectedBeforeAssetInit = COLLATERAL_UI * price * NATIVE_ASSET_WEIGHT_INIT;
  const expectedBeforeLiabInit = BORROW_UI_BEFORE_ENABLE * price * NATIVE_LIAB_WEIGHT_INIT;
  const expectedBeforeAssetMaint = COLLATERAL_UI * price * NATIVE_ASSET_WEIGHT_MAINT;
  const expectedBeforeLiabMaint = BORROW_UI_BEFORE_ENABLE * price * NATIVE_LIAB_WEIGHT_MAINT;

  const expectedAfterAssetInit = COLLATERAL_UI * price * sameAssetInitWeight;
  const expectedAfterLiabInit = totalBorrowUi * price * NATIVE_LIAB_WEIGHT_INIT;
  const expectedAfterAssetMaint = COLLATERAL_UI * price * sameAssetMaintWeight;
  const expectedAfterLiabMaint = totalBorrowUi * price * NATIVE_LIAB_WEIGHT_MAINT;

  const valueTolerance = 8;
  const healthTolerance = 12;

  assertI80F48Approx(beforeAcc.healthCache.assetValue, expectedBeforeAssetInit, valueTolerance);
  assertI80F48Approx(beforeAcc.healthCache.liabilityValue, expectedBeforeLiabInit, valueTolerance);
  assertI80F48Approx(beforeAcc.healthCache.assetValueMaint, expectedBeforeAssetMaint, valueTolerance);
  assertI80F48Approx(beforeAcc.healthCache.liabilityValueMaint, expectedBeforeLiabMaint, valueTolerance);

  assertI80F48Approx(afterAcc.healthCache.assetValue, expectedAfterAssetInit, valueTolerance);
  assertI80F48Approx(afterAcc.healthCache.liabilityValue, expectedAfterLiabInit, valueTolerance);
  assertI80F48Approx(afterAcc.healthCache.assetValueMaint, expectedAfterAssetMaint, valueTolerance);
  assertI80F48Approx(afterAcc.healthCache.liabilityValueMaint, expectedAfterLiabMaint, valueTolerance);

  const expectedBeforeInitHealth = expectedBeforeAssetInit - expectedBeforeLiabInit;
  const expectedBeforeMaintHealth = expectedBeforeAssetMaint - expectedBeforeLiabMaint;
  const expectedAfterInitHealth = expectedAfterAssetInit - expectedAfterLiabInit;
  const expectedAfterMaintHealth = expectedAfterAssetMaint - expectedAfterLiabMaint;

  assert.approximately(before.init.toNumber(), expectedBeforeInitHealth, healthTolerance);
  assert.approximately(before.maint.toNumber(), expectedBeforeMaintHealth, healthTolerance);
  assert.approximately(after.init.toNumber(), expectedAfterInitHealth, healthTolerance);
  assert.approximately(after.maint.toNumber(), expectedAfterMaintHealth, healthTolerance);
}

describe("e08 same-asset emode with integration collateral", () => {
  before(async () => {
    await ensureMultiSuiteIntegrationsSetup();
  });

  it("Kamino Alpha collateral: health improves after enabling same-asset emode", async () => {
    await runIntegrationScenario("kamino", 0);
  });

  it("Drift Alpha collateral: health improves after enabling same-asset emode", async () => {
    await runIntegrationScenario("drift", 1);
  });

  it("Juplend Alpha collateral: health improves after enabling same-asset emode", async () => {
    await runIntegrationScenario("juplend", 2);
  });
});
