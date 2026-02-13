import { PublicKey } from "@solana/web3.js";

import type { JuplendPrograms } from "./types";
import type {
  JuplendBorrowConfig,
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
  args: InitJuplendLiquidityArgs
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
  args: InitJuplendLendingRewardsAdminArgs
) => {
  return programs.rewards.methods
    .initLendingRewardsAdmin(
      args.authority,
      args.lendingProgram ?? JUPLEND_LENDING_PROGRAM_ID
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
  args: InitJuplendLendingAdminArgs
) => {
  return programs.lending.methods
    .initLendingAdmin(
      args.liquidityProgram ?? JUPLEND_LIQUIDITY_PROGRAM_ID,
      args.adminAuthority,
      args.rebalancer
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
  args: InitJuplendProtocolPositionsArgs
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
  args: UpdateJuplendUserClassArgs
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
  args: UpdateJuplendUserBorrowConfigArgs
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
      protocol: args.protocol,
      authList: args.authList,
      rateModel: args.rateModel,
      mint: args.mint,
      tokenReserve: args.tokenReserve,
      userBorrowPosition: args.userBorrowPosition,
    })
    .instruction();
};
