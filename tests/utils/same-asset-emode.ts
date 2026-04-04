import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import BigNumber from "bignumber.js";
import { CONF_INTERVAL_MULTIPLE_FLOAT } from "./types";

const ORACLE_PRICE_LOWER_FACTOR = new BigNumber(
  1 - CONF_INTERVAL_MULTIPLE_FLOAT
);
const ORACLE_PRICE_UPPER_FACTOR = new BigNumber(
  1 + CONF_INTERVAL_MULTIPLE_FLOAT
);

const toBigNumber = (
  value: BN | BigNumber | number | string
): BigNumber => {
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
  liabilityOriginationFeeRate?: number;
  gapPosition?: number;
};

export const computeSameAssetBoundaryBorrowNative = ({
  collateralNative,
  collateralDecimals,
  collateralPrice,
  liabilityDecimals,
  liabilityPrice,
  healthyInitLeverage,
  tightenedRequirementLeverage,
  liabilityOriginationFeeRate = 0,
  gapPosition = 0.25,
}: BoundaryBorrowParams) => {
  const collateralUi = toBigNumber(collateralNative).div(
    new BigNumber(10).pow(collateralDecimals)
  );
  const liabilityScale = new BigNumber(10).pow(liabilityDecimals);
  const liabilityWithFeeFactor = new BigNumber(1).plus(
    liabilityOriginationFeeRate
  );
  const healthyInitBoundaryUi = collateralUi
    .times(collateralPrice)
    .times(ORACLE_PRICE_LOWER_FACTOR)
    .times(getSameAssetWeight(healthyInitLeverage))
    .div(new BigNumber(liabilityPrice).times(ORACLE_PRICE_UPPER_FACTOR));
  const tightenedRequirementBoundaryUi = collateralUi
    .times(collateralPrice)
    .times(ORACLE_PRICE_LOWER_FACTOR)
    .times(getSameAssetWeight(tightenedRequirementLeverage))
    .div(new BigNumber(liabilityPrice).times(ORACLE_PRICE_UPPER_FACTOR));
  const boundaryGapUi = healthyInitBoundaryUi.minus(
    tightenedRequirementBoundaryUi
  );
  const effectiveLiabilityUi = tightenedRequirementBoundaryUi.plus(
    boundaryGapUi.times(gapPosition)
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

  assert.isTrue(
    liabilityUi.isGreaterThan(tightenedRequirementBoundaryUi),
    "fee-adjusted liability should stay above the tightened boundary"
  );
  assert.isTrue(
    liabilityUi.isLessThan(healthyInitBoundaryUi),
    "fee-adjusted liability should stay below the healthy init boundary"
  );

  return borrowNative;
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
