import { RiskTierRaw } from "@mrgnlabs/marginfi-client-v2";
import { bigNumberToWrappedI80F48, WrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { PublicKey } from "@solana/web3.js";

import BN from "bn.js";

export const I80F48_ZERO = bigNumberToWrappedI80F48(0);
export const I80F48_ONE = bigNumberToWrappedI80F48(1);
/** Equivalent in value to u64::MAX in Rust */
export const u64MAX_BN = new BN("18446744073709551615");
export const u32_MAX: number = 4294967295;
/** Token account size in bytes */
export const TOKEN_ACCOUNT_SIZE = 165;
export const SINGLE_POOL_PROGRAM_ID = new PublicKey(
  "SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE"
);
export const KLEND_PROGRAM_ID = new PublicKey(
  "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
);
export const FARMS_PROGRAM_ID = new PublicKey(
  "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
);
export const DRIFT_PROGRAM_ID = new PublicKey(
  "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
);
/**
 * Drift Oracle Receiver Program ID
 *
 * IMPORTANT: This is the REQUIRED owner for Drift Pyth Pull oracle accounts.
 * When creating oracle accounts for Drift spot markets with OracleSource::PythPull,
 * the oracle account MUST be owned by this program ID, NOT the Mocks program.
 *
 * See: drift-protocol/protocol-v2 oracle validation (EXTERNAL_ORACLE_PROGRAM_IDS)
 */
export const DRIFT_ORACLE_RECEIVER_PROGRAM_ID = new PublicKey(
  "G6EoTTTgpkNBtVXo96EQp2m6uwwVh2Kt6YidjkmQqoha"
);
export const SOLEND_PROGRAM_ID = new PublicKey(
  "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
);

export const EMISSIONS_FLAG_NONE = 0;
export const EMISSIONS_FLAG_BORROW_ACTIVE = 1;
export const EMISSIONS_FLAG_LENDING_ACTIVE = 2;
export const PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG = 4;
export const FREEZE_SETTINGS = 8;
export const CLOSE_ENABLED_FLAG = 16;
export const TOKENLESS_REPAYMENTS_ALLOWED = 32;
export const TOKENLESS_REPAYMENTS_COMPLETE = 64;

export const ASSET_TAG_DEFAULT = 0;
export const ASSET_TAG_SOL = 1;
export const ASSET_TAG_STAKED = 2;
export const ASSET_TAG_KAMINO = 3;
export const ASSET_TAG_DRIFT = 4;
export const ASSET_TAG_SOLEND = 5;

export const ORACLE_SETUP_NONE = 0;
export const ORACLE_SETUP_SWITCHBOARD_v2 = 2;
/**
 * Yes, it should technically be called "pull", but this is what we call it on-chain, and
 * consistency is king...
 * */
export const ORACLE_SETUP_PYTH_PUSH = 3;
export const ORACLE_SETUP_SWITCHBOARD_PULL = 4;
export const ORACLE_SETUP_STAKED_WITH_PYTH_PUSH = 5;

export const HEALTH_CACHE_NONE = 0;
export const HEALTH_CACHE_HEALTHY = 1;
export const HEALTH_CACHE_ENGINE_OK = 2;
export const HEALTH_CACHE_ORACLE_OK = 4;

/** For 0.1.3, this is how the cache represents the version */
export const HEALTH_CACHE_PROGRAM_VERSION_0_1_3 = 1;
/** For 0.1.4, this is how the cache represents the version */
export const HEALTH_CACHE_PROGRAM_VERSION_0_1_4 = 2;
/** For 0.1.4, this is how the cache represents the version */
export const HEALTH_CACHE_PROGRAM_VERSION_0_1_5 = 3;
/** Confidence intervals are multiplied by this constant internally */
export const CONF_INTERVAL_MULTIPLE = 2.12;
/** Oracles return values with this confidence for testing purposes */
export const ORACLE_CONF_INTERVAL = 0.01;
export const ONE_WEEK_IN_SECONDS = 7 * 24 * 60 * 60;

// By convention, all tags must be in 13375p34k (kidding, but only sorta)
export const EMODE_STABLE_TAG = 5748; // STAB because 574813 is out of range
export const EMODE_SOL_TAG = 501;
export const EMODE_LST_TAG = 157;

export const PYTH_PULL_MIGRATED = 1;
/** In marginfiAccount.flags, indicates an account is disabled and cannot be used. */
export const ACCOUNT_DISABLED = 1;
/** In marginfiAccount.flags, indicates the admin has frozen the account. */
export const ACCOUNT_FROZEN = 1 << 6;

/** In lamports, charged when transfering to a new account */
export const ACCOUNT_TRANSFER_FEE = 5_000_000;

export const FLAG_PAUSED = 1;
export const PAUSE_DURATION_SECONDS = 30 * 60; // 30 minutes
export const MAX_CONSECUTIVE_PAUSES = 2;
export const MAX_DAILY_PAUSES = 3;
export const DAILY_RESET_INTERVAL = 24 * 60 * 60; // 24 hours

export const INTEREST_CURVE_LEGACY = 0;
export const INTEREST_CURVE_SEVEN_POINT = 1;

/**
 * The default bank config has
 * * all weights are 1
 * * state = operational, risk tier = collateral
 * * 100_000_000_000 deposit/borrow limit
 * * 1_000_000_000_000 total asset value limit
 * * asset tag default (`ASSET_TAG_DEFAULT`)
 * @returns
 */
export const defaultBankConfig = () => {
  let config: BankConfig = {
    assetWeightInit: I80F48_ONE,
    assetWeightMaint: I80F48_ONE,
    liabilityWeightInit: I80F48_ONE,
    liabilityWeightMain: I80F48_ONE,
    depositLimit: new BN(100_000_000_000),
    interestRateConfig: defaultInterestRateConfigRaw(),
    operationalState: {
      operational: undefined,
    },
    borrowLimit: new BN(100_000_000_000),
    riskTier: {
      collateral: undefined,
    },
    configFlags: PYTH_PULL_MIGRATED,
    assetTag: ASSET_TAG_DEFAULT,
    totalAssetValueInitLimit: new BN(1_000_000_000_000),
    oracleMaxAge: 240,
    // Note: banks that use 0 (all created prior to 0.1.4) will fall back to the default (10%)
    oracleMaxConfidence: 0,
  };
  return config;
};

/**
 * The same parameters as `defaultBankConfig`
 * @returns
 */
export const defaultBankConfigOptRaw = () => {
  let bankConfigOpt: BankConfigOptRaw = {
    assetWeightInit: I80F48_ONE,
    assetWeightMaint: I80F48_ONE,
    liabilityWeightInit: I80F48_ONE,
    liabilityWeightMaint: I80F48_ONE,
    depositLimit: new BN(1_000_000_000),
    borrowLimit: new BN(1_000_000_000),
    riskTier: {
      collateral: undefined,
    },
    assetTag: ASSET_TAG_DEFAULT,
    totalAssetValueInitLimit: new BN(100_000_000_000),
    interestRateConfig: defaultInterestRateConfigRaw(),
    operationalState: {
      operational: undefined,
    },
    oracleMaxAge: 240,
    permissionlessBadDebtSettlement: null,
    freezeSettings: null,
    oracleMaxConfidence: 0,
    tokenlessRepaymentsAllowed: false,
  };

  return bankConfigOpt;
};

export const blankBankConfigOptRaw = () => {
  let bankConfigOpt: BankConfigOptRaw = {
    assetWeightInit: null,
    assetWeightMaint: null,
    liabilityWeightInit: null,
    liabilityWeightMaint: null,
    depositLimit: null,
    borrowLimit: null,
    riskTier: null,
    assetTag: null,
    totalAssetValueInitLimit: null,
    interestRateConfig: null,
    operationalState: null,
    oracleMaxConfidence: 0,
    oracleMaxAge: null,
    permissionlessBadDebtSettlement: null,
    freezeSettings: null,
    tokenlessRepaymentsAllowed: null,
  };

  return bankConfigOpt;
};

/**
 * The default interest config has
 * * optimalUtilizationRate (point at .5, .6) = .5
 * * plateauInterestRate (point at .5, .6) = .6
 * * maxInterestRate (hundredUtilRate) = 3
 * * starting rate (zeroUtilRate) = 0
 * * insuranceFeeFixedApr = .01
 * * insuranceIrFee = .02
 * * protocolFixedFeeApr = .03
 * * protocolIrFee = .04
 * * originationFee = .01
 * @returns
 */
export const defaultInterestRateConfigRaw = () => {
  let config: InterestRateConfig1_6 = {
    insuranceFeeFixedApr: bigNumberToWrappedI80F48(0.01),
    insuranceIrFee: bigNumberToWrappedI80F48(0.02),
    protocolFixedFeeApr: bigNumberToWrappedI80F48(0.03),
    protocolIrFee: bigNumberToWrappedI80F48(0.04),
    protocolOriginationFee: bigNumberToWrappedI80F48(0.01),
    zeroUtilRate: 0,
    hundredUtilRate: aprToU32(3),
    points: makeRatePoints([0.5], [0.6]),
    curveType: INTEREST_CURVE_SEVEN_POINT,
  };
  return config;
};

/**
 * Same params as `defaultInterestRateConfigRaw`
 * @returns
 */
export const defaultInterestRateConfigOptRaw = () => {
  let config: InterestRateConfigOpt1_6 = {
    insuranceFeeFixedApr: bigNumberToWrappedI80F48(0.01),
    insuranceIrFee: bigNumberToWrappedI80F48(0.02),
    protocolFixedFeeApr: bigNumberToWrappedI80F48(0.03),
    protocolIrFee: bigNumberToWrappedI80F48(0.04),
    protocolOriginationFee: bigNumberToWrappedI80F48(0.01),
    zeroUtilRate: 0,
    hundredUtilRate: aprToU32(3),
    points: makeRatePoints([0.5], [0.6]),
  };
  return config;
};

export const makeRatePoint = (util: number, apr: number) => {
  const point: RatePoint = {
    util: utilToU32(util),
    rate: aprToU32(apr),
  };
  return point;
};

// TODO validation here could be more robust. E.g. zero < all. hundred > all. no gaps, etc. In
// principle this is low-priority since the program already validates this properly.
export const makeRatePoints = (util: number[], apr: number[]) => {
  // Validate input lengths
  if (util.length > 5 || apr.length > 5) {
    throw new Error("makeRatePoints: maximum of 5 points allowed");
  }
  if (util.length != apr.length) {
    throw new Error("must be one rate per util.");
  }

  // Validate that utils are in ascending order
  for (let i = 1; i < util.length; i++) {
    if (util[i] == 0) {
      continue;
    }
    if (util[i] < util[i - 1]) {
      throw new Error("makeRatePoints: util values must be in ascending order");
    }
    if (apr[i] <= apr[i - 1]) {
      throw new Error("makeRatePoints: apr values must be in ascending order");
    }
  }

  // Pad arrays with zeros to ensure 5 entries
  const paddedUtil = [...util];
  const paddedApr = [...apr];
  while (paddedUtil.length < 5) paddedUtil.push(0);
  while (paddedApr.length < 5) paddedApr.push(0);

  const points: RatePoint[] = [];
  for (let i = 0; i < 5; i++) {
    points.push(makeRatePoint(paddedUtil[i], paddedApr[i]));
  }

  return points;
};

export type RatePoint = {
  util: number;
  rate: number;
};

export const aprToU32 = (apr: number): number => {
  if (apr < 0 || apr > 10) {
    console.error("apr out of range, exp 0-1000% (0-10), will clamp: " + apr);
  }
  const clamped = Math.max(0, Math.min(apr, 10));
  return Math.round((clamped / 10) * u32_MAX);
};
export const utilToU32 = (util: number): number => {
  if (util < 0 || util > 1) {
    console.error("util out of range, exp 0-100% (0-1), will clamp: " + util);
  }
  const clamped = Math.max(0, Math.min(util, 1));
  return Math.round(clamped * u32_MAX);
};

export const defaultStakedInterestSettings = (oracle: PublicKey) => {
  let settings: StakedSettingsConfig = {
    oracle: oracle,
    assetWeightInit: bigNumberToWrappedI80F48(0.8),
    assetWeightMaint: bigNumberToWrappedI80F48(0.9),
    depositLimit: new BN(1_000_000_000_000), // 1000 SOL
    totalAssetValueInitLimit: new BN(150_000_000),
    oracleMaxAge: 60,
    riskTier: {
      collateral: undefined,
    },
  };
  return settings;
};

export type InterestRateConfig1_6 = {
  // Fees
  insuranceFeeFixedApr: WrappedI80F48;
  insuranceIrFee: WrappedI80F48;
  protocolFixedFeeApr: WrappedI80F48;
  protocolIrFee: WrappedI80F48;

  protocolOriginationFee: WrappedI80F48;

  zeroUtilRate: number;
  hundredUtilRate: number;
  points: RatePoint[];
  curveType: number;
};

export type InterestRateConfigOpt1_6 = {
  // Fees
  insuranceFeeFixedApr: WrappedI80F48 | null;
  insuranceIrFee: WrappedI80F48 | null;
  protocolFixedFeeApr: WrappedI80F48 | null;
  protocolIrFee: WrappedI80F48 | null;

  protocolOriginationFee: WrappedI80F48 | null;

  zeroUtilRate: number | null;
  hundredUtilRate: number | null;
  points: RatePoint[] | null;
};

export type OperationalStateRaw =
  | { paused: {} }
  | { operational: {} }
  | { reduceOnly: {} };

export type BankConfig = {
  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;

  liabilityWeightInit: WrappedI80F48;
  liabilityWeightMain: WrappedI80F48;

  depositLimit: BN;
  interestRateConfig: InterestRateConfig1_6;

  /** Paused = 0, Operational = 1, ReduceOnly = 2 */
  operationalState: OperationalStateRaw;

  borrowLimit: BN;
  /** Collateral = 0, Isolated = 1 */
  riskTier: RiskTierRaw;
  assetTag: number;
  configFlags: number;
  totalAssetValueInitLimit: BN;
  oracleMaxAge: number;
  /** A u32, e.g. for 100% pass u32::MAX */
  oracleMaxConfidence: number;
};

export type BankConfigOptRaw = {
  assetWeightInit: WrappedI80F48 | null;
  assetWeightMaint: WrappedI80F48 | null;

  liabilityWeightInit: WrappedI80F48 | null;
  liabilityWeightMaint: WrappedI80F48 | null;

  depositLimit: BN | null;
  borrowLimit: BN | null;
  riskTier: { collateral: {} } | { isolated: {} } | null;
  assetTag: number | null;
  totalAssetValueInitLimit: BN | null;

  interestRateConfig: InterestRateConfigOpt1_6 | null;
  operationalState:
    | { paused: {} }
    | { operational: {} }
    | { reduceOnly: {} }
    | null;

  oracleMaxConfidence: number | null;
  oracleMaxAge: number | null;
  permissionlessBadDebtSettlement: boolean | null;
  freezeSettings: boolean | null;
  tokenlessRepaymentsAllowed: boolean | null;
};

export type StakedSettingsConfig = {
  oracle: PublicKey;

  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;

  depositLimit: BN;
  totalAssetValueInitLimit: BN;

  oracleMaxAge: number;
  /** Collateral = 0, Isolated = 1 */
  riskTier: RiskTierRaw;
};

export interface StakedSettingsEdit {
  oracle: PublicKey | null;

  assetWeightInit: WrappedI80F48 | null;
  assetWeightMaint: WrappedI80F48 | null;

  depositLimit: BN | null;
  totalAssetValueInitLimit: BN | null;

  oracleMaxAge: number | null;
  riskTier: { collateral: {} } | { isolated: {} } | null;
}

export const MAX_EMODE_ENTRIES = 10;
export const EMODE_APPLIES_TO_ISOLATED = 1;

export type EmodeEntry = {
  collateralBankEmodeTag: number;
  flags: number;
  pad0: number[];
  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;
};

export function newEmodeEntry(
  collateralBankEmodeTag: number,
  flags: number,
  assetWeightInit: WrappedI80F48,
  assetWeightMaint: WrappedI80F48
): EmodeEntry {
  return {
    collateralBankEmodeTag,
    flags,
    pad0: [0, 0, 0, 0, 0],
    assetWeightInit,
    assetWeightMaint,
  };
}
