import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";

import { bankRunProvider, bankrunContext } from "../../rootHooks";
import { assertBNEqual, assertKeysEqual } from "../genericTests";
import { processBankrunTransaction } from "../tools";
import {
  JUPLEND_EARN_REWARDS_PROGRAM_ID,
  JUPLEND_LENDING_PROGRAM_ID,
  JUPLEND_LIQUIDITY_PROGRAM_ID,
  deriveJuplendGlobalKeys,
  deriveJuplendPoolKeys,
  findJuplendRewardsRateModelPdaBestEffort,
} from "./juplend-pdas";
import { getJuplendPrograms } from "./programs";
import {
  initJuplendLendingAdminIx,
  initJuplendLendingRewardsAdminIx,
  initJuplendLiquidityIx,
} from "./admin-instructions";
import { assertDebtCeilingIsSupported } from "./assertions";
import {
  DEFAULT_BORROW_CONFIG,
  DEFAULT_RATE_CONFIG,
  DEFAULT_SUPPLY_CONFIG,
  DEFAULT_TOKEN_CONFIG,
} from "./types";
import type {
  JuplendBorrowConfig,
  JuplendGlobals,
  JuplendPoolFetched,
  JuplendPoolKeys,
  JuplendPrograms,
  JuplendRateConfig,
  JuplendSupplyConfig,
  JuplendTokenConfig,
  JuplendUserClassEntry,
} from "./types";

async function getTokenProgramForMint(mint: PublicKey): Promise<PublicKey> {
  const info = await bankRunProvider.connection.getAccountInfo(mint);
  if (!info) throw new Error(`Mint account missing: ${mint.toBase58()}`);

  if (
    info.owner.equals(TOKEN_PROGRAM_ID) ||
    info.owner.equals(TOKEN_2022_PROGRAM_ID)
  ) {
    return info.owner;
  }

  throw new Error(
    `Unsupported mint owner for ${mint.toBase58()}: ${info.owner.toBase58()}`,
  );
}

function toBn(value: BN | number | bigint): BN {
  if (typeof value === "bigint") return new BN(value.toString());
  if (typeof value === "number") return new BN(value);
  return value;
}

export async function fetchJuplendPool(args: {
  mint: PublicKey;
  tokenProgram?: PublicKey;
  programs?: JuplendPrograms;
}): Promise<JuplendPoolFetched> {
  const programs = args.programs ?? getJuplendPrograms();
  const tokenProgram =
    args.tokenProgram ?? (await getTokenProgramForMint(args.mint));
  const keys = deriveJuplendPoolKeys({
    mint: args.mint,
    tokenProgram,
  });

  const [
    liquidity,
    authList,
    rateModel,
    tokenReserve,
    supplyPosition,
    borrowPosition,
    lendingAdmin,
    rewardsAdmin,
    rewardsRateModel,
    lendingState,
  ] = await Promise.all([
    programs.liquidity.account.liquidity.fetch(keys.liquidity),
    programs.liquidity.account.authorizationList.fetch(keys.authList),
    programs.liquidity.account.rateModel.fetch(keys.rateModel),
    programs.liquidity.account.tokenReserve.fetch(keys.tokenReserve),
    programs.liquidity.account.userSupplyPosition.fetch(
      keys.supplyPositionOnLiquidity,
    ),
    programs.liquidity.account.userBorrowPosition.fetch(
      keys.borrowPositionOnLiquidity,
    ),
    programs.lending.account.lendingAdmin.fetch(keys.lendingAdmin),
    programs.rewards.account.lendingRewardsAdmin.fetch(
      keys.lendingRewardsAdmin,
    ),
    programs.rewards.account.lendingRewardsRateModel.fetch(
      keys.lendingRewardsRateModel,
    ),
    programs.lending.account.lending.fetch(keys.lending),
  ]);

  return {
    keys,
    accounts: {
      liquidity,
      authList,
      rateModel,
      tokenReserve,
      supplyPosition,
      borrowPosition,
      lendingAdmin,
      rewardsAdmin,
      rewardsRateModel,
      lendingState,
    },
  };
}

export async function initJuplendGlobals(args: {
  admin: Keypair;
  programs?: JuplendPrograms;
}): Promise<JuplendGlobals> {
  const programs = args.programs ?? getJuplendPrograms();
  const { liquidity, authList, lendingAdmin, lendingRewardsAdmin } =
    deriveJuplendGlobalKeys();

  const initLiquidity = await initJuplendLiquidityIx(programs, {
    signer: args.admin.publicKey,
    authority: args.admin.publicKey,
    revenueCollector: args.admin.publicKey,
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(initLiquidity),
    [args.admin],
    false,
    true,
  );

  const initRewardsAdmin = await initJuplendLendingRewardsAdminIx(programs, {
    signer: args.admin.publicKey,
    authority: args.admin.publicKey,
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(initRewardsAdmin),
    [args.admin],
    false,
    true,
  );

  const initLendingAdmin = await initJuplendLendingAdminIx(programs, {
    authority: args.admin.publicKey,
    adminAuthority: args.admin.publicKey,
    rebalancer: args.admin.publicKey,
  });

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(initLendingAdmin),
    [args.admin],
    false,
    true,
  );

  const liquidityAcc = await programs.liquidity.account.liquidity.fetch(
    liquidity,
  );
  assertKeysEqual(liquidityAcc.authority, args.admin.publicKey);
  assertKeysEqual(liquidityAcc.revenueCollector, args.admin.publicKey);

  const rewardsAdminAcc =
    await programs.rewards.account.lendingRewardsAdmin.fetch(
      lendingRewardsAdmin,
    );
  assertKeysEqual(rewardsAdminAcc.authority, args.admin.publicKey);
  assertKeysEqual(rewardsAdminAcc.lendingProgram, JUPLEND_LENDING_PROGRAM_ID);

  const lendingAdminAcc = await programs.lending.account.lendingAdmin.fetch(
    lendingAdmin,
  );
  assertKeysEqual(lendingAdminAcc.authority, args.admin.publicKey);
  assertKeysEqual(
    lendingAdminAcc.liquidityProgram,
    JUPLEND_LIQUIDITY_PROGRAM_ID,
  );
  assertKeysEqual(lendingAdminAcc.rebalancer, args.admin.publicKey);

  return {
    liquidity,
    authList,
    lendingAdmin,
    lendingRewardsAdmin,
  };
}

async function initJuplendLiquidityForMint(args: {
  admin: Keypair;
  mint: PublicKey;
  tokenProgram: PublicKey;
  rateConfig?: JuplendRateConfig;
  tokenConfig?: JuplendTokenConfig;
  programs?: JuplendPrograms;
}): Promise<void> {
  const programs = args.programs ?? getJuplendPrograms();
  const rateConfig = args.rateConfig ?? DEFAULT_RATE_CONFIG;
  const tokenConfig = args.tokenConfig ?? DEFAULT_TOKEN_CONFIG;

  const { liquidity, authList } = deriveJuplendGlobalKeys();
  const poolKeys = deriveJuplendPoolKeys({
    mint: args.mint,
    tokenProgram: args.tokenProgram,
  });

  const initTokenReserveIx = await programs.liquidity.methods
    .initTokenReserve()
    .accounts({
      authority: args.admin.publicKey,
      liquidity,
      authList,
      mint: args.mint,
      tokenProgram: args.tokenProgram,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(initTokenReserveIx),
    [args.admin],
    false,
    true,
  );

  const updateRateIx = await programs.liquidity.methods
    .updateRateDataV1({
      kink: rateConfig.kink,
      rateAtUtilizationZero: rateConfig.rateAtUtilizationZero,
      rateAtUtilizationKink: rateConfig.rateAtUtilizationKink,
      rateAtUtilizationMax: rateConfig.rateAtUtilizationMax,
    })
    .accounts({
      authority: args.admin.publicKey,
      authList,
      rateModel: poolKeys.rateModel,
      tokenReserve: poolKeys.tokenReserve,
    })
    .instruction();

  const updateTokenConfigIx = await programs.liquidity.methods
    .updateTokenConfig({
      token: args.mint,
      fee: tokenConfig.fee,
      maxUtilization: tokenConfig.maxUtilization,
    })
    .accounts({
      authority: args.admin.publicKey,
      authList,
      rateModel: poolKeys.rateModel,
      tokenReserve: poolKeys.tokenReserve,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(updateRateIx, updateTokenConfigIx),
    [args.admin],
    false,
    true,
  );

  const reserveAfter = await programs.liquidity.account.tokenReserve.fetch(
    poolKeys.tokenReserve,
  );
  assertKeysEqual(reserveAfter.mint, args.mint);
  assertKeysEqual(reserveAfter.vault, poolKeys.vault);
  assertBNEqual(tokenConfig.maxUtilization, reserveAfter.maxUtilization);
  assert.equal(reserveAfter.feeOnInterest, tokenConfig.fee.toNumber());

  const rateModelAfter = await programs.liquidity.account.rateModel.fetch(
    poolKeys.rateModel,
  );
  assertKeysEqual(rateModelAfter.mint, args.mint);
  assert.equal(rateModelAfter.kink1Utilization, rateConfig.kink.toNumber());
  assertBNEqual(rateConfig.rateAtUtilizationZero, rateModelAfter.rateAtZero);
  assertBNEqual(rateConfig.rateAtUtilizationKink, rateModelAfter.rateAtKink1);
  assert.equal(0, rateModelAfter.rateAtKink2);
  assertBNEqual(rateConfig.rateAtUtilizationMax, rateModelAfter.rateAtMax);
}

async function initJuplendRewardsRateModel(args: {
  admin: Keypair;
  mint: PublicKey;
  programs?: JuplendPrograms;
}): Promise<PublicKey> {
  const programs = args.programs ?? getJuplendPrograms();
  const { lendingRewardsAdmin } = deriveJuplendGlobalKeys();
  const [lendingRewardsRateModel] = findJuplendRewardsRateModelPdaBestEffort(
    args.mint,
    JUPLEND_EARN_REWARDS_PROGRAM_ID,
  );

  const ix = await programs.rewards.methods
    .initLendingRewardsRateModel()
    .accounts({
      authority: args.admin.publicKey,
      lendingRewardsAdmin,
      mint: args.mint,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [args.admin],
    false,
    true,
  );

  const rewards = await programs.rewards.account.lendingRewardsRateModel.fetch(
    lendingRewardsRateModel,
  );
  assertKeysEqual(rewards.mint, args.mint);
  assertBNEqual(rewards.startTvl, 0);
  assertBNEqual(rewards.duration, 0);

  return lendingRewardsRateModel;
}

async function initJuplendLendingForMint(args: {
  admin: Keypair;
  mint: PublicKey;
  symbol: string;
  decimals: number;
  tokenProgram: PublicKey;
  tokenReserve: PublicKey;
  lendingRewardsRateModel: PublicKey;
  programs?: JuplendPrograms;
}): Promise<void> {
  const programs = args.programs ?? getJuplendPrograms();
  const poolKeys = deriveJuplendPoolKeys({
    mint: args.mint,
    tokenProgram: args.tokenProgram,
  });

  const initLendingIx = await programs.lending.methods
    .initLending(args.symbol, JUPLEND_LIQUIDITY_PROGRAM_ID)
    .accounts({
      signer: args.admin.publicKey,
      lendingAdmin: poolKeys.lendingAdmin,
      tokenReservesLiquidity: args.tokenReserve,
      tokenProgram: args.tokenProgram,
    })
    .accountsPartial({
      mint: args.mint,
      lending: poolKeys.lending,
      fTokenMint: poolKeys.fTokenMint,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(initLendingIx),
    [args.admin],
    false,
    true,
  );

  const setRewardsIx = await programs.lending.methods
    .setRewardsRateModel(args.mint)
    .accounts({
      signer: args.admin.publicKey,
      lendingAdmin: poolKeys.lendingAdmin,
      lending: poolKeys.lending,
      newRewardsRateModel: args.lendingRewardsRateModel,
      supplyTokenReservesLiquidity: args.tokenReserve,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(setRewardsIx),
    [args.admin],
    false,
    true,
  );

  const lendingState = await programs.lending.account.lending.fetch(
    poolKeys.lending,
  );
  assertKeysEqual(lendingState.mint, args.mint);
  assert.equal(lendingState.decimals, args.decimals);
  assertKeysEqual(lendingState.tokenReservesLiquidity, args.tokenReserve);
  assertKeysEqual(lendingState.rewardsRateModel, args.lendingRewardsRateModel);
}

async function initJuplendLiquidityPositions(args: {
  admin: Keypair;
  mint: PublicKey;
  tokenProgram: PublicKey;
  lending: PublicKey;
  programs?: JuplendPrograms;
}): Promise<void> {
  const programs = args.programs ?? getJuplendPrograms();
  const { authList } = deriveJuplendGlobalKeys();
  const poolKeys = deriveJuplendPoolKeys({
    mint: args.mint,
    tokenProgram: args.tokenProgram,
  });
  assertKeysEqual(poolKeys.lending, args.lending);

  const ix = await programs.liquidity.methods
    .initNewProtocol(args.mint, args.mint, args.lending)
    .accounts({
      authority: args.admin.publicKey,
      authList,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [args.admin],
    false,
    true,
  );

  const supplyPos = await programs.liquidity.account.userSupplyPosition.fetch(
    poolKeys.supplyPositionOnLiquidity,
  );
  assertKeysEqual(supplyPos.protocol, args.lending);
  assertKeysEqual(supplyPos.mint, args.mint);

  const borrowPos = await programs.liquidity.account.userBorrowPosition.fetch(
    poolKeys.borrowPositionOnLiquidity,
  );
  assertKeysEqual(borrowPos.protocol, args.lending);
  assertKeysEqual(borrowPos.mint, args.mint);
}

export async function configureJuplendProtocolPermissions(args: {
  admin: Keypair;
  mint: PublicKey;
  lending: PublicKey;
  rateModel: PublicKey;
  tokenReserve: PublicKey;
  supplyPositionOnLiquidity: PublicKey;
  borrowPositionOnLiquidity: PublicKey;
  tokenProgram: PublicKey;
  supplyConfig?: JuplendSupplyConfig;
  borrowConfig?: JuplendBorrowConfig;
  userClassEntries?: JuplendUserClassEntry[];
  programs?: JuplendPrograms;
}): Promise<void> {
  const programs = args.programs ?? getJuplendPrograms();
  const supplyConfig = args.supplyConfig ?? DEFAULT_SUPPLY_CONFIG;
  const borrowConfig = args.borrowConfig ?? DEFAULT_BORROW_CONFIG;
  const userClassEntries = args.userClassEntries ?? [
    { addr: args.lending, value: 1 },
  ];

  const { authList } = deriveJuplendGlobalKeys();

  const supplyIx = await programs.liquidity.methods
    .updateUserSupplyConfig({
      mode: supplyConfig.mode,
      expandPercent: supplyConfig.expandPercent,
      expandDuration: supplyConfig.expandDuration,
      baseWithdrawalLimit: supplyConfig.baseWithdrawalLimit,
    })
    .accounts({
      authority: args.admin.publicKey,
      authList,
      rateModel: args.rateModel,
      tokenReserve: args.tokenReserve,
      userSupplyPosition: args.supplyPositionOnLiquidity,
    })
    .instruction();

  await assertDebtCeilingIsSupported({
    mint: args.mint,
    tokenProgram: args.tokenProgram,
    maxDebtCeiling: borrowConfig.maxDebtCeiling,
  });

  const borrowIx = await programs.liquidity.methods
    .updateUserBorrowConfig({
      mode: borrowConfig.mode,
      expandPercent: borrowConfig.expandPercent,
      expandDuration: borrowConfig.expandDuration,
      baseDebtCeiling: borrowConfig.baseDebtCeiling,
      maxDebtCeiling: borrowConfig.maxDebtCeiling,
    })
    .accounts({
      authority: args.admin.publicKey,
      authList,
      rateModel: args.rateModel,
      tokenReserve: args.tokenReserve,
      userBorrowPosition: args.borrowPositionOnLiquidity,
    })
    .instruction();

  const userClassIx = await programs.liquidity.methods
    .updateUserClass(userClassEntries)
    .accounts({
      authority: args.admin.publicKey,
      authList,
    })
    .instruction();

  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(supplyIx, borrowIx, userClassIx),
    [args.admin],
    false,
    true,
  );

  const supplyPos = await programs.liquidity.account.userSupplyPosition.fetch(
    args.supplyPositionOnLiquidity,
  );
  assert.equal(supplyPos.withInterest, supplyConfig.mode);
  assert.equal(supplyPos.expandPct, supplyConfig.expandPercent.toNumber());
  assertBNEqual(toBn(supplyPos.expandDuration), supplyConfig.expandDuration);
  assertBNEqual(
    toBn(supplyPos.baseWithdrawalLimit),
    supplyConfig.baseWithdrawalLimit,
  );

  const borrowPos = await programs.liquidity.account.userBorrowPosition.fetch(
    args.borrowPositionOnLiquidity,
  );
  assert.equal(borrowPos.withInterest, borrowConfig.mode);
  assert.equal(borrowPos.expandPct, borrowConfig.expandPercent.toNumber());
  assert.equal(
    Number(borrowPos.expandDuration),
    borrowConfig.expandDuration.toNumber(),
  );
  assertBNEqual(borrowPos.baseDebtCeiling, borrowConfig.baseDebtCeiling);
  assertBNEqual(toBn(borrowPos.maxDebtCeiling), borrowConfig.maxDebtCeiling);

  const authListAcc = await programs.liquidity.account.authorizationList.fetch(
    authList,
  );
  const userClass = authListAcc.userClasses.find((entry: any) =>
    entry.addr.equals(args.lending),
  );
  assert.ok(userClass, "missing user class entry");
  assert.equal(
    userClass.class,
    userClassEntries.find((entry) => entry.addr.equals(args.lending))?.value,
  );
}

export async function initJuplendPool(args: {
  admin: Keypair;
  mint: PublicKey;
  symbol: string;
  decimals: number;
  rateConfig?: JuplendRateConfig;
  tokenConfig?: JuplendTokenConfig;
  programs?: JuplendPrograms;
}): Promise<JuplendPoolKeys> {
  const programs = args.programs ?? getJuplendPrograms();
  const tokenProgram = await getTokenProgramForMint(args.mint);

  const keys = deriveJuplendPoolKeys({
    mint: args.mint,
    tokenProgram,
  });

  await initJuplendLiquidityForMint({
    admin: args.admin,
    mint: args.mint,
    tokenProgram,
    rateConfig: args.rateConfig,
    tokenConfig: args.tokenConfig,
    programs,
  });

  const rewardsRateModel = await initJuplendRewardsRateModel({
    admin: args.admin,
    mint: args.mint,
    programs,
  });

  await initJuplendLendingForMint({
    admin: args.admin,
    mint: args.mint,
    symbol: args.symbol,
    decimals: args.decimals,
    tokenProgram,
    tokenReserve: keys.tokenReserve,
    lendingRewardsRateModel: rewardsRateModel,
    programs,
  });

  await initJuplendLiquidityPositions({
    admin: args.admin,
    mint: args.mint,
    tokenProgram,
    lending: keys.lending,
    programs,
  });

  return keys;
}
