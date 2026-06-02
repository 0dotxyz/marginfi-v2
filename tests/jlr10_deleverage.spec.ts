import { BN } from "@coral-xyz/anchor";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { Clock } from "solana-bankrun";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";

import {
  bankRunProvider,
  banksClient,
  bankrunContext,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  juplendAccounts,
  oracles,
  riskAdmin,
  users,
} from "./rootHooks";
import { groupConfigure } from "./utils/group-instructions";
import { assertBankrunTxFailed, getTokenBalance } from "./utils/genericTests";
import {
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  accountInit,
  borrowIx,
  depositIx,
  endDeleverageIx,
  initLiquidationRecordIx,
  repayIx,
  startDeleverageIx,
} from "./utils/user-instructions";
import {
  mintToTokenAccount,
  processBankrunTransaction,
  dumpBankrunLogs,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { dummyIx } from "./utils/bankrunConnection";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
} from "./utils/juplend/juplend-pdas";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import { initJuplendClaimAccountIx } from "./utils/juplend/admin-instructions";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { ONE_WEEK_IN_SECONDS } from "./utils/types";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";

const BORROWER_ACCOUNT_SEED = Buffer.from("JLR10_BORROWER_ACCOUNT_SEED_0000");
const ADMIN_ACCOUNT_SEED = Buffer.from("JLR10_ADMIN_ACCOUNT_SEED_0000000");
const borrowerMarginfiAccount = Keypair.fromSeed(BORROWER_ACCOUNT_SEED);
const adminMarginfiAccount = Keypair.fromSeed(ADMIN_ACCOUNT_SEED);

// Reuse zb02-style amounts so assertions can remain almost identical.
const USER_DEPOSIT_USDC = new BN(100 * 10 ** ecosystem.usdcDecimals);
const USER_BORROW_USDC = new BN(70 * 10 ** ecosystem.usdcDecimals);
const DELEVERAGE_WITHDRAWN = new BN(2000);
const DELEVERAGE_REPAID = new BN(5 * 10 ** ecosystem.usdcDecimals);

describe("jlr10: JupLend deleverage allowlist + flow (bankrun)", () => {
  let borrower: (typeof users)[number];

  let groupPk = PublicKey.default;
  let juplendUsdcBankPk = PublicKey.default;
  let regularUsdcBankPk = PublicKey.default;
  let jupUsdcPool = deriveJuplendPoolKeys({
    mint: ecosystem.usdcMint.publicKey,
  });
  let juplendPrograms: ReturnType<typeof getJuplendPrograms>;

  const requireStateKey = (key: string): PublicKey => {
    const value = juplendAccounts.get(key);
    if (!value) {
      throw new Error(`missing juplend test state key: ${key}`);
    }
    return value;
  };

  const remainingGroups = (): PublicKey[][] => [
    [juplendUsdcBankPk, oracles.usdcOracle.publicKey, jupUsdcPool.lending],
    [regularUsdcBankPk, oracles.usdcOracle.publicKey],
  ];
  const refreshAllOracles = async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  };

  const deleverageTx = async (args: {
    withdrawAmount: BN;
    repayAmount: BN;
    prependUpdateRate?: boolean;
  }) => {
    const groups = remainingGroups();
    const instructions = [];

    if (args.prependUpdateRate) {
      instructions.push(
        await refreshJupSimple(juplendPrograms.lending, { pool: jupUsdcPool })
      );
    }

    instructions.push(
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(groups),
      })
    );

    instructions.push(
      await makeJuplendWithdrawSimpleIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        destinationTokenAccount: riskAdmin.usdcAccount,
        bank: juplendUsdcBankPk,
        pool: jupUsdcPool,
        amount: args.withdrawAmount,
        withdrawAll: false,
        remainingAccounts: composeRemainingAccounts(groups),
      }),
      await repayIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        bank: regularUsdcBankPk,
        tokenAccount: riskAdmin.usdcAccount,
        amount: args.repayAmount,
        remaining: composeRemainingAccounts(groups),
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        remaining: composeRemainingAccountsMetaBanksOnly(groups),
      })
    );

    return new Transaction().add(...instructions);
  };

  before(async () => {
    juplendPrograms = getJuplendPrograms();
    borrower = users[2];

    groupPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01Group);
    juplendUsdcBankPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    regularUsdcBankPk = requireStateKey(
      JUPLEND_STATE_KEYS.jlr01RegularBankUsdc
    );
    jupUsdcPool = deriveJuplendPoolKeys({ mint: ecosystem.usdcMint.publicKey });

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      juplendUsdcBankPk
    );
    const withdrawIntermediaryAta = getAssociatedTokenAddressSync(
      jupUsdcPool.mint,
      liquidityVaultAuthority,
      true,
      jupUsdcPool.tokenProgram
    );
    const [claimAccount] = findJuplendClaimAccountPda(
      liquidityVaultAuthority,
      jupUsdcPool.mint
    );
    const claimAccountInfo = await bankRunProvider.connection.getAccountInfo(
      claimAccount
    );
    const initIntegrationIxs = [
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        withdrawIntermediaryAta,
        liquidityVaultAuthority,
        jupUsdcPool.mint,
        jupUsdcPool.tokenProgram
      ),
    ];
    if (!claimAccountInfo) {
      initIntegrationIxs.push(
        await initJuplendClaimAccountIx(juplendPrograms, {
          signer: groupAdmin.wallet.publicKey,
          mint: jupUsdcPool.mint,
          accountFor: liquidityVaultAuthority,
          claimAccount,
        })
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...initIntegrationIxs),
      [groupAdmin.wallet],
      false,
      true
    );

    // Configure risk admin on this shared Juplend test group.
    const configureRiskAdminIx = await groupConfigure(
      groupAdmin.mrgnBankrunProgram!,
      {
        marginfiGroup: groupPk,
        newRiskAdmin: riskAdmin.wallet.publicKey,
      }
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(configureRiskAdminIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Init borrower + admin marginfi accounts in the Juplend group for this spec.
    const initBorrowerIx = await accountInit(borrower.mrgnBankrunProgram!, {
      marginfiGroup: groupPk,
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      authority: borrower.wallet.publicKey,
      feePayer: borrower.wallet.publicKey,
    });
    const initAdminIx = await accountInit(groupAdmin.mrgnBankrunProgram!, {
      marginfiGroup: groupPk,
      marginfiAccount: adminMarginfiAccount.publicKey,
      authority: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initBorrowerIx, initAdminIx),
      [
        borrower.wallet,
        groupAdmin.wallet,
        borrowerMarginfiAccount,
        adminMarginfiAccount,
      ],
      false,
      true
    );

    // Fund borrower and admin with USDC for setup actions.
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      borrower.usdcAccount,
      USER_DEPOSIT_USDC.mul(new BN(2))
    );
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      groupAdmin.usdcAccount,
      USER_DEPOSIT_USDC.mul(new BN(2))
    );
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      riskAdmin.usdcAccount,
      USER_DEPOSIT_USDC
    );

    // Seed regular USDC bank liquidity (zb02-style setup).
    const seedRegularLiquidityIx = await depositIx(
      groupAdmin.mrgnBankrunProgram!,
      {
        marginfiAccount: adminMarginfiAccount.publicKey,
        bank: regularUsdcBankPk,
        tokenAccount: groupAdmin.usdcAccount,
        amount: USER_DEPOSIT_USDC,
        depositUpToLimit: false,
      }
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(seedRegularLiquidityIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Borrower deposits to Juplend bank, then borrows from regular USDC bank.
    await refreshAllOracles();
    const borrowerDepositJupIx = await makeJuplendDepositIx(
      borrower.mrgnBankrunProgram!,
      {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        signerTokenAccount: borrower.usdcAccount,
        bank: juplendUsdcBankPk,
        pool: jupUsdcPool,
        amount: USER_DEPOSIT_USDC,
      }
    );
    const borrowerBorrowRegularIx = await borrowIx(
      borrower.mrgnBankrunProgram!,
      {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        bank: regularUsdcBankPk,
        tokenAccount: borrower.usdcAccount,
        remaining: composeRemainingAccounts(remainingGroups()),
        amount: USER_BORROW_USDC,
      }
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(borrowerDepositJupIx),
      [borrower.wallet],
      false,
      true
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(juplendPrograms.lending, { pool: jupUsdcPool }),
        borrowerBorrowRegularIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey)
      ),
      [borrower.wallet],
      false,
      true
    );

    const initLiqRecordIx = await initLiquidationRecordIx(
      riskAdmin.mrgnBankrunProgram!,
      {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        feePayer: riskAdmin.wallet.publicKey,
      }
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initLiqRecordIx),
      [riskAdmin.wallet],
      false,
      true
    );
  });

  it("fails deleverage on stale Juplend lending without update_rate, succeeds when update_rate is included", async () => {
    const currentClock = await banksClient.getClock();
    const newTimestamp =
      currentClock.unixTimestamp + BigInt(ONE_WEEK_IN_SECONDS);
    const slotsToAdvance = ONE_WEEK_IN_SECONDS * 0.4;
    const newClock = new Clock(
      currentClock.slot + BigInt(slotsToAdvance),
      0n,
      currentClock.epoch,
      currentClock.epochStartTimestamp,
      newTimestamp
    );
    bankrunContext.setClock(newClock);

    // Keep Pyth data fresh so the failure is specifically Juplend stale-lending.
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const staleTx = new Transaction().add(
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(remainingGroups()),
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        remaining: composeRemainingAccountsMetaBanksOnly(remainingGroups()),
      })
    );
    const staleResult = await processBankrunTransaction(
      bankrunContext,
      staleTx,
      [riskAdmin.wallet],
      true,
      false
    );
    assertBankrunTxFailed(staleResult, 6504);

    const allowlistedTx = new Transaction().add(
      await refreshJupSimple(juplendPrograms.lending, { pool: jupUsdcPool }),
      await startDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        riskAdmin: riskAdmin.wallet.publicKey,
        remaining: composeRemainingAccountsWriteableMeta(remainingGroups()),
      }),
      await endDeleverageIx(riskAdmin.mrgnBankrunProgram!, {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        remaining: composeRemainingAccountsMetaBanksOnly(remainingGroups()),
      })
    );
    const allowlistedResult = await processBankrunTransaction(
      bankrunContext,
      allowlistedTx,
      [riskAdmin.wallet],
      true,
      false
    );

    // trySend=true => success has `result === null`
    assert(
      "result" in allowlistedResult && !allowlistedResult.result,
      "expected start/end deleverage to succeed with update_rate in tx"
    );
  });

  it("runs full Juplend deleverage flow (start + update_rate + withdraw + repay + end) and yields expected admin delta", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const borrowerBefore = await bankrunProgram.account.marginfiAccount.fetch(
      borrowerMarginfiAccount.publicKey
    );
    const regularBalanceBefore = borrowerBefore.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(regularUsdcBankPk)
    );
    assert.ok(regularBalanceBefore, "missing borrower regular-usdc balance");
    const liabSharesBefore = wrappedI80F48toBigNumber(
      regularBalanceBefore!.liabilityShares
    ).toNumber();

    const usdcBefore = await getTokenBalance(
      bankRunProvider,
      riskAdmin.usdcAccount
    );

    const tx = await deleverageTx({
      withdrawAmount: DELEVERAGE_WITHDRAWN,
      repayAmount: DELEVERAGE_REPAID,
      prependUpdateRate: true,
    });

    const result = await processBankrunTransaction(
      bankrunContext,
      tx,
      [riskAdmin.wallet],
      true,
      false
    );
    if ("result" in result && result.result) {
      dumpBankrunLogs(result);
      assert.fail("full deleverage tx failed");
    }

    const usdcAfter = await getTokenBalance(
      bankRunProvider,
      riskAdmin.usdcAccount
    );
    assert.equal(
      usdcAfter - usdcBefore,
      DELEVERAGE_WITHDRAWN.toNumber() - DELEVERAGE_REPAID.toNumber()
    );

    const borrowerAfter = await bankrunProgram.account.marginfiAccount.fetch(
      borrowerMarginfiAccount.publicKey
    );
    const regularBalanceAfter = borrowerAfter.lendingAccount.balances.find(
      (b) => b.active && b.bankPk.equals(regularUsdcBankPk)
    );
    assert.ok(
      regularBalanceAfter,
      "missing borrower regular-usdc balance after"
    );
    const liabSharesAfter = wrappedI80F48toBigNumber(
      regularBalanceAfter!.liabilityShares
    ).toNumber();
    assert.ok(
      liabSharesAfter < liabSharesBefore,
      "liability shares should decline"
    );
  });
});
