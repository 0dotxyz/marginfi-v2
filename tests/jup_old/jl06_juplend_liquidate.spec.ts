import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { ComputeBudgetProgram, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";

import { genericMultiBankTestSetup } from "./genericSetups";
import { configureBank } from "./utils/group-instructions";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  liquidateIx,
} from "./utils/user-instructions";
import {
  blankBankConfigOptRaw,
  CONF_INTERVAL_MULTIPLE,
  HEALTH_CACHE_HEALTHY,
} from "./utils/types";
import { processBankrunTransaction } from "./utils/tools";

import { ensureJuplendPoolForMint } from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/juplend-test-env";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import { juplendUpdateRateIx } from "./utils/juplend/juplend-instructions";
import { bigNumberToWrappedI80F48, wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

/** deterministic 32 bytes */
const THROWAWAY_GROUP_SEED_JL06 = Buffer.from(
  "JUPLEND_LIQUIDATE_SEED_000001___"
);

const USER_ACCOUNT_THROWAWAY_JL06 = "throwaway_account_jl06";
const startingSeed = 60;

describe("jl06: JupLend liquidation (bankrun)", () => {
  const throwawayGroup = Keypair.fromSeed(THROWAWAY_GROUP_SEED_JL06);

  // One regular bank acts as the liability (borrowed) bank.
  let liabBank: PublicKey;

  // One JupLend bank acts as the collateral bank.
  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;
  let juplendBank: PublicKey;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;

  const JUplend_BANK_SEED = new BN(startingSeed + 99);
  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)

  const seedAmountLst = new BN(50 * 10 ** ecosystem.lstAlphaDecimals);
  // Deposit enough USDC to support borrowing 3 LST (~$534) with 0.8 asset weight
  // Need: $534 / 0.8 = $667.50 minimum, use 700 for safety
  const depositAmountUsdc = new BN(700 * 10 ** ecosystem.usdcDecimals);
  const borrowAmountLst = new BN(3 * 10 ** ecosystem.lstAlphaDecimals);
  const liquidateAmountUsdc = new BN(5 * 10 ** ecosystem.usdcDecimals);

  before(async () => {
    // 1) Init a fresh group + one regular bank (LST) + user accounts.
    // NOTE: genericMultiBankTestSetup uses Keypair.fromSeed(groupSeed) internally.
    const setup = await genericMultiBankTestSetup(
      1,
      USER_ACCOUNT_THROWAWAY_JL06,
      THROWAWAY_GROUP_SEED_JL06,
      startingSeed
    );
    liabBank = setup.banks[0];

    // 2) Ensure Juplend pool exists for USDC
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // 3) Fund groupAdmin + users with USDC so we can seed/operate the Juplend bank.
    const mintUsdcTx = new Transaction();
    const mintAmount = new BN(1_000 * 10 ** ecosystem.usdcDecimals).toNumber();
    for (const u of [groupAdmin, users[0], users[1]]) {
      mintUsdcTx.add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          u.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          mintAmount
        )
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      mintUsdcTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // 4) Seed liquidity into the regular (liability) bank so borrows work.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY_JL06),
          bank: liabBank,
          tokenAccount: groupAdmin.lstAlphaAccount,
          amount: seedAmountLst,
          depositUpToLimit: false,
        })
      ),
      [groupAdmin.wallet],
      false,
      true
    );

    // 5) Add Juplend bank (USDC) into the same group.
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: throwawayGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: JUplend_BANK_SEED,
      fTokenMint: pool.fTokenMint,
      tokenProgram: pool.tokenProgram,
    });
    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;

    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    const addJuplendIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: throwawayGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: JUplend_BANK_SEED,
      oracle: oracles.usdcOracle.publicKey,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
      tokenProgram: pool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addJuplendIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // 6) Activate Juplend bank (create ATA + seed deposit)
    const initPosIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: groupAdmin.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      tokenProgram: pool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPosIx),
      [groupAdmin.wallet],
      false,
      true
    );
  });

  it("(user 0) deposits USDC through JupLend and borrows LST", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY_JL06);

    const depositIx = await makeJuplendDepositIx(liquidatee.mrgnBankrunProgram!, {
      group: throwawayGroup.publicKey,
      marginfiAccount: liquidateeAccount,
      authority: liquidatee.wallet.publicKey,
      signerTokenAccount: liquidatee.usdcAccount,

      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: depositAmountUsdc,
      tokenProgram: pool.tokenProgram,
    });

    // Borrow needs Juplend updated in-slot (risk engine will read the lending state).
    const updateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    const borrowRemaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
      [liabBank, oracles.pythPullLst.publicKey],
    ]);

    const borrow = await borrowIx(liquidatee.mrgnBankrunProgram!, {
      marginfiAccount: liquidateeAccount,
      bank: liabBank,
      tokenAccount: liquidatee.lstAlphaAccount,
      remaining: borrowRemaining,
      amount: borrowAmountLst,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx, updateRateIx, borrow),
      [liquidatee.wallet],
      false,
      true
    );
  });

  it("(admin) increases liability weights to make user 0 unhealthy", async () => {
    const liabWeights = blankBankConfigOptRaw();
    // Aggressively increase liabilities so liquidation becomes possible.
    liabWeights.liabilityWeightInit = bigNumberToWrappedI80F48(2.1);
    liabWeights.liabilityWeightMaint = bigNumberToWrappedI80F48(2.0);
    liabWeights.confidenceWeight = bigNumberToWrappedI80F48(
      0.01 * CONF_INTERVAL_MULTIPLE
    );

    const tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: liabBank,
        bankConfigOpt: liabWeights,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    // Optional sanity check: pulse health in the same slot as update_rate.
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY_JL06);
    const updateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
      [liabBank, oracles.pythPullLst.publicKey],
    ]);

    const pulseTx = new Transaction().add(
      updateRateIx,
      await healthPulse(liquidatee.mrgnBankrunProgram!, {
        marginfiAccount: liquidateeAccount,
        remaining,
      })
    );

    await processBankrunTransaction(
      bankrunContext,
      pulseTx,
      [liquidatee.wallet],
      false,
      true
    );

    const acc = await liquidatee.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    // If healthy, liquidation will fail with HealthyAccount.
    if (verbose) {
      const av = wrappedI80F48toBigNumber(acc.healthCache.assetValueMaint);
      const lv = wrappedI80F48toBigNumber(acc.healthCache.liabilityValueMaint);
      console.log(`health maint: ${av.minus(lv).toString()}`);
    }
    assert.notOk(
      acc.healthCache.flags & HEALTH_CACHE_HEALTHY,
      "expected user 0 to be unhealthy after liability weights increase"
    );
  });

  it("(user 1) liquidates user 0 (classic liquidation instruction)", async () => {
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT_THROWAWAY_JL06);
    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT_THROWAWAY_JL06);

    // Liquidator needs some deposit in the liability bank.
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await depositIx(liquidator.mrgnBankrunProgram!, {
          marginfiAccount: liquidatorAccount,
          bank: liabBank,
          tokenAccount: liquidator.lstAlphaAccount,
          amount: new BN(10 * 10 ** ecosystem.lstAlphaDecimals),
          depositUpToLimit: false,
        })
      ),
      [liquidator.wallet],
      false,
      true
    );

    const updateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    // For Juplend collateral we need: asset oracle + lending state.
    // (Pattern matches Drift: oracle + spot market; Kamino: oracle + reserve).
    const liquidatorObs = composeRemainingAccounts([
      [liabBank, oracles.pythPullLst.publicKey],
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);
    const liquidateeObs = composeRemainingAccounts([
      [liabBank, oracles.pythPullLst.publicKey],
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

    const remainingForLiq: PublicKey[] = [
      oracles.usdcOracle.publicKey, // asset oracle
      pool.lending, // asset state (for price conversion + owner checks)
      oracles.pythPullLst.publicKey, // liability oracle
      ...liquidatorObs,
      ...liquidateeObs,
    ];

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      updateRateIx,
      await liquidateIx(liquidator.mrgnBankrunProgram!, {
        assetBankKey: juplendBank,
        liabilityBankKey: liabBank,
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: remainingForLiq,
        amount: liquidateAmountUsdc,
        liquidatorAccounts: liquidatorObs.length,
        liquidateeAccounts: liquidateeObs.length,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [liquidator.wallet]);

    // Minimal post-conditions: liquidator should now have a USDC (juplend) balance.
    const accAfter = await liquidator.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    const hasJuplendBal = (accAfter.lendingAccount.balances as any[]).some(
      (b) => b.active && (b.bankPk as PublicKey).equals(juplendBank)
    );
    assert.isTrue(hasJuplendBal, "expected liquidator to receive juplend collateral");
  });
});
