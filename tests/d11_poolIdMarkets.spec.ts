import { assert } from "chai";
import { PublicKey, Transaction } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
import { createMintToInstruction } from "@solana/spl-token";
import {
  bankrunContext,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  users,
  oracles,
  bankrunProgram,
  driftBankrunProgram,
  driftAccounts,
  driftGroup,
  DRIFT_TOKEN_A_PULL_ORACLE,
  bankRunProvider,
} from "./rootHooks";
import { USER_ACCOUNT_D } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import { assertKeysEqual, getTokenBalance } from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { makeInitializeSpotMarketIx } from "./utils/drift-sdk";
import { deriveSpotMarketPDA } from "./utils/pdas";
import {
  getSpotMarketAccount,
  defaultSpotMarketConfig,
  DriftOracleSourceValues,
  defaultDriftBankConfig,
  USDC_POOL2_MARKET_INDEX,
  TOKEN_A_POOL3_MARKET_INDEX,
  POOL2_ID,
  POOL3_ID,
} from "./utils/drift-utils";
import {
  makeAddDriftBankIx,
  makeDriftDepositIx as makeMarginfiDriftDepositIx,
  makeInitDriftUserIx,
} from "./utils/drift-instructions";
import { deriveBankWithSeed } from "./utils/pdas";

describe("d11: Drift Pool ID Markets", () => {
  let usdcPool2Market: PublicKey;
  let tokenAPool3Market: PublicKey;
  let usdcPool2Bank: PublicKey;
  let tokenAPool3Bank: PublicKey;
  let driftState: PublicKey;

  before(async () => {
    const [statePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("drift_state")],
      driftBankrunProgram.programId
    );
    driftState = statePDA;
  });

  describe("Initialize Drift markets with custom pool IDs", () => {
    it("Initialize USDC spot market with pool ID 2", async () => {
      // 1. Next available market index, drift uses global incrementing market indexes
      const marketIndex = USDC_POOL2_MARKET_INDEX;

      // 2. Initialize spot market (starts with pool_id = 0)
      const config = defaultSpotMarketConfig();

      const initMarketIx = await makeInitializeSpotMarketIx(
        driftBankrunProgram,
        {
          admin: groupAdmin.wallet.publicKey,
          spotMarketMint: ecosystem.usdcMint.publicKey,
          oracle: PublicKey.default, // No oracle for quote asset
        },
        {
          optimalUtilization: config.optimalUtilization,
          optimalRate: config.optimalRate,
          maxRate: config.maxRate,
          oracleSource: DriftOracleSourceValues.quoteAsset, // Quote asset oracle source
          initialAssetWeight: config.initialAssetWeight,
          maintenanceAssetWeight: config.maintenanceAssetWeight,
          initialLiabilityWeight: config.initialLiabilityWeight,
          maintenanceLiabilityWeight: config.maintenanceLiabilityWeight,
          marketIndex,
          activeStatus: false, // Create as initialized (not active) so we can update pool ID
        }
      );

      // 3. Update pool ID to 2
      const [spotMarketPDA] = deriveSpotMarketPDA(
        driftBankrunProgram.programId,
        marketIndex
      );
      const updatePoolIdIx = await driftBankrunProgram.methods
        .updateSpotMarketPoolId(POOL2_ID)
        .accounts({
          admin: groupAdmin.wallet.publicKey,
          state: driftState,
          spotMarket: spotMarketPDA,
        })
        .instruction();

      // 4. Activate the market
      const activateIx = await driftBankrunProgram.methods
        .updateSpotMarketStatus({ active: {} })
        .accounts({
          admin: groupAdmin.wallet.publicKey,
          state: driftState,
          spotMarket: spotMarketPDA,
        })
        .instruction();

      const tx = new Transaction()
        .add(initMarketIx)
        .add(updatePoolIdIx)
        .add(activateIx);

      await processBankrunTransaction(
        bankrunContext,
        tx,
        [groupAdmin.wallet],
        false,
        true
      );

      usdcPool2Market = spotMarketPDA;
      driftAccounts.set("DRIFT_USDC_POOL2_MARKET", spotMarketPDA);

      const spotMarket = await getSpotMarketAccount(
        driftBankrunProgram,
        marketIndex
      );
      assert.equal(spotMarket.poolId, POOL2_ID);
      assert.equal(spotMarket.marketIndex, marketIndex);
    });

    it("Initialize Token A spot market with pool ID 3", async () => {
      const marketIndex = TOKEN_A_POOL3_MARKET_INDEX;

      const tokenAOracle = driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE);

      const config = defaultSpotMarketConfig();

      const initMarketIx = await makeInitializeSpotMarketIx(
        driftBankrunProgram,
        {
          admin: groupAdmin.wallet.publicKey,
          spotMarketMint: ecosystem.tokenAMint.publicKey,
          oracle: tokenAOracle, // Use existing Drift-specific Token A oracle
        },
        {
          optimalUtilization: config.optimalUtilization,
          optimalRate: config.optimalRate,
          maxRate: config.maxRate,
          oracleSource: DriftOracleSourceValues.pythPull,
          initialAssetWeight: config.initialAssetWeight,
          maintenanceAssetWeight: config.maintenanceAssetWeight,
          initialLiabilityWeight: config.initialLiabilityWeight,
          maintenanceLiabilityWeight: config.maintenanceLiabilityWeight,
          marketIndex,
          activeStatus: false, // Create as initialized (not active) so we can update pool ID
        }
      );

      // Update pool ID to 3
      const [spotMarketPDA] = deriveSpotMarketPDA(
        driftBankrunProgram.programId,
        marketIndex
      );
      const updatePoolIdIx = await driftBankrunProgram.methods
        .updateSpotMarketPoolId(POOL3_ID)
        .accounts({
          admin: groupAdmin.wallet.publicKey,
          state: driftState,
          spotMarket: spotMarketPDA,
        })
        .instruction();

      const activateIx = await driftBankrunProgram.methods
        .updateSpotMarketStatus({ active: {} })
        .accounts({
          admin: groupAdmin.wallet.publicKey,
          state: driftState,
          spotMarket: spotMarketPDA,
        })
        .instruction();

      const tx = new Transaction()
        .add(initMarketIx)
        .add(updatePoolIdIx)
        .add(activateIx);

      await processBankrunTransaction(
        bankrunContext,
        tx,
        [groupAdmin.wallet],
        false,
        true
      );

      tokenAPool3Market = spotMarketPDA;
      driftAccounts.set("DRIFT_TOKEN_A_POOL3_MARKET", spotMarketPDA);

      const spotMarket = await getSpotMarketAccount(
        driftBankrunProgram,
        marketIndex
      );
      assert.equal(spotMarket.poolId, POOL3_ID);
      assert.equal(spotMarket.marketIndex, marketIndex);
    });
  });

  describe("Add banks to existing Drift group", () => {
    it("Add Drift bank for USDC (pool ID 2)", async () => {
      const seed = new BN(700);
      const [bankKey] = deriveBankWithSeed(
        bankrunProgram.programId,
        driftGroup.publicKey,
        ecosystem.usdcMint.publicKey,
        seed
      );

      const config = defaultDriftBankConfig(oracles.usdcOracle.publicKey);

      const addBankIx = await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          integrationAcc1: usdcPool2Market,
          oracle: oracles.usdcOracle.publicKey,
        },
        { config, seed }
      );

      const tx = new Transaction().add(addBankIx);
      await processBankrunTransaction(
        bankrunContext,
        tx,
        [groupAdmin.wallet],
        false,
        true
      );

      usdcPool2Bank = bankKey;
      driftAccounts.set("DRIFT_USDC_POOL2_BANK", bankKey);

      const bank = await groupAdmin.mrgnBankrunProgram.account.bank.fetch(
        bankKey
      );
      assertKeysEqual(bank.integrationAcc1, usdcPool2Market);
    });

    it("Add Drift bank for Token A (pool ID 3)", async () => {
      const seed = new BN(701);
      const [bankKey] = deriveBankWithSeed(
        bankrunProgram.programId,
        driftGroup.publicKey,
        ecosystem.tokenAMint.publicKey,
        seed
      );

      const config = defaultDriftBankConfig(oracles.tokenAOracle.publicKey);

      const addBankIx = await makeAddDriftBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: driftGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenAPool3Market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        { config, seed }
      );

      const tx = new Transaction().add(addBankIx);
      await processBankrunTransaction(
        bankrunContext,
        tx,
        [groupAdmin.wallet],
        false,
        true
      );

      tokenAPool3Bank = bankKey;
      driftAccounts.set("DRIFT_TOKEN_A_POOL3_BANK", bankKey);

      const bank = await groupAdmin.mrgnBankrunProgram.account.bank.fetch(
        bankKey
      );
      assertKeysEqual(bank.integrationAcc1, tokenAPool3Market);
    });
  });

  describe("Initialize Drift user accounts for banks", () => {
    it("Fund groupAdmin with tokens for initialization", async () => {
      const fundAmount = new BN(1000);
      const fundTx = new Transaction()
        .add(
          createMintToInstruction(
            ecosystem.usdcMint.publicKey,
            groupAdmin.usdcAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.toNumber()
          )
        )
        .add(
          createMintToInstruction(
            ecosystem.tokenAMint.publicKey,
            groupAdmin.tokenAAccount,
            globalProgramAdmin.wallet.publicKey,
            fundAmount.toNumber()
          )
        );
      await processBankrunTransaction(bankrunContext, fundTx, [
        globalProgramAdmin.wallet,
      ]);
    });

    it("Initialize Drift user for USDC bank (pool ID 2)", async () => {
      const initUserAmount = new BN(100);
      const initUserTx = new Transaction().add(
        await makeInitDriftUserIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: usdcPool2Bank,
            signerTokenAccount: groupAdmin.usdcAccount,
          },
          {
            amount: initUserAmount,
          },
          USDC_POOL2_MARKET_INDEX // USDC market index (pool 2)
        )
      );
      await processBankrunTransaction(
        bankrunContext,
        initUserTx,
        [groupAdmin.wallet],
        false,
        true
      );
    });

    it("Initialize Drift user for Token A bank (pool ID 3)", async () => {
      const initUserAmount = new BN(100);
      const initUserTx = new Transaction().add(
        await makeInitDriftUserIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: tokenAPool3Bank,
            signerTokenAccount: groupAdmin.tokenAAccount,
            driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
          },
          {
            amount: initUserAmount,
          },
          TOKEN_A_POOL3_MARKET_INDEX
        )
      );
      await processBankrunTransaction(
        bankrunContext,
        initUserTx,
        [groupAdmin.wallet],
        false,
        true
      );
    });
  });

  describe("User operations with pool ID validation", () => {
    it("User deposits to USDC bank (pool ID 2) - should succeed", async () => {
      const userAccount = users[0].accounts.get(USER_ACCOUNT_D);
      assert.ok(userAccount);

      const usdcBalance = await getTokenBalance(
        bankRunProvider,
        users[0].usdcAccount
      );
      assert.ok(usdcBalance > 0);

      const depositAmount = new BN(1);

      const depositIx = await makeMarginfiDriftDepositIx(
        users[0].mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: usdcPool2Bank,
          signerTokenAccount: users[0].usdcAccount,
        },
        depositAmount,
        USDC_POOL2_MARKET_INDEX
      );

      const depositTx = new Transaction().add(depositIx);
      await processBankrunTransaction(
        bankrunContext,
        depositTx,
        [users[0].wallet],
        false,
        true
      );

      const bankAccount = await users[0].mrgnBankrunProgram.account.bank.fetch(
        usdcPool2Bank
      );
      const driftUserAccount = await driftBankrunProgram.account.user.fetch(
        bankAccount.integrationAcc2
      );
      assert.equal(driftUserAccount.poolId, POOL2_ID);

      const userAccountData =
        await users[0].mrgnBankrunProgram.account.marginfiAccount.fetch(
          userAccount
        );
      const balanceIndex = userAccountData.lendingAccount.balances.findIndex(
        (b) => b.active && b.bankPk.equals(usdcPool2Bank)
      );
      assert.ok(balanceIndex >= 0);
      const balance = userAccountData.lendingAccount.balances[balanceIndex];
      const assetSharesBN = new BN(
        wrappedI80F48toBigNumber(balance.assetShares).toString()
      );
      assert.ok(assetSharesBN.gt(new BN(0)));
    });

    it("User deposits to Token A bank (pool ID 3) - should succeed", async () => {
      const userAccount = users[1].accounts.get(USER_ACCOUNT_D);
      assert.ok(userAccount);

      const tokenABalance = await getTokenBalance(
        bankRunProvider,
        users[1].tokenAAccount
      );
      assert.ok(tokenABalance > 0);

      // Deposit just 1 unit to Token A bank with pool ID 3
      const depositAmount = new BN(1);

      const depositIx = await makeMarginfiDriftDepositIx(
        users[1].mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: tokenAPool3Bank,
          signerTokenAccount: users[1].tokenAAccount,
          driftOracle: driftAccounts.get(DRIFT_TOKEN_A_PULL_ORACLE),
        },
        depositAmount,
        TOKEN_A_POOL3_MARKET_INDEX // marketIndex for Token A pool 3
      );

      const depositTx = new Transaction().add(depositIx);
      await processBankrunTransaction(
        bankrunContext,
        depositTx,
        [users[1].wallet],
        false,
        true
      );

      const bankAccount = await users[1].mrgnBankrunProgram.account.bank.fetch(
        tokenAPool3Bank
      );
      const driftUserAccount = await driftBankrunProgram.account.user.fetch(
        bankAccount.integrationAcc2
      );
      assert.equal(driftUserAccount.poolId, POOL3_ID);

      const userAccountData =
        await users[1].mrgnBankrunProgram.account.marginfiAccount.fetch(
          userAccount
        );
      const balanceIndex = userAccountData.lendingAccount.balances.findIndex(
        (b) => b.active && b.bankPk.equals(tokenAPool3Bank)
      );
      assert.ok(balanceIndex >= 0);
      const balance = userAccountData.lendingAccount.balances[balanceIndex];
      const assetSharesBN = new BN(
        wrappedI80F48toBigNumber(balance.assetShares).toString()
      );
      assert.ok(assetSharesBN.gt(new BN(0)));
    });

    it("Verify deposits from multiple users work with different pool IDs", async () => {
      // Both users should be able to deposit to their respective banks
      // even though the banks have different pool IDs

      const user0Data =
        await users[0].mrgnBankrunProgram.account.marginfiAccount.fetch(
          users[0].accounts.get(USER_ACCOUNT_D)
        );
      const user1Data =
        await users[1].mrgnBankrunProgram.account.marginfiAccount.fetch(
          users[1].accounts.get(USER_ACCOUNT_D)
        );

      const user0Balance = user0Data.lendingAccount.balances.find(
        (b) => b.active && b.bankPk.equals(usdcPool2Bank)
      );
      assert.ok(user0Balance);

      const user1Balance = user1Data.lendingAccount.balances.find(
        (b) => b.active && b.bankPk.equals(tokenAPool3Bank)
      );
      assert.ok(user1Balance);
    });
  });
});
