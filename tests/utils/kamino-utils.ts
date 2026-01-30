import { BN, Program } from "@coral-xyz/anchor";
import { AccountMeta, PublicKey } from "@solana/web3.js";
import { KaminoLending } from "../fixtures/kamino_lending";
import { I80F48_ONE, PYTH_PULL_MIGRATED, OperationalStateRaw } from "./types";
import { WrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { RiskTierRaw } from "@mrgnlabs/marginfi-client-v2";
import { Reserve } from "@kamino-finance/klend-sdk";
import Decimal from "decimal.js";
import { Fraction } from "@kamino-finance/klend-sdk";
import BigNumber from "bignumber.js";

export const LENDING_MARKET_SIZE = 4656;
export const RESERVE_SIZE = 8616;
export const OBLIGATION_SIZE = 3336;

// Oracle setups specific to Kamino
export const ORACLE_SETUP_KAMINO_PYTH_PUSH = 6;
export const ORACLE_SETUP_KAMINO_SWITCHBOARD_PULL = 7;

const INITIAL_COLLATERAL_RATIO = 1;
export const INITIAL_COLLATERAL_RATE = new Decimal(INITIAL_COLLATERAL_RATIO);

const I68F60_TOTAL_BYTES = 16;
const I68F60_FRACTIONAL_BITS = 60;
const I68F60_DIVISOR = new Decimal(2).pow(I68F60_FRACTIONAL_BITS);

export const COLLATERAL_MINT_DECIMALS = 6;

export type WrappedI68F60 = { value: number[] };

/**
 * Refresh a generic Kamino reserve with a legacy Pyth oracle
 * @param program
 * @param reserve
 * @param market
 * @param oracle
 * @returns
 */
export const simpleRefreshReserve = (
  program: Program<KaminoLending>,
  reserve: PublicKey,
  market: PublicKey,
  oracle: PublicKey
) => {
  const ix = program.methods
    .refreshReserve()
    .accounts({
      reserve: reserve,
      lendingMarket: market,
      pythOracle: oracle,
      switchboardPriceOracle: null,
      switchboardTwapOracle: null,
      scopePrices: null,
    })
    .instruction();

  return ix;
};

/**
 * Refresh a generic Kamino obligation
 * @param program
 * @param market
 * @param obligation
 * @param remaining - pack the reserves used in this obligation, in the order they appear, starting
 * with lending reserves. For example, a user lending USDC at index 0, SOL at index 1, borrowing
 * BONK at index 0, pass [USDC, SOL, BONK] reserves
 * @returns
 */
export const simpleRefreshObligation = (
  program: Program<KaminoLending>,
  market: PublicKey,
  obligation: PublicKey,
  remaining: PublicKey[] = []
) => {
  const accMeta: AccountMeta[] = remaining.map((pubkey) => ({
    pubkey,
    isSigner: false,
    isWritable: false,
  }));

  const ix = program.methods
    .refreshObligation()
    .accounts({
      lendingMarket: market,
      obligation: obligation,
    })
    .remainingAccounts(accMeta)
    .instruction();

  return ix;
};

// TODO remove when package updates
/** `OracleSetupRaw` with `kaminoPythPush` and `kaminoSwitchboardPull` */
export type OracleSetupRawWithKamino =
  | { none: {} }
  | { pythLegacy: {} }
  | { switchboardV2: {} }
  | { pythPushOracle: {} }
  | { switchboardPull: {} }
  | { stakedWithPythPush: {} }
  | { kaminoPythPush: {} }
  | { kaminoSwitchboardPull: {} };

/**
 * Kamino bank configuration type for direct integration
 */
export type KaminoConfigCompact = {
  oracle: PublicKey;
  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;
  depositLimit: BN;
  oracleSetup: OracleSetupRawWithKamino;
  /** Paused = 0, Operational = 1, ReduceOnly = 2 */
  operationalState: OperationalStateRaw;
  /** Collateral = 0, Isolated = 1 */
  riskTier: RiskTierRaw;
  configFlags: number;
  totalAssetValueInitLimit: BN;
  oracleMaxAge: number;
  /** A u32, e.g. for 100% pass u32::MAX */
  oracleMaxConfidence: number;
};

/**
 * The default Kamino bank has:
 * * all weights are 1 (Note: borrow weights are not used, cannot borrow Kamino assets)
 * * uses Kamino Pyth Push oracles, max age 100
 * * operational state (can be paused for initial setup)
 * * collateral risk tier (not isolated)
 * * config flags set for Pyth push migration
 * * 100_000_000_000 deposit limit
 * * 1_000_000_000_000 total asset value limit
 * * oracle max confidence 0 (uses default 10%)
 * * asset tag kamino (`ASSET_TAG_KAMINO`)
 * @param oracle
 * @returns
 */
export const defaultKaminoBankConfig = (
  oracle: PublicKey
): KaminoConfigCompact => {
  let config: KaminoConfigCompact = {
    oracle: oracle,
    assetWeightInit: I80F48_ONE,
    assetWeightMaint: I80F48_ONE,
    depositLimit: new BN(10000000000000),
    oracleSetup: {
      kaminoPythPush: {},
    },
    operationalState: { operational: {} },
    riskTier: { collateral: {} },
    configFlags: PYTH_PULL_MIGRATED,
    totalAssetValueInitLimit: new BN(1000000000000),
    oracleMaxAge: 100,
    oracleMaxConfidence: 0, // Use default 10%
  };
  return config;
};

/**
 * @returns the total borrowed amount of the reserve in lamports
 */
export function getBorrowedAmount(state: Reserve): Decimal {
  // borrowedAmountSf is a scaled-fraction (u128)
  return new Fraction(state.liquidity.borrowedAmountSf).toDecimal();
}

/**
 * @returns the available liquidity amount of the reserve in lamports
 */
export function getLiquidityAvailableAmount(state: Reserve): Decimal {
  return new Decimal(state.liquidity.availableAmount.toString());
}

/**
 * @returns the total protocol fees accrued (in lamports) since last refresh
 */
export function getAccumulatedProtocolFees(state: Reserve): Decimal {
  return new Decimal(state.liquidity.accumulatedProtocolFeesSf.toString());
}

/**
 * @returns the total referrer fees accrued (in lamports) since last refresh
 */
export function getAccumulatedReferrerFees(state: Reserve): Decimal {
  return new Decimal(state.liquidity.accumulatedReferrerFeesSf.toString());
}

/**
 * @returns the total pending referrer fees (in lamports) since last refresh
 */
export function getPendingReferrerFees(state: Reserve): Decimal {
  return new Decimal(state.liquidity.pendingReferrerFeesSf.toString());
}

/**
 * Use getEstimatedTotalSupply() for the most accurate (post-refresh) value.
 * @param state
 * @returns the stale total liquidity supply of the reserve from the last on-chain refresh
 */
export function getTotalSupply(state: Reserve): Decimal {
  return getLiquidityAvailableAmount(state)
    .add(getBorrowedAmount(state))
    .sub(getAccumulatedProtocolFees(state))
    .sub(getAccumulatedReferrerFees(state))
    .sub(getPendingReferrerFees(state));
}

/**
 * For the given reserve, get the liquidity and collateral supply with no decimals, i.e. as a float
 * @param state
 * @returns
 */
export function scaledSupplies(state: Reserve): [Decimal, Decimal] {
  const liqMintDecimals = new Decimal(state.liquidity.mintDecimals.toString());
  const totalSupplyLamports = getTotalSupply(state);
  const mintTotalSupplyLam = new Decimal(
    state.collateral.mintTotalSupply.toString()
  );

  const liqScale = new Decimal(10).pow(liqMintDecimals);
  const collScale = new Decimal(10).pow(liqMintDecimals);

  const totalSupply = totalSupplyLamports.div(liqScale);
  const totalCollateral = mintTotalSupplyLam.div(collScale);

  return [totalSupply, totalCollateral];
}

/**
 * Given a value that is currently using `fromDec` decimals, convert into `toDec` decimals
 * @param n
 * @param fromDec
 * @param toDec
 * @returns
 */
export function scaleDecimals(
  n: Decimal,
  fromDec: number,
  toDec: number
): Decimal {
  const scaleFrom = new Decimal(10).pow(fromDec);
  const scaleTo = new Decimal(10).pow(toDec);
  const amt = n.mul(scaleTo).div(scaleFrom);
  return amt;
}

/**
 * For the given reserve, the exchange rate of supply:collateral tokens, i.e. if you have some given
 * amount of liquidity/supply tokens and want to know how many collateral tokens you would get,
 * already accounting for any decimal conversions.
 *
 * E.g. if 1000 supply tokens would give you 100 collateral tokens, returns 10.
 * @param state
 * @returns
 */
export function getCollateralExchangeRate(state: Reserve): Decimal {
  const [totalSupply, totalCollateral] = scaledSupplies(state);

  // These should be technically impossible.
  if (totalCollateral.eq(0)) {
    return INITIAL_COLLATERAL_RATE;
  }
  if (totalSupply.eq(0)) {
    // 0 / X = 0
    return new Decimal(0);
  }
  return totalCollateral.div(totalSupply);
}

/**
 * The inverse of `getCollateralExchangeRate`, i.e. if you have some amount in collateral tokens and want
 * to see how many liquidity tokens it is worth.
 * @param state
 * @returns
 */
export function getLiquidityExchangeRate(state: Reserve): Decimal {
  const [totalSupply, totalCollateral] = scaledSupplies(state);

  // These should be technically impossible.
  if (totalCollateral.eq(0)) {
    return INITIAL_COLLATERAL_RATE;
  }
  if (totalSupply.eq(0)) {
    // 0 / X = 0
    return new Decimal(0);
  }
  return totalSupply.div(totalCollateral);
}

/**
 * Estimate collateral token lamports received for a deposit of raw underlying lamports. This is
 * generally useful only in testing.
 * @param state
 * @param depositAmountRaw
 * @returns collateral token, in native decimals (always 6)
 */
export function estimateCollateralFromDeposit(
  state: Reserve,
  depositAmountRaw: BN
): BN {
  let depositRawDecimal = new Decimal(depositAmountRaw.toString());
  const rate = getCollateralExchangeRate(state);

  const collateralTokens = depositRawDecimal.mul(rate);

  // round off the fractional component
  const rounded = collateralTokens.toDecimalPlaces(0, Decimal.ROUND_FLOOR);
  return new BN(rounded.toString());
}

// TODO when porting this to the package, let's not convert buffer -> hex -> Decimal -> BigNumber,
// this seems straight up goofy.
/**
 * Same thing as `wrappedI80F48toBigNumber`, but for Kamino's numbering system, which uses U68F60.
 * @param wrapped
 * @returns
 */
export function wrappedU68F60toBigNumber(
  wrapped: WrappedI68F60 | BN // Note: Kamino generally encodes as BN instead of number[]
): BigNumber {
  let bytesLE: number[];
  if (wrapped instanceof BN) {
    bytesLE = wrapped.toArray("le", I68F60_TOTAL_BYTES);
  } else {
    bytesLE = wrapped.value;
  }

  if (bytesLE.length !== I68F60_TOTAL_BYTES) {
    throw new Error(`Expected a ${I68F60_TOTAL_BYTES}-byte buffer`);
  }

  // convert little-endian → big-endian
  const bytesBE = [...bytesLE].reverse();

  // detect sign (high bit of first byte), flip bits if negative
  let sign = "";
  if (bytesBE[0] & 0x80) {
    sign = "-";
    for (let i = 0; i < bytesBE.length; i++) {
      bytesBE[i] = ~bytesBE[i] & 0xff;
    }
  }

  const hex =
    sign + "0x" + bytesBE.map((b) => b.toString(16).padStart(2, "0")).join("");
  const dec = new Decimal(hex).dividedBy(I68F60_DIVISOR);
  return new BigNumber(dec.toString());
}
