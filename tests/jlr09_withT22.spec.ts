import { BN } from "@coral-xyz/anchor";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createInitializeMint2Instruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  getMintLen,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  bankRunProvider,
  banksClient,
  bankrunContext,
  bankrunProgram,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import { configureBank } from "./utils/group-instructions";
import {
  accountInit,
  borrowIx,
  depositIx,
  healthPulse,
  liquidateIx,
  repayIx,
} from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/group-instructions";
import { initJuplendClaimAccountIx } from "./utils/juplend/admin-instructions";
import {
  configureJuplendProtocolPermissions,
  initJuplendPool,
} from "./utils/juplend/jlr-pool-setup";
import {
  deriveJuplendMrgnAddresses,
  deriveJuplendPoolKeys,
} from "./utils/juplend/juplend-pdas";
import { makeJuplendDepositIx } from "./utils/juplend/user-instructions";
import {
  makeJuplendWithdrawSimpleIx,
  refreshJupSimple,
} from "./utils/juplend/shorthand-instructions";
import {
  defaultJuplendBankConfig,
  DEFAULT_BORROW_CONFIG_MIN,
} from "./utils/juplend/types";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import {
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  HEALTH_CACHE_ORACLE_OK,
  MarginfiHealthCacheRaw,
  blankBankConfigOptRaw,
} from "./utils/types";
import { getJuplendPrograms } from "./utils/juplend/programs";
import { dummyIx } from "./utils/bankrunConnection";

const T22_MINT_SEED = Buffer.from("JLR09_T22_MINT_SEED_000000000000");
const T22_BANK_SEED = new BN(90_009);
const BORROWER_ACCOUNT_SEED = Buffer.from("JLR09_BORROWER_ACCOUNT_SEED_0000");
const LIQUIDATOR_ACCOUNT_SEED = Buffer.from("JLR09_LIQUIDATOR_ACCOUNT_SEED_00");
const T22_DECIMALS = 6;
const REWEIGHTED_LIAB_WEIGHT = 5;

const t22Mint = Keypair.fromSeed(T22_MINT_SEED);
const borrowerMarginfiAccount = Keypair.fromSeed(BORROWER_ACCOUNT_SEED);
const liquidatorMarginfiAccount = Keypair.fromSeed(LIQUIDATOR_ACCOUNT_SEED);

const tokenB = (ui: number) =>
  new BN(Math.round(ui * 10 ** ecosystem.tokenBDecimals));
const t22 = (ui: number) => new BN(Math.round(ui * 10 ** T22_DECIMALS));

const USER_DEPOSIT_T22 = t22(35);
const USER_WITHDRAW_T22 = t22(5);
const USER_BORROW_TOKEN_B = tokenB(5.2);
const USER_REPAY_TOKEN_B = tokenB(1);
const LIQUIDATOR_DEPOSIT_TOKEN_B = tokenB(10);
const LIQUIDATION_T22 = t22(1);

describe("jlr09: Token-2022 JupLend flow (bankrun)", () => {
  let borrower: (typeof users)[number];
  let liquidator: (typeof users)[number];

  let groupPk = PublicKey.default;
  let regularTokenBBankPk = PublicKey.default;
  let t22JuplendBankPk = PublicKey.default;
  let t22Pool = deriveJuplendPoolKeys({
    mint: t22Mint.publicKey,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
  });
  let borrowerT22Ata = PublicKey.default;
  let adminT22Ata = PublicKey.default;

  const requireStateKey = (key: string): PublicKey => {
    const value = juplendAccounts.get(key);
    if (!value) {
      throw new Error(`missing juplend test state key: ${key}`);
    }
    return value;
  };

  const refreshAllOracles = async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  };

  const mintT22 = async (destination: PublicKey, amount: BN) => {
    const ix = createMintToInstruction(
      t22Mint.publicKey,
      destination,
      globalProgramAdmin.wallet.publicKey,
      BigInt(amount.toString()),
      [],
      TOKEN_2022_PROGRAM_ID,
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [globalProgramAdmin.wallet],
      false,
      true,
    );
  };

  const pulseHealthFor = async (
    user: (typeof users)[number],
    marginfiAccountPk: PublicKey,
  ) => {
    await refreshAllOracles();

    const remaining = await buildHealthRemainingAccounts(marginfiAccountPk);
    const pulseIx = await healthPulse(user.mrgnBankrunProgram!, {
      marginfiAccount: marginfiAccountPk,
      remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, {
          pool: t22Pool,
        }),
        pulseIx,
        dummyIx(user.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [user.wallet],
      false,
      true,
    );

    return bankrunProgram.account.marginfiAccount.fetch(marginfiAccountPk);
  };

  const netHealth = (healthCache: MarginfiHealthCacheRaw) =>
    wrappedI80F48toBigNumber(healthCache.assetValue).minus(
      wrappedI80F48toBigNumber(healthCache.liabilityValue),
    );

  before(async () => {
    borrower = users[2];
    liquidator = users[3];

    groupPk = requireStateKey(JUPLEND_STATE_KEYS.jlr01Group);
    regularTokenBBankPk = requireStateKey(
      JUPLEND_STATE_KEYS.jlr01RegularBankTokenB,
    );

    const mintLen = getMintLen([]);
    const mintRent =
      await bankRunProvider.connection.getMinimumBalanceForRentExemption(mintLen);
    const createMintIx = SystemProgram.createAccount({
      fromPubkey: globalProgramAdmin.wallet.publicKey,
      newAccountPubkey: t22Mint.publicKey,
      space: mintLen,
      lamports: mintRent,
      programId: TOKEN_2022_PROGRAM_ID,
    });
    const initMintIx = createInitializeMint2Instruction(
      t22Mint.publicKey,
      T22_DECIMALS,
      globalProgramAdmin.wallet.publicKey,
      globalProgramAdmin.wallet.publicKey,
      TOKEN_2022_PROGRAM_ID,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createMintIx, initMintIx),
      [globalProgramAdmin.wallet, t22Mint],
      false,
      true,
    );

    adminT22Ata = getAssociatedTokenAddressSync(
      t22Mint.publicKey,
      groupAdmin.wallet.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );
    borrowerT22Ata = getAssociatedTokenAddressSync(
      t22Mint.publicKey,
      borrower.wallet.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    );
    const createAtasIx = [
      createAssociatedTokenAccountIdempotentInstruction(
        globalProgramAdmin.wallet.publicKey,
        adminT22Ata,
        groupAdmin.wallet.publicKey,
        t22Mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
      ),
      createAssociatedTokenAccountIdempotentInstruction(
        globalProgramAdmin.wallet.publicKey,
        borrowerT22Ata,
        borrower.wallet.publicKey,
        t22Mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
      ),
    ];
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(...createAtasIx),
      [globalProgramAdmin.wallet],
      false,
      true,
    );

    await mintT22(adminT22Ata, t22(2_000));
    await mintT22(borrowerT22Ata, USER_DEPOSIT_T22.mul(new BN(2)));

    t22Pool = await initJuplendPool({
      admin: groupAdmin.wallet,
      mint: t22Mint.publicKey,
      symbol: "jlT22",
      decimals: T22_DECIMALS,
    });
    await configureJuplendProtocolPermissions({
      admin: groupAdmin.wallet,
      mint: t22Mint.publicKey,
      lending: t22Pool.lending,
      rateModel: t22Pool.rateModel,
      tokenReserve: t22Pool.tokenReserve,
      supplyPositionOnLiquidity: t22Pool.supplyPositionOnLiquidity,
      borrowPositionOnLiquidity: t22Pool.borrowPositionOnLiquidity,
      tokenProgram: t22Pool.tokenProgram,
      borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
    });

    const t22Addresses = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: groupPk,
      bankMint: t22Mint.publicKey,
      bankSeed: T22_BANK_SEED,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });
    t22JuplendBankPk = t22Addresses.bank;

    const t22Config = defaultJuplendBankConfig(
      oracles.tokenAOracle.publicKey,
      T22_DECIMALS,
    );
    const addBankIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram!, {
      group: groupPk,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: t22Mint.publicKey,
      bankSeed: T22_BANK_SEED,
      oracle: oracles.tokenAOracle.publicKey,
      jupLendingState: t22Pool.lending,
      fTokenMint: t22Pool.fTokenMint,
      config: t22Config,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const initPositionIx = await makeJuplendInitPositionIx(
      groupAdmin.mrgnBankrunProgram!,
      {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: adminT22Ata,
        bank: t22JuplendBankPk,
        pool: t22Pool,
        seedDepositAmount: t22(1),
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPositionIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const createWithdrawIntermediaryAtaIx =
      createAssociatedTokenAccountIdempotentInstruction(
        groupAdmin.wallet.publicKey,
        t22Addresses.withdrawIntermediaryAta,
        t22Addresses.liquidityVaultAuthority,
        t22Mint.publicKey,
        TOKEN_2022_PROGRAM_ID,
      );
    const initClaimIx = await initJuplendClaimAccountIx(getJuplendPrograms(), {
      signer: groupAdmin.wallet.publicKey,
      mint: t22Mint.publicKey,
      accountFor: t22Addresses.liquidityVaultAuthority,
      claimAccount: t22Addresses.claimAccount,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createWithdrawIntermediaryAtaIx, initClaimIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    juplendAccounts.set(JUPLEND_STATE_KEYS.jlr09BankToken2022, t22JuplendBankPk);

    const [initBorrowerIx, initLiquidatorIx] = await Promise.all([
      accountInit(borrower.mrgnBankrunProgram!, {
        marginfiGroup: groupPk,
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        authority: borrower.wallet.publicKey,
        feePayer: borrower.wallet.publicKey,
      }),
      accountInit(liquidator.mrgnBankrunProgram!, {
        marginfiGroup: groupPk,
        marginfiAccount: liquidatorMarginfiAccount.publicKey,
        authority: liquidator.wallet.publicKey,
        feePayer: liquidator.wallet.publicKey,
      }),
    ]);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initBorrowerIx),
      [borrower.wallet, borrowerMarginfiAccount],
      false,
      true,
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initLiquidatorIx),
      [liquidator.wallet, liquidatorMarginfiAccount],
      false,
      true,
    );

    await mintToTokenAccount(
      ecosystem.tokenBMint.publicKey,
      liquidator.tokenBAccount,
      LIQUIDATOR_DEPOSIT_TOKEN_B.mul(new BN(2)),
    );
  });

  it("(borrower) deposits into T22 Juplend bank", async () => {
    await refreshAllOracles();

    const depositIx = await makeJuplendDepositIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      signerTokenAccount: borrowerT22Ata,
      bank: t22JuplendBankPk,
      pool: t22Pool,
      amount: USER_DEPOSIT_T22,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [borrower.wallet],
      false,
      true,
    );

    const accountAfterPulse = await pulseHealthFor(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const flags = accountAfterPulse.healthCache.flags;
    assert.ok((flags & HEALTH_CACHE_HEALTHY) !== 0);
    assert.ok((flags & HEALTH_CACHE_ENGINE_OK) !== 0);
    assert.ok((flags & HEALTH_CACHE_ORACLE_OK) !== 0);
  });

  it("(liquidator) deposits TokenB to prepare for liquidation", async () => {
    const depositTokenBIx = await depositIx(liquidator.mrgnBankrunProgram!, {
      marginfiAccount: liquidatorMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: liquidator.tokenBAccount,
      amount: LIQUIDATOR_DEPOSIT_TOKEN_B,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositTokenBIx),
      [liquidator.wallet],
      false,
      true,
    );
  });

  it("(borrower) withdraws, borrows TokenB, repays partially, and stays healthy pre-reweight", async () => {
    await refreshAllOracles();

    const withdrawRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );
    const withdrawIx = await makeJuplendWithdrawSimpleIx(
      borrower.mrgnBankrunProgram!,
      {
        marginfiAccount: borrowerMarginfiAccount.publicKey,
        destinationTokenAccount: borrowerT22Ata,
        bank: t22JuplendBankPk,
        pool: t22Pool,
        amount: USER_WITHDRAW_T22,
        remainingAccounts: withdrawRemaining,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      },
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        withdrawIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const borrowRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
      { includedBankPks: [regularTokenBBankPk] },
    );
    const borrowTokenBIx = await borrowIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: borrower.tokenBAccount,
      remaining: borrowRemaining,
      amount: USER_BORROW_TOKEN_B,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, { pool: t22Pool }),
        borrowTokenBIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const repayRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );
    const repayTokenBIx = await repayIx(borrower.mrgnBankrunProgram!, {
      marginfiAccount: borrowerMarginfiAccount.publicKey,
      bank: regularTokenBBankPk,
      tokenAccount: borrower.tokenBAccount,
      amount: USER_REPAY_TOKEN_B,
      remaining: repayRemaining,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await refreshJupSimple(getJuplendPrograms().lending, { pool: t22Pool }),
        repayTokenBIx,
        dummyIx(borrower.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [borrower.wallet],
      false,
      true,
    );

    const accountAfterPulse = await pulseHealthFor(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    assert.ok(
      netHealth(accountAfterPulse.healthCache).gt(0),
      "borrower should be healthy before liability reweight",
    );
  });

  it("(liquidator) liquidates borrower after TokenB liability reweight", async () => {
    const liabBankBefore = await bankrunProgram.account.bank.fetch(
      regularTokenBBankPk,
    );

    const reweightConfig = blankBankConfigOptRaw();
    reweightConfig.liabilityWeightInit = bigNumberToWrappedI80F48(
      REWEIGHTED_LIAB_WEIGHT,
    );
    reweightConfig.liabilityWeightMaint = bigNumberToWrappedI80F48(
      REWEIGHTED_LIAB_WEIGHT,
    );
    const reweightIx = await configureBank(groupAdmin.mrgnBankrunProgram!, {
      bank: regularTokenBBankPk,
      bankConfigOpt: reweightConfig,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(reweightIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const accountBeforeLiq = await pulseHealthFor(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const netBeforeLiq = netHealth(accountBeforeLiq.healthCache);
    assert.ok(netBeforeLiq.lt(0), "borrower should be unhealthy after reweight");

    const [assetBank, liabBank] = await Promise.all([
      bankrunProgram.account.bank.fetch(t22JuplendBankPk),
      bankrunProgram.account.bank.fetch(regularTokenBBankPk),
    ]);
    const liquidatorRemaining = await buildHealthRemainingAccounts(
      liquidatorMarginfiAccount.publicKey,
      { includedBankPks: [t22JuplendBankPk] },
    );
    const liquidateeRemaining = await buildHealthRemainingAccounts(
      borrowerMarginfiAccount.publicKey,
    );
    const liqOracleAccounts: PublicKey[] = [
      assetBank.config.oracleKeys[0],
      assetBank.config.oracleKeys[1],
      liabBank.config.oracleKeys[0],
    ];

    const liqIx = await liquidateIx(liquidator.mrgnBankrunProgram!, {
      assetBankKey: t22JuplendBankPk,
      liabilityBankKey: regularTokenBBankPk,
      liquidatorMarginfiAccount: liquidatorMarginfiAccount.publicKey,
      liquidateeMarginfiAccount: borrowerMarginfiAccount.publicKey,
      remaining: [
        ...liqOracleAccounts,
        ...liquidatorRemaining,
        ...liquidateeRemaining,
      ],
      amount: LIQUIDATION_T22,
      liquidateeAccounts: liquidateeRemaining.length,
      liquidatorAccounts: liquidatorRemaining.length,
    });

    await refreshAllOracles();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 500_000 }),
        await refreshJupSimple(getJuplendPrograms().lending, { pool: t22Pool }),
        liqIx,
        dummyIx(liquidator.wallet.publicKey, groupAdmin.wallet.publicKey),
      ),
      [liquidator.wallet],
      false,
      true,
    );

    const accountAfterLiq = await pulseHealthFor(
      borrower,
      borrowerMarginfiAccount.publicKey,
    );
    const netAfterLiq = netHealth(accountAfterLiq.healthCache);
    assert.ok(
      netAfterLiq.gt(netBeforeLiq),
      "liquidation should improve borrower net health",
    );

    const restoreConfig = blankBankConfigOptRaw();
    restoreConfig.liabilityWeightInit =
      liabBankBefore.config.liabilityWeightInit;
    restoreConfig.liabilityWeightMaint =
      liabBankBefore.config.liabilityWeightMaint;
    const restoreIx = await configureBank(groupAdmin.mrgnBankrunProgram!, {
      bank: regularTokenBBankPk,
      bankConfigOpt: restoreConfig,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(restoreIx),
      [groupAdmin.wallet],
      false,
      true,
    );
  });
});
