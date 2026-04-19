import { BN, Program } from "@coral-xyz/anchor";
import { assert } from "chai";
import BigNumber from "bignumber.js";
import { bigNumberToWrappedI80F48, WrappedI80F48, wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { CONF_INTERVAL_MULTIPLE_FLOAT } from "./types";
import { PublicKey } from "@solana/web3.js";
import { ProgramTestContext } from "solana-bankrun";
import { Marginfi } from "target/types/marginfi";
import { BanksClient } from "solana-bankrun";

const ORACLE_PRICE_LOWER_FACTOR = new BigNumber(
  1 - CONF_INTERVAL_MULTIPLE_FLOAT
);
const ORACLE_PRICE_UPPER_FACTOR = new BigNumber(
  1 + CONF_INTERVAL_MULTIPLE_FLOAT
);

const toBigNumber = (value: BN | BigNumber | number | string): BigNumber => {
  if (BigNumber.isBigNumber(value)) {
    return value;
  }

  if (value instanceof BN) {
    return new BigNumber(value.toString());
  }

  return new BigNumber(value.toString());
};

const getSameAssetWeight = (leverage: number) =>
  new BigNumber(leverage - 1).div(leverage);

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
  const collateralUi = toBigNumber(collateralNative).div(
    new BigNumber(10).pow(collateralDecimals)
  );
  const haircutFactor = haircut
    ? new BigNumber(haircut.numerator).div(haircut.denominator)
    : new BigNumber(1);
  const requirementCollateralUi = collateralUi.times(haircutFactor);
  const liabilityScale = new BigNumber(10).pow(liabilityDecimals);
  const liabilityWithFeeFactor = new BigNumber(1).plus(
    liabilityOriginationFeeRate
  );
  const liabilityPriceWithConfidence = new BigNumber(liabilityPrice).times(
    ORACLE_PRICE_UPPER_FACTOR
  );
  const effectiveGapPosition = gapPosition ?? (haircut ? 0.5 : 0.25);
  const healthyInitBoundaryUi = collateralUi
    .times(collateralPrice)
    .times(ORACLE_PRICE_LOWER_FACTOR)
    .times(getSameAssetWeight(healthyInitLeverage))
    .div(liabilityPriceWithConfidence);
  const tightenedRequirementBoundaryUi = requirementCollateralUi
    .times(collateralPrice)
    .times(ORACLE_PRICE_LOWER_FACTOR)
    .times(getSameAssetWeight(tightenedRequirementLeverage))
    .div(liabilityPriceWithConfidence);
  const boundaryGapUi = healthyInitBoundaryUi.minus(
    tightenedRequirementBoundaryUi
  );
  const effectiveLiabilityUi = tightenedRequirementBoundaryUi.plus(
    boundaryGapUi.times(effectiveGapPosition)
  );
  const borrowNative = new BN(
    effectiveLiabilityUi
      .div(liabilityWithFeeFactor)
      .times(liabilityScale)
      .integerValue(BigNumber.ROUND_FLOOR)
      .toFixed(0)
  );
  const borrowUi = new BigNumber(borrowNative.toString()).div(liabilityScale);
  const liabilityUi = borrowUi.times(liabilityWithFeeFactor);
  const requirementLabel = haircut ? "post-haircut maintenance" : "tightened";

  assert.isTrue(
    liabilityUi.isGreaterThan(tightenedRequirementBoundaryUi),
    `fee-adjusted liability should stay above the ${requirementLabel} boundary`
  );
  assert.isTrue(
    liabilityUi.isLessThan(healthyInitBoundaryUi),
    "fee-adjusted liability should stay below the healthy init boundary"
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
    healthCache.assetValueEquity
  );
  const assetValueMaint = wrappedI80F48toBigNumber(healthCache.assetValueMaint);
  const liabilityValueEquity = wrappedI80F48toBigNumber(
    healthCache.liabilityValueEquity
  );
  const liabilityValueMaint = wrappedI80F48toBigNumber(
    healthCache.liabilityValueMaint
  );
  const minBuffer = originalAssetValueEquity.times(0.005); // 50bps
  const assetBuffer = assetValueEquity.minus(assetValueMaint);
  const equityHealth = assetValueEquity.minus(liabilityValueEquity);
  const maintHealth = assetValueMaint.minus(liabilityValueMaint);

  assert.isTrue(
    assetBuffer.gte(minBuffer),
    `${label}: equity-to-maint asset buffer ${assetBuffer.toFixed()} should be at least 50bp of original equity assets ${minBuffer.toFixed()}`
  );
  assert.isTrue(
    equityHealth.gt(0),
    `${label}: account should remain equity-solvent after the haircut`
  );
  if (requireMaintenanceUnderwater) {
    assert.isTrue(
      maintHealth.lt(0),
      `${label}: account should be maintenance-underwater after the haircut`
    );
  } else {
    assert.isTrue(
      maintHealth.gt(0),
      `${label}: account should remain maintenance-healthy before the haircut`
    );
  }

  return {
    assetBuffer,
    equityHealth,
    maintHealth,
  };
};


export const setAssetShareValueHaircut = async (
  bankrunProgram:Program<Marginfi>,
  banksClient: BanksClient,
  bankrunContext:ProgramTestContext,
  bank: PublicKey,
  numerator: number,
  denominator: number
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
      ASSET_SHARE_VALUE_OFFSET + I80F48_BYTES
    )
  );
  const updatedAssetShareValue = bigNumberToWrappedI80F48(
    wrappedI80F48toBigNumber(bankAccount.assetShareValue)
      .times(numerator)
      .div(denominator)
  );
  Buffer.from(updatedAssetShareValue.value).copy(
    originalData,
    ASSET_SHARE_VALUE_OFFSET
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
  const sourceUi = toBigNumber(sourceBorrowNative).div(
    new BigNumber(10).pow(sourceDecimals)
  );
  const sourceLiabilityValue = sourceUi
    .times(new BigNumber(1).plus(sourceOriginationFeeRate))
    .times(sourcePrice);
  const targetUi = sourceLiabilityValue.div(
    new BigNumber(targetPrice).times(
      new BigNumber(1).plus(targetOriginationFeeRate)
    )
  );

  return new BN(
    targetUi
      .times(new BigNumber(10).pow(targetDecimals))
      .integerValue(BigNumber.ROUND_FLOOR)
      .toFixed(0)
  );
};
