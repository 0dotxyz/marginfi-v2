import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Transaction,
  ComputeBudgetProgram,
  Keypair,
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
  SOLEND_USDC_LIQUIDITY_SUPPLY,
} from "./rootHooks";
import { MockUser, USER_ACCOUNT } from "./utils/mocks";
import {
  healthPulse,
  depositIx,
  borrowIx,
  accountInit,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import {
  makeAddSolendBankIx,
  makeSolendInitObligationIx as makeInitObligationIx,
  makeSolendDepositIx,
} from "./utils/solend-instructions";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { genericMultiBankTestSetup } from "./genericSetups";
import { HEALTH_CACHE_HEALTHY } from "./utils/types";
import { defaultSolendBankConfig } from "./utils/solend-utils";
import {
  processBankrunTransaction as processBankrunTx,
  bytesToF64,
} from "./utils/tools";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assert } from "chai";

describe("sl08: 16 Banks Stress Test", () => {
  let program: Program<Marginfi>;

  before(() => {
    program = bankrunProgram;
  });

  const startingSeed = 800;
  const REGULAR_BANKS_COUNT = 15;
  const SOLEND_BANKS_COUNT = 8; // Limited by MAX_INTEGRATION_POSITIONS

  const seedAmountLst = new BN(10 * 10 ** ecosystem.lstAlphaDecimals);
  const userDepositAmountUsdc = new BN(1000 * 10 ** ecosystem.usdcDecimals);
  const borrowAmountLst = new BN(1 * 10 ** ecosystem.lstAlphaDecimals);
  const THROWAWAY_GROUP_SEED_SL08 = Buffer.from(
    "throwaway_group_sl08_stress_test"
  );

  let regularBanks: PublicKey[] = [];
  let solendBanks: PublicKey[] = [];
  let allBanks: PublicKey[] = [];
  let throwawayGroup: { publicKey: PublicKey };

  let userA: MockUser;
  let userB: MockUser;
  let userC: MockUser;
  let liquidator: MockUser;

  let userAAccount: PublicKey;
  let userBAccount: PublicKey;
  let userCAccount: PublicKey;
  let liquidatorAccount: PublicKey;

  it("init group and setup 15 regular banks", async () => {
    const result = await genericMultiBankTestSetup(
      REGULAR_BANKS_COUNT,
      USER_ACCOUNT,
      THROWAWAY_GROUP_SEED_SL08,
      startingSeed
    );

    regularBanks = result.banks;
    throwawayGroup = result.throwawayGroup;
  });

  it("setup Solend banks with obligations", async () => {
    const bankConfigs = [
      ...Array.from({ length: SOLEND_BANKS_COUNT }, (_, i) => ({
        mint: ecosystem.usdcMint.publicKey,
        reserve: SOLEND_USDC_RESERVE,
        collateralMint: SOLEND_USDC_COLLATERAL_MINT,
        liquiditySupply: SOLEND_USDC_LIQUIDITY_SUPPLY,
        oracle: oracles.usdcOracle.publicKey,
        oracleFeed: oracles.usdcOracleFeed.publicKey,
        seed: startingSeed + REGULAR_BANKS_COUNT + i,
      })),
    ];

    for (let i = 0; i < bankConfigs.length; i++) {
      const config = bankConfigs[i];

      const [solendBank] = deriveBankWithSeed(
        program.programId,
        throwawayGroup.publicKey,
        config.mint,
        new BN(config.seed)
      );

      const addBankTx = new Transaction().add(
        await makeAddSolendBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: throwawayGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: config.mint,
            integrationAcc1: solendAccounts.get(config.reserve)!,
            solendMarket: solendAccounts.get(SOLEND_MARKET)!,
            oracle: config.oracle,
          },
          {
            config: defaultSolendBankConfig(config.oracle),
            seed: new BN(config.seed),
          }
        )
      );

      await processBankrunTx(
        bankrunContext,
        addBankTx,
        [groupAdmin.wallet],
        false,
        true
      );

      const collateralMint = solendAccounts.get(config.collateralMint)!;

      const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
        program.programId,
        solendBank
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
            bank: solendBank,
            signerTokenAccount: config.mint.equals(ecosystem.usdcMint.publicKey)
              ? groupAdmin.usdcAccount
              : groupAdmin.tokenAAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: config.oracleFeed,
          },
          {
            amount: new BN(100),
          }
        )
      );

      await processBankrunTx(
        bankrunContext,
        initObligationTx,
        [groupAdmin.wallet],
        false,
        true
      );

      solendBanks.push(solendBank);
    }

    allBanks = [...regularBanks, ...solendBanks];
  });

  it("setup users and create marginfi accounts", async () => {
    userA = users[0];
    userB = users[1];
    userC = users[2];
    liquidator = users[3];

    const userAAccountKeypair = Keypair.generate();
    userAAccount = userAAccountKeypair.publicKey;

    const userBAccountKeypair = Keypair.generate();
    userBAccount = userBAccountKeypair.publicKey;

    const userCAccountKeypair = Keypair.generate();
    userCAccount = userCAccountKeypair.publicKey;

    const liquidatorAccountKeypair = Keypair.generate();
    liquidatorAccount = liquidatorAccountKeypair.publicKey;

    const accounts = [
      { user: userA, account: userAAccount, keypair: userAAccountKeypair },
      { user: userB, account: userBAccount, keypair: userBAccountKeypair },
      { user: userC, account: userCAccount, keypair: userCAccountKeypair },
      {
        user: liquidator,
        account: liquidatorAccount,
        keypair: liquidatorAccountKeypair,
      },
    ];

    for (const { user, account, keypair } of accounts) {
      const tx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: account,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      );
      await processBankrunTx(bankrunContext, tx, [user.wallet, keypair]);
    }
  });

  it("fund users with tokens", async () => {
    const users_to_fund = [userA, userB, userC, liquidator];

    for (const user of users_to_fund) {
      const fundLstTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.lstAlphaMint.publicKey,
          user.lstAlphaAccount,
          globalProgramAdmin.wallet.publicKey,
          100 * 10 ** ecosystem.lstAlphaDecimals
        )
      );
      await processBankrunTx(bankrunContext, fundLstTx, [
        globalProgramAdmin.wallet,
      ]);

      const fundUsdcTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          user.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          10000 * 10 ** ecosystem.usdcDecimals
        )
      );
      await processBankrunTx(bankrunContext, fundUsdcTx, [
        globalProgramAdmin.wallet,
      ]);

      const fundTokenATx = new Transaction().add(
        createMintToInstruction(
          ecosystem.tokenAMint.publicKey,
          user.tokenAAccount,
          globalProgramAdmin.wallet.publicKey,
          1000 * 10 ** ecosystem.tokenADecimals
        )
      );
      await processBankrunTx(bankrunContext, fundTokenATx, [
        globalProgramAdmin.wallet,
      ]);
    }
  });

  it("refresh oracles before operations", async () => {
    await refreshPullOraclesBankrun(
      oracles,
      bankrunContext,
      bankrunContext.banksClient
    );
  });

  it("(admin) seed liquidity in regular banks only", async () => {
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

    const fundAdminTx = new Transaction().add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        groupAdmin.lstAlphaAccount,
        globalProgramAdmin.wallet.publicKey,
        200 * 10 ** ecosystem.lstAlphaDecimals
      )
    );
    await processBankrunTx(bankrunContext, fundAdminTx, [
      globalProgramAdmin.wallet,
    ]);

    const depositsPerTx = 4;
    for (let i = 0; i < regularBanks.length; i += depositsPerTx) {
      const chunk = regularBanks.slice(i, i + depositsPerTx);
      const tx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_500_000 })
      );

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

  it("(user A) worst-case deposit complexity: 8 Solend deposits + 1 regular borrow", async () => {
    for (let i = 0; i < solendBanks.length; i++) {
      const bank = solendBanks[i];

      const tokenAccount = userA.usdcAccount;
      const amount = userDepositAmountUsdc;

      const tx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_500_000 }),
        await makeSolendDepositIx(
          userA.mrgnBankrunProgram,
          {
            marginfiAccount: userAAccount,
            bank,
            signerTokenAccount: tokenAccount,
            lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
            pythPrice: oracles.usdcOracle.publicKey,
          },
          {
            amount,
          }
        )
      );

      await processBankrunTx(bankrunContext, tx, [userA.wallet], false, true);
    }

    const borrowTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await borrowIx(userA.mrgnBankrunProgram, {
        marginfiAccount: userAAccount,
        bank: regularBanks[0],
        tokenAccount: userA.lstAlphaAccount,
        amount: borrowAmountLst,
        remaining: composeRemainingAccounts([
          ...solendBanks.map((bank) => [
            bank,
            oracles.usdcOracle.publicKey,
            solendAccounts.get(SOLEND_USDC_RESERVE)!,
          ]),
          [regularBanks[0], oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(
      bankrunContext,
      borrowTx,
      [userA.wallet],
      false,
      true
    );
  });

  it("(user A) test health calculation with worst-case complexity", async () => {
    const remainingAccounts: PublicKey[][] = [];

    for (const bank of solendBanks) {
      remainingAccounts.push([
        bank,
        oracles.usdcOracle.publicKey,
        solendAccounts.get(SOLEND_USDC_RESERVE)!,
      ]);
    }

    remainingAccounts.push([regularBanks[0], oracles.pythPullLst.publicKey]);

    const healthTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await healthPulse(userA.mrgnBankrunProgram, {
        marginfiAccount: userAAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
      })
    );

    await processBankrunTx(
      bankrunContext,
      healthTx,
      [userA.wallet],
      false,
      true
    );

    const accountAfter =
      await userA.mrgnBankrunProgram.account.marginfiAccount.fetch(
        userAAccount
      );
    const healthCache = accountAfter.healthCache;

    assert.ok(healthCache.flags & HEALTH_CACHE_HEALTHY);

    const nonZeroPrices = healthCache.prices.filter((priceWrapped: any) => {
      const price = bytesToF64(priceWrapped);
      return price !== 0;
    });
    // 8 Solend deposits + 1 borrow = 9 positions
    assert.equal(nonZeroPrices.length, SOLEND_BANKS_COUNT + 1);
  });

  it("(user B) worst-case borrow complexity: 1 Solend deposit + 15 regular borrows", async () => {
    const depositTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_500_000 }),
      await makeSolendDepositIx(
        userB.mrgnBankrunProgram,
        {
          marginfiAccount: userBAccount,
          bank: solendBanks[0],
          signerTokenAccount: userB.usdcAccount,
          lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
          pythPrice: oracles.usdcOracle.publicKey,
        },
        {
          amount: new BN(5000 * 10 ** ecosystem.usdcDecimals),
        }
      )
    );
    await processBankrunTx(
      bankrunContext,
      depositTx,
      [userB.wallet],
      false,
      true
    );

    for (let i = 0; i < regularBanks.length; i++) {
      const bank = regularBanks[i];

      const remainingAccounts: PublicKey[][] = [];

      remainingAccounts.push([
        solendBanks[0],
        oracles.usdcOracle.publicKey,
        solendAccounts.get(SOLEND_USDC_RESERVE)!,
      ]);

      for (let j = 0; j <= i; j++) {
        remainingAccounts.push([
          regularBanks[j],
          oracles.pythPullLst.publicKey,
        ]);
      }

      const borrowTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_500_000 }),
        await borrowIx(userB.mrgnBankrunProgram, {
          marginfiAccount: userBAccount,
          bank,
          tokenAccount: userB.lstAlphaAccount,
          amount: new BN(0.1 * 10 ** ecosystem.lstAlphaDecimals),
          remaining: composeRemainingAccounts(remainingAccounts),
        })
      );

      await processBankrunTx(
        bankrunContext,
        borrowTx,
        [userB.wallet],
        false,
        true
      );
    }
  });

  it("(user B) test health calculation with worst-case borrow complexity", async () => {
    const remainingAccounts: PublicKey[][] = [];

    remainingAccounts.push([
      solendBanks[0],
      oracles.usdcOracle.publicKey,
      solendAccounts.get(SOLEND_USDC_RESERVE)!,
    ]);

    for (const bank of regularBanks) {
      remainingAccounts.push([bank, oracles.pythPullLst.publicKey]);
    }

    const healthTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await healthPulse(userB.mrgnBankrunProgram, {
        marginfiAccount: userBAccount,
        remaining: composeRemainingAccounts(remainingAccounts),
      })
    );

    await processBankrunTx(
      bankrunContext,
      healthTx,
      [userB.wallet],
      false,
      true
    );

    const accountAfter =
      await userB.mrgnBankrunProgram.account.marginfiAccount.fetch(
        userBAccount
      );
    const healthCache = accountAfter.healthCache;

    assert.ok(healthCache.flags & HEALTH_CACHE_HEALTHY);

    const nonZeroPrices = healthCache.prices.filter((priceWrapped: any) => {
      const price = bytesToF64(priceWrapped);
      return price !== 0;
    });
    assert.equal(nonZeroPrices.length, 16);
  });
});
