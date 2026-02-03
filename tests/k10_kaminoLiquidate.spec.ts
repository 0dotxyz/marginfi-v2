import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  MARKET,
  oracles,
  USDC_RESERVE,
  verbose,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  users,
  THROWAWAY_GROUP_SEED_K10,
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
} from "./utils/tools";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { blankBankConfigOptRaw } from "./utils/types";
import { configureBank } from "./utils/group-instructions";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
} from "./utils/kamino-instructions";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { ProgramTestContext } from "solana-bankrun";

const startingSeed: number = 6;
const USER_ACCOUNT_THROWAWAY = "throwaway_account_k";
let ctx: ProgramTestContext;

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;
let kaminoUsdcBank: PublicKey;
let kaminoObligation: PublicKey;
let mrgnID: PublicKey;

const seedAmountLst = new BN(5 * 10 ** ecosystem.lstAlphaDecimals);
const depositAmountUsdc = new BN(1000 * 10 ** ecosystem.usdcDecimals);
const borrowAmountLst = new BN(3 * 10 ** ecosystem.lstAlphaDecimals);

describe("k10: Kamino Liquidation", () => {
  before(async () => {
    ctx = bankrunContext;
    mrgnID = bankrunProgram.programId;
  });

  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      2,
      USER_ACCOUNT_THROWAWAY,
      THROWAWAY_GROUP_SEED_K10,
      startingSeed
    );
    banks = result.banks;
    throwawayGroup = result.throwawayGroup;

    [kaminoUsdcBank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      new BN(startingSeed).addn(1)
    );
  });

  it("(admin) init kamino USDC bank", async () => {
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    let defaultConfig = defaultKaminoBankConfig(oracles.usdcOracle.publicKey);

    let tx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          kaminoReserve: usdcReserve,
          kaminoMarket: market,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: new BN(startingSeed).addn(1),
        }
      )
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);

    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      mrgnID,
      kaminoUsdcBank
    );
    [kaminoObligation] = deriveBaseObligation(liquidityVaultAuthority, market);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(groupAdmin.mrgnBankrunProgram, {
        feePayer: groupAdmin.wallet.publicKey,
        bank: kaminoUsdcBank,
        signerTokenAccount: groupAdmin.usdcAccount,
        lendingMarket: market,
        reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        pythOracle: oracles.usdcOracle.publicKey,
      })
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);
  });

  it("(admin) Seeds liquidity in all banks", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    // Note: This is about the max per TX without using LUTs.
    const depositsPerTx = 5;

    for (let i = 0; i < banks.length; i += depositsPerTx) {
      const chunk = banks.slice(i, i + depositsPerTx);
      const tx = new Transaction();
      let k = 0;
      for (const bank of chunk) {
        tx.add(
          await depositIx(user.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            bank,
            tokenAccount: user.lstAlphaAccount,
            amount: seedAmountLst,
            depositUpToLimit: false,
          })
        );
        if (verbose) {
          console.log(
            "seed bank " + k + " with liquidity " + seedAmountLst.toNumber()
          );
        }
        k++;
      }
      await processBankrunTx(ctx, tx, [user.wallet]);
    }
  });

  it("(user 0) deposits into Kamino USDC, borrows from bank [0]", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        kaminoObligation,
        [usdcReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: kaminoUsdcBank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        depositAmountUsdc
      ),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [user.wallet]);

    const accBefore = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheBefore = accBefore.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache before ", cacheBefore);
    }

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
        amount: borrowAmountLst,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [user.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = accAfter.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache after ", cacheAfter);
    }
  });

  it("(admin) vastly increase bank [0] liability ratio to make user 0 unhealthy", async () => {
    let config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(2.1); // 210%
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(2.0); // 200%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        bankConfigOpt: config,
      })
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);

    // Observe that user 0 is now unhealthy
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tx = new Transaction().add(
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [banks[0], oracles.pythPullLst.publicKey],
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
    }
  });

  it("(user 1) Liquidates user 0", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const depositAmount = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);
    const liquidateAmount = new BN(5 * 10 ** ecosystem.usdcDecimals);

    // Deposit some funds to operate as a liquidator...
    let tx = new Transaction().add(
      await depositIx(liquidator.mrgnBankrunProgram, {
        marginfiAccount: liquidatorAccount,
        bank: banks[0],
        tokenAccount: liquidator.lstAlphaAccount,
        amount: depositAmount,
        depositUpToLimit: false,
      })
    );
    await processBankrunTx(ctx, tx, [liquidator.wallet]);

    const liquidateeAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    // dumpAccBalances(liquidateeAcc);
    const liquidatorAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    // dumpAccBalances(liquidatorAcc);

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await liquidateIx(liquidator.mrgnBankrunProgram, {
        assetBankKey: kaminoUsdcBank,
        liabilityBankKey: banks[0],
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: [
          oracles.usdcOracle.publicKey, // asset oracle
          usdcReserve, // asset reserve
          oracles.pythPullLst.publicKey, // liab oracle

          ...composeRemainingAccounts([
            // liquidator accounts
            [banks[0], oracles.pythPullLst.publicKey],
            [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          ]),

          ...composeRemainingAccounts([
            // liquidatee accounts
            [banks[0], oracles.pythPullLst.publicKey],
            [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          ]),
        ],
        amount: liquidateAmount,
        liquidateeAccounts: 5,
        liquidatorAccounts: 5,
      })
    );
    await processBankrunTx(ctx, tx, [liquidator.wallet]);

    tx = new Transaction().add(
      await healthPulse(liquidatee.mrgnBankrunProgram, {
        marginfiAccount: liquidateeAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [banks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(ctx, tx, [liquidatee.wallet]);

    const accAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    const cacheAfter = accAfter.healthCache;
    if (verbose) {
      logHealthCache("user 0 cache post-liquidate ", cacheAfter);
    }

    // TODO assert asset balances
  });

  it("(admin) restore bank 0 default liability ratios", async () => {
    let config = blankBankConfigOptRaw();

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: banks[0],
        bankConfigOpt: config,
      })
    );
    await processBankrunTx(ctx, tx, [groupAdmin.wallet]);
  });

  // TODO test OOM limits at max accounts
});
