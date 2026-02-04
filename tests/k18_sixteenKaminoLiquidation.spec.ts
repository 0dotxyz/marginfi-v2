/**
 * # k18: 16 Total Position Liquidation Test (15 Kamino + 1 Borrow)
 *
 * This test verifies that liquidations work correctly when accounts have
 * the maximum 16 positions with 15 of them being Kamino positions.
 * This is enabled by the custom forward allocating heap (see allocator.rs)
 * which allows processing up to 16 positions without requestHeapFrame.
 *
 * Note: MAX_LENDING_ACCOUNT_BALANCES = 16 total positions, so we test 15 Kamino deposits + 1 borrow.
 */

import { BN } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  banksClient,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  globalProgramAdmin,
  kaminoGroup,
  klendBankrunProgram,
  oracles,
  users,
  verbose,
  bankrunProgram,
} from "./rootHooks";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
} from "./utils/kamino-instructions";
import {
  defaultKaminoBankConfig,
  simpleRefreshReserve,
  simpleRefreshObligation,
} from "./utils/kamino-utils";
import {
  deriveBankWithSeed,
  deriveLiquidityVaultAuthority,
  deriveBaseObligation,
} from "./utils/pdas";
import { dumpAccBalances, processBankrunTransaction } from "./utils/tools";
import {
  lendingMarketAuthPda,
  reserveLiqSupplyPda,
  reserveFeeVaultPda,
  reserveCollateralMintPda,
  reserveCollateralSupplyPda,
  LendingMarket,
  MarketWithAddress,
  BorrowRateCurve,
  CurvePoint,
  BorrowRateCurveFields,
  PriceFeed,
  AssetReserveConfig,
  updateEntireReserveConfigIx,
} from "@kamino-finance/klend-sdk";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { ComputeBudgetProgram } from "@solana/web3.js";
import Decimal from "decimal.js";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";

/** Number of Kamino banks to create for this test (16 total banks)
 * User will deposit into 15 Kamino banks + 1 USDC borrow = 16 total positions */
const NUM_KAMINO_BANKS = 16;
/** User deposits into 15 Kamino banks (leaves room for 1 liability = 16 total positions) */
const NUM_KAMINO_DEPOSITS = 15;
const USER_ACCOUNT = "user_account_k18";
const STARTING_SEED = 18000;
const LENDING_MARKET_SIZE = 4656;
const RESERVE_SIZE = 8616;

describe("k18: 16 Kamino position liquidation test", () => {
  let kaminoMarkets: PublicKey[] = [];
  let kaminoReserves: PublicKey[] = [];
  let kaminoBanks: PublicKey[] = [];
  let usdcBank: PublicKey;
  let lutAddress: PublicKey;
  let lut: AddressLookupTableAccount;

  it("Refresh oracles", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("Create 16 Kamino markets + reserves + banks", async () => {
    // Create all 16 markets/reserves/banks sequentially
    for (let i = 0; i < NUM_KAMINO_BANKS; i++) {
      // Create Kamino market
      const marketKeypair = Keypair.generate();
      const quoteCurrency = Array(32).fill(0); // USD quote currency
      const id = klendBankrunProgram.programId;
      const [lendingMarketAuthority] = lendingMarketAuthPda(
        marketKeypair.publicKey,
        id
      );

      const createMarketTx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: groupAdmin.wallet.publicKey,
          newAccountPubkey: marketKeypair.publicKey,
          space: LENDING_MARKET_SIZE + 8,
          lamports:
            await bankRunProvider.connection.getMinimumBalanceForRentExemption(
              LENDING_MARKET_SIZE + 8
            ),
          programId: id,
        }),
        await klendBankrunProgram.methods
          .initLendingMarket(quoteCurrency)
          .accounts({
            lendingMarketOwner: groupAdmin.wallet.publicKey,
            lendingMarket: marketKeypair.publicKey,
            lendingMarketAuthority,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .instruction()
      );

      await processBankrunTransaction(bankrunContext, createMarketTx, [
        groupAdmin.wallet,
        marketKeypair,
      ]);

      // Create Kamino reserve
      const reserveKeypair = Keypair.generate();
      const mint = ecosystem.tokenAMint.publicKey;

      const [reserveLiquiditySupply] = reserveLiqSupplyPda(
        marketKeypair.publicKey,
        mint,
        id
      );
      const [reserveFeeVault] = reserveFeeVaultPda(
        marketKeypair.publicKey,
        mint,
        id
      );
      const [collatMint] = reserveCollateralMintPda(
        marketKeypair.publicKey,
        mint,
        id
      );
      const [collatSupply] = reserveCollateralSupplyPda(
        marketKeypair.publicKey,
        mint,
        id
      );

      const createReserveTx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: groupAdmin.wallet.publicKey,
          newAccountPubkey: reserveKeypair.publicKey,
          space: RESERVE_SIZE + 8,
          lamports:
            await bankRunProvider.connection.getMinimumBalanceForRentExemption(
              RESERVE_SIZE + 8
            ),
          programId: id,
        }),
        await klendBankrunProgram.methods
          .initReserve()
          .accounts({
            lendingMarketOwner: groupAdmin.wallet.publicKey,
            lendingMarket: marketKeypair.publicKey,
            lendingMarketAuthority,
            reserve: reserveKeypair.publicKey,
            reserveLiquidityMint: mint,
            reserveLiquiditySupply,
            feeReceiver: reserveFeeVault,
            reserveCollateralMint: collatMint,
            reserveCollateralSupply: collatSupply,
            initialLiquiditySource: groupAdmin.tokenAAccount,
            rent: SYSVAR_RENT_PUBKEY,
            liquidityTokenProgram: TOKEN_PROGRAM_ID,
            collateralTokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .instruction()
      );

      await processBankrunTransaction(bankrunContext, createReserveTx, [
        groupAdmin.wallet,
        reserveKeypair,
      ]);

      // Update reserve config to make it operational
      const marketAcc: LendingMarket = LendingMarket.decode(
        (
          await bankRunProvider.connection.getAccountInfo(
            marketKeypair.publicKey
          )
        ).data
      );
      const marketWithAddress: MarketWithAddress = {
        address: marketKeypair.publicKey,
        state: marketAcc,
      };

      const borrowRateCurve = new BorrowRateCurve({
        points: [
          new CurvePoint({ utilizationRateBps: 0, borrowRateBps: 50000 }),
          new CurvePoint({ utilizationRateBps: 5000, borrowRateBps: 100000 }),
          new CurvePoint({ utilizationRateBps: 8000, borrowRateBps: 500000 }),
          new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
          ...Array(7).fill(
            new CurvePoint({
              utilizationRateBps: 10000,
              borrowRateBps: 1000000,
            })
          ),
        ],
      } as BorrowRateCurveFields);

      const assetReserveConfigParams = {
        loanToValuePct: 75,
        liquidationThresholdPct: 85,
        borrowRateCurve,
        depositLimit: new Decimal(1_000_000_000),
        borrowLimit: new Decimal(1_000_000_000),
      };

      const priceFeed: PriceFeed = {
        pythPrice: oracles.tokenAOracle.publicKey,
      };

      const assetReserveConfig = new AssetReserveConfig({
        mint: mint,
        mintTokenProgram: TOKEN_PROGRAM_ID,
        tokenName: "TOKEN_A",
        mintDecimals: ecosystem.tokenADecimals,
        priceFeed: priceFeed,
        ...assetReserveConfigParams,
      }).getReserveConfig();

      const updateReserveIx = updateEntireReserveConfigIx(
        marketWithAddress,
        reserveKeypair.publicKey,
        assetReserveConfig,
        klendBankrunProgram.programId
      );

      const updateReserveTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 }),
        updateReserveIx
      );

      await processBankrunTransaction(bankrunContext, updateReserveTx, [
        groupAdmin.wallet,
      ]);

      // Create marginfi Kamino bank
      const seed = new BN(STARTING_SEED + i);
      const config = defaultKaminoBankConfig(oracles.tokenAOracle.publicKey);

      const [bankKey] = deriveBankWithSeed(
        groupAdmin.mrgnBankrunProgram.programId,
        kaminoGroup.publicKey,
        mint,
        seed
      );

      const createBankTx = new Transaction().add(
        await makeAddKaminoBankIx(
          groupAdmin.mrgnBankrunProgram,
          {
            group: kaminoGroup.publicKey,
            feePayer: groupAdmin.wallet.publicKey,
            bankMint: mint,
            kaminoReserve: reserveKeypair.publicKey,
            kaminoMarket: marketKeypair.publicKey,
            oracle: oracles.tokenAOracle.publicKey,
          },
          {
            config,
            seed,
          }
        )
      );

      await processBankrunTransaction(bankrunContext, createBankTx, [
        groupAdmin.wallet,
      ]);

      // Initialize obligation for the Kamino bank
      const initObligationTx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
        await makeInitObligationIx(
          groupAdmin.mrgnBankrunProgram,
          {
            feePayer: groupAdmin.wallet.publicKey,
            bank: bankKey,
            signerTokenAccount: groupAdmin.tokenAAccount,
            lendingMarket: marketKeypair.publicKey,
            reserveLiquidityMint: mint,
            pythOracle: oracles.tokenAOracle.publicKey,
          },
          new BN(100)
        )
      );

      await processBankrunTransaction(bankrunContext, initObligationTx, [
        groupAdmin.wallet,
      ]);

      kaminoBanks.push(bankKey);
      kaminoMarkets.push(marketKeypair.publicKey);
      kaminoReserves.push(reserveKeypair.publicKey);
    }

    assert.equal(kaminoBanks.length, 16, "Should have created 16 Kamino banks");
  });

  it("(admin) Create USDC bank for borrowing", async () => {
    const { addBankWithSeed } = await import("./utils/group-instructions");
    const { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } = await import(
      "./utils/types"
    );
    const { bigNumberToWrappedI80F48 } = await import("@mrgnlabs/mrgn-common");

    const seed = new BN(STARTING_SEED + 100);
    const [bankKey] = deriveBankWithSeed(
      groupAdmin.mrgnBankrunProgram.programId,
      kaminoGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );

    const config = defaultBankConfig();
    config.assetWeightInit = bigNumberToWrappedI80F48(0.5);
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.6);
    config.depositLimit = new BN(100_000_000_000_000);
    config.borrowLimit = new BN(100_000_000_000_000);

    const configOracleIx = await groupAdmin.mrgnBankrunProgram.methods
      .lendingPoolConfigureBankOracle(
        ORACLE_SETUP_PYTH_PUSH,
        oracles.usdcOracle.publicKey
      )
      .accountsPartial({
        group: kaminoGroup.publicKey,
        bank: bankKey,
        admin: groupAdmin.wallet.publicKey,
      })
      .remainingAccounts([
        {
          pubkey: oracles.usdcOracle.publicKey,
          isSigner: false,
          isWritable: false,
        },
      ])
      .instruction();

    const tx = new Transaction().add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        config,
        seed,
      }),
      configOracleIx
    );

    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    usdcBank = bankKey;
  });

  it("(admin) Create account and seed USDC liquidity", async () => {
    const { accountInit, depositIx } = await import("./utils/user-instructions");
    const { createMintToInstruction } = await import("@solana/spl-token");

    // Create admin account
    const accountKeypair = Keypair.generate();
    let tx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [
      groupAdmin.wallet,
      accountKeypair,
    ]);

    groupAdmin.accounts.set(USER_ACCOUNT, accountKeypair.publicKey);

    // Seed USDC liquidity
    const depositAmount = new BN(100_000 * 10 ** ecosystem.usdcDecimals);

    tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        10_000_000 * 10 ** ecosystem.usdcDecimals
      ),
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: accountKeypair.publicKey,
        bank: usdcBank,
        tokenAccount: groupAdmin.usdcAccount,
        amount: depositAmount,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [
      globalProgramAdmin.wallet,
      groupAdmin.wallet,
    ]);
  });

  it("(user 0) Create marginfi account", async () => {
    const { accountInit } = await import("./utils/user-instructions");

    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
      await accountInit(users[0].mrgnProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: users[0].wallet.publicKey,
        feePayer: users[0].wallet.publicKey,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [
      users[0].wallet,
      accountKeypair,
    ]);

    users[0].accounts.set(USER_ACCOUNT, accountKeypair.publicKey);
  });

  it("(user 0) Deposits into 15 Kamino banks (leaves room for 1 borrow = 16 total)", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    // Small deposit amount per position (15 * 5 = 75 TOKEN_A worth $750)
    const depositAmount = new BN(5 * 10 ** ecosystem.tokenADecimals);

    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      const bank = kaminoBanks[i];
      const market = kaminoMarkets[i];
      const reserve = kaminoReserves[i];

      const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
        groupAdmin.mrgnBankrunProgram.programId,
        bank
      );
      const [obligation] = deriveBaseObligation(
        liquidityVaultAuthority,
        market
      );

      const tx = new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          reserve,
          market,
          oracles.tokenAOracle.publicKey
        ),
        await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
          reserve,
        ]),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: bank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          },
          depositAmount
        )
      );

      await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
    }

    // Verify all 15 positions were created
    const account = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const activeBalances = account.lendingAccount.balances.filter(
      (b) => !b.bankPk.equals(PublicKey.default)
    );
    assert.equal(
      activeBalances.length,
      15,
      "User should have 15 active positions (Kamino deposits)"
    );
  });

  it("(user 1) Create Address Lookup Table for liquidation", async () => {
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");
    const { getEpochAndSlot } = await import("./utils/stake-utils");

    const user = users[1];

    // Create the LUT
    const recentSlot = Number(await banksClient.getSlot());
    const [createLutIx, lutAddr] = AddressLookupTableProgram.createLookupTable({
      authority: user.wallet.publicKey,
      payer: user.wallet.publicKey,
      recentSlot: recentSlot - 1,
    });

    lutAddress = lutAddr;

    let createLutTx = new Transaction().add(createLutIx);
    createLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    createLutTx.sign(user.wallet);
    await banksClient.processTransaction(createLutTx);

    // Extend the LUT with all required accounts
    const allAddresses: PublicKey[] = [];

    // Add all 16 Kamino banks, oracles, and reserves
    for (let i = 0; i < NUM_KAMINO_BANKS; i++) {
      allAddresses.push(kaminoBanks[i]);
      allAddresses.push(oracles.tokenAOracle.publicKey);
      allAddresses.push(kaminoReserves[i]);
    }

    // Add the USDC bank and oracle
    allAddresses.push(usdcBank);
    allAddresses.push(oracles.usdcOracle.publicKey);

    // Extend in chunks of 20 addresses to avoid transaction size limits
    const chunkSize = 20;
    for (let i = 0; i < allAddresses.length; i += chunkSize) {
      const chunk = allAddresses.slice(i, i + chunkSize);

      let extendLutTx = new Transaction().add(
        AddressLookupTableProgram.extendLookupTable({
          authority: user.wallet.publicKey,
          payer: user.wallet.publicKey,
          lookupTable: lutAddress,
          addresses: chunk,
        })
      );
      extendLutTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      extendLutTx.sign(user.wallet);
      await banksClient.processTransaction(extendLutTx);
    }

    // Activate the LUT by warping the slot forward
    const ONE_MINUTE = 60;
    const slotsToAdvance = ONE_MINUTE * 0.4; // ~24 slots
    let { epoch: _, slot } = await getEpochAndSlot(banksClient);
    bankrunContext.warpToSlot(BigInt(slot + slotsToAdvance));
  });

  it("(user 0) Borrow USDC using 15 Kamino positions as collateral (16 total positions)", async () => {
    const { borrowIx, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");

    // Refresh oracles after slot warp
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    // Borrow enough to create a health risk when liability weight increases
    const borrowAmount = new BN(500 * 10 ** ecosystem.usdcDecimals);

    // Batch refresh the 15 Kamino reserves that user deposited into
    const reserveAccounts: {
      pubkey: PublicKey;
      isSigner: boolean;
      isWritable: boolean;
    }[] = [];
    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      reserveAccounts.push({
        pubkey: kaminoReserves[i],
        isSigner: false,
        isWritable: true,
      });
      reserveAccounts.push({
        pubkey: kaminoMarkets[i],
        isSigner: false,
        isWritable: false,
      });
    }

    const batchRefreshIx = await klendBankrunProgram.methods
      .refreshReservesBatch(true) // skip_price_updates = true
      .remainingAccounts(reserveAccounts)
      .instruction();

    // Build remaining accounts: all 15 Kamino positions + USDC bank
    const remainingAccounts: PublicKey[][] = [];
    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      remainingAccounts.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }
    // Include the borrow bank (USDC)
    remainingAccounts.push([usdcBank, oracles.usdcOracle.publicKey]);

    // Create the borrow instruction
    const borrowInstruction = await borrowIx(user.mrgnBankrunProgram, {
      marginfiAccount: userAccount,
      bank: usdcBank,
      tokenAccount: user.usdcAccount,
      remaining: composeRemainingAccounts(remainingAccounts),
      amount: borrowAmount,
    });

    // Fetch the activated LUT
    const lutRaw = await banksClient.getAccount(lutAddress);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    lut = new AddressLookupTableAccount({
      key: lutAddress,
      state: lutState,
    });

    // Add compute budget instruction
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    // Create versioned transaction with LUT
    const messageV0 = new TransactionMessage({
      payerKey: user.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: [computeBudgetIx, batchRefreshIx, borrowInstruction],
    }).compileToV0Message([lut]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([user.wallet]);
    await banksClient.processTransaction(versionedTx);

    // Verify user now has 16 positions (15 Kamino deposits + 1 USDC borrow)
    const account = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const activeBalances = account.lendingAccount.balances.filter(
      (b) => !b.bankPk.equals(PublicKey.default)
    );
    assert.equal(
      activeBalances.length,
      16,
      "User should have 16 active positions (15 Kamino + 1 USDC borrow)"
    );
  });

  it("(admin) Make user 0 unhealthy by increasing USDC liability weight", async () => {
    const { configureBank } = await import("./utils/group-instructions");
    const { blankBankConfigOptRaw } = await import("./utils/types");
    const { bigNumberToWrappedI80F48 } = await import("@mrgnlabs/mrgn-common");
    const { healthPulse, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");

    let config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(1.8); // 180%
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(1.7); // 170%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: usdcBank,
        bankConfigOpt: config,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    // Health pulse to update cache - need to use LUT due to 16 positions
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const positionAccounts: PublicKey[][] = [];
    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      positionAccounts.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }
    positionAccounts.push([usdcBank, oracles.usdcOracle.publicKey]);

    const healthPulseIx = await healthPulse(user.mrgnBankrunProgram, {
      marginfiAccount: userAccount,
      remaining: composeRemainingAccounts(positionAccounts),
    });

    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    const messageV0 = new TransactionMessage({
      payerKey: user.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: [computeBudgetIx, healthPulseIx],
    }).compileToV0Message([lut]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([user.wallet]);
    await banksClient.processTransaction(versionedTx);
  });

  it("(user 1) Create marginfi account and prepare for liquidation", async () => {
    const { accountInit, depositIx } = await import("./utils/user-instructions");
    const { makeKaminoDepositIx } = await import("./utils/kamino-instructions");
    const { simpleRefreshReserve, simpleRefreshObligation } = await import(
      "./utils/kamino-utils"
    );

    const user = users[1];

    // Create account
    const accountKeypair = Keypair.generate();
    let tx = new Transaction().add(
      await accountInit(user.mrgnProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
      accountKeypair,
    ]);
    user.accounts.set(USER_ACCOUNT, accountKeypair.publicKey);

    // Deposit USDC as collateral for liquidation
    const depositAmountUsdc = new BN(10_000 * 10 ** ecosystem.usdcDecimals);
    tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: accountKeypair.publicKey,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        amount: depositAmountUsdc,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    // Deposit into Kamino bank 0 to receive liquidated collateral
    const depositAmountTokenA = new BN(1 * 10 ** ecosystem.tokenADecimals);

    const [liquidityVaultAuthority0] = deriveLiquidityVaultAuthority(
      user.mrgnBankrunProgram.programId,
      kaminoBanks[0]
    );
    const [obligation0] = deriveBaseObligation(
      liquidityVaultAuthority0,
      kaminoMarkets[0]
    );

    tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        kaminoReserves[0],
        kaminoMarkets[0],
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        kaminoMarkets[0],
        obligation0,
        [kaminoReserves[0]]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: accountKeypair.publicKey,
          bank: kaminoBanks[0],
          signerTokenAccount: user.tokenAAccount,
          lendingMarket: kaminoMarkets[0],
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        },
        depositAmountTokenA
      )
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  });

  it("(user 1) Liquidate user 0 who has 16 Kamino positions", async () => {
    const { liquidateIx, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");
    const { dumpBankrunLogs } = await import("./utils/tools");

    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT);
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT);

    const liqorAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    const liqeeAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    if (verbose) {
      console.log("Liquidatee positions before liquidation:");
      dumpAccBalances(liqeeAcc);
    }

    const liabIndex = liqeeAcc.lendingAccount.balances.findIndex((balance) =>
      balance.bankPk.equals(usdcBank)
    );
    const liqeeAssetIndex = liqeeAcc.lendingAccount.balances.findIndex(
      (balance) => balance.bankPk.equals(kaminoBanks[0])
    );
    const liqorAssetIndex = liqorAcc.lendingAccount.balances.findIndex(
      (balance) => balance.bankPk.equals(kaminoBanks[0])
    );
    const liabBefore = wrappedI80F48toBigNumber(
      liqeeAcc.lendingAccount.balances[liabIndex].liabilityShares
    );
    const liqeeAssetBefore = wrappedI80F48toBigNumber(
      liqeeAcc.lendingAccount.balances[liqeeAssetIndex].assetShares
    );
    const liqorAssetBefore = wrappedI80F48toBigNumber(
      liqorAcc.lendingAccount.balances[liqorAssetIndex].assetShares
    );

    // Liquidate from Kamino bank 0
    const assetBankKey = kaminoBanks[0];
    const assetReserve = kaminoReserves[0];
    const liquidateAmount = new BN(3 * 10 ** ecosystem.tokenADecimals);

    // Build remaining accounts
    const remainingForLiq: PublicKey[] = [
      oracles.tokenAOracle.publicKey, // asset oracle
      assetReserve, // asset reserve
      oracles.usdcOracle.publicKey, // liability oracle
    ];

    // Liquidator positions (USDC + Kamino bank 0)
    const liquidatorPositions: PublicKey[][] = [
      [kaminoBanks[0], oracles.tokenAOracle.publicKey, kaminoReserves[0]],
      [usdcBank, oracles.usdcOracle.publicKey],
    ];

    // Liquidatee positions (15 Kamino banks + USDC borrow = 16 total)
    const liquidateePositions: PublicKey[][] = [];
    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      liquidateePositions.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }
    liquidateePositions.push([usdcBank, oracles.usdcOracle.publicKey]);

    const liquidatorAccounts = composeRemainingAccounts(liquidatorPositions);
    remainingForLiq.push(...liquidatorAccounts);
    const liquidateeAccounts = composeRemainingAccounts(liquidateePositions);
    remainingForLiq.push(...liquidateeAccounts);

    const liquidateInstruction = await liquidateIx(
      liquidator.mrgnBankrunProgram,
      {
        assetBankKey: assetBankKey,
        liabilityBankKey: usdcBank,
        liquidatorMarginfiAccount: liquidatorAccount,
        liquidateeMarginfiAccount: liquidateeAccount,
        remaining: remainingForLiq,
        amount: liquidateAmount,
        liquidateeAccounts: liquidateeAccounts.length,
        liquidatorAccounts: liquidatorAccounts.length,
      }
    );

    // Fetch the LUT
    const lutRaw = await banksClient.getAccount(lutAddress);
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lutAccount = new AddressLookupTableAccount({
      key: lutAddress,
      state: lutState,
    });

    // Batch refresh all 15 reserves (the ones liquidatee deposited into)
    const reserveAccounts: {
      pubkey: PublicKey;
      isSigner: boolean;
      isWritable: boolean;
    }[] = [];
    for (let i = 0; i < NUM_KAMINO_DEPOSITS; i++) {
      reserveAccounts.push({
        pubkey: kaminoReserves[i],
        isSigner: false,
        isWritable: true,
      });
      reserveAccounts.push({
        pubkey: kaminoMarkets[i],
        isSigner: false,
        isWritable: false,
      });
    }

    const batchRefreshIx = await klendBankrunProgram.methods
      .refreshReservesBatch(true)
      .remainingAccounts(reserveAccounts)
      .instruction();

    // Add compute budget
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    // Create versioned transaction with LUT
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: [computeBudgetIx, batchRefreshIx, liquidateInstruction],
    }).compileToV0Message([lutAccount]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([liquidator.wallet]);

    // Execute liquidation
    const result = await banksClient.tryProcessTransaction(versionedTx);
    if (result.result) {
      dumpBankrunLogs(result);
      throw new Error("Liquidation transaction failed");
    }
    if (verbose) {
      console.log("Liquidation succeeded!");
      dumpBankrunLogs(result);
    }

    // Verify liquidation occurred
    const liqorAccAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    const liqeeAccAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    if (verbose) {
      console.log("Liquidatee positions after liquidation:");
      dumpAccBalances(liqeeAccAfter);
    }

    const liabAfter = wrappedI80F48toBigNumber(
      liqeeAccAfter.lendingAccount.balances[liabIndex].liabilityShares
    );
    const liqeeAssetAfter = wrappedI80F48toBigNumber(
      liqeeAccAfter.lendingAccount.balances[liqeeAssetIndex].assetShares
    );
    const liqorAssetAfter = wrappedI80F48toBigNumber(
      liqorAccAfter.lendingAccount.balances[liqorAssetIndex].assetShares
    );

    // Verify liabilities decreased (liquidation worked)
    const liabDelta = Number(liabBefore) - Number(liabAfter);
    assert.isTrue(liabDelta > 0, "Liquidatee liabilities should have decreased");

    // Expected repayment using on-chain liquidation math (mirrors Rust ix)
    const conf = ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE; // 0.0212 with defaults
    const tokenALow = oracles.tokenAPrice * (1 - conf);
    const usdcHigh = oracles.usdcPrice * (1 + conf);
    const totalDiscount = 1 - 0.025 - 0.025; // liquidator fee + insurance fee
    const liquidateAmountTokens =
      liquidateAmount.toNumber() / 10 ** ecosystem.tokenADecimals;
    const expectedLiqeeRepayUsd =
      liquidateAmountTokens * tokenALow * totalDiscount;
    const expectedLiabDeltaNative =
      expectedLiqeeRepayUsd / usdcHigh * 10 ** ecosystem.usdcDecimals;

    assert.approximately(
      liabDelta,
      expectedLiabDeltaNative,
      5_000, // tolerance in native units
      "Liability repayment should match liquidation math"
    );

    // Asset movements: liquidator gains assets, liquidatee loses assets
    assert.isTrue(
      Number(liqorAssetAfter) > Number(liqorAssetBefore),
      "Liquidator should receive asset shares"
    );
    assert.isTrue(
      Number(liqeeAssetAfter) < Number(liqeeAssetBefore),
      "Liquidatee asset shares should decrease"
    );
  });
});
