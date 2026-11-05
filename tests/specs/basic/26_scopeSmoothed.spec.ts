import { BN, Program } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import { bnToBigIntSafe } from "../../utils/bn-utils";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankIxs,
  configureBankOracle,
  configureBankOracleScope,
  groupInitialize,
} from "../../utils/group-instructions";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsMetaBanksOnly,
  composeRemainingAccountsWriteableMeta,
  depositIx,
  endLiquidationIx,
  healthPulse,
  initLiquidationRecordIx,
  pulseBankPrice,
  repayIx,
  startLiquidationIx,
  withdrawIx,
} from "../../utils/user-instructions";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "../../rootHooks";
import { expectFailedTxWithError } from "../../utils/genericTests";
import {
  blankBankConfigOptRaw,
  defaultBankConfig,
  ORACLE_SETUP_PYTH_PUSH,
  ORACLE_SETUP_SCOPE,
} from "../../utils/types";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";
import {
  bytesToF64,
  getBankrunTime,
  processBankrunTransaction,
} from "../../utils/tools";

const smoothedGroup = Keypair.generate();
const scopeBank = Keypair.generate();
const debtBank = Keypair.generate();
const feed = Keypair.generate().publicKey;

/** Entry 13 = SOL spot 175.0, entry 113 = its EMA 140.0. */
const SPOT = { index: 13, value: 17_500_000_000n, exp: 8n };
const EMA = { index: 113, value: 14_000_000_000n, exp: 8n };

let program: Program<Marginfi>;

describe("Scope smoothed entry", () => {
  // Assigned in `before`: the root hooks populate users and oracles first.
  let borrower: (typeof users)[number];
  let liquidator: (typeof users)[number];
  let debtGroup: PublicKey[];
  const borrowerAcc = Keypair.generate();
  const liquidatorAcc = Keypair.generate();
  const wsol = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);
  const usdc = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const scopeGroup = [scopeBank.publicKey, feed];

  /** Fresh spot entry; the smoothed entry `emaAge` seconds old. */
  const setFeed = async (emaAge = 0) => {
    const now = await getBankrunTime(bankrunContext);
    setScopeFeed(
      bankrunContext,
      feed,
      makeScopePrices([
        { ...SPOT, timestamp: now },
        { ...EMA, timestamp: now - emaAge },
      ])
    );
  };

  const configureSmoothed = async (smoothedEntryIndex = EMA.index) =>
    processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBankOracleScope(groupAdmin.mrgnBankrunProgram, {
          type: ORACLE_SETUP_SCOPE,
          bank: scopeBank.publicKey,
          oracle: feed,
          entryIndex: SPOT.index,
          smoothedEntryIndex,
        })
      ),
      [groupAdmin.wallet]
    );

  const mintTo = (mint: PublicKey, account: PublicKey, amount: BN) =>
    processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        createMintToInstruction(
          mint,
          account,
          globalProgramAdmin.wallet.publicKey,
          bnToBigIntSafe(amount)
        )
      ),
      [globalProgramAdmin.wallet]
    );

  before(async () => {
    program = bankrunProgram;
    borrower = users[0];
    liquidator = users[2];
    debtGroup = [debtBank.publicKey, oracles.usdcOracle.publicKey];
    await setFeed();

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: smoothedGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      ),
      [groupAdmin.wallet, smoothedGroup]
    );
    for (const [bank, mint] of [
      [scopeBank, ecosystem.wsolMint.publicKey],
      [debtBank, ecosystem.usdcMint.publicKey],
    ] as const) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await addBank(groupAdmin.mrgnBankrunProgram, {
            marginfiGroup: smoothedGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            bank: bank.publicKey,
            config: defaultBankConfig(),
          })
        ),
        [groupAdmin.wallet, bank]
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await configureBankOracle(groupAdmin.mrgnBankrunProgram, {
          bank: debtBank.publicKey,
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: oracles.usdcOracle.publicKey,
        })
      ),
      [groupAdmin.wallet]
    );

    await mintTo(ecosystem.wsolMint.publicKey, borrower.wsolAccount, wsol(2));
    await mintTo(
      ecosystem.usdcMint.publicKey,
      liquidator.usdcAccount,
      usdc(200)
    );
    for (const [user, acc] of [
      [borrower, borrowerAcc],
      [liquidator, liquidatorAcc],
    ] as const) {
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          await accountInit(user.mrgnBankrunProgram, {
            marginfiGroup: smoothedGroup.publicKey,
            marginfiAccount: acc.publicKey,
            authority: user.wallet.publicKey,
            feePayer: user.wallet.publicKey,
          })
        ),
        [user.wallet, acc]
      );
    }
    // The liquidator's USDC funds the borrow.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: liquidatorAcc.publicKey,
          bank: debtBank.publicKey,
          tokenAccount: liquidator.usdcAccount,
          amount: usdc(100),
        })
      ),
      [liquidator.wallet]
    );
  });

  it("(admin) rejects a smoothed entry of 0 or the price entry itself", async () => {
    for (const smoothedEntryIndex of [0, SPOT.index]) {
      await expectFailedTxWithError(
        async () => {
          await configureSmoothed(smoothedEntryIndex);
        },
        "ScopeInvalidSmoothedEntry",
        6804
      );
    }
  });

  it("(admin) sets the smoothed entry - initial margin reads it, liquidation reads spot", async () => {
    await configureSmoothed();
    const { config } = await program.account.bank.fetch(scopeBank.publicKey);
    assert.equal(config.scopeEntryIndex, SPOT.index);
    assert.equal(config.scopeSmoothedEntryIndex, EMA.index);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction()
        .add(
          await depositIx(borrower.mrgnBankrunProgram, {
            marginfiAccount: borrowerAcc.publicKey,
            bank: scopeBank.publicKey,
            tokenAccount: borrower.wsolAccount,
            amount: wsol(2),
          })
        )
        .add(
          await initLiquidationRecordIx(borrower.mrgnBankrunProgram, {
            marginfiAccount: borrowerAcc.publicKey,
            feePayer: borrower.wallet.publicKey,
          })
        )
        .add(
          await healthPulse(borrower.mrgnBankrunProgram, {
            marginfiAccount: borrowerAcc.publicKey,
            remaining: composeRemainingAccounts([scopeGroup]),
          })
        ),
      [borrower.wallet]
    );
    const { healthCache } = await program.account.marginfiAccount.fetch(
      borrowerAcc.publicKey
    );
    assert.equal(healthCache.mrgnErr, 0);
    // Weights are 1: initial values 2 SOL at the EMA, maintenance at spot.
    assert.approximately(bytesToF64(healthCache.prices[0]), 140, 1e-9);
    assert.approximately(
      wrappedI80F48toBigNumber(healthCache.assetValue).toNumber(),
      280,
      1e-6
    );
    assert.approximately(
      wrappedI80F48toBigNumber(healthCache.assetValueMaint).toNumber(),
      350,
      1e-6
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await pulseBankPrice(groupAdmin.mrgnBankrunProgram, {
          bank: scopeBank.publicKey,
          remaining: [feed],
        })
      ),
      [groupAdmin.wallet]
    );
    const bank = await program.account.bank.fetch(scopeBank.publicKey);
    assert.approximately(
      wrappedI80F48toBigNumber(bank.cache.lastOraclePrice).toNumber(),
      175,
      1e-9
    );
  });

  it("a stale smoothed slot blocks pricing entirely until it is refreshed", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    await setFeed();
    const borrow = async (amount: BN) =>
      processBankrunTransaction(
        bankrunContext,
        new Transaction().add(
          ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
          await borrowIx(borrower.mrgnBankrunProgram, {
            marginfiAccount: borrowerAcc.publicKey,
            bank: debtBank.publicKey,
            tokenAccount: borrower.usdcAccount,
            remaining: composeRemainingAccounts([scopeGroup, debtGroup]),
            amount,
          })
        ),
        [borrower.wallet]
      );
    await borrow(usdc(50));

    // Cut the collateral weights so the borrower is liquidatable...
    const weights = blankBankConfigOptRaw();
    weights.assetWeightInit = bigNumberToWrappedI80F48(0.01);
    weights.assetWeightMaint = bigNumberToWrappedI80F48(0.02);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ...(await configureBankIxs(groupAdmin.mrgnBankrunProgram, {
          bank: scopeBank.publicKey,
          bankConfigOpt: weights,
        }))
      ),
      [groupAdmin.wallet]
    );
    // ...then let the smoothed slot go stale while spot stays fresh: like a stale Pyth update,
    // nothing prices off this bank until it is refreshed - not a borrow, not a liquidation.
    await setFeed(10_000);
    const groups = [scopeGroup, debtGroup];

    // The collateral bank cannot be priced, so the risk engine rejects the borrow (6009), the
    // same way it does for a stale Pyth feed.
    await expectFailedTxWithError(
      async () => {
        await borrow(usdc(1));
      },
      "RiskEngineInitRejected",
      6009
    );
    await expectFailedTxWithError(
      async () => {
        await processBankrunTransaction(
          bankrunContext,
          new Transaction().add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
            await startLiquidationIx(liquidator.mrgnBankrunProgram, {
              marginfiAccount: borrowerAcc.publicKey,
              liquidationReceiver: liquidator.wallet.publicKey,
              remaining: composeRemainingAccountsWriteableMeta(groups),
            })
          ),
          [liquidator.wallet]
        );
      },
      "ScopeStalePrice",
      6802
    );

    // Refreshed: the receivership goes through.
    await setFeed();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
        await startLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: borrowerAcc.publicKey,
          liquidationReceiver: liquidator.wallet.publicKey,
          remaining: composeRemainingAccountsWriteableMeta(groups),
        }),
        await withdrawIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: borrowerAcc.publicKey,
          bank: scopeBank.publicKey,
          tokenAccount: liquidator.wsolAccount,
          remaining: composeRemainingAccounts(groups),
          amount: wsol(0.04),
        }),
        await repayIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: borrowerAcc.publicKey,
          bank: debtBank.publicKey,
          tokenAccount: liquidator.usdcAccount,
          amount: usdc(7.5),
        }),
        await endLiquidationIx(liquidator.mrgnBankrunProgram, {
          marginfiAccount: borrowerAcc.publicKey,
          remaining: composeRemainingAccountsMetaBanksOnly(groups),
        })
      ),
      [liquidator.wallet]
    );

    const after = await program.account.marginfiAccount.fetch(
      borrowerAcc.publicKey
    );
    const scopeBalance = after.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(scopeBank.publicKey)
    );
    assert.approximately(
      wrappedI80F48toBigNumber(scopeBalance!.assetShares).toNumber(),
      wsol(1.96).toNumber(),
      1
    );
  });
});
