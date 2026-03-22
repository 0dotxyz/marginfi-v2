import { WrappedI80F48, bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { PublicKey } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import BN from "bn.js";
import type { IdlAccounts, Program } from "@coral-xyz/anchor";
import type { Marginfi } from "../../../target/types/marginfi";
import type { Liquidity } from "./idl-types/liquidity";
import type { Lending } from "./idl-types/juplend-earn";
import type { LendingRewardRateModel } from "./idl-types/lending-reward-rate-model";

export type MarginfiBankRaw = IdlAccounts<Marginfi>["bank"];

export type JuplendLiquidityIdl = Liquidity;
export type JuplendLendingIdl = Lending;
export type JuplendRewardsIdl = LendingRewardRateModel;

export type JuplendPrograms = {
  liquidity: Program<JuplendLiquidityIdl>;
  lending: Program<JuplendLendingIdl>;
  rewards: Program<JuplendRewardsIdl>;
};

export type JuplendPoolKeys = {
  /** Underlying token mint used by this JupLend pool (e.g. USDC mint). */
  mint: PublicKey;
  /** Token program owning the underlying mint (SPL Token or Token-2022). */
  tokenProgram: PublicKey;
  /** Global Liquidity account PDA (singleton per liquidity program). */
  liquidity: PublicKey;
  /** Global authorization list PDA used by liquidity/lending permission checks. */
  authList: PublicKey;
  /** TokenReserve PDA for this mint; links mint, vault, rate model, and positions. */
  tokenReserve: PublicKey;
  /** Rate model PDA storing utilization->rate parameters for this reserve. */
  rateModel: PublicKey;
  /** Liquidity vault PDA (underlying token vault on the native Jup side). */
  vault: PublicKey;
  /** Global rewards-admin PDA used by lending rewards program instructions. */
  lendingRewardsAdmin: PublicKey;
  /** Rewards rate model PDA associated with this pool's lending state. */
  lendingRewardsRateModel: PublicKey;
  /** Global lending-admin PDA used for lending protocol configuration. */
  lendingAdmin: PublicKey;
  /** Lending state PDA for this mint/fToken pair (exchange-rate source). */
  lending: PublicKey;
  /** fToken mint PDA for this reserve (interest-bearing share token). */
  fTokenMint: PublicKey;
  /** Metaplex metadata PDA for the pool's fToken mint. */
  fTokenMetadata: PublicKey;
  /** Liquidity-side user supply position PDA owned by the lending state. */
  supplyPositionOnLiquidity: PublicKey;
  /** Liquidity-side user borrow position PDA owned by the lending state. */
  borrowPositionOnLiquidity: PublicKey;
  /** Expected withdraw intermediary ATA for a marginfi bank when derivation context is provided. */
  withdrawIntermediaryAta?: PublicKey;
};

type LiquidityAccounts = IdlAccounts<JuplendLiquidityIdl>;
type LendingAccounts = IdlAccounts<JuplendLendingIdl>;
type RewardsAccounts = IdlAccounts<JuplendRewardsIdl>;

export type JuplendPoolFetched = {
  keys: JuplendPoolKeys;
  accounts: {
    liquidity: LiquidityAccounts["liquidity"];
    authList: LiquidityAccounts["authorizationList"];
    rateModel: LiquidityAccounts["rateModel"];
    tokenReserve: LiquidityAccounts["tokenReserve"];
    supplyPosition: LiquidityAccounts["userSupplyPosition"];
    borrowPosition: LiquidityAccounts["userBorrowPosition"];
    lendingAdmin: LendingAccounts["lendingAdmin"];
    rewardsAdmin: RewardsAccounts["lendingRewardsAdmin"];
    rewardsRateModel: RewardsAccounts["lendingRewardsRateModel"];
    lendingState: LendingAccounts["lending"];
  };
};

export type JuplendGlobals = {
  /** Global Liquidity account PDA (singleton per liquidity program). */
  liquidity: PublicKey;
  /** Global authorization list PDA used by liquidity/lending permission checks. */
  authList: PublicKey;
  /** Global lending-admin PDA used for lending protocol configuration. */
  lendingAdmin: PublicKey;
  /** Global rewards-admin PDA used by lending rewards program instructions. */
  lendingRewardsAdmin: PublicKey;
};

/**
 * Compact bank config for the JupLend integration.
 *
 * Mirrors the Rust type: `JuplendConfigCompact`.
 *
 * Note: JupLend banks always start in `Paused` state. Only `juplend_init_position`
 * can activate them to `Operational`.
 */
export interface JuplendConfigCompact {
  oracle: PublicKey;

  assetWeightInit: WrappedI80F48;
  assetWeightMaint: WrappedI80F48;

  depositLimit: BN;

  /**
   * Oracle setup enum.
   *
   * For JupLend we use either:
   * - { juplendPythPull: {} }
   * - { juplendSwitchboardPull: {} }
   */
  oracleSetup: { juplendPythPull: {} } | { juplendSwitchboardPull: {} };

  riskTier: { collateral: {} } | { isolated: {} };
  configFlags: number;

  totalAssetValueInitLimit: BN;
  oracleMaxAge: number;
  oracleMaxConfidence: number;
}

/**
 * Default JupLend bank config used in tests.
 *
 * Notes:
 * - JupLend banks always start `Paused` and are activated via `juplendInitPosition`.
 * - We use `juplendPythPull` oracle setup which expects:
 *   - oracleKeys[0] = Pyth PriceUpdateV2
 *   - oracleKeys[1] = JupLend `lending` state account
 */
export const defaultJuplendBankConfig = (
  oracle: PublicKey,
  decimals: number,
): JuplendConfigCompact => {
  return {
    oracle,
    assetWeightInit: bigNumberToWrappedI80F48(new BigNumber(0.8)),
    assetWeightMaint: bigNumberToWrappedI80F48(new BigNumber(0.9)),
    depositLimit: new BN(1_000_000).mul(new BN(10).pow(new BN(decimals))),
    oracleSetup: { juplendPythPull: {} },
    riskTier: { collateral: {} },
    configFlags: 1,
    totalAssetValueInitLimit: new BN(1_000_000_000).mul(
      new BN(10).pow(new BN(decimals)),
    ),
    oracleMaxAge: 60,
    oracleMaxConfidence: 0,
  };
};

/**
 * A simple three-point curve with one kink: (0, rateAtUtilizationZero), (kink %,
 * rateAtUtilizationKink), (100%, rateAtUtilizationMax). Jup also supports a two-kink curve, which
 * is updated with updateRateDataV2 instead, see Jup's `RateDataV1Params` or `RateDataV2Params` for
 * more details.
 */
export type JuplendRateConfig = {
  /** Note: in 1e2 encoding, 100% = 10_000; 1% = 100, u16::MAX is the max */
  kink: BN;
  /** Note: in 1e2 encoding, 100% = 10_000; 1% = 100, u16::MAX is the max */
  rateAtUtilizationZero: BN;
  /** Note: in 1e2 encoding, 100% = 10_000; 1% = 100, u16::MAX is the max */
  rateAtUtilizationKink: BN;
  /** Note: in 1e2 encoding, 100% = 10_000; 1% = 100, u16::MAX is the max */
  rateAtUtilizationMax: BN;
};

/**
 * Token-level liquidity config.
 *
 * - `fee` is the protocol fee taken from borrower interest (1e2 encoding).
 * - `maxUtilization` caps utilization for the reserve (1e2 encoding).
 * Both values are passed as BN/u128 in instruction args.
 */
export type JuplendTokenConfig = {
  /** In 1e2 encoding, 100% = 10_000; 1% = 100 */
  fee: BN;
  /** In 1e2 encoding, 100% = 10_000; 1% = 100 */
  maxUtilization: BN;
};

/**
 * Per-protocol supply/withdraw policy on the liquidity side. Note that:
 * * maxWithdrawable = supplyAmount * expandPercent / 10_000
 * * withdrawalAllowanceUnlocked = maxWithdrawable * elapsed / expandDuration
 * * currentWithdrawalLimit = lastLimit - withdrawalAllowanceUnlocked
 *      * with a floor of supply - supply*expandPercent/10_000
 *
 * Example 1:
 * * user balance = 1000 and current limit ~= 1000
 * * Expand percent = 20%, expand duration = 2 days, baseWithdrawalLimit = 100
 * * Day 0 - can't withdraw anything, limit reached
 * * Day 1 - 20% unlocks every 2 days, so after 1 day, 10% (100) is unlocked
 * * Day 2 - 20% is unlocked (200, limit = 800)
 * * Let's say User withdraws 200 at this point, leaving 800 with current limit 800
 * * Day 3 - 20% * 800 * (1/2) = 80 unlocked (limit now 720)
 * * Day 4 - 20% * 800 = 160 unlocked (limit now 640)
 *
 * Example 2:
 * * baseWithdrawalLimit = 1000, percent = 20%.
 * * User deposits 5000
 * * User can withdraw only 1000, as minimum remaining = 5000 * (1 - 0.20) = 4000
 * * Day 1 - still 1000
 * * Day 2 - still 1000
 *
 * * Example 3:
 * * baseWithdrawalLimit = 1000, percent = 20%, expansion = 2 days
 * * User deposits 5000, immediately withdraws 1000
 * * User can withdraw 0, having hit the limit
 * * Day 1 - can withdraw 400 = (20% * 4000 * (1/2))
 * * Day 2 - can withdraw 800 = (20% * 4000)
 *
 * `mode`:
 * - `0` = without interest accounting
 * - `1` = with interest accounting (default used in these tests)
 */
export type JuplendSupplyConfig = {
  /** 0 = without interest, 1 = with interest */
  mode: number;
  /** Step expansion percent for withdrawal limit (1e2 encoding) */
  expandPercent: BN;
  /** Expansion window in seconds */
  expandDuration: BN;
  /** Base withdrawal threshold before stepped limits kick in. Can be considered the minimum balance
   * you must leave in the pool, with the caveat that if the user balance falls below this, full
   * withdrawal is allowed.
   * * if mode 0, in token
   * * if mode 1, in raw share units, converted via exchange rate
   */
  baseWithdrawalLimit: BN;
};

/**
 * Per-protocol borrow/payback policy on the liquidity side. Similar setup to `JuplendSupplyConfig`
 * but mostly irrelevant in this test suite because we do not borrow, except to trigger interest
 * generation.
 *
 * `mode`:
 * - `0` = without interest accounting
 * - `1` = with interest accounting (default used in these tests)
 */
export type JuplendBorrowConfig = {
  /** 0 = without interest, 1 = with interest */
  mode: number;
  /** Step expansion percent for borrow limit (1e2 encoding) */
  expandPercent: BN;
  /** Expansion window in seconds */
  expandDuration: BN;
  /** Base debt ceiling before stepped limits apply */
  baseDebtCeiling: BN;
  /**  Hard upper bound for borrow limit */
  maxDebtCeiling: BN;
};

/**
 * Entry passed to `updateUserClass`.
 *
 * Maps an address (typically the protocol/lending account) to a class value
 * consumed by Liquidity auth/permission checks.
 */
export type JuplendUserClassEntry = {
  addr: PublicKey;
  /** Class id/value written into authorization list */
  value: number;
};

// With these settings, there are essentially no limits to withdraws due to the
// window, i.e. withdraw throttling is effectively disabled.
export const U64_MAX = new BN("18446744073709551615");
export const DEFAULT_PERCENT_PRECISION = new BN(100);
export const DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT = new BN(20).mul(
  DEFAULT_PERCENT_PRECISION,
);
export const DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS = new BN(
  2 * 24 * 60 * 60,
);

const LAMPORTS_PER_SOL = 1_000_000_000;
export const DEFAULT_BASE_DEBT_CEILING = new BN(1e4 * LAMPORTS_PER_SOL);
export const DEFAULT_MAX_DEBT_CEILING = new BN(1e6 * LAMPORTS_PER_SOL);

export const percent = (
  value: number,
  precision: BN = DEFAULT_PERCENT_PRECISION,
) => new BN(value).mul(precision);

/** (0, 4), (80, 10), (100, 150) */
export const DEFAULT_RATE_CONFIG: JuplendRateConfig = {
  kink: percent(80),
  rateAtUtilizationZero: percent(4),
  rateAtUtilizationKink: percent(10),
  rateAtUtilizationMax: percent(150),
};

/** No fee, unlimited borrow (up to 100% of deposits) */
export const DEFAULT_TOKEN_CONFIG: JuplendTokenConfig = {
  fee: new BN(0),
  maxUtilization: percent(100),
};

export const DEFAULT_SUPPLY_CONFIG: JuplendSupplyConfig = {
  mode: 1,
  expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
  expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
  baseWithdrawalLimit: U64_MAX,
};

export const DEFAULT_BORROW_CONFIG: JuplendBorrowConfig = {
  mode: 1,
  expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
  expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
  baseDebtCeiling: DEFAULT_BASE_DEBT_CEILING,
  maxDebtCeiling: DEFAULT_MAX_DEBT_CEILING,
};

export const DEFAULT_BORROW_CONFIG_DISABLED: JuplendBorrowConfig = {
  mode: 1,
  expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
  expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
  baseDebtCeiling: new BN(0),
  maxDebtCeiling: new BN(0),
};

// Smallest non-zero borrow limits accepted by Liquidity admin module.
// Useful for init-only tests that don't exercise borrowing.
export const DEFAULT_BORROW_CONFIG_MIN: JuplendBorrowConfig = {
  mode: 1,
  expandPercent: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_PERCENT,
  expandDuration: DEFAULT_EXPAND_WITHDRAWAL_LIMIT_DURATION_SECONDS,
  baseDebtCeiling: new BN(1),
  maxDebtCeiling: new BN(1),
};
