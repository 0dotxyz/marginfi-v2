import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

import type { JuplendPrograms } from "./types";
import type {
  JuplendBorrowConfig,
  JuplendPoolKeys,
  JuplendUserClassEntry,
} from "./types";
import {
  JUPLEND_LENDING_PROGRAM_ID,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
} from "./juplend-pdas";

export type InitJuplendLiquidityArgs = {
  signer: PublicKey;
  authority: PublicKey;
  revenueCollector: PublicKey;
};

export const initJuplendLiquidityIx = (
  programs: JuplendPrograms,
  args: InitJuplendLiquidityArgs,
) => {
  return programs.liquidity.methods
    .initLiquidity(args.authority, args.revenueCollector)
    .accounts({
      signer: args.signer,
    })
    .instruction();
};

export type InitJuplendLendingRewardsAdminArgs = {
  signer: PublicKey;
  authority: PublicKey;
  lendingProgram?: PublicKey;
};

export const initJuplendLendingRewardsAdminIx = (
  programs: JuplendPrograms,
  args: InitJuplendLendingRewardsAdminArgs,
) => {
  return programs.rewards.methods
    .initLendingRewardsAdmin(
      args.authority,
      args.lendingProgram ?? JUPLEND_LENDING_PROGRAM_ID,
    )
    .accounts({
      signer: args.signer,
    })
    .instruction();
};

export type InitJuplendLendingAdminArgs = {
  authority: PublicKey;
  adminAuthority: PublicKey;
  rebalancer: PublicKey;
  liquidityProgram?: PublicKey;
};

export const initJuplendLendingAdminIx = (
  programs: JuplendPrograms,
  args: InitJuplendLendingAdminArgs,
) => {
  return programs.lending.methods
    .initLendingAdmin(
      args.liquidityProgram ?? JUPLEND_LIQUIDITY_PROGRAM_ID,
      args.adminAuthority,
      args.rebalancer,
    )
    .accounts({
      authority: args.authority,
    })
    .instruction();
};

export type InitJuplendProtocolPositionsArgs = {
  authority: PublicKey;
  authList: PublicKey;
  supplyMint: PublicKey;
  borrowMint: PublicKey;
  protocol: PublicKey;
};

export const initJuplendProtocolPositionsIx = (
  programs: JuplendPrograms,
  args: InitJuplendProtocolPositionsArgs,
) => {
  return programs.liquidity.methods
    .initNewProtocol(args.supplyMint, args.borrowMint, args.protocol)
    .accounts({
      authority: args.authority,
      authList: args.authList,
    })
    .instruction();
};

export type UpdateJuplendUserClassArgs = {
  authority: PublicKey;
  authList: PublicKey;
  entries: JuplendUserClassEntry[];
};

export const updateJuplendUserClassIx = (
  programs: JuplendPrograms,
  args: UpdateJuplendUserClassArgs,
) => {
  return programs.liquidity.methods
    .updateUserClass(args.entries)
    .accounts({
      authority: args.authority,
      authList: args.authList,
    })
    .instruction();
};

export type UpdateJuplendUserBorrowConfigArgs = {
  authority: PublicKey;
  protocol: PublicKey;
  authList: PublicKey;
  rateModel: PublicKey;
  mint: PublicKey;
  tokenReserve: PublicKey;
  userBorrowPosition: PublicKey;
  config: JuplendBorrowConfig;
};

export const updateJuplendUserBorrowConfigIx = (
  programs: JuplendPrograms,
  args: UpdateJuplendUserBorrowConfigArgs,
) => {
  return programs.liquidity.methods
    .updateUserBorrowConfig({
      mode: args.config.mode,
      expandPercent: args.config.expandPercent,
      expandDuration: args.config.expandDuration,
      baseDebtCeiling: args.config.baseDebtCeiling,
      maxDebtCeiling: args.config.maxDebtCeiling,
    })
    .accounts({
      authority: args.authority,
      // protocol: args.protocol,
      authList: args.authList,
      rateModel: args.rateModel,
      // mint: args.mint,
      tokenReserve: args.tokenReserve,
      userBorrowPosition: args.userBorrowPosition,
    })
    .accountsPartial({
      protocol: args.protocol,
    })
    .instruction();
};

export type InitJuplendClaimAccountArgs = {
  signer: PublicKey;
  mint: PublicKey;
  accountFor: PublicKey;
  claimAccount?: PublicKey;
};

/**
 * (Permissionless) Any wallet can create a claim account a jup user. This means that if jup starts
 * a reward, somebody (anybody) must create one for the liquidity vault authority.
 * @param programs
 * @param args
 * @returns
 */
export const initJuplendClaimAccountIx = (
  programs: JuplendPrograms,
  args: InitJuplendClaimAccountArgs,
) => {
  return programs.liquidity.methods
    .initClaimAccount(args.mint, args.accountFor)
    .accounts({
      signer: args.signer,
    })
    .accountsPartial({
      claimAccount: args.claimAccount,
    })
    .instruction();
};

export type StartJuplendRewardsArgs = {
  authority: PublicKey;
  pool: JuplendPoolKeys;
  rewardAmount: BN;
  duration: BN;
  startTime?: BN;
  startTvl?: BN;
  lendingProgram?: PublicKey;
};

export const startJuplendRewardsIx = (
  programs: JuplendPrograms,
  args: StartJuplendRewardsArgs,
) => {
  return programs.rewards.methods
    .startRewards(
      args.rewardAmount,
      args.duration,
      args.startTime ?? new BN(0),
      args.startTvl ?? new BN(0),
    )
    .accounts({
      authority: args.authority,
      lendingRewardsAdmin: args.pool.lendingRewardsAdmin,
      lendingAccount: args.pool.lending,
      mint: args.pool.mint,
      fTokenMint: args.pool.fTokenMint,
      supplyTokenReservesLiquidity: args.pool.tokenReserve,
      lendingRewardsRateModel: args.pool.lendingRewardsRateModel,
      lendingProgram: args.lendingProgram ?? JUPLEND_LENDING_PROGRAM_ID,
    })
    .instruction();
};

export type StopJuplendRewardsArgs = {
  authority: PublicKey;
  pool: JuplendPoolKeys;
  lendingProgram?: PublicKey;
};

export const stopJuplendRewardsIx = (
  programs: JuplendPrograms,
  args: StopJuplendRewardsArgs,
) => {
  return programs.rewards.methods
    .stopRewards()
    .accounts({
      authority: args.authority,
      lendingRewardsAdmin: args.pool.lendingRewardsAdmin,
      lendingAccount: args.pool.lending,
      mint: args.pool.mint,
      fTokenMint: args.pool.fTokenMint,
      supplyTokenReservesLiquidity: args.pool.tokenReserve,
      lendingRewardsRateModel: args.pool.lendingRewardsRateModel,
      lendingProgram: args.lendingProgram ?? JUPLEND_LENDING_PROGRAM_ID,
    })
    .instruction();
};
