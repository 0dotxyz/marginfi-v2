import { BN, Program } from "@coral-xyz/anchor";
import { assert } from "chai";
import type BigNumber from "bignumber.js";
import Decimal from "decimal.js";
import {
  bigNumberToWrappedI80F48,
  WrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { CONF_INTERVAL_MULTIPLE_FLOAT } from "./types";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "target/types/marginfi";
import { bigIntToBnSafe, bnToDecimalStringSafe } from "./bn-utils";
import {
  initSameAssetEmodeRegistry,
  setBankSameAssetEmodeEligibility,
} from "./group-instructions";
import { deriveSameAssetEmodeRegistry } from "./pdas";
import { processBankrunTransaction } from "./tools";
import { ProgramTestContext } from "./litesvm";

const isBnLike = (value: unknown): value is BN =>
  BN.isBN(value) ||
  (!!value &&
    typeof value === "object" &&
    "abs" in value &&
    "toArray" in value &&
    "isNeg" in value);

const toDecimal = (value: BN | BigNumber | Decimal | number | string): Decimal => {
  if (isBnLike(value)) {
    return new Decimal(bnToDecimalStringSafe(value));
  }

  return new Decimal(value.toString());
};

const toSafeNumber = (
  value: BN | BigNumber | Decimal | number | string,
  label: string,
) => {
  const out = Number(
    isBnLike(value) ? bnToDecimalStringSafe(value) : value.toString(),
  );
  if (!Number.isFinite(out)) {
    throw new Error(`Invalid ${label}: ${value.toString()}`);
  }
  if (Math.abs(out) > Number.MAX_SAFE_INTEGER) {
    throw new Error(`Unsafe ${label}: ${value.toString()}`);
  }
  return out;
};

export const decimalScale = (decimals: number) => {
  const normalizedDecimals = Number(decimals);
  if (
    !Number.isSafeInteger(normalizedDecimals) ||
    normalizedDecimals < 0 ||
    normalizedDecimals > 18
  ) {
    throw new Error(`Invalid token decimals: ${decimals}`);
  }

  return new Decimal(`1e${normalizedDecimals}`);
};

export type HealthCacheSnapshot = {
  assetValue: WrappedI80F48;
  liabilityValue: WrappedI80F48;
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
};

export const getNetHealth = (cache: HealthCacheSnapshot) => {
  const init = wrappedI80F48toBigNumber(cache.assetValue).minus(
    wrappedI80F48toBigNumber(cache.liabilityValue),
  );
  const maint = wrappedI80F48toBigNumber(cache.assetValueMaint).minus(
    wrappedI80F48toBigNumber(cache.liabilityValueMaint),
  );
  return { init, maint };
};

export const enableSameAssetEmodeForBanks = async ({
  program,
  bankrunContext,
  group,
  signer,
  banks,
}: {
  program: Program<Marginfi>;
  bankrunContext: ProgramTestContext;
  group: PublicKey;
  signer: Keypair;
  banks: PublicKey[];
}) => {
  const [sameAssetEmodeRegistry] = deriveSameAssetEmodeRegistry(
    program.programId,
    group,
  );
  const registryAccount = await bankrunContext.banksClient.getAccount(
    sameAssetEmodeRegistry,
  );
  if (!registryAccount) {
    const initTx = new Transaction().add(
      await initSameAssetEmodeRegistry(program, {
        group,
        signer: signer.publicKey,
      }),
    );
    await processBankrunTransaction(bankrunContext, initTx, [signer]);
  }

  const tx = new Transaction();
  for (const bank of banks) {
    tx.add(
      await setBankSameAssetEmodeEligibility(program, {
        // group,
        signer: signer.publicKey,
        bank,
        enabled: true,
      }),
    );
  }
  await processBankrunTransaction(bankrunContext, tx, [signer]);
};

type BoundaryBorrowParams = {
  collateralNative: BN | BigNumber | number | string;
  collateralDecimals: number;
  collateralPrice: number;
  liabilityDecimals: number;
  liabilityPrice: number;
  healthyInitLeverage: number;
  tightenedRequirementLeverage: number;
  haircut?: {
    numerator: number;
    denominator: number;
  };
  liabilityOriginationFeeRate?: number;
  gapPosition?: number;
};

// Size a borrow between a healthy init boundary and a stricter requirement boundary.
// If `haircut` is present, only the requirement-side collateral is scaled down. That
// models opening the position before a bad-debt share-value drop, then checking the
// post-haircut maintenance boundary while preserving the pre-haircut init boundary.
export const computeSameAssetBoundaryBorrowNative = ({
  collateralNative,
  collateralDecimals,
  collateralPrice,
  liabilityDecimals,
  liabilityPrice,
  healthyInitLeverage,
  tightenedRequirementLeverage,
  haircut,
  liabilityOriginationFeeRate = 0,
  gapPosition,
}: BoundaryBorrowParams) => {
  const collateralUi =
    toSafeNumber(collateralNative, "collateral native") /
    10 ** collateralDecimals;
  const haircutFactor = haircut
    ? haircut.numerator / haircut.denominator
    : 1;
  const requirementCollateralUi = collateralUi * haircutFactor;
  const liabilityScale = 10 ** liabilityDecimals;
  const liabilityWithFeeFactor = 1 + liabilityOriginationFeeRate;
  const liabilityPriceWithConfidence =
    liabilityPrice * (1 + CONF_INTERVAL_MULTIPLE_FLOAT);
  const effectiveGapPosition = gapPosition ?? (haircut ? 0.5 : 0.25);
  const healthyInitBoundaryUi =
    (collateralUi *
      collateralPrice *
      (1 - CONF_INTERVAL_MULTIPLE_FLOAT) *
      ((healthyInitLeverage - 1) / healthyInitLeverage)) /
    liabilityPriceWithConfidence;
  const tightenedRequirementBoundaryUi =
    (requirementCollateralUi *
      collateralPrice *
      (1 - CONF_INTERVAL_MULTIPLE_FLOAT) *
      ((tightenedRequirementLeverage - 1) / tightenedRequirementLeverage)) /
    liabilityPriceWithConfidence;
  const boundaryGapUi = healthyInitBoundaryUi - tightenedRequirementBoundaryUi;
  const effectiveLiabilityUi =
    tightenedRequirementBoundaryUi + boundaryGapUi * effectiveGapPosition;
  const borrowNativeNumber = Math.floor(
    (effectiveLiabilityUi / liabilityWithFeeFactor) * liabilityScale,
  );
  if (!Number.isSafeInteger(borrowNativeNumber) || borrowNativeNumber < 0) {
    throw new Error(`Unsafe borrow native: ${borrowNativeNumber}`);
  }
  const borrowNative = bigIntToBnSafe(BigInt(borrowNativeNumber)) as BN;
  const borrowUi = borrowNativeNumber / liabilityScale;
  const liabilityUi = borrowUi * liabilityWithFeeFactor;
  const requirementLabel = haircut ? "post-haircut maintenance" : "tightened";

  assert.isTrue(
    liabilityUi > tightenedRequirementBoundaryUi,
    `fee-adjusted liability ${liabilityUi} should stay above the ${requirementLabel} boundary ${tightenedRequirementBoundaryUi}`,
  );
  assert.isTrue(
    liabilityUi < healthyInitBoundaryUi,
    `fee-adjusted liability ${liabilityUi} should stay below the healthy init boundary ${healthyInitBoundaryUi}`,
  );

  return borrowNative;
};

type HealthCacheWithEquity = {
  assetValueMaint: WrappedI80F48;
  liabilityValueMaint: WrappedI80F48;
  assetValueEquity: WrappedI80F48;
  liabilityValueEquity: WrappedI80F48;
};

export const assertSameAssetBadDebtSurvivability = ({
  healthCache,
  originalAssetValueEquity,
  label,
  requireMaintenanceUnderwater = true,
}: {
  healthCache: HealthCacheWithEquity;
  originalAssetValueEquity: BigNumber;
  label: string;
  requireMaintenanceUnderwater?: boolean;
}) => {
  const assetValueEquity = wrappedI80F48toBigNumber(
    healthCache.assetValueEquity,
  );
  const assetValueMaint = wrappedI80F48toBigNumber(healthCache.assetValueMaint);
  const liabilityValueEquity = wrappedI80F48toBigNumber(
    healthCache.liabilityValueEquity,
  );
  const liabilityValueMaint = wrappedI80F48toBigNumber(
    healthCache.liabilityValueMaint,
  );
  const minBuffer = originalAssetValueEquity.times(0.005); // 50bps
  const assetBuffer = assetValueEquity.minus(assetValueMaint);
  const equityHealth = assetValueEquity.minus(liabilityValueEquity);
  const maintHealth = assetValueMaint.minus(liabilityValueMaint);

  assert.isTrue(
    assetBuffer.gte(minBuffer),
    `${label}: equity-to-maint asset buffer ${assetBuffer.toFixed()} should be at least 50bp of original equity assets ${minBuffer.toFixed()}`,
  );
  assert.isTrue(
    equityHealth.gt(0),
    `${label}: account should remain equity-solvent after the haircut`,
  );
  if (requireMaintenanceUnderwater) {
    assert.isTrue(
      maintHealth.lt(0),
      `${label}: account should be maintenance-underwater after the haircut`,
    );
  } else {
    assert.isTrue(
      maintHealth.gt(0),
      `${label}: account should remain maintenance-healthy before the haircut`,
    );
  }

  return {
    assetBuffer,
    equityHealth,
    maintHealth,
  };
};

export const setAssetShareValueHaircut = async (
  bankrunProgram: Program<Marginfi>,
  banksClient: BanksClient,
  bankrunContext: ProgramTestContext,
  bank: PublicKey,
  numerator: number,
  denominator: number,
) => {
  const ASSET_SHARE_VALUE_OFFSET = 80;
  const I80F48_BYTES = 16;
  const bankAccount = await bankrunProgram.account.bank.fetch(bank);
  const existingAccount = await banksClient.getAccount(bank);
  if (!existingAccount) {
    throw new Error(`Bank ${bank.toString()} not found in bankrun`);
  }
  const originalData = Buffer.from(existingAccount.data);
  const originalAssetShareValueBytes = Buffer.from(
    originalData.subarray(
      ASSET_SHARE_VALUE_OFFSET,
      ASSET_SHARE_VALUE_OFFSET + I80F48_BYTES,
    ),
  );
  const updatedAssetShareValue = bigNumberToWrappedI80F48(
    new Decimal(wrappedI80F48toBigNumber(bankAccount.assetShareValue).toString())
      .times(numerator)
      .div(denominator)
      .toString(),
  );
  Buffer.from(updatedAssetShareValue.value).copy(
    originalData,
    ASSET_SHARE_VALUE_OFFSET,
  );
  bankrunContext.setAccount(bank, {
    ...existingAccount,
    data: originalData,
  });

  return async () => {
    const currentAccount = await banksClient.getAccount(bank);
    if (!currentAccount) {
      throw new Error(`Bank ${bank.toString()} not found in bankrun`);
    }
    const currentData = Buffer.from(currentAccount.data);
    originalAssetShareValueBytes.copy(currentData, ASSET_SHARE_VALUE_OFFSET);
    bankrunContext.setAccount(bank, {
      ...currentAccount,
      data: currentData,
    });
  };
};

export const warpToNextBankrunSlot = async (
  bankrunContext: ProgramTestContext,
) => {
  const clock = await bankrunContext.banksClient.getClock();
  bankrunContext.warpToSlot(clock.slot + BigInt(1));
};

type SameValueBorrowParams = {
  sourceBorrowNative: BN | BigNumber | number | string;
  sourceDecimals: number;
  sourcePrice: number;
  targetDecimals: number;
  targetPrice: number;
  sourceOriginationFeeRate?: number;
  targetOriginationFeeRate?: number;
};

export const computeSameValueBorrowNative = ({
  sourceBorrowNative,
  sourceDecimals,
  sourcePrice,
  targetDecimals,
  targetPrice,
  sourceOriginationFeeRate = 0,
  targetOriginationFeeRate = 0,
}: SameValueBorrowParams) => {
  const sourceUi =
    toSafeNumber(sourceBorrowNative, "source borrow native") /
    10 ** sourceDecimals;
  const sourceLiabilityValue =
    sourceUi * (1 + sourceOriginationFeeRate) * sourcePrice;
  const targetUi =
    sourceLiabilityValue / (targetPrice * (1 + targetOriginationFeeRate));
  const targetNative = Math.floor(targetUi * 10 ** targetDecimals);
  if (!Number.isSafeInteger(targetNative) || targetNative < 0) {
    throw new Error(`Unsafe same-value borrow native: ${targetNative}`);
  }

  return bigIntToBnSafe(BigInt(targetNative)) as BN;
};
