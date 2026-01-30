import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";
import {
  ecosystem,
  groupAdmin,
  globalProgramAdmin,
  driftAccounts,
  DRIFT_TOKEN_A_SPOT_MARKET,
  DRIFT_TOKEN_A_PULL_ORACLE,
  oracles,
  verbose,
  bankrunContext,
  bankrunProgram,
  driftBankrunProgram,
  users,
  THROWAWAY_GROUP_SEED_D09,
} from "./rootHooks";
import {
  depositIx,
  borrowIx,
  liquidateIx,
  healthPulse,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import {
  processBankrunTransaction as processBankrunTx,
  logHealthCache,
  dumpAccBalances,
} from "./utils/tools";
import { BanksTransactionResultWithMeta } from "solana-bankrun";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { blankBankConfigOptRaw, CONF_INTERVAL_MULTIPLE } from "./utils/types";
import { configureBank } from "./utils/group-instructions";
import {
  defaultDriftBankConfig,
  getDriftUserAccount,
  formatTokenAmount,
  formatDepositAmount,
  getSpotMarketAccount,
  tokenAmountToScaledBalance,
  TOKEN_A_MARKET_INDEX,
  assertBankBalance,
  TOKEN_A_INIT_DEPOSIT_AMOUNT,
  TOKEN_A_SCALING_FACTOR,
} from "./utils/drift-utils";
import {
  makeAddDriftBankIx,
  makeDriftDepositIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";
import { genericMultiBankTestSetup } from "./genericSetups";
import { deriveBankWithSeed } from "./utils/pdas";
import { ProgramTestContext } from "solana-bankrun";
import { assertBNEqual } from "./utils/genericTests";

const confidenceInterval = 0.01 * CONF_INTERVAL_MULTIPLE;

const startingSeed: number = 10;
const USER_ACCOUNT_THROWAWAY_D = "throwaway_account_d";
let ctx: ProgramTestContext;

let throwawayGroup: Keypair;
let regularLstBank: PublicKey;
let driftTokenABank: PublicKey;
let mrgnID: PublicKey;

const seedAmountLst = new BN(50 * 10 ** ecosystem.lstAlphaDecimals);
const depositAmountTokenA = new BN(100 * 10 ** ecosystem.tokenADecimals);
const borrowAmountLst = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);
const depositAmountLst = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);

describe("d09: Drift Liquidation", () => {
  before(async () => {
    ctx = bankrunContext;
    mrgnID = bankrunProgram.programId;
  });

  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_THROWAWAY_D,
      THROWAWAY_GROUP_SEED_D09,
      startingSeed
    );
    regularLstBank = result.banks[0];
    throwawayGroup = result.throwawayGroup;

    [driftTokenABank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      new BN(startingSeed).addn(1)
    );
  });

  it("(admin) init drift Token A bank", async () => {
    const driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);

    let defaultConfig = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

    let tx = new Transaction().add(
      await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: driftSpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: new BN(startingSeed).addn(1),
        }
      )
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);

    const fundAdminTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        TOKEN_A_INIT_DEPOSIT_AMOUNT.toNumber()
      )
    );
    await processBankrunTx(ctx, fundAdminTx, [globalProgramAdmin.wallet]);

    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          bank: driftTokenABank,
          signerTokenAccount: groupAdmin.tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
        },
        {
          amount: TOKEN_A_INIT_DEPOSIT_AMOUNT,
        },
        1
      )
    );
    await processBankrunTx(ctx, initUserTx, [groupAdmin.wallet]);
  });

  it("(admin) Seeds liquidity in a regular LST bank", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY_D);

    const tx = new Transaction();
    tx.add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: regularLstBank,
        tokenAccount: user.lstAlphaAccount,
        amount: seedAmountLst,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, tx, [user.wallet]);
  });

  it("(user 0) Deposits Token A into Drift, borrows LST from bank [0]", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY_D);
    const driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);

    if (verbose) {
      console.log(
        `\n💰 Depositing ${formatTokenAmount(
          depositAmountTokenA,
          ecosystem.tokenADecimals,
          "Token A"
        )} through Drift`
      );
    }
    const bank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const driftUserBefore = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const spotPositionBefore = driftUserBefore.spotPositions[1];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    let tx = new Transaction().add(
      await makeDriftDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: driftTokenABank,
          signerTokenAccount: user.tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
        },
        depositAmountTokenA,
        TOKEN_A_MARKET_INDEX
      ),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [driftTokenABank, oracles.tokenAOracle.publicKey, driftSpotMarket],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [user.wallet]);

    const driftUserAfter = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const spotPositionAfter = driftUserAfter.spotPositions[1];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;
    const scaledBalanceChange = scaledBalanceAfter.sub(scaledBalanceBefore);

    if (verbose) {
      console.log(
        `   Scaled balance change: ${formatDepositAmount(scaledBalanceChange)}`
      );
    }

    const accBefore = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheBefore = accBefore.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache before borrow", cacheBefore);
    }

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: regularLstBank,
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [driftTokenABank, oracles.tokenAOracle.publicKey, driftSpotMarket],
          [regularLstBank, oracles.pythPullLst.publicKey],
        ]),
        amount: borrowAmountLst,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [driftTokenABank, oracles.tokenAOracle.publicKey, driftSpotMarket],
          [regularLstBank, oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [user.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = accAfter.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache after borrow", cacheAfter);
    }
  });

  it("(admin) Increases LST bank liability ratio to make user 0 just unhealthy", async () => {
    // weird thing to debug with a slop machine
    // const to = bigNumberToWrappedI80F48(995317139.49974820595);
    // const to = bigNumberToWrappedI80F48(995317139);
    // const back = new BN(wrappedI80F48toBigNumber(to).toString());

    let config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(6.0); // 600%
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(5.5); // 550%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: regularLstBank,
        bankConfigOpt: config,
      })
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);

    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY_D);
    const driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
    tx = new Transaction().add(
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts([
          [driftTokenABank, oracles.tokenAOracle.publicKey, driftSpotMarket],
          [regularLstBank, oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [liquidatee.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    const cacheAfter = accAfter.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache after liability rate raised ", cacheAfter);

      const assetValue = wrappedI80F48toBigNumber(cacheAfter.assetValueMaint);
      const liabilityValue = wrappedI80F48toBigNumber(
        cacheAfter.liabilityValueMaint
      );
      const health =
        parseFloat(assetValue.toString()) -
        parseFloat(liabilityValue.toString());
      console.log(
        `   Health calculation: ${assetValue} - ${liabilityValue} = ${health.toFixed(
          2
        )}`
      );
      console.log(
        `   Account is ${
          health > 0 ? "HEALTHY" : "UNHEALTHY"
        } (health: ${health.toFixed(2)})`
      );
    }
  });

  it("(user 1) Liquidates user 0", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY_D);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY_D);
    const driftSpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);

    const spotMarket = await getSpotMarketAccount(
      driftBankrunProgram,
      TOKEN_A_MARKET_INDEX
    );
    let tx = new Transaction().add(
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: regularLstBank,
        tokenAccount: liquidator.lstAlphaAccount,
        amount: depositAmountLst,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, tx, [liquidator.wallet]);

    let totalLiquidated = new BN(0);
    let liquidationCount = 0;
    const smallLiquidationAmount = new BN(
      0.01 * 10 ** ecosystem.tokenADecimals
    );
    const scaledLiquidateAmount = tokenAmountToScaledBalance(
      smallLiquidationAmount,
      spotMarket
    );

    const driftBank = await bankrunProgram.account.bank.fetch(driftTokenABank);
    const driftUserBefore = await getDriftUserAccount(
      driftBankrunProgram,
      driftBank.integrationAcc2
    );
    const spotPositionBefore = driftUserBefore.spotPositions[1];
    const scaledBalanceBefore = spotPositionBefore.scaledBalance;

    const liquidateeAccBefore =
      await bankrunProgram.account.marginfiAccount.fetch(liquidateeAccount);
    const liquidateeTokenABalanceBefore =
      liquidateeAccBefore.lendingAccount.balances.find(
        (b) => b.bankPk.equals(driftTokenABank) && b.active === 1
      );
    const liquidateeAssetSharesBefore = new BN(
      wrappedI80F48toBigNumber(
        liquidateeTokenABalanceBefore.assetShares
      ).toString()
    );

    while (true) {
      const liquidateTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
        // dummy ix to trick bankrun
        SystemProgram.transfer({
          fromPubkey: liquidator.wallet.publicKey,
          toPubkey: bankrunProgram.provider.publicKey,
          lamports: 42 + liquidationCount,
        }),
        await liquidateIx(liquidator.mrgnBankrunProgram, {
          assetBankKey: driftTokenABank,
          liabilityBankKey: regularLstBank,
          liquidatorMarginfiAccount: liquidatorAccount,
          liquidateeMarginfiAccount: liquidateeAccount,
          remaining: [
            oracles.tokenAOracle.publicKey, // asset oracle
            driftSpotMarket, // asset spot market
            oracles.pythPullLst.publicKey, // liab oracle

            ...composeRemainingAccounts([
              [regularLstBank, oracles.pythPullLst.publicKey],
              [
                driftTokenABank,
                oracles.tokenAOracle.publicKey,
                driftSpotMarket,
              ],
            ]),
            ...composeRemainingAccounts([
              [regularLstBank, oracles.pythPullLst.publicKey],
              [
                driftTokenABank,
                oracles.tokenAOracle.publicKey,
                driftSpotMarket,
              ],
            ]),
          ],
          amount: scaledLiquidateAmount,
          liquidateeAccounts: 5,
          liquidatorAccounts: 5,
        })
      );

      const result = (await processBankrunTx(
        ctx,
        liquidateTx,
        [liquidator.wallet],
        true,
        true
      )) as BanksTransactionResultWithMeta;
      if (result.result && result.meta && result.meta.logMessages) {
        const hasTooSevereError = result.meta.logMessages.some(
          (log) =>
            log.includes("custom program error: 0x17b7") ||
            log.includes("TooSevereLiquidation")
        );

        if (hasTooSevereError) {
          if (liquidationCount === 0) {
            console.error("\n❌ First liquidation attempt failed!");
            console.error("Error logs:");
            result.meta.logMessages.forEach((log) => console.error("  " + log));
            throw new Error(
              "First liquidation failed - check setup and amounts"
            );
          }
          if (verbose) {
            console.log(`\n📊 Liquidation Loop Summary:`);
            console.log(`   Total liquidations: ${liquidationCount}`);
            console.log(
              `   Total amount liquidated: ${formatTokenAmount(
                totalLiquidated,
                ecosystem.tokenADecimals,
                "Token A"
              )}`
            );
          }
          break;
        }
      }

      liquidationCount++;
      totalLiquidated = totalLiquidated.add(smallLiquidationAmount);
      console.log("liquidationCount: ", liquidationCount);
      console.log("\ntotalLiquidated: ", totalLiquidated.toString());

      if (liquidationCount >= 50) {
        if (verbose) {
          console.log(
            "\n⚠️  Safety limit reached (50 liquidations) - stopping loop"
          );
        }
        assert(false);
      }
    }

    const healthTx = new Transaction().add(
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts([
          [driftTokenABank, oracles.tokenAOracle.publicKey, driftSpotMarket],
          [regularLstBank, oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(ctx, healthTx, [liquidatee.wallet]);

    const liquidateeAccAfter =
      await bankrunProgram.account.marginfiAccount.fetch(liquidateeAccount);
    const liquidatorAccAfter =
      await bankrunProgram.account.marginfiAccount.fetch(liquidatorAccount);

    if (verbose) {
      logHealthCache(
        "user 0 cache post-liquidate ",
        liquidateeAccAfter.healthCache
      );
      console.log("\n📊 Final State After All Liquidations:");

      console.log("Liquidatee final balances:");
      dumpAccBalances(liquidateeAccAfter);
      console.log("\nLiquidator final balances:");
      dumpAccBalances(liquidatorAccAfter);
    }

    const driftUser = await getDriftUserAccount(
      driftBankrunProgram,
      driftBank.integrationAcc2
    );
    const spotPositionAfter = driftUser.spotPositions[1];
    const scaledBalanceAfter = spotPositionAfter.scaledBalance;

    // Nore: the amount of deposited tokens A is still the same, only the ownership changed (for some)
    assertBNEqual(scaledBalanceBefore, scaledBalanceAfter);

    await assertBankBalance(
      liquidateeAccount,
      driftTokenABank,
      liquidateeAssetSharesBefore.sub(
        totalLiquidated.mul(TOKEN_A_SCALING_FACTOR)
      )
    );

    await assertBankBalance(
      liquidatorAccount,
      driftTokenABank,
      totalLiquidated.mul(TOKEN_A_SCALING_FACTOR)
    );

    const tokenALowPrice = ecosystem.tokenAPrice * (1 - confidenceInterval);
    const lstHighPrice = ecosystem.lstAlphaPrice * (1 + confidenceInterval);
    const totalLiquidatedLst =
      ((totalLiquidated.toNumber() * tokenALowPrice) / lstHighPrice) *
      10 ** (ecosystem.lstAlphaDecimals - ecosystem.tokenADecimals);

    const liquidateeReceived = new BN(totalLiquidatedLst * 0.95);
    const liquidatorPaid = new BN(totalLiquidatedLst * 0.975);

    await assertBankBalance(
      liquidateeAccount,
      regularLstBank,
      borrowAmountLst.sub(liquidateeReceived).toNumber(),
      true // liability
    );

    await assertBankBalance(
      liquidatorAccount,
      regularLstBank,
      depositAmountLst.sub(liquidatorPaid).toNumber()
    );

    if (verbose) {
      console.log(`   Performed ${liquidationCount} liquidations`);
      console.log(
        `   Total liquidated: ${formatTokenAmount(
          totalLiquidated,
          ecosystem.tokenADecimals,
          "Token A"
        )}`
      );
    }
  });
});
