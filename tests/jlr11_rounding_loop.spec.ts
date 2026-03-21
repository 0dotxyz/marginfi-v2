import { BN } from "@coral-xyz/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";

import {
  bankRunProvider,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import { getTokenBalance } from "./utils/genericTests";
import { accountInit } from "./utils/user-instructions";
import { configureJuplendProtocolPermissions } from "./utils/juplend/jlr-pool-setup";
import {
  buildHealthRemainingAccounts,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { dummyIx } from "./utils/bankrunConnection";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
  findJuplendLiquidityBorrowPositionPda,
  findJuplendLiquiditySupplyPositionPda,
} from "./utils/juplend/juplend-pdas";
import {
  makeJuplendDepositIx,
  makeJuplendNativeBorrowIx,
  makeJuplendNativePreOperateIx,
} from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import { initJuplendClaimAccountIx } from "./utils/juplend/admin-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";
import { EXCHANGE_PRICES_PRECISION } from "./utils/juplend/constants";

const USER_ACCOUNT_SEED = Buffer.from("JLR11_ROUNDING_USER_SEED_0000000");
const roundingUserMarginfiAccount = Keypair.fromSeed(USER_ACCOUNT_SEED);

const LOOP_ITERATIONS = 25;
const SEARCH_MAX_AMOUNT = 2_000_000n;
const SEARCH_STEP = 1n;
const MIN_LOOP_SHARES = 10n;
const FUND_USDC = new BN(100 * 10 ** ecosystem.usdcDecimals);
const BOOTSTRAP_WARP_SECONDS = 3_255;
const UTILIZATION_BORROW_USDC = new BN(500_051);

type RoundingProbe = {
  amount: bigint;
  shares: bigint;
  redeem: bigint;
  loss: bigint;
};

const P = BigInt(EXCHANGE_PRICES_PRECISION);

const previewSharesForDeposit = (
  assets: bigint,
  liquidityExchangePrice: bigint,
  tokenExchangePrice: bigint,
): bigint => {
  const registeredRaw = (assets * P) / liquidityExchangePrice;
  const registered = (registeredRaw * liquidityExchangePrice) / P;
  return (registered * P) / tokenExchangePrice;
};

const previewAssetsForRedeem = (
  shares: bigint,
  tokenExchangePrice: bigint,
): bigint => {
  return (shares * tokenExchangePrice) / P;
};

const findFirstPositiveLossAmount = (
  tokenExchangePrice: bigint,
  liquidityExchangePrice: bigint,
): RoundingProbe | null => {
  for (let amount = 1n; amount <= SEARCH_MAX_AMOUNT; amount += SEARCH_STEP) {
    const shares = previewSharesForDeposit(
      amount,
      liquidityExchangePrice,
      tokenExchangePrice,
    );
    if (shares < MIN_LOOP_SHARES) continue;

    const redeem = previewAssetsForRedeem(shares, tokenExchangePrice);
    const loss = amount - redeem;

    if (loss > 0n) {
      return { amount, shares, redeem, loss };
    }
  }

  return null;
};

describe("jlr11: JupLend rounding loop (bankrun)", () => {
  let user: (typeof users)[number];
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;

  let juplendUsdcBankPk = PublicKey.default;
  let jupUsdcPool = deriveJuplendPoolKeys({
    mint: ecosystem.usdcMint.publicKey,
  });

  let perLoopDeposit = new BN(0);
  let perLoopExpectedRedeem = 0n;
  let perLoopExpectedLoss = 0n;

  const requireStateKey = (key: string): PublicKey => {
    const value = juplendAccounts.get(key);
    if (!value) {
      throw new Error(`missing juplend test state key: ${key}`);
    }
    return value;
  };

  const fetchExchangePrices = async () => {
    const lending = await juplendPrograms.lending.account.lending.fetch(
      jupUsdcPool.lending,
    );

    return {
      tokenExchangePrice: BigInt(lending.tokenExchangePrice.toString()),
      liquidityExchangePrice: BigInt(lending.liquidityExchangePrice.toString()),
    };
  };

  const bootstrapNonIntegerExchangePrice = async () => {
    const protocol = groupAdmin.wallet.publicKey;
    const [supplyPosition] = findJuplendLiquiditySupplyPositionPda(
      jupUsdcPool.mint,
      protocol,
    );
    const [borrowPosition] = findJuplendLiquidityBorrowPositionPda(
      jupUsdcPool.mint,
      protocol,
    );

    const [supplyInfo, borrowInfo] = await Promise.all([
      bankRunProvider.connection.getAccountInfo(supplyPosition),
      bankRunProvider.connection.getAccountInfo(borrowPosition),
    ]);
    if (!supplyInfo || !borrowInfo) {
      const setupBorrowerIxs = [
        await juplendPrograms.liquidity.methods
          .initNewProtocol(jupUsdcPool.mint, jupUsdcPool.mint, protocol)
          .accounts({
            authority: groupAdmin.wallet.publicKey,
            authList: jupUsdcPool.authList,
            userSupplyPosition: supplyPosition,
            userBorrowPosition: borrowPosition,
            systemProgram: SystemProgram.programId,
          })
          .instruction(),
      ];
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(...setupBorrowerIxs),
        [groupAdmin.wallet],
        false,
        true,
      );

      await configureJuplendProtocolPermissions({
        admin: groupAdmin.wallet,
        mint: jupUsdcPool.mint,
        lending: protocol,
        rateModel: jupUsdcPool.rateModel,
        tokenReserve: jupUsdcPool.tokenReserve,
        supplyPositionOnLiquidity: supplyPosition,
        borrowPositionOnLiquidity: borrowPosition,
        tokenProgram: jupUsdcPool.tokenProgram,
        borrowConfig: {
          mode: 1,
          expandPercent: new BN(20).mul(new BN(100)),
          expandDuration: new BN(2 * 24 * 60 * 60),
          baseDebtCeiling: new BN(100_000_000),
          maxDebtCeiling: new BN(1_000_000_000),
        },
        programs: juplendPrograms,
      });
    }

    const createUtilizationIxs = await Promise.all([
      makeJuplendNativePreOperateIx(juplendPrograms.liquidity, {
        protocol,
        mint: jupUsdcPool.mint,
        pool: jupUsdcPool,
        userSupplyPosition: supplyPosition,
        userBorrowPosition: borrowPosition,
      }),
      makeJuplendNativeBorrowIx(juplendPrograms.liquidity, {
        protocol,
        pool: jupUsdcPool,
        userSupplyPosition: supplyPosition,
        userBorrowPosition: borrowPosition,
        borrowTo: groupAdmin.wallet.publicKey,
        borrowAmount: UTILIZATION_BORROW_USDC,
      }),
    ]);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...createUtilizationIxs),
      [groupAdmin.wallet],
      false,
      true,
    );

    const clock = await banksClient.getClock();
    bankrunContext.setClock(
      new Clock(
        clock.slot + 1n,
        0n,
        clock.epoch,
        clock.epochStartTimestamp,
        clock.unixTimestamp + BigInt(BOOTSTRAP_WARP_SECONDS),
      ),
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(juplendPrograms.lending, { pool: jupUsdcPool }),
        dummyIx(groupAdmin.wallet.publicKey, user.wallet.publicKey),
      ),
      [groupAdmin.wallet],
      false,
      true,
    );
  };

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    user = users[2];

    juplendUsdcBankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    jupUsdcPool = deriveJuplendPoolKeys({ mint: ecosystem.usdcMint.publicKey });

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      juplendUsdcBankPk,
    );
    const withdrawIntermediaryAta = getAssociatedTokenAddressSync(
      jupUsdcPool.mint,
      liquidityVaultAuthority,
      true,
      jupUsdcPool.tokenProgram,
    );
    const [claimAccount] = findJuplendClaimAccountPda(
      liquidityVaultAuthority,
      jupUsdcPool.mint,
    );

    const claimInfo = await bankRunProvider.connection.getAccountInfo(
      claimAccount,
    );
    const integrationSetupIxs = [
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        jupUsdcPool.mint,
        jupUsdcPool.tokenProgram,
      ),
    ];
    if (!claimInfo) {
      integrationSetupIxs.push(
        await initJuplendClaimAccountIx(juplendPrograms, {
          signer: groupAdmin.wallet.publicKey,
          mint: jupUsdcPool.mint,
          accountFor: liquidityVaultAuthority,
          claimAccount,
        }),
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...integrationSetupIxs),
      [groupAdmin.wallet],
      false,
      true,
    );

    const initUserIx = await accountInit(user.mrgnBankrunProgram!, {
      marginfiGroup: requireStateKey(JUPLEND_STATE_KEYS.jlr01Group),
      marginfiAccount: roundingUserMarginfiAccount.publicKey,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initUserIx),
      [user.wallet, roundingUserMarginfiAccount],
      false,
      true,
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      user.usdcAccount,
      FUND_USDC,
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(juplendPrograms.lending, { pool: jupUsdcPool }),
        dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [user.wallet],
      false,
      true,
    );

    let prices = await fetchExchangePrices();
    let probe = findFirstPositiveLossAmount(
      prices.tokenExchangePrice,
      prices.liquidityExchangePrice,
    );
    if (!probe) {
      await bootstrapNonIntegerExchangePrice();
      prices = await fetchExchangePrices();
      probe = findFirstPositiveLossAmount(
        prices.tokenExchangePrice,
        prices.liquidityExchangePrice,
      );
    }

    assert.ok(
      probe,
      `could not find a positive rounding-loss case up to amount=${SEARCH_MAX_AMOUNT.toString()}`,
    );
    assert.isTrue(probe!.loss > 0n, "expected a positive per-round-trip loss");

    perLoopDeposit = new BN(probe!.amount.toString());
    perLoopExpectedRedeem = probe!.redeem;
    perLoopExpectedLoss = probe!.loss;
  });

  it("loops tiny deposit/withdraw-all cycles and applies a consistent rounding loss per cycle with stable prices", async () => {
    const beforePrices = await fetchExchangePrices();
    const bankBefore = await bankrunProgram.account.bank.fetch(juplendUsdcBankPk);
    const userStart = BigInt(await getTokenBalance(bankRunProvider, user.usdcAccount));

    for (let i = 0; i < LOOP_ITERATIONS; i++) {
      const cycleStart = BigInt(
        await getTokenBalance(bankRunProvider, user.usdcAccount),
      );

      const depositIx = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
        marginfiAccount: roundingUserMarginfiAccount.publicKey,
        signerTokenAccount: user.usdcAccount,
        bank: juplendUsdcBankPk,
        pool: jupUsdcPool,
        amount: perLoopDeposit,
      });
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          depositIx,
          dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
        ),
        [user.wallet],
        false,
        true,
      );

      const remaining = await buildHealthRemainingAccounts(
        roundingUserMarginfiAccount.publicKey,
      );
      const withdrawAllIx = await makeJuplendWithdrawSimpleIx(
        user.mrgnBankrunProgram!,
        {
          marginfiAccount: roundingUserMarginfiAccount.publicKey,
          destinationTokenAccount: user.usdcAccount,
          bank: juplendUsdcBankPk,
          pool: jupUsdcPool,
          amount: new BN(0),
          withdrawAll: true,
          remainingAccounts: remaining,
        },
      );
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          withdrawAllIx,
          dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
        ),
        [user.wallet],
        false,
        true,
      );

      const cycleEnd = BigInt(await getTokenBalance(bankRunProvider, user.usdcAccount));
      const cycleLoss = cycleStart - cycleEnd;
      assert.equal(
        cycleLoss.toString(),
        perLoopExpectedLoss.toString(),
        `cycle ${i} should lose exactly ${perLoopExpectedLoss.toString()} lamports`,
      );

      const cycleRedeem =
        cycleEnd - (cycleStart - BigInt(perLoopDeposit.toString()));
      assert.equal(
        cycleRedeem.toString(),
        perLoopExpectedRedeem.toString(),
        `cycle ${i} should redeem exact floor(shares*price/1e12)`,
      );

      const marginfiAccount = await bankrunProgram.account.marginfiAccount.fetch(
        roundingUserMarginfiAccount.publicKey,
      );
      const activeBalance = marginfiAccount.lendingAccount.balances.find(
        (b) => b.active && b.bankPk.equals(juplendUsdcBankPk),
      );
      assert.isUndefined(activeBalance, `cycle ${i} should close the bank balance`);

      const pricesNow = await fetchExchangePrices();
      assert.equal(
        pricesNow.tokenExchangePrice.toString(),
        beforePrices.tokenExchangePrice.toString(),
        `cycle ${i} token exchange price drifted`,
      );
      assert.equal(
        pricesNow.liquidityExchangePrice.toString(),
        beforePrices.liquidityExchangePrice.toString(),
        `cycle ${i} liquidity exchange price drifted`,
      );
    }

    const userEnd = BigInt(await getTokenBalance(bankRunProvider, user.usdcAccount));
    const expectedTotalLoss = BigInt(LOOP_ITERATIONS) * perLoopExpectedLoss;
    assert.equal(
      (userStart - userEnd).toString(),
      expectedTotalLoss.toString(),
      "total rounding loss should be exactly per-cycle loss times iteration count",
    );

    const bankAfter = await bankrunProgram.account.bank.fetch(juplendUsdcBankPk);
    assert.equal(
      bankAfter.assetShareValue.toString(),
      bankBefore.assetShareValue.toString(),
      "asset share value should remain unchanged (no interest accrual in loop)",
    );
    assert.equal(
      bankAfter.liabilityShareValue.toString(),
      bankBefore.liabilityShareValue.toString(),
      "liability share value should remain unchanged (no interest accrual in loop)",
    );
  });
});
