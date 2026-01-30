import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Transaction,
  ComputeBudgetProgram,
  Keypair,
  SystemProgram,
} from "@solana/web3.js";
import {
  createMintToInstruction,
  createAssociatedTokenAccountIdempotentInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import BN from "bn.js";
import { Marginfi } from "../target/types/marginfi";
import {
  groupAdmin,
  oracles,
  ecosystem,
  globalProgramAdmin,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  users,
  solendAccounts,
  SOLEND_USDC_RESERVE,
  SOLEND_MARKET,
  SOLEND_USDC_COLLATERAL_MINT,
} from "./rootHooks";
import { MockUser } from "./utils/mocks";
import {
  liquidateIx,
  healthPulse,
  depositIx,
  borrowIx,
  accountInit,
} from "./utils/user-instructions";
import { configureBank } from "./utils/group-instructions";
import {
  makeAddSolendBankIx,
  makeSolendInitObligationIx as makeInitObligationIx,
  makeSolendDepositIx,
  makeSolendWithdrawIx,
} from "./utils/solend-instructions";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { genericMultiBankTestSetup } from "./genericSetups";
import { blankBankConfigOptRaw } from "./utils/types";
import { defaultSolendBankConfig } from "./utils/solend-utils";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { processBankrunTransaction as processBankrunTx } from "./utils/tools";
import { getTokenBalance } from "./utils/genericTests";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { BanksTransactionResultWithMeta } from "solana-bankrun";
import { assert } from "chai";

describe("sl07: Solend Liquidation", () => {
  let program: Program<Marginfi>;

  before(() => {
    program = bankrunProgram;
  });

  const startingSeed = 7;
  const USER_ACCOUNT_THROWAWAY = "throwaway_account_sl07";
  const seedAmountLst = new BN(5 * 10 ** ecosystem.lstAlphaDecimals);
  const depositAmountUsdc = new BN(1250 * 10 ** ecosystem.usdcDecimals);
  const borrowAmountLst = new BN(5 * 10 ** ecosystem.lstAlphaDecimals);
  const liquidateAmountUsdc = new BN(0.1 * 10 ** ecosystem.usdcDecimals);

  const THROWAWAY_GROUP_SEED_SL07 = Buffer.from(
    "throwaway_group_sl07_00000000000"
  );

  let banks: PublicKey[] = [];
  let throwawayGroup: { publicKey: PublicKey };
  let solendUsdcBank: PublicKey;
  let user: MockUser;
  let liquidator: MockUser;
  let userAccount: PublicKey;
  let liquidatorAccount: PublicKey;

  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      2,
      USER_ACCOUNT_THROWAWAY,
      THROWAWAY_GROUP_SEED_SL07,
      startingSeed
    );

    banks = result.banks;
    throwawayGroup = result.throwawayGroup;

    [solendUsdcBank] = deriveBankWithSeed(
      program.programId,
      throwawayGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      new BN(startingSeed + 1)
    );
  });

  it("(admin) init solend USDC bank", async () => {
    const fundAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        fundAmount.toNumber()
      )
    );
    await processBankrunTx(bankrunContext, fundTx, [globalProgramAdmin.wallet]);

    const tx = new Transaction().add(
      await makeAddSolendBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: solendAccounts.get(SOLEND_USDC_RESERVE)!,
          solendMarket: solendAccounts.get(SOLEND_MARKET)!,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultSolendBankConfig(oracles.usdcOracle.publicKey),
          seed: new BN(startingSeed + 1),
        }
      )
    );

    await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);

    const collateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT)!;

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      program.programId,
      solendUsdcBank
    );

    const userCollateral = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );

    const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
      groupAdmin.wallet.publicKey,
      userCollateral,
      liquidityVaultAuthority,
      collateralMint,
      TOKEN_PROGRAM_ID
    );

    const initObligationTx = new Transaction().add(
      createAtaIx,
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank: solendUsdcBank,
          signerTokenAccount: groupAdmin.usdcAccount,
          lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
          pythPrice: oracles.usdcOracleFeed.publicKey,
        },
        {
          amount: new BN(1000),
        }
      )
    );

    await processBankrunTx(bankrunContext, initObligationTx, [
      groupAdmin.wallet,
    ]);
  });

  it("(admin) Seeds liquidity in all banks", async () => {
    const adminAccountKeypair = Keypair.generate();
    const adminAccount = adminAccountKeypair.publicKey;

    const tx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        marginfiAccount: adminAccount,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );
    await processBankrunTx(bankrunContext, tx, [
      groupAdmin.wallet,
      adminAccountKeypair,
    ]);

    const fundAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        groupAdmin.lstAlphaAccount,
        globalProgramAdmin.wallet.publicKey,
        fundAmount.toNumber()
      )
    );
    await processBankrunTx(bankrunContext, fundTx, [globalProgramAdmin.wallet]);

    const depositsPerTx = 5;
    for (let i = 0; i < banks.length; i += depositsPerTx) {
      const chunk = banks.slice(i, i + depositsPerTx);
      const tx = new Transaction();

      for (const bank of chunk) {
        tx.add(
          await depositIx(groupAdmin.mrgnBankrunProgram, {
            marginfiAccount: adminAccount,
            bank,
            tokenAccount: groupAdmin.lstAlphaAccount,
            amount: seedAmountLst,
            depositUpToLimit: false,
          })
        );
      }

      await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);
    }
  });

  it("(user 0) deposits USDC into Solend, borrows LST from bank[0]", async () => {
    user = users[0];
    liquidator = users[1];

    const userAccountKeypair = Keypair.generate();
    userAccount = userAccountKeypair.publicKey;

    const liquidatorAccountKeypair = Keypair.generate();
    liquidatorAccount = liquidatorAccountKeypair.publicKey;

    let userTx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTx(bankrunContext, userTx, [
      user.wallet,
      userAccountKeypair,
    ]);

    let liquidatorTx = new Transaction().add(
      await accountInit(liquidator.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        marginfiAccount: liquidatorAccount,
        authority: liquidator.wallet.publicKey,
        feePayer: liquidator.wallet.publicKey,
      })
    );
    await processBankrunTx(bankrunContext, liquidatorTx, [
      liquidator.wallet,
      liquidatorAccountKeypair,
    ]);

    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        user.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        depositAmountUsdc.toNumber()
      )
    );
    await processBankrunTx(bankrunContext, fundTx, [globalProgramAdmin.wallet]);

    const collateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT)!;
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      program.programId,
      solendUsdcBank
    );
    const userCollateral = getAssociatedTokenAddressSync(
      collateralMint,
      liquidityVaultAuthority,
      true
    );
    const createUserCollateralIx =
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        userCollateral,
        liquidityVaultAuthority,
        collateralMint,
        TOKEN_PROGRAM_ID
      );

    let depositTx = new Transaction().add(
      createUserCollateralIx,
      await makeSolendDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: solendUsdcBank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
          pythPrice: oracles.usdcOracle.publicKey,
        },
        {
          amount: depositAmountUsdc,
        }
      )
    );
    await processBankrunTx(bankrunContext, depositTx, [user.wallet]);

    let borrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        amount: borrowAmountLst,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(bankrunContext, borrowTx, [user.wallet]);

    let healthTx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(bankrunContext, healthTx, [user.wallet]);
  });

  it("(admin) increase bank[0] liability ratio to make user 0 unhealthy", async () => {
    let tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      }),
      // Dummy transfer to differentiate from previous test's identical health pulse tx
      SystemProgram.transfer({
        fromPubkey: user.wallet.publicKey,
        toPubkey: groupAdmin.wallet.publicKey,
        lamports: 2,
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);

    let liabilityConfig = blankBankConfigOptRaw();
    liabilityConfig.liabilityWeightInit = bigNumberToWrappedI80F48(100.0);
    liabilityConfig.liabilityWeightMaint = bigNumberToWrappedI80F48(80.0);

    let assetConfig = blankBankConfigOptRaw();
    assetConfig.assetWeightInit = bigNumberToWrappedI80F48(0.01);
    assetConfig.assetWeightMaint = bigNumberToWrappedI80F48(0.01);

    tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        bankConfigOpt: liabilityConfig,
      }),
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: solendUsdcBank,
        bankConfigOpt: assetConfig,
      })
    );
    await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);

    tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      }),
      // Dummy transfer to differentiate from previous identical health pulse tx
      SystemProgram.transfer({
        fromPubkey: user.wallet.publicKey,
        toPubkey: groupAdmin.wallet.publicKey,
        lamports: 1,
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);

    const account = await user.mrgnBankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = account.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cacheAfter.assetValueMaint);
    const liabilityValue = wrappedI80F48toBigNumber(
      cacheAfter.liabilityValueMaint
    );
    const health =
      parseFloat(assetValue.toString()) - parseFloat(liabilityValue.toString());
    assert.ok(
      health <= 0,
      `User should be unhealthy, but health is ${health.toFixed(2)}`
    );
  });

  it("(user 1) Liquidates user 0", async () => {
    const liquidatorFundAmount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    const fundTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        liquidator.lstAlphaAccount,
        globalProgramAdmin.wallet.publicKey,
        liquidatorFundAmount.toNumber()
      )
    );
    await processBankrunTx(bankrunContext, fundTx, [globalProgramAdmin.wallet]);

    let tx = new Transaction().add(
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: banks[0],
        tokenAccount: liquidator.lstAlphaAccount,
        amount: liquidatorFundAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(bankrunContext, tx, [liquidator.wallet]);

    const liquidateeBefore =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);

    let liquidationCount = 0;
    let totalLiquidated = new BN(0);
    const maxLiquidations = 200;

    while (liquidationCount < maxLiquidations) {
      const liquidateTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
        await liquidateIx(liquidator.mrgnBankrunProgram, {
          assetBankKey: solendUsdcBank,
          liabilityBankKey: banks[0],
          liquidatorMarginfiAccount: liquidatorAccount,
          liquidateeMarginfiAccount: userAccount,
          remaining: [
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
            oracles.pythPullLst.publicKey,

            ...composeRemainingAccounts([
              [banks[0], oracles.pythPullLst.publicKey],
              [
                solendUsdcBank,
                oracles.usdcOracle.publicKey,
                solendAccounts.get(SOLEND_USDC_RESERVE)!,
              ],
            ]),

            ...composeRemainingAccounts([
              [banks[0], oracles.pythPullLst.publicKey],
              [
                solendUsdcBank,
                oracles.usdcOracle.publicKey,
                solendAccounts.get(SOLEND_USDC_RESERVE)!,
              ],
            ]),
          ],
          amount: liquidateAmountUsdc,
          liquidateeAccounts: 5,
          liquidatorAccounts: 5,
        })
      );

      const result = (await processBankrunTx(
        bankrunContext,
        liquidateTx,
        [liquidator.wallet],
        true,
        false
      )) as BanksTransactionResultWithMeta;

      if (result.result && result.meta && result.meta.logMessages) {
        const hasTooSevereError = result.meta.logMessages.some(
          (log: any) =>
            log.includes("custom program error: 0x17b7") ||
            log.includes("TooSevereLiquidation")
        );

        const hasExhaustedLiabilityError = result.meta.logMessages.some(
          (log: any) =>
            log.includes("custom program error: 0x17b5") ||
            log.includes("ExhaustedLiability")
        );

        if (hasTooSevereError || hasExhaustedLiabilityError) {
          break;
        }
      }

      liquidationCount++;
      totalLiquidated = totalLiquidated.add(liquidateAmountUsdc);
    }

    tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);

    const liquidateeAfter =
      await user.mrgnBankrunProgram.account.marginfiAccount.fetch(userAccount);
    const liquidatorAfter =
      await liquidator.mrgnBankrunProgram.account.marginfiAccount.fetch(
        liquidatorAccount
      );

    assert.ok(
      liquidationCount > 20,
      `Should have done more than 20 liquidations, but only did ${liquidationCount}`
    );

    assert.ok(
      liquidationCount <= 200,
      `Liquidation count should not exceed 200, but was ${liquidationCount}`
    );

    const liquidateeSolendBalBefore =
      liquidateeBefore.lendingAccount.balances.find((b: any) =>
        b.bankPk.equals(solendUsdcBank)
      );
    const liquidateeSolendBalAfter =
      liquidateeAfter.lendingAccount.balances.find((b: any) =>
        b.bankPk.equals(solendUsdcBank)
      );

    if (liquidateeSolendBalBefore && liquidateeSolendBalAfter) {
      const beforeShares = wrappedI80F48toBigNumber(
        liquidateeSolendBalBefore.assetShares
      );
      const afterShares = wrappedI80F48toBigNumber(
        liquidateeSolendBalAfter.assetShares
      );
      assert.ok(
        new BN(afterShares.toString()).lt(new BN(beforeShares.toString()))
      );
    }

    const liquidatorSolendBalAfter =
      liquidatorAfter.lendingAccount.balances.find((b: any) =>
        b.bankPk.equals(solendUsdcBank)
      );
    if (liquidatorSolendBalAfter) {
      const liquidatorShares = wrappedI80F48toBigNumber(
        liquidatorSolendBalAfter.assetShares
      );
      assert.ok(new BN(liquidatorShares.toString()).gt(new BN(0)));
    }
  });

  it("(admin) restore bank 0 default liability ratios", async () => {
    let config = blankBankConfigOptRaw();
    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        bankConfigOpt: config,
      })
    );
    await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);

    tx = new Transaction().add(
      // dummy ix to trick bankrun
      SystemProgram.transfer({
        fromPubkey: user.wallet.publicKey,
        toPubkey: groupAdmin.wallet.publicKey,
        lamports: 42,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [
            solendUsdcBank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);
  });

  it("(user 1) Liquidator withdraws USDC from Solend bank", async () => {
    const liquidatorUsdcBefore = await getTokenBalance(
      bankRunProvider,
      liquidator.usdcAccount
    );

    const liquidatorAccountData =
      await liquidator.mrgnBankrunProgram.account.marginfiAccount.fetch(
        liquidatorAccount
      );
    const liquidatorSolendBalance =
      liquidatorAccountData.lendingAccount.balances.find(
        (b: any) => b.active === 1 && b.bankPk.equals(solendUsdcBank)
      );

    if (liquidatorSolendBalance) {
      const assetShares = wrappedI80F48toBigNumber(
        liquidatorSolendBalance.assetShares
      );
      const withdrawAmount = new BN(assetShares.toString()).div(new BN(2));

      if (withdrawAmount.gt(new BN(0))) {
        const withdrawTx = new Transaction()
          .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }))
          .add(
            await makeSolendWithdrawIx(
              liquidator.mrgnBankrunProgram,
              {
                marginfiAccount: liquidatorAccount,
                bank: solendUsdcBank,
                destinationTokenAccount: liquidator.usdcAccount,
                lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
                pythPrice: oracles.usdcOracle.publicKey,
              },
              {
                amount: withdrawAmount,
                withdrawAll: false,
                remaining: composeRemainingAccounts([
                  [
                    solendUsdcBank,
                    oracles.usdcOracle.publicKey,
                    solendAccounts.get(SOLEND_USDC_RESERVE)!,
                  ],
                  [banks[0], oracles.pythPullLst.publicKey],
                ]),
              }
            )
          );

        await processBankrunTx(bankrunContext, withdrawTx, [liquidator.wallet]);

        const liquidatorUsdcAfter = await getTokenBalance(
          bankRunProvider,
          liquidator.usdcAccount
        );
        assert.ok(liquidatorUsdcAfter > liquidatorUsdcBefore);
      }
    }
  });
});
