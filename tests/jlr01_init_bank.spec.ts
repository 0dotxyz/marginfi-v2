import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { TOKEN_PROGRAM_ID, createMintToInstruction } from "@solana/spl-token";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalFeeWallet,
  globalProgramAdmin,
  groupAdmin,
  juplendAccounts,
  oracles,
  PROGRAM_FEE_FIXED,
  PROGRAM_FEE_RATE,
  users,
} from "./rootHooks";

import {
  addBankWithSeed,
  configureBankOracle,
  groupInitialize,
} from "./utils/group-instructions";
import {
  assertBankrunTxFailed,
  assertBNEqual,
  assertI80F48Approx,
  assertI80F48Equal,
  assertKeyDefault,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveFeeVaultAuthority,
  deriveInsuranceVault,
  deriveInsuranceVaultAuthority,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import { processBankrunTransaction, safeGetAccountInfo } from "./utils/tools";
import {
  ASSET_TAG_DEFAULT,
  BANK_SEED_KNOWN_FLAG,
  CLOSE_ENABLED_FLAG,
  ORACLE_SETUP_PYTH_PUSH,
  PYTH_PULL_MIGRATED,
  defaultBankConfig,
} from "./utils/types";

import {
  configureJuplendProtocolPermissions,
  initJuplendGlobals,
  initJuplendPool,
} from "./utils/juplend/jlr-pool-setup";
import {
  assertJuplendBankState,
  assertJuplendPoolInitialized,
} from "./utils/juplend/assertions";
import {
  deriveJuplendMrgnAddresses,
  type JuplendMrgnAddresses,
} from "./utils/juplend/juplend-pdas";
import {
  addJuplendBankIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/group-instructions";
import {
  DEFAULT_BORROW_CONFIG_MIN,
  DEFAULT_RATE_CONFIG,
  DEFAULT_SUPPLY_CONFIG,
  DEFAULT_TOKEN_CONFIG,
  defaultJuplendBankConfig,
  type JuplendPoolKeys,
} from "./utils/juplend/types";
import {
  JUPLEND_STATE_KEYS,
  jlr01BankStateKey,
  jlr01RegularBankStateKey,
} from "./utils/juplend/test-state";

const GROUP_SEED = Buffer.from("JLR01_GROUP_SEED_000000000000000");
const jlrGroup = Keypair.fromSeed(GROUP_SEED);

const toUnit = (decimals: number) => new BN(10).pow(new BN(decimals));

const getAdminTokenAccountForMint = (mint: PublicKey): PublicKey => {
  if (mint.equals(ecosystem.usdcMint.publicKey)) {
    return groupAdmin.usdcAccount;
  }
  if (mint.equals(ecosystem.tokenAMint.publicKey)) {
    return groupAdmin.tokenAAccount;
  }
  if (mint.equals(ecosystem.wsolMint.publicKey)) {
    return groupAdmin.wsolAccount;
  }
  throw new Error(`Unsupported mint for init position: ${mint.toBase58()}`);
};

const mintToAdmin = async (mint: PublicKey, amount: BN) => {
  const destination = getAdminTokenAccountForMint(mint);
  const ix = createMintToInstruction(
    mint,
    destination,
    globalProgramAdmin.wallet.publicKey,
    BigInt(amount.toString()),
    [],
    TOKEN_PROGRAM_ID,
  );
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [globalProgramAdmin.wallet],
    false,
    true,
  );
};

const mintToTokenAccount = async (
  mint: PublicKey,
  destination: PublicKey,
  amount: BN,
) => {
  const ix = createMintToInstruction(
    mint,
    destination,
    globalProgramAdmin.wallet.publicKey,
    BigInt(amount.toString()),
    [],
    TOKEN_PROGRAM_ID,
  );
  await processBankrunTransaction(
    bankrunContext,
    new Transaction().add(ix),
    [globalProgramAdmin.wallet],
    false,
    true,
  );
};

const JUPLEND_BANK_SEEDS = {
  usdc: new BN(1),
  tokenA: new BN(2),
  wsol: new BN(3),
};

const REGULAR_BANK_SEEDS = {
  usdc: new BN(101),
  tokenB: new BN(102),
  lst: new BN(103),
};

type JuplendPoolSpec = {
  name: string;
  symbol: string;
  mint: Keypair;
  decimals: number;
  oracle: PublicKey;
  seed: BN;
};

type RegularBankSpec = {
  name: string;
  mint: Keypair;
  decimals: number;
  oracle: PublicKey;
  seed: BN;
};

describe("jlr01: JupLend init banks/pools (bankrun)", () => {
  const juplendPools: Record<string, JuplendPoolKeys> = {};
  const juplendAddresses: Record<string, JuplendMrgnAddresses> = {};
  let juplendSpecs: JuplendPoolSpec[];
  let regularSpecs: RegularBankSpec[];

  before(() => {
    juplendSpecs = [
      {
        name: "USDC",
        symbol: "jlUSDC",
        mint: ecosystem.usdcMint,
        decimals: ecosystem.usdcDecimals,
        oracle: oracles.usdcOracle.publicKey,
        seed: JUPLEND_BANK_SEEDS.usdc,
      },
      {
        name: "TokenA",
        symbol: "jlTokenA",
        mint: ecosystem.tokenAMint,
        decimals: ecosystem.tokenADecimals,
        oracle: oracles.tokenAOracle.publicKey,
        seed: JUPLEND_BANK_SEEDS.tokenA,
      },
      {
        name: "WSOL",
        symbol: "jlWSOL",
        mint: ecosystem.wsolMint,
        decimals: ecosystem.wsolDecimals,
        oracle: oracles.wsolOracle.publicKey,
        seed: JUPLEND_BANK_SEEDS.wsol,
      },
    ];

    regularSpecs = [
      {
        name: "USDC",
        mint: ecosystem.usdcMint,
        decimals: ecosystem.usdcDecimals,
        oracle: oracles.usdcOracle.publicKey,
        seed: REGULAR_BANK_SEEDS.usdc,
      },
      {
        name: "TokenB",
        mint: ecosystem.tokenBMint,
        decimals: ecosystem.tokenBDecimals,
        oracle: oracles.tokenBOracle.publicKey,
        seed: REGULAR_BANK_SEEDS.tokenB,
      },
      {
        name: "LST",
        mint: ecosystem.lstAlphaMint,
        decimals: ecosystem.lstAlphaDecimals,
        oracle: oracles.pythPullLst.publicKey,
        seed: REGULAR_BANK_SEEDS.lst,
      },
    ];
  });

  it("(admin) initialize marginfi group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: jlrGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [groupAdmin.wallet, jlrGroup],
      false,
      true,
    );

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      jlrGroup.publicKey,
    );
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
    assertI80F48Approx(group.feeStateCache.programFeeFixed, PROGRAM_FEE_FIXED);
    assertI80F48Approx(group.feeStateCache.programFeeRate, PROGRAM_FEE_RATE);
    assertKeysEqual(group.feeStateCache.globalFeeWallet, globalFeeWallet);

    juplendAccounts.set(JUPLEND_STATE_KEYS.jlr01Group, jlrGroup.publicKey);
  });

  it("(admin) initialize regular mrgn banks", async () => {
    const config = defaultBankConfig();

    for (const spec of regularSpecs) {
      const addIx = await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: jlrGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: spec.mint.publicKey,
        config,
        seed: spec.seed,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(addIx),
        [groupAdmin.wallet],
        false,
        true,
      );

      const oracleIx = await configureBankOracle(
        groupAdmin.mrgnBankrunProgram,
        {
          bank: deriveBankWithSeed(
            bankrunProgram.programId,
            jlrGroup.publicKey,
            spec.mint.publicKey,
            spec.seed,
          )[0],
          type: ORACLE_SETUP_PYTH_PUSH,
          oracle: spec.oracle,
        },
      );

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(oracleIx),
        [groupAdmin.wallet],
        false,
        true,
      );

      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        jlrGroup.publicKey,
        spec.mint.publicKey,
        spec.seed,
      );

      const bank = await bankrunProgram.account.bank.fetch(bankPk);
      juplendAccounts.set(jlr01RegularBankStateKey(spec.name), bankPk);

      assertKeysEqual(bank.mint, spec.mint.publicKey);
      assert.equal(bank.mintDecimals, spec.decimals);
      assertKeysEqual(bank.group, jlrGroup.publicKey);
      assert.equal(bank.config.assetTag, ASSET_TAG_DEFAULT);
      assert.deepEqual(bank.config.oracleSetup, { pythPushOracle: {} });
      assertKeysEqual(bank.config.oracleKeys[0], spec.oracle);
      assertKeysEqual(bank.config.oracleKeys[1], PublicKey.default);
      assertI80F48Equal(bank.config.assetWeightInit, config.assetWeightInit);
      assertI80F48Equal(bank.config.assetWeightMaint, config.assetWeightMaint);
      assertI80F48Equal(
        bank.config.liabilityWeightInit,
        config.liabilityWeightInit,
      );
      assertI80F48Equal(
        bank.config.liabilityWeightMaint,
        config.liabilityWeightMain,
      );
      assertBNEqual(bank.config.depositLimit, config.depositLimit);
      assertBNEqual(bank.config.borrowLimit, config.borrowLimit);
      assert.deepEqual(bank.config.operationalState, { operational: {} });
      assert.deepEqual(bank.config.riskTier, { collateral: {} });
      assert.equal(bank.config.configFlags, PYTH_PULL_MIGRATED);
      assertBNEqual(
        bank.config.totalAssetValueInitLimit,
        config.totalAssetValueInitLimit,
      );
      assert.equal(bank.config.oracleMaxAge, config.oracleMaxAge);
      assert.equal(bank.config.oracleMaxConfidence, config.oracleMaxConfidence);

      assertI80F48Equal(bank.assetShareValue, 1);
      assertI80F48Equal(bank.liabilityShareValue, 1);
      assertI80F48Equal(bank.totalAssetShares, 0);
      assertI80F48Equal(bank.totalLiabilityShares, 0);
      assertBNEqual(bank.flags, CLOSE_ENABLED_FLAG + BANK_SEED_KNOWN_FLAG);
      assertBNEqual(bank.bankSeed, spec.seed);
      assertKeyDefault(bank.emissionsMint);
      assertBNEqual(bank.emissionsRate, 0);
      assertI80F48Equal(bank.emissionsRemaining, 0);

      const [_liqAuth, liqAuthBump] = deriveLiquidityVaultAuthority(
        bankrunProgram.programId,
        bankPk,
      );
      const [liqVault, liqVaultBump] = deriveLiquidityVault(
        bankrunProgram.programId,
        bankPk,
      );
      assertKeysEqual(bank.liquidityVault, liqVault);
      assert.equal(bank.liquidityVaultAuthorityBump, liqAuthBump);
      assert.equal(bank.liquidityVaultBump, liqVaultBump);

      const [_insAuth, insAuthBump] = deriveInsuranceVaultAuthority(
        bankrunProgram.programId,
        bankPk,
      );
      const [insVault, insVaultBump] = deriveInsuranceVault(
        bankrunProgram.programId,
        bankPk,
      );
      assertKeysEqual(bank.insuranceVault, insVault);
      assert.equal(bank.insuranceVaultAuthorityBump, insAuthBump);
      assert.equal(bank.insuranceVaultBump, insVaultBump);

      const [_feeAuth, feeAuthBump] = deriveFeeVaultAuthority(
        bankrunProgram.programId,
        bankPk,
      );
      const [feeVault, feeVaultBump] = deriveFeeVault(
        bankrunProgram.programId,
        bankPk,
      );
      assertKeysEqual(bank.feeVault, feeVault);
      assert.equal(bank.feeVaultAuthorityBump, feeAuthBump);
      assert.equal(bank.feeVaultBump, feeVaultBump);
    }
  });

  it("(admin) fund globalProgramAdmin with tokens to seed deposits", async () => {
    for (const spec of juplendSpecs) {
      await mintToAdmin(spec.mint.publicKey, toUnit(spec.decimals));
    }
  });

  it("(admin) initialize JupLend pools - happy path", async () => {
    await initJuplendGlobals({ admin: groupAdmin.wallet });

    for (const spec of juplendSpecs) {
      const pool = await initJuplendPool({
        admin: groupAdmin.wallet,
        mint: spec.mint.publicKey,
        symbol: spec.symbol,
        decimals: spec.decimals,
        rateConfig: DEFAULT_RATE_CONFIG,
        tokenConfig: DEFAULT_TOKEN_CONFIG,
      });

      juplendPools[spec.name] = pool;

      await configureJuplendProtocolPermissions({
        admin: groupAdmin.wallet,
        mint: spec.mint.publicKey,
        lending: pool.lending,
        rateModel: pool.rateModel,
        tokenReserve: pool.tokenReserve,
        supplyPositionOnLiquidity: pool.supplyPositionOnLiquidity,
        borrowPositionOnLiquidity: pool.borrowPositionOnLiquidity,
        tokenProgram: pool.tokenProgram,
        supplyConfig: DEFAULT_SUPPLY_CONFIG,
        borrowConfig: DEFAULT_BORROW_CONFIG_MIN,
      });

      await assertJuplendPoolInitialized({
        pool,
        mint: spec.mint.publicKey,
        decimals: spec.decimals,
      });
    }
  });

  it("(admin) add JupLend banks with bad params - should fail", async () => {
    const usdcSpec = juplendSpecs.find((s) => s.name === "USDC");
    const tokenASpec = juplendSpecs.find((s) => s.name === "TokenA");
    const usdcPool = juplendPools.USDC;
    const wsolPool = juplendPools.WSOL;

    const badCases = [
      {
        name: "wrong oracle account (in keys)",
        seed: new BN(10_001),
        expectedErrorCode: 6052, // WrongOracleAccountKeys
        params: {
          bankMint: usdcSpec.mint.publicKey,
          oracle: tokenASpec.oracle, // sneaky sneaky
          jupLendingState: usdcPool.lending,
          fTokenMint: usdcPool.fTokenMint,
          tokenProgram: usdcPool.tokenProgram,
          config: defaultJuplendBankConfig(usdcSpec.oracle, usdcSpec.decimals),
        },
      },
      {
        name: "wrong oracle account (in spec)",
        seed: new BN(10_002),
        expectedErrorCode: 6052, // WrongOracleAccountKeys
        params: {
          bankMint: usdcSpec.mint.publicKey,
          oracle: usdcSpec.oracle,
          jupLendingState: usdcPool.lending,
          fTokenMint: usdcPool.fTokenMint,
          config: defaultJuplendBankConfig(
            tokenASpec.oracle, // sneaky sneaky
            usdcSpec.decimals,
          ),
        },
      },
      // Note: there is no protection from passing the wrong oracle in BOTH keys and spec!
      {
        name: "wrong jup lending state and mint",
        seed: new BN(10_003),
        expectedErrorCode: 6506, // JuplendLendingMintMismatch
        params: {
          bankMint: usdcSpec.mint.publicKey,
          oracle: usdcSpec.oracle,
          jupLendingState: wsolPool.lending, // sneaky sneaky
          fTokenMint: wsolPool.fTokenMint, // sneaky sneaky
          config: defaultJuplendBankConfig(usdcSpec.oracle, usdcSpec.decimals),
        },
      },
      {
        name: "wrong fToken mint (does not match jup lending state)",
        seed: new BN(10_004),
        expectedErrorCode: 6505, // InvalidJuplendLending
        params: {
          bankMint: usdcSpec.mint.publicKey,
          oracle: usdcSpec.oracle,
          jupLendingState: usdcPool.lending,
          fTokenMint: wsolPool.fTokenMint, // sneaky sneaky
          config: defaultJuplendBankConfig(usdcSpec.oracle, usdcSpec.decimals),
        },
      },
      {
        name: "wrong lending state (different asseT)",
        seed: new BN(10_005),
        expectedErrorCode: 6505, // InvalidJuplendLending
        params: {
          bankMint: usdcSpec.mint.publicKey,
          oracle: usdcSpec.oracle,
          jupLendingState: wsolPool.lending, // sneaky sneaky
          fTokenMint: usdcPool.fTokenMint,
          config: defaultJuplendBankConfig(usdcSpec.oracle, usdcSpec.decimals),
        },
      },
    ];

    for (const testCase of badCases) {
      const [candidateBankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        jlrGroup.publicKey,
        testCase.params.bankMint,
        testCase.seed,
      );

      const addIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: jlrGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: testCase.params.bankMint,
        bankSeed: testCase.seed,
        oracle: testCase.params.oracle,
        jupLendingState: testCase.params.jupLendingState,
        fTokenMint: testCase.params.fTokenMint,
        config: testCase.params.config,
      });

      const result = await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(addIx),
        [groupAdmin.wallet],
        true,
        false,
      );
      assertBankrunTxFailed(result, testCase.expectedErrorCode);

      const candidateBank = await safeGetAccountInfo(
        bankRunProvider.connection,
        candidateBankPk,
      );
      assert.isNull(candidateBank, `failed: ${testCase.name}`);
    }
  });

  it("(admin) add JupLend banks - happy path", async () => {
    for (const spec of juplendSpecs) {
      const pool = juplendPools[spec.name];
      const addresses = deriveJuplendMrgnAddresses({
        mrgnProgramId: bankrunProgram.programId,
        group: jlrGroup.publicKey,
        bankMint: spec.mint.publicKey,
        bankSeed: spec.seed,
      });

      juplendAddresses[spec.name] = addresses;

      const config = defaultJuplendBankConfig(spec.oracle, spec.decimals);

      const addIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: jlrGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: spec.mint.publicKey,
        bankSeed: spec.seed,
        oracle: spec.oracle,
        jupLendingState: pool.lending,
        fTokenMint: pool.fTokenMint,
        config,
        tokenProgram: pool.tokenProgram,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(addIx),
        [groupAdmin.wallet],
        false,
        true,
      );

      const bank = await bankrunProgram.account.bank.fetch(addresses.bank);

      assertJuplendBankState({
        bankPk: addresses.bank,
        bank,
        group: jlrGroup.publicKey,
        mint: spec.mint.publicKey,
        decimals: spec.decimals,
        oracle: spec.oracle,
        pool,
        addresses,
        config,
        expectedState: { paused: {} },
      });

      juplendAccounts.set(jlr01BankStateKey(spec.name), addresses.bank);
    }
  });

  it("(attacker) init position with bad params - should fail", async () => {
    const attacker = users[0];
    const usdcPool = juplendPools.USDC;
    const usdcAddresses = juplendAddresses.USDC;

    const badInitCases = [
      {
        name: "signer token account has wrong authority",
        bank: usdcAddresses.bank,
        signerTokenAccount: groupAdmin.usdcAccount,
        pool: usdcPool,
        seedDepositAmount: toUnit(ecosystem.usdcDecimals),
        expectedErrorCode: "7df", // token owner constraint
      },
      {
        name: "signer token account has wrong mint",
        bank: usdcAddresses.bank,
        signerTokenAccount: attacker.tokenAAccount,
        pool: usdcPool,
        seedDepositAmount: toUnit(ecosystem.usdcDecimals),
        expectedErrorCode: "7de", // token mint contraint
      },
      {
        name: "seed deposit below minimum",
        bank: usdcAddresses.bank,
        signerTokenAccount: attacker.usdcAccount,
        pool: usdcPool,
        seedDepositAmount: new BN(1),
        expectedErrorCode: 6511, // JuplendInitPositionDepositInsufficient
      },
    ];

    for (const badCase of badInitCases) {
      const ix = await makeJuplendInitPositionIx(attacker.mrgnBankrunProgram!, {
        feePayer: attacker.wallet.publicKey,
        signerTokenAccount: badCase.signerTokenAccount,
        bank: badCase.bank,
        pool: badCase.pool,
        seedDepositAmount: badCase.seedDepositAmount,
      });

      const result = await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [attacker.wallet],
        true,
        false,
      );
      assertBankrunTxFailed(result, badCase.expectedErrorCode);
    }
  });

  it("(attacker) init position with bad jup params - should fail", async () => {
    const attacker = users[0];
    const usdcSpec = juplendSpecs.find((s) => s.name === "USDC");
    const usdcPool = juplendPools.USDC;
    const tokenAPool = juplendPools.TokenA;
    const wsolPool = juplendPools.WSOL;
    const tokenAAddresses = juplendAddresses.TokenA;

    // Mint some USDC to ensure attacker can pass the initial transfer so failures come from
    // Jup-side validation.
    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      attacker.usdcAccount,
      toUnit(ecosystem.usdcDecimals).mul(new BN(10)),
    );

    const badJupCases = [
      {
        name: "wrong lendingAdmin",
        pool: {
          ...usdcPool,
          lendingAdmin: tokenAAddresses.bank,
        },
      },
      {
        name: "wrong rateModel",
        pool: {
          ...usdcPool,
          rateModel: wsolPool.rateModel,
        },
      },
      {
        name: "wrong lendingRewardsRateModel",
        pool: {
          ...usdcPool,
          lendingRewardsRateModel: tokenAPool.lendingRewardsRateModel,
        },
      },
    ];

    for (const [idx, badCase] of badJupCases.entries()) {
      const throwawaySeed = new BN(90_000 + idx);
      const throwawayAddresses = deriveJuplendMrgnAddresses({
        mrgnProgramId: bankrunProgram.programId,
        group: jlrGroup.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: throwawaySeed,
      });

      const config = defaultJuplendBankConfig(
        usdcSpec.oracle,
        ecosystem.usdcDecimals,
      );
      const addIx = await addJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: jlrGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: throwawaySeed,
        oracle: usdcSpec.oracle,
        jupLendingState: usdcPool.lending,
        fTokenMint: usdcPool.fTokenMint,
        config,
        tokenProgram: usdcPool.tokenProgram,
      });
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(addIx),
        [groupAdmin.wallet],
        false,
        true,
      );

      const ix = await makeJuplendInitPositionIx(attacker.mrgnBankrunProgram!, {
        feePayer: attacker.wallet.publicKey,
        signerTokenAccount: attacker.usdcAccount,
        bank: throwawayAddresses.bank,
        pool: badCase.pool,
        seedDepositAmount: toUnit(ecosystem.usdcDecimals),
      });

      const result = await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [attacker.wallet],
        true,
        false,
      );
      // Something on juplend's end, we don't really care about the specific error
      assertBankrunTxFailed(result, "custom program error");

      const bankAfter = await bankrunProgram.account.bank.fetch(
        throwawayAddresses.bank,
      );
      assert.deepEqual(
        bankAfter.config.operationalState,
        { paused: {} },
        `bank init when it should have failed: ${badCase.name}`,
      );
    }
  });

  it("(admin) activate juplend banks via init_position - happy path", async () => {
    for (const spec of juplendSpecs) {
      const pool = juplendPools[spec.name];
      const addresses = juplendAddresses[spec.name];
      assert.ok(addresses, `missing bank addresses for ${spec.name}`);

      const config = defaultJuplendBankConfig(spec.oracle, spec.decimals);

      const initIx = await makeJuplendInitPositionIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: groupAdmin.wallet.publicKey,
          signerTokenAccount: getAdminTokenAccountForMint(spec.mint.publicKey),
          bank: addresses.bank,
          pool,
          seedDepositAmount: toUnit(spec.decimals),
        },
      );

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(initIx),
        [groupAdmin.wallet],
        false,
        true,
      );

      const bankAfter = await bankrunProgram.account.bank.fetch(addresses.bank);

      assertJuplendBankState({
        bankPk: addresses.bank,
        bank: bankAfter,
        group: jlrGroup.publicKey,
        mint: spec.mint.publicKey,
        decimals: spec.decimals,
        oracle: spec.oracle,
        pool,
        addresses,
        config,
        expectedState: { operational: {} },
      });

      const fTokenBalance = await getTokenBalance(
        bankRunProvider,
        addresses.fTokenVault,
      );
      assert.isAbove(fTokenBalance, 0, `${spec.name} fToken vault balance`);

      // Seed deposit mints protocol fTokens but does not create user lending shares on marginfi.
      assertI80F48Equal(bankAfter.totalAssetShares, 0);
    }
  });

  it("(admin) create JupLend LUT for downstream tests", async () => {
    const lutAddresses: PublicKey[] = [];
    const seen = new Set<string>();
    const addAddress = (pk: PublicKey) => {
      const key = pk.toBase58();
      if (!seen.has(key)) {
        seen.add(key);
        lutAddresses.push(pk);
      }
    };

    addAddress(groupAdmin.wallet.publicKey);
    addAddress(jlrGroup.publicKey);
    addAddress(bankrunProgram.programId);
    addAddress(TOKEN_PROGRAM_ID);

    for (const spec of regularSpecs) {
      const [regularBankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        jlrGroup.publicKey,
        spec.mint.publicKey,
        spec.seed,
      );
      addAddress(spec.mint.publicKey);
      addAddress(spec.oracle);
      addAddress(regularBankPk);
    }

    for (const spec of juplendSpecs) {
      const pool = juplendPools[spec.name];
      const addresses = juplendAddresses[spec.name];

      addAddress(spec.mint.publicKey);
      addAddress(spec.oracle);

      for (const pk of Object.values(pool)) addAddress(pk);
      for (const pk of Object.values(addresses)) addAddress(pk);
    }

    const recentSlot = Number(await banksClient.getSlot());
    const [createLutIx, lookupTable] = AddressLookupTableProgram.createLookupTable({
      authority: groupAdmin.wallet.publicKey,
      payer: groupAdmin.wallet.publicKey,
      recentSlot: recentSlot - 1,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createLutIx),
      [groupAdmin.wallet],
      false,
      true,
    );

    const LUT_CHUNK_SIZE = 20;
    for (let i = 0; i < lutAddresses.length; i += LUT_CHUNK_SIZE) {
      const extendLutIx = AddressLookupTableProgram.extendLookupTable({
        authority: groupAdmin.wallet.publicKey,
        payer: groupAdmin.wallet.publicKey,
        lookupTable,
        addresses: lutAddresses.slice(i, i + LUT_CHUNK_SIZE),
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(extendLutIx),
        [groupAdmin.wallet],
        false,
        true,
      );
    }

    const currentSlot = Number(await banksClient.getSlot());
    bankrunContext.warpToSlot(BigInt(currentSlot + 10));

    juplendAccounts.set(JUPLEND_STATE_KEYS.jlr01LookupTable, lookupTable);
  });
});
