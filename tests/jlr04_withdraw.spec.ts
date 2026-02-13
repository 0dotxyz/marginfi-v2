import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  juplendAccounts,
  users,
} from "./rootHooks";
import {
  assertBNEqual,
  assertBNGreaterThan,
  assertI80F48Equal,
  getTokenBalance,
} from "./utils/genericTests";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";
import {
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
} from "./utils/juplend/juplend-pdas";
import { getJuplendPrograms } from "./utils/juplend/programs";
import type { JuplendPoolKeys } from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import { makeJuplendWithdrawIx } from "./utils/juplend/user-instructions";
import { processBankrunTransaction } from "./utils/tools";

const USER_WITHDRAW_AMOUNT = new BN(10 * 10 ** ecosystem.usdcDecimals);

describe("jlr04: JupLend withdraws (bankrun)", () => {
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;
  let user = users[0];

  let usdcJupBankPk = PublicKey.default;
  let userMarginfiAccountPk = PublicKey.default;
  let usdcJupPool: JuplendPoolKeys;
  let liquidityVaultPk = PublicKey.default;
  let liquidityVaultAuthorityPk = PublicKey.default;
  let fTokenVaultPk = PublicKey.default;
  let withdrawIntermediaryAtaPk = PublicKey.default;
  let claimAccountPk = PublicKey.default;
  let healthRemainingAccounts: PublicKey[] = [];

  before(async () => {
    user = users[0];
    juplendPrograms = getJuplendPrograms();
    usdcJupBankPk = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    userMarginfiAccountPk = juplendAccounts.get(
      JUPLEND_STATE_KEYS.jlr02User0MarginfiAccount,
    );

    const bank = await bankrunProgram.account.bank.fetch(usdcJupBankPk);
    usdcJupPool = deriveJuplendPoolKeys({ mint: bank.mint });
    liquidityVaultPk = bank.liquidityVault;
    fTokenVaultPk = bank.integrationAcc2;
    withdrawIntermediaryAtaPk = bank.integrationAcc3;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      usdcJupBankPk,
    );
    liquidityVaultAuthorityPk = liquidityVaultAuthority;
    const expectedWithdrawIntermediaryAta = getAssociatedTokenAddressSync(
      bank.mint,
      liquidityVaultAuthority,
      true,
      usdcJupPool.tokenProgram,
    );
    assert.equal(
      withdrawIntermediaryAtaPk.toBase58(),
      expectedWithdrawIntermediaryAta.toBase58(),
    );
    [claimAccountPk] = findJuplendClaimAccountPda(
      liquidityVaultAuthority,
      bank.mint,
      usdcJupPool.liquidityProgram,
    );

    healthRemainingAccounts = [
      usdcJupBankPk,
      bank.config.oracleKeys[0],
      bank.config.oracleKeys[1],
    ];
  });

  it("(user 0) withdraws from JupLend USDC bank - happy path", async () => {
    const [
      userUsdcBefore,
      liquidityVaultBefore,
      fTokenVaultBefore,
      jupReserveVaultBefore,
      lendingBefore,
      tokenReserveBefore,
      supplyPositionBefore,
      bankBefore,
      userAccountBefore,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVaultPk),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.lendingSupplyPositionOnLiquidity,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(userMarginfiAccountPk),
    ]);

    const withdrawIx = await makeJuplendWithdrawIx(user.mrgnBankrunProgram!, {
      marginfiAccount: userMarginfiAccountPk,
      destinationTokenAccount: user.usdcAccount,
      bank: usdcJupBankPk,
      withdrawIntermediaryAta: withdrawIntermediaryAtaPk,
      pool: usdcJupPool,
      claimAccount: claimAccountPk,
      amount: USER_WITHDRAW_AMOUNT,
      remainingAccounts: healthRemainingAccounts,
    });
    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        withdrawIntermediaryAtaPk,
        liquidityVaultAuthorityPk,
        usdcJupPool.mint,
        usdcJupPool.tokenProgram,
      );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx, withdrawIx),
      [user.wallet],
      false,
      true,
    );

    const [
      userUsdcAfter,
      liquidityVaultAfter,
      fTokenVaultAfter,
      jupReserveVaultAfter,
      lendingAfter,
      tokenReserveAfter,
      supplyPositionAfter,
      bankAfter,
      userAccountAfter,
    ] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVaultPk),
      getTokenBalance(bankRunProvider, fTokenVaultPk),
      getTokenBalance(bankRunProvider, usdcJupPool.vault),
      juplendPrograms.lending.account.lending.fetch(usdcJupPool.lending),
      juplendPrograms.liquidity.account.tokenReserve.fetch(
        usdcJupPool.tokenReserve,
      ),
      juplendPrograms.liquidity.account.userSupplyPosition.fetch(
        usdcJupPool.lendingSupplyPositionOnLiquidity,
      ),
      bankrunProgram.account.bank.fetch(usdcJupBankPk),
      bankrunProgram.account.marginfiAccount.fetch(userMarginfiAccountPk),
    ]);

    const userBalanceBefore = userAccountBefore.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(usdcJupBankPk),
    );
    const userBalanceAfter = userAccountAfter.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(usdcJupBankPk),
    );

    assert.ok(userBalanceBefore, "missing user balance before withdraw");
    assert.ok(userBalanceAfter, "missing user balance after withdraw");

    assert.equal(
      userUsdcAfter - userUsdcBefore,
      USER_WITHDRAW_AMOUNT.toNumber(),
    );
    assert.equal(liquidityVaultAfter, liquidityVaultBefore);
    assert.equal(
      jupReserveVaultBefore - jupReserveVaultAfter,
      USER_WITHDRAW_AMOUNT.toNumber(),
    );

    const burnedShares = new BN(fTokenVaultBefore - fTokenVaultAfter);
    assertBNGreaterThan(burnedShares, 0);

    const reserveRawSupplyDelta = tokenReserveBefore.totalSupplyWithInterest.sub(
      tokenReserveAfter.totalSupplyWithInterest,
    );
    const supplyPositionRawDelta = supplyPositionBefore.amount.sub(
      supplyPositionAfter.amount,
    );
    assertBNEqual(reserveRawSupplyDelta, supplyPositionRawDelta);
    assertBNEqual(supplyPositionRawDelta, burnedShares);
    assert.isTrue(
      reserveRawSupplyDelta.lte(USER_WITHDRAW_AMOUNT),
      `raw=${reserveRawSupplyDelta.toString()} assets=${USER_WITHDRAW_AMOUNT.toString()}`,
    );
    assertBNEqual(
      tokenReserveAfter.totalBorrowWithInterest,
      tokenReserveBefore.totalBorrowWithInterest,
    );

    assertBNEqual(
      lendingAfter.tokenExchangePrice,
      lendingBefore.tokenExchangePrice,
    );
    assertBNEqual(
      lendingAfter.liquidityExchangePrice,
      lendingBefore.liquidityExchangePrice,
    );

    const userAssetShareDelta = wrappedI80F48toBigNumber(
      userBalanceBefore.assetShares,
    ).minus(wrappedI80F48toBigNumber(userBalanceAfter.assetShares));
    assert.equal(userAssetShareDelta.toFixed(0), burnedShares.toString());

    const bankTotalAssetSharesDelta = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares,
    ).minus(wrappedI80F48toBigNumber(bankAfter.totalAssetShares));
    assert.equal(bankTotalAssetSharesDelta.toFixed(0), burnedShares.toString());

    assertI80F48Equal(bankAfter.assetShareValue, bankBefore.assetShareValue);
    assertI80F48Equal(
      bankAfter.liabilityShareValue,
      bankBefore.liabilityShareValue,
    );
  });
});
