import { BN } from "@coral-xyz/anchor";
import { ComputeBudgetProgram, PublicKey, Transaction } from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  kaminoGroup,
  MARKET,
  oracles,
  USDC_RESERVE,
  verbose,
  users,
  bankrunContext,
  bankRunProvider,
  bankrunProgram,
  klendBankrunProgram,
  TOKEN_A_RESERVE,
  KAMINO_TOKEN_A_BANK,
} from "./rootHooks";
import {
  assertBNEqual,
  assertU68F60Approx,
  assertI68F60Equal,
  assertI80F48Equal,
  assertKeysEqual,
  getTokenBalance,
  assertBankrunTxFailed,
} from "./utils/genericTests";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import { assert } from "chai";
import { processBankrunTransaction, safeGetAccountInfo, getBankrunTime } from "./utils/tools";
import { lendingMarketAuthPda } from "@kamino-finance/klend-sdk";
import { createAssociatedTokenAccountInstruction } from "@mrgnlabs/mrgn-common";
import { createMintToInstruction, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
} from "./utils/kamino-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveFeeVault,
  deriveInsuranceVault,
  deriveLiquidityVault,
  deriveLiquidityVaultAuthority,
  deriveUserMetadata,
} from "./utils/pdas";
import { ASSET_TAG_KAMINO, KLEND_PROGRAM_ID } from "./utils/types";

let ctx: ProgramTestContext;
const seed = new BN(555);
let mrgnID: PublicKey;

let market: PublicKey;
let usdcReserve: PublicKey;
let tokenAReserve: PublicKey;

describe("k05: Init Kamino banks", () => {
  before(async () => {
    ctx = bankrunContext;
    mrgnID = bankrunProgram.programId;

    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  });

  it("(admin) Add Kamino bank (kamino USDC) - happy path", async () => {
    let defaultConfig = defaultKaminoBankConfig(
      oracles.usdcOracle.publicKey
    );
    const now = await getBankrunTime(ctx);

    const [bankKey] = deriveBankWithSeed(
      mrgnID,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );

    const tx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: usdcReserve,
          kaminoMarket: market,
          oracle: oracles.usdcOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: seed,
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

    kaminoAccounts.set(KAMINO_USDC_BANK, bankKey);
    if (verbose) {
      console.log("Init USDC Kamino bank: " + bankKey);
    }

    const group = await bankrunProgram.account.marginfiGroup.fetch(
      kaminoGroup.publicKey
    );
    assert.equal(group.banks, 1);

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    const config = bank.config;

    assertKeysEqual(bank.mint, ecosystem.usdcMint.publicKey);
    assert.equal(bank.mintDecimals, ecosystem.usdcDecimals);
    assertKeysEqual(bank.group, kaminoGroup.publicKey);

    assertKeysEqual(config.oracleKeys[0], oracles.usdcOracle.publicKey);

    assertBNEqual(bank.flags, 16); // CLOSE_ENABLED_FLAG

    // Note: this doesn't work if we've warped the banks clock
    // let lastUpdate = bank.lastUpdate.toNumber();
    // assert.approximately(now, lastUpdate, 2);
    assertI80F48Equal(config.assetWeightInit, 1);
    assertI80F48Equal(config.assetWeightMaint, 1);
    assertBNEqual(config.depositLimit, 10_000_000_000_000); // 10mil usdc because big interest needed

    assert.deepEqual(config.operationalState, { operational: {} });
    assert.deepEqual(config.oracleSetup, { kaminoPythPush: {} });
    assertBNEqual(config.borrowLimit, 0);
    assert.deepEqual(config.riskTier, { collateral: {} });
    assertBNEqual(config.totalAssetValueInitLimit, 1_000_000_000_000);
    assert.equal(config.oracleMaxAge, 100);

    // Check that the vaults were initialized properly
    const [liquidityVault] = deriveLiquidityVault(mrgnID, bankKey);
    assertKeysEqual(bank.liquidityVault, liquidityVault);
    const [insuranceVault] = deriveInsuranceVault(mrgnID, bankKey);
    assertKeysEqual(bank.insuranceVault, insuranceVault);
    const [feeVault] = deriveFeeVault(mrgnID, bankKey);
    assertKeysEqual(bank.feeVault, feeVault);

    // Kamino specific fields
    assertKeysEqual(bank.integrationAcc1, usdcReserve);
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      mrgnID,
      bankKey
    );
    const [obligation] = deriveBaseObligation(liquidityVaultAuthority, market);
    assertKeysEqual(bank.integrationAcc2, obligation);
    assert.equal(config.assetTag, ASSET_TAG_KAMINO);
    assertKeysEqual(bank.config.oracleKeys[0], oracles.usdcOracle.publicKey);
    assertKeysEqual(bank.config.oracleKeys[1], usdcReserve);
    assertKeysEqual(bank.integrationAcc1, usdcReserve);

    // Compare against the underlying Kamino reserve
    const usdcReserveData = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve
    );
    assertKeysEqual(usdcReserveData.liquidity.mintPubkey, bank.mint);
    assertBNEqual(usdcReserveData.liquidity.mintDecimals, bank.mintDecimals);
  });

  it("(permissionless - user 0) inits obligation with the wrong mint - should fail", async () => {
    const usr = users[0];
    const bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    let tx1 = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: usr.wallet.publicKey,
          bank: bank,
          signerTokenAccount: usr.tokenAAccount, // wrong
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey, // wrong
          pythOracle: oracles.tokenAOracle.publicKey, // wrong
        },
        new BN(999)
      )
    );
    let result1 = await processBankrunTransaction(ctx, tx1, [usr.wallet], true);
    // Generic ConstraintTokenMint
    assertBankrunTxFailed(result1, 2014);

    let tx2 = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: usr.wallet.publicKey,
          bank: bank,
          signerTokenAccount: usr.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          pythOracle: oracles.tokenAOracle.publicKey, // wrong
        },
        new BN(999)
      )
    );
    let result2 = await processBankrunTransaction(ctx, tx2, [usr.wallet], true);
    // PythPushWrongAccountOwner
    assertBankrunTxFailed(result2, 6054);
  });

  /*
    Note: If the reserve has farms enabled, creating the obligation also requires passing those (see
    k14 for an example), and if the farm changes, you will also have to run
    `InitObligationFarmsForReserve` (this can be run directly through Kamino's program and is
    permissionless.)
   */
  const nominalAmount = 500;
  it("(permissionless - user 0) initializes obligation for the USDC bank", async () => {
    const user = users[0];
    const bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    const [authority] = deriveLiquidityVaultAuthority(mrgnID, bank);

    const [userMetadata] = deriveUserMetadata(KLEND_PROGRAM_ID, authority);
    const [obligation] = deriveBaseObligation(authority, market);

    if (verbose) {
      console.log("Initializing obligation for USDC bank:");
      console.log("- Liq Vault Auth:", authority.toString());
      console.log("- Obligation:", obligation.toString());
      console.log("- User Metadata:", userMetadata.toString());
      console.log("- Lending Market:", market.toString());
    }

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      user.usdcAccount
    );

    let tx = new Transaction().add(
      // Note: This can BARELY complete within the default limit most of the time, but you should
      // increase the CU limit just in case.
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        groupAdmin.mrgnBankrunProgram,
        {
          feePayer: user.wallet.publicKey,
          bank: bank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          pythOracle: oracles.usdcOracle.publicKey,
        },
        new BN(nominalAmount)
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    // Store these to avoid needing to derive again in later tests
    const bankKey = bank.toString();
    kaminoAccounts.set(`${bankKey}_LIQUIDITY_VAULT_AUTHORITY`, authority);
    kaminoAccounts.set(`${bankKey}_OBLIGATION`, obligation);
    kaminoAccounts.set(`${bankKey}_USER_METADATA`, userMetadata);

    const [metaAcc, obAcc, userUsdcAfter] = await Promise.all([
      klendBankrunProgram.account.userMetadata.fetch(userMetadata),
      klendBankrunProgram.account.obligation.fetch(obligation),
      getTokenBalance(bankRunProvider, user.usdcAccount),
    ]);

    assertKeysEqual(metaAcc.owner, authority);
    assertKeysEqual(obAcc.owner, authority);
    // We have deposited a nominal amount into the obligation
    assertKeysEqual(obAcc.deposits[0].depositReserve, usdcReserve);
    assertBNEqual(obAcc.deposits[0].depositedAmount, nominalAmount);
    // Note: 0 until the obligation is refreshed
    assertI68F60Equal(obAcc.deposits[0].marketValueSf, 0);

    tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ])
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
    let obAfterRefresh = await klendBankrunProgram.account.obligation.fetch(
      obligation
    );
    // Note: Now we see the expected value (USDC = $1 so 500 = $(500/10^decimals))
    assertU68F60Approx(
      obAfterRefresh.deposits[0].marketValueSf,
      nominalAmount / 10 ** ecosystem.usdcDecimals
    );

    // User was debited the norminal fee to keep the obligation open.
    assert.equal(userUsdcBefore - userUsdcAfter, 500);
  });

  it("(admin) Init Token A bank + obligation", async () => {
    const user = groupAdmin;
    let defaultConfig = defaultKaminoBankConfig(oracles.tokenAOracle.publicKey);

    const [tokenABankKey] = deriveBankWithSeed(
      mrgnID,
      kaminoGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );

    const tx = new Transaction().add(
      await makeAddKaminoBankIx(
        user.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: user.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: seed,
        }
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
    kaminoAccounts.set(KAMINO_TOKEN_A_BANK, tokenABankKey);

    let initObligationTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(
        user.mrgnBankrunProgram,
        {
          feePayer: user.wallet.publicKey,
          bank: tokenABankKey,
          signerTokenAccount: user.tokenAAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          pythOracle: oracles.tokenAOracle.publicKey,
        },
        new BN(nominalAmount)
      )
    );
    await processBankrunTransaction(ctx, initObligationTx, [user.wallet]);

    // Store the Token A bank obligation (same as USDC bank pattern)
    const [tokenAAuthority] = deriveLiquidityVaultAuthority(
      mrgnID,
      tokenABankKey
    );
    const [tokenAUserMetadata] = deriveUserMetadata(
      KLEND_PROGRAM_ID,
      tokenAAuthority
    );
    const [tokenAObligation] = deriveBaseObligation(tokenAAuthority, market);

    const tokenABankKey_str = tokenABankKey.toString();
    kaminoAccounts.set(
      `${tokenABankKey_str}_LIQUIDITY_VAULT_AUTHORITY`,
      tokenAAuthority
    );
    kaminoAccounts.set(`${tokenABankKey_str}_OBLIGATION`, tokenAObligation);
    kaminoAccounts.set(
      `${tokenABankKey_str}_USER_METADATA`,
      tokenAUserMetadata
    );
  });

  it("(user 0) Tries to init bank - admin only, should fail", async () => {
    const throwawaySeed = new BN(999);
    const user = users[0];
    let defaultConfig = defaultKaminoBankConfig(oracles.tokenAOracle.publicKey);

    const tx = new Transaction().add(
      await makeAddKaminoBankIx(
        user.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: user.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
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

  it("(admin) Tries pass the wrong reserve/mint for this asset - should fail", async () => {
    const throwawaySeed = new BN(999);
    const usr = groupAdmin;
    let defaultConfig = defaultKaminoBankConfig(oracles.tokenAOracle.publicKey);

    const tx1 = new Transaction().add(
      await makeAddKaminoBankIx(
        usr.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: usdcReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: throwawaySeed,
        }
      )
    );
    let result1 = await processBankrunTransaction(ctx, tx1, [usr.wallet], true);
    // KaminoReserveMintAddressMismatch
    assertBankrunTxFailed(result1, 6203);

    const tx2 = new Transaction().add(
      await makeAddKaminoBankIx(
        usr.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: defaultConfig,
          seed: throwawaySeed,
        }
      )
    );
    let result2 = await processBankrunTransaction(ctx, tx2, [usr.wallet], true);
    // KaminoReserveMintAddressMismatch
    assertBankrunTxFailed(result2, 6203);
  });

  it("(admin) Tries to use the wrong oracle type - should fail", async () => {
    const throwawaySeed = new BN(999);
    const usr = groupAdmin;
    let config1 = defaultKaminoBankConfig(oracles.tokenAOracleFeed.publicKey);
    config1.oracleSetup = {
      pythPushOracle: {},
    };
    const tx1 = new Transaction().add(
      await makeAddKaminoBankIx(
        usr.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: config1,
          seed: throwawaySeed,
        }
      )
    );
    let result1 = await processBankrunTransaction(ctx, tx1, [usr.wallet], true);
    // KaminoInvalidOracleSetup
    assertBankrunTxFailed(result1, 6211);

    let config2 = defaultKaminoBankConfig(oracles.tokenAOracleFeed.publicKey);
    // For Pyth pull, should be the feed, not the oracle itself
    config2.oracle = oracles.tokenAOracle.publicKey;
    const tx2 = new Transaction().add(
      await makeAddKaminoBankIx(
        usr.mrgnBankrunProgram,
        {
          group: kaminoGroup.publicKey,
          feePayer: usr.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: config2,
          seed: throwawaySeed,
        }
      )
    );
    let result2 = await processBankrunTransaction(ctx, tx2, [usr.wallet], true);
    // KaminoReserveMintAddressMismatch
    assertBankrunTxFailed(result2, 6203);
  });
});
