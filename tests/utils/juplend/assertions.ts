import { AccountLayout, getMint } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { BN } from "@coral-xyz/anchor";

import { bankRunProvider } from "../../rootHooks";
import {
  assertBNEqual,
  assertI80F48Equal,
  assertKeysEqual,
} from "../genericTests";

import {
  TOKEN_METADATA_PROGRAM_ID,
  JUPLEND_EARN_REWARDS_PROGRAM_ID,
  JUPLEND_LENDING_PROGRAM_ID,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
  deriveJuplendLendingPdas,
  deriveJuplendLiquidityVaultAta,
  type JuplendMrgnAddresses,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquidityPda,
  findJuplendLiquidityRateModelPda,
  findJuplendLiquiditySupplyPositionPda,
  findJuplendLiquidityTokenReservePda,
  findJuplendRewardsRateModelPdaBestEffort,
} from "./juplend-pdas";
import { getJuplendPrograms } from "./programs";
import type {
  JuplendConfigCompact,
  JuplendPoolKeys,
  MarginfiBankRaw,
} from "./types";
import { ASSET_TAG_JUPLEND } from "../types";

export type AssertJuplendPoolArgs = {
  pool: JuplendPoolKeys;
  mint: PublicKey;
  decimals: number;
};

const toBigInt = (value: BN | number | bigint): bigint => {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") return BigInt(value);
  return BigInt(value.toString());
};

export const assertDebtCeilingIsSupported = async (args: {
  mint: PublicKey;
  tokenProgram: PublicKey;
  maxDebtCeiling: BN;
}) => {
  if (args.maxDebtCeiling.isZero()) return;

  const mintInfo = await getMint(
    bankRunProvider.connection,
    args.mint,
    undefined,
    args.tokenProgram,
  );

  const supply = mintInfo.supply;
  const maxDebt = toBigInt(args.maxDebtCeiling);
  assert.ok(
    maxDebt <= supply * 10n,
    `max_debt_ceiling (${maxDebt.toString()}) exceeds 10x total supply (${supply.toString()})`,
  );
};

export const assertJuplendPoolInitialized = async (
  args: AssertJuplendPoolArgs
) => {
  const { pool, mint, decimals } = args;
  const connection = bankRunProvider.connection;
  const programs = getJuplendPrograms();

  const lendingPdas = deriveJuplendLendingPdas(mint, JUPLEND_LENDING_PROGRAM_ID);
  assertKeysEqual(pool.lendingAdmin, lendingPdas.lendingAdmin);
  assertKeysEqual(pool.fTokenMint, lendingPdas.fTokenMint);
  assertKeysEqual(pool.lending, lendingPdas.lending);

  const [liquidity] = findJuplendLiquidityPda(JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(pool.liquidity, liquidity);

  const [tokenReserve] = findJuplendLiquidityTokenReservePda(
    mint,
    JUPLEND_LIQUIDITY_PROGRAM_ID
  );
  assertKeysEqual(pool.tokenReserve, tokenReserve);

  const [rateModel] = findJuplendLiquidityRateModelPda(
    mint,
    JUPLEND_LIQUIDITY_PROGRAM_ID
  );
  assertKeysEqual(pool.rateModel, rateModel);

  const expectedVault = deriveJuplendLiquidityVaultAta(
    mint,
    pool.liquidity,
    pool.tokenProgram
  );
  assertKeysEqual(pool.vault, expectedVault);

  const [supplyPositionOnLiquidity] = findJuplendLiquiditySupplyPositionPda(
    mint,
    pool.lending,
    JUPLEND_LIQUIDITY_PROGRAM_ID
  );
  assertKeysEqual(pool.supplyPositionOnLiquidity, supplyPositionOnLiquidity);

  const [borrowPositionOnLiquidity] = findJuplendLiquidityBorrowPositionPda(
    mint,
    pool.lending,
    JUPLEND_LIQUIDITY_PROGRAM_ID
  );
  assertKeysEqual(pool.borrowPositionOnLiquidity, borrowPositionOnLiquidity);

  const [rewardsRateModel] = findJuplendRewardsRateModelPdaBestEffort(mint);
  assertKeysEqual(pool.lendingRewardsRateModel, rewardsRateModel);

  const [
    lendingInfo,
    liquidityInfo,
    authListInfo,
    reserveInfo,
    rateInfo,
    supplyPosInfo,
    borrowPosInfo,
    rewardsInfo,
    rewardsAdminInfo,
    lendingAdminInfo,
    fTokenMintInfo,
    vaultInfo,
    metadataInfo,
  ] = await Promise.all([
    connection.getAccountInfo(pool.lending),
    connection.getAccountInfo(pool.liquidity),
    connection.getAccountInfo(pool.authList),
    connection.getAccountInfo(pool.tokenReserve),
    connection.getAccountInfo(pool.rateModel),
    connection.getAccountInfo(pool.supplyPositionOnLiquidity),
    connection.getAccountInfo(pool.borrowPositionOnLiquidity),
    connection.getAccountInfo(pool.lendingRewardsRateModel),
    connection.getAccountInfo(pool.lendingRewardsAdmin),
    connection.getAccountInfo(pool.lendingAdmin),
    connection.getAccountInfo(pool.fTokenMint),
    connection.getAccountInfo(pool.vault),
    connection.getAccountInfo(pool.fTokenMetadata),
  ]);

  assert.ok(lendingInfo, "missing lending state");
  assert.ok(liquidityInfo, "missing liquidity state");
  assert.ok(authListInfo, "missing auth list");
  assert.ok(reserveInfo, "missing token reserve");
  assert.ok(rateInfo, "missing rate model");
  assert.ok(supplyPosInfo, "missing supply position");
  assert.ok(borrowPosInfo, "missing borrow position");
  assert.ok(rewardsInfo, "missing rewards rate model");
  assert.ok(rewardsAdminInfo, "missing rewards admin");
  assert.ok(lendingAdminInfo, "missing lending admin");
  assert.ok(fTokenMintInfo, "missing fToken mint");
  assert.ok(vaultInfo, "missing vault");
  assert.ok(metadataInfo, "missing fToken metadata");

  assertKeysEqual(lendingInfo.owner, JUPLEND_LENDING_PROGRAM_ID);
  assertKeysEqual(liquidityInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(authListInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(reserveInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(rateInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(supplyPosInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(borrowPosInfo.owner, JUPLEND_LIQUIDITY_PROGRAM_ID);
  assertKeysEqual(rewardsInfo.owner, JUPLEND_EARN_REWARDS_PROGRAM_ID);
  assertKeysEqual(rewardsAdminInfo.owner, JUPLEND_EARN_REWARDS_PROGRAM_ID);
  assertKeysEqual(lendingAdminInfo.owner, JUPLEND_LENDING_PROGRAM_ID);
  assertKeysEqual(fTokenMintInfo.owner, pool.tokenProgram);
  assertKeysEqual(vaultInfo.owner, pool.tokenProgram);
  assertKeysEqual(metadataInfo.owner, TOKEN_METADATA_PROGRAM_ID);

  const lendingState = await programs.lending.account.lending.fetch(pool.lending);
  assertKeysEqual(lendingState.mint, mint);
  assertKeysEqual(lendingState.fTokenMint, pool.fTokenMint);
  assert.equal(lendingState.decimals, decimals);
  assertKeysEqual(lendingState.rewardsRateModel, pool.lendingRewardsRateModel);
  assertKeysEqual(lendingState.tokenReservesLiquidity, pool.tokenReserve);
  assertKeysEqual(
    lendingState.supplyPositionOnLiquidity,
    pool.supplyPositionOnLiquidity
  );
  assert.notEqual(lendingState.tokenExchangePrice.toString(), "0");

  const fTokenMint = await getMint(
    connection,
    pool.fTokenMint,
    undefined,
    pool.tokenProgram
  );
  assert.equal(fTokenMint.decimals, decimals);

  const vault = AccountLayout.decode(vaultInfo.data);
  assertKeysEqual(new PublicKey(vault.owner), pool.liquidity);
  assertKeysEqual(new PublicKey(vault.mint), mint);
};

export type AssertJuplendBankArgs = {
  bankPk: PublicKey;
  bank: MarginfiBankRaw;
  group: PublicKey;
  mint: PublicKey;
  decimals: number;
  oracle: PublicKey;
  pool: JuplendPoolKeys;
  addresses: JuplendMrgnAddresses;
  config: JuplendConfigCompact;
  expectedState: MarginfiBankRaw["config"]["operationalState"];
};

export const assertJuplendBankState = (args: AssertJuplendBankArgs) => {
  const {
    bankPk,
    bank,
    group,
    mint,
    decimals,
    oracle,
    pool,
    addresses,
    config,
    expectedState,
  } = args;

  assertKeysEqual(bankPk, addresses.bank);
  assertKeysEqual(bank.group, group);
  assertKeysEqual(bank.mint, mint);
  assert.equal(bank.mintDecimals, decimals);

  assertKeysEqual(bank.integrationAcc1, pool.lending);
  assertKeysEqual(bank.integrationAcc2, addresses.fTokenVault);
  assertKeysEqual(bank.integrationAcc3, addresses.withdrawIntermediaryAta);

  assertKeysEqual(bank.liquidityVault, addresses.liquidityVault);
  assertKeysEqual(bank.insuranceVault, addresses.insuranceVault);
  assertKeysEqual(bank.feeVault, addresses.feeVault);

  assert.equal(bank.config.assetTag, ASSET_TAG_JUPLEND);
  assertKeysEqual(bank.config.oracleKeys[0], oracle);
  assertKeysEqual(bank.config.oracleKeys[1], pool.lending);
  assert.deepEqual(bank.config.oracleSetup, config.oracleSetup);

  assertI80F48Equal(bank.config.assetWeightInit, config.assetWeightInit);
  assertI80F48Equal(bank.config.assetWeightMaint, config.assetWeightMaint);
  assertBNEqual(bank.config.depositLimit, config.depositLimit);
  assertBNEqual(
    bank.config.totalAssetValueInitLimit,
    config.totalAssetValueInitLimit
  );
  assert.deepEqual(bank.config.riskTier, config.riskTier);
  assert.equal(bank.config.configFlags, config.configFlags);
  assert.equal(bank.config.oracleMaxAge, config.oracleMaxAge);
  assert.equal(bank.config.oracleMaxConfidence, config.oracleMaxConfidence);

  assert.deepEqual(bank.config.operationalState, expectedState);
};
