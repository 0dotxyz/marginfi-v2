import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  driftAccounts,
  DRIFT_USDC_BANK,
  driftGroup,
  DRIFT_USDC_SPOT_MARKET,
  oracles,
  DRIFT_TOKEN_A_SPOT_MARKET,
  DRIFT_TOKEN_A_BANK,
  DRIFT_TOKEN_A_PULL_ORACLE,
  users,
  bankrunContext,
  bankrunProgram,
  driftBankrunProgram,
} from "./rootHooks";
import {
  assertBNEqual,
  assertI80F48Equal,
  assertKeysEqual,
  assertBankrunTxFailed,
} from "./utils/genericTests";
import {
  defaultDriftBankConfig,
  DRIFT_SCALED_BALANCE_DECIMALS,
  getDriftUserAccount,
  TOKEN_A_INIT_DEPOSIT_AMOUNT,
  TOKEN_A_MARKET_INDEX,
  TOKEN_A_SCALING_FACTOR,
  USDC_INIT_DEPOSIT_AMOUNT,
  USDC_MARKET_INDEX,
  USDC_SCALING_FACTOR,
} from "./utils/drift-utils";
import {
  ASSET_TAG_DRIFT,
  ASSET_TAG_KAMINO,
  CLOSE_ENABLED_FLAG,
  blankBankConfigOptRaw,
} from "./utils/types";
import { assert } from "chai";
import { processBankrunTransaction, safeGetAccountInfo, getBankrunTime } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";
import {
  makeAddDriftBankIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";
import {
  deriveBankWithSeed,
  deriveFeeVault,
  deriveInsuranceVault,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
  deriveUserPDA,
  deriveUserStatsPDA,
} from "./utils/pdas";
import { DRIFT_PROGRAM_ID } from "./utils/types";
import { configureBank } from "./utils/group-instructions";

let ctx: ProgramTestContext;
const seed = new BN(555);
let mrgnID: PublicKey;

let usdcSpotMarket: PublicKey;
let tokenASpotMarket: PublicKey;

describe("d06: Init Drift banks", () => {
  before(async () => {
    ctx = bankrunContext;
    mrgnID = bankrunProgram.programId;

    usdcSpotMarket = driftAccounts.get(DRIFT_USDC_SPOT_MARKET);
    tokenASpotMarket = driftAccounts.get(DRIFT_TOKEN_A_SPOT_MARKET);
  });

  it("(admin) Add Drift bank (drift USDC) and init user - happy path", async () => {
    let defaultConfig = defaultDriftBankConfig(oracles.usdcOracle.publicKey);

    const [bankKey] = deriveBankWithSeed(
      mrgnID,
      driftGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );

    const tx = new Transaction().add(
      await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: usdcSpotMarket,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: seed,
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet], false, true);

    driftAccounts.set(DRIFT_USDC_BANK, bankKey);
    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        users[0].mrgnBankrunProgram,
        {
          feePayer: users[0].wallet.publicKey,
          bank: bankKey,
          signerTokenAccount: users[0].usdcAccount,
        },
        {
          amount: USDC_INIT_DEPOSIT_AMOUNT,
        },
        0 // USDC market index
      )
    );
    await processBankrunTransaction(
      ctx,
      initUserTx,
      [users[0].wallet],
      false,
      true
    );

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      driftGroup.publicKey
    );
    assert.equal(group.banks, 1);

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    const config = bank.config;

    assertKeysEqual(bank.mint, ecosystem.usdcMint.publicKey);
    assert.equal(bank.mintDecimals, ecosystem.usdcDecimals);
    assertKeysEqual(bank.group, driftGroup.publicKey);

    assertKeysEqual(config.oracleKeys[0], oracles.usdcOracle.publicKey);

    assertBNEqual(bank.flags, CLOSE_ENABLED_FLAG);

    assertI80F48Equal(config.assetWeightInit, 1);
    assertI80F48Equal(config.assetWeightMaint, 1);
    // deposit_limit is stored in native decimals for Drift banks
    // Input: 100M USDC in native decimals = 100_000_000 * 10^usdcDecimals
    const inputDepositLimit = new BN(100_000_000).mul(new BN(10 ** ecosystem.usdcDecimals));
    assertBNEqual(config.depositLimit, inputDepositLimit);

    assert.deepEqual(config.operationalState, { operational: {} });
    assert.deepEqual(config.oracleSetup, { driftPythPull: {} });
    assertBNEqual(config.borrowLimit, 0);
    assert.deepEqual(config.riskTier, { collateral: {} });
    assertBNEqual(
      config.totalAssetValueInitLimit,
      new BN(10_000_000 * 10 ** ecosystem.usdcDecimals)
    );
    assert.equal(config.oracleMaxAge, 100);

    const [liquidityVault] = deriveLiquidityVault(mrgnID, bankKey);
    assertKeysEqual(bank.liquidityVault, liquidityVault);
    const [insuranceVault] = deriveInsuranceVault(mrgnID, bankKey);
    assertKeysEqual(bank.insuranceVault, insuranceVault);
    const [feeVault] = deriveFeeVault(mrgnID, bankKey);
    assertKeysEqual(bank.feeVault, feeVault);

    assertKeysEqual(bank.integrationAcc1, usdcSpotMarket);
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      mrgnID,
      bankKey
    );
    const [driftUser] = deriveUserPDA(
      DRIFT_PROGRAM_ID,
      liquidityVaultAuthority,
      0
    );
    const [driftUserStats] = deriveUserStatsPDA(
      DRIFT_PROGRAM_ID,
      liquidityVaultAuthority
    );
    assertKeysEqual(bank.integrationAcc2, driftUser);
    assertKeysEqual(bank.integrationAcc3, driftUserStats);
    assert.equal(config.assetTag, ASSET_TAG_DRIFT);
    assertKeysEqual(bank.config.oracleKeys[1], usdcSpotMarket);

    const usdcSpotMarketData =
      await driftBankrunProgram.account.spotMarket.fetch(usdcSpotMarket);
    assertKeysEqual(usdcSpotMarketData.mint, bank.mint);
    assert.equal(usdcSpotMarketData.decimals, ecosystem.usdcDecimals);

    const driftUserAccount = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );
    const usdcPosition = driftUserAccount.spotPositions[0];
    assert.equal(usdcPosition.marketIndex, USDC_MARKET_INDEX);
    assertBNEqual(
      usdcPosition.scaledBalance,
      USDC_INIT_DEPOSIT_AMOUNT.mul(USDC_SCALING_FACTOR)
    );
  });

  it("(admin) Init Token A bank - happy path", async () => {
    const user = groupAdmin;
    let defaultConfig = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

    const [tokenABankKey] = deriveBankWithSeed(
      mrgnID,
      driftGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );

    const tx = new Transaction().add(
      await makeAddDriftBankIx(
        user.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: user.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenASpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: seed,
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet], false, true);
    driftAccounts.set(DRIFT_TOKEN_A_BANK, tokenABankKey);

    const bank = await bankrunProgram.account.bank.fetch(tokenABankKey);
    assert.equal(bank.mintDecimals, ecosystem.tokenADecimals);
  });

  it("(admin) Configure wrong asset tag for Token A bank - happy path (but all Drift operations will now fail on it)", async () => {
    const user = groupAdmin;
    let bankConfigOpt = blankBankConfigOptRaw();
    bankConfigOpt.assetTag = ASSET_TAG_KAMINO;

    const configureTx = new Transaction().add(
      await configureBank(user.mrgnBankrunProgram, {
        bank: driftAccounts.get(DRIFT_TOKEN_A_BANK),
        bankConfigOpt,
      })
    );

    await processBankrunTransaction(
      ctx,
      configureTx,
      [user.wallet],
      false,
      true
    );
  });

  it("(user 1) Tries to init Drift user for Token A bank - wrong asset tag", async () => {
    const user = users[1];
    const initUserAmount = new BN(100);
    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        user.mrgnBankrunProgram,
        {
          feePayer: user.wallet.publicKey,
          bank: driftAccounts.get(DRIFT_TOKEN_A_BANK),
          signerTokenAccount: users[1].tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE)!,
        },
        {
          amount: initUserAmount,
        },
        1
      )
    );
    const result = await processBankrunTransaction(
      ctx,
      initUserTx,
      [user.wallet],
      true,
      true
    );
    // WrongBankAssetTagForDriftOperation
    assertBankrunTxFailed(result, 6302);
  });

  it("(admin) Restore proper asset tag for Token A bank - happy path", async () => {
    const user = groupAdmin;
    let bankConfigOpt = blankBankConfigOptRaw();
    bankConfigOpt.assetTag = ASSET_TAG_DRIFT;

    const configureTx = new Transaction().add(
      await configureBank(user.mrgnBankrunProgram, {
        bank: driftAccounts.get(DRIFT_TOKEN_A_BANK),
        bankConfigOpt,
      })
    );

    await processBankrunTransaction(
      ctx,
      configureTx,
      [user.wallet],
      false,
      true
    );
  });

  it("(user 1) Tries to init Drift user for Token A bank - too small deposit", async () => {
    const user = users[1];
    const initUserAmount = new BN(9); // minimal allowed amount is 10
    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        user.mrgnBankrunProgram,
        {
          feePayer: user.wallet.publicKey,
          bank: driftAccounts.get(DRIFT_TOKEN_A_BANK),
          signerTokenAccount: user.tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE)!,
        },
        {
          amount: initUserAmount,
        },
        1
      )
    );
    const result = await processBankrunTransaction(
      ctx,
      initUserTx,
      [user.wallet],
      true,
      true
    );
    // DriftUserInitDepositInsufficient
    assertBankrunTxFailed(result, 6310);
  });

  it("(user 1) Init Drift user for Token A bank - happy path", async () => {
    const user = users[1];
    const bankKey = driftAccounts.get(DRIFT_TOKEN_A_BANK);
    const initUserTx = new Transaction().add(
      await makeInitDriftUserIx(
        user.mrgnBankrunProgram,
        {
          feePayer: user.wallet.publicKey,
          bank: bankKey,
          signerTokenAccount: user.tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE)!,
        },
        {
          amount: TOKEN_A_INIT_DEPOSIT_AMOUNT,
        },
        1
      )
    );
    await processBankrunTransaction(
      ctx,
      initUserTx,
      [user.wallet],
      false,
      true
    );

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    const driftUserAccount = await getDriftUserAccount(
      driftBankrunProgram,
      bank.integrationAcc2
    );

    // All non-USDC tokens are deposited to position 1
    const tokenAPosition = driftUserAccount.spotPositions[1];
    assert.equal(tokenAPosition.marketIndex, TOKEN_A_MARKET_INDEX);
    assertBNEqual(
      tokenAPosition.scaledBalance,
      TOKEN_A_INIT_DEPOSIT_AMOUNT.mul(TOKEN_A_SCALING_FACTOR)
    );

    // USDC is still zero
    const usdcPosition = driftUserAccount.spotPositions[0];
    assert.equal(usdcPosition.marketIndex, USDC_MARKET_INDEX);
    assertBNEqual(usdcPosition.scaledBalance, 0);
  });

  it("(user 0) Tries to init bank - admin only, should fail", async () => {
    const throwawaySeed = new BN(999);
    const user = users[0];
    let defaultConfig = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

    const tx = new Transaction().add(
      await makeAddDriftBankIx(
        user.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: user.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenASpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: throwawaySeed,
        }
      )
    );
    let result = await processBankrunTransaction(ctx, tx, [user.wallet], true);
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(admin) Tries pass the wrong spot market/mint for this asset - should fail", async () => {
    const throwawaySeed = new BN(999);
    const usr = groupAdmin;
    let defaultConfig = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

    const tx1 = new Transaction().add(
      await makeAddDriftBankIx(
        usr.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: usdcSpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: throwawaySeed,
        }
      )
    );
    let result1 = await processBankrunTransaction(
      ctx,
      tx1,
      [usr.wallet],
      true,
      false
    );
    // DriftSpotMarketMintMismatch
    assertBankrunTxFailed(result1, 6301);

    const tx2 = new Transaction().add(
      await makeAddDriftBankIx(
        usr.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: tokenASpotMarket, // wrong
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: throwawaySeed,
        }
      )
    );
    let result2 = await processBankrunTransaction(
      ctx,
      tx2,
      [usr.wallet],
      true,
      false
    );
    // DriftSpotMarketMintMismatch
    assertBankrunTxFailed(result2, 6301);
  });

  it("(admin) Tries to use the wrong oracle type - should fail", async () => {
    const throwawaySeed = new BN(999);
    const usr = groupAdmin;
    let config1 = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);
    config1.oracleSetup = {
      pythPushOracle: {},
    } as any; // Cast to any to test invalid oracle setup
    const tx1 = new Transaction().add(
      await makeAddDriftBankIx(
        usr.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenASpotMarket,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: config1,
          seed: throwawaySeed,
        }
      )
    );
    let result1 = await processBankrunTransaction(
      ctx,
      tx1,
      [usr.wallet],
      true,
      false
    );
    assertBankrunTxFailed(result1, 6300);
  });

  it("(admin) Tries to re-init USDC bank when drift user already exists - should fail", async () => {
    const user = groupAdmin;
    let defaultConfig = defaultDriftBankConfig(oracles.usdcOracle.publicKey);

    const tx = new Transaction().add(
      await makeAddDriftBankIx(
        user.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: user.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: usdcSpotMarket,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: seed,
        }
      )
    );

    const result = await processBankrunTransaction(
      ctx,
      tx,
      [user.wallet],
      true,
      false
    );

    // This should fail because the drift user and user stats accounts already exist
    // (0xbc3 = AccountNotSystemOwned.)
    assertBankrunTxFailed(result, 0xbc3);
  });
});
