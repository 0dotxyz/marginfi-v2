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
  Reserve,
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


const NUM_KAMINO_BANKS_FOR_TESTING = 15;
const NUM_REGULAR_TOKEN_A_BANKS = 7;
const USER_ACCOUNT = "user_account_k17";
const STARTING_SEED = 17000;
const LENDING_MARKET_SIZE = 4656;
const RESERVE_SIZE = 8616;

describe("k17: Limits test - 8 Kamino + 7 regular TOKEN_A deposits, liquidation with LUT", () => {
  let kaminoMarkets: PublicKey[] = [];
  let kaminoReserves: PublicKey[] = [];
  let kaminoBanks: PublicKey[] = [];
  let regularTokenABanks: PublicKey[] = [];
  let regularBank: PublicKey;
  let lutAddress: PublicKey;
  let lut: AddressLookupTableAccount;

  it("Refresh oracles", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("Create 9 markets + 9 reserves + 9 Kamino banks", async () => {
    // Create all markets/reserves/banks sequentially
    for (let i = 0; i < NUM_KAMINO_BANKS_FOR_TESTING; i++) {
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
  });

  it("Create 7 regular TOKEN_A banks (non-Kamino)", async () => {
    const { addBankWithSeed } = await import("./utils/group-instructions");
    const { deriveBankWithSeed } = await import("./utils/pdas");
    const { defaultBankConfig, ORACLE_SETUP_PYTH_PUSH } = await import(
      "./utils/types"
    );

    // Create 7 regular TOKEN_A banks with different seeds
    const tokenASeedOffset = 20000; // Use different seed range to avoid conflicts with Kamino banks

    for (let i = 0; i < NUM_REGULAR_TOKEN_A_BANKS; i++) {
      const seed = new BN(STARTING_SEED + tokenASeedOffset + i);
      const config = defaultBankConfig();

      const [bankKey] = deriveBankWithSeed(
        groupAdmin.mrgnBankrunProgram.programId,
        kaminoGroup.publicKey,
        ecosystem.tokenAMint.publicKey,
        seed
      );

      // Create the bank
      const addBankTx = new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: kaminoGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          config,
          seed,
        })
      );

      await processBankrunTransaction(bankrunContext, addBankTx, [
        groupAdmin.wallet,
      ]);

      // Configure oracle separately - PYTH_PUSH oracles pass oracle account in remaining accounts
      const configOracleIx = await groupAdmin.mrgnBankrunProgram.methods
        .lendingPoolConfigureBankOracle(
          ORACLE_SETUP_PYTH_PUSH,
          oracles.tokenAOracle.publicKey
        )
        .accountsPartial({
          group: kaminoGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
          bank: bankKey,
        })
        .remainingAccounts([
          {
            pubkey: oracles.tokenAOracle.publicKey,
            isSigner: false,
            isWritable: false,
          },
        ])
        .instruction();

      const oracleTx = new Transaction().add(configOracleIx);
      await processBankrunTransaction(bankrunContext, oracleTx, [
        groupAdmin.wallet,
      ]);

      regularTokenABanks.push(bankKey);
    }
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

  it("(user 0) Deposits into 8 Kamino + 7 regular TOKEN_A banks", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    // Reduced deposit amount from 20 to 10 to maintain similar total collateral with 15 positions (15*10=150 vs original 8*20=160)
    const depositAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);

    // Deposit into first 8 Kamino banks (0-7), leaving bank 8 unused for liquidator test
    for (let i = 0; i < 8; i++) {
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

    // Now deposit into 7 regular TOKEN_A banks

    const { depositIx } = await import("./utils/user-instructions");

    for (let i = 0; i < NUM_REGULAR_TOKEN_A_BANKS; i++) {
      const bank = regularTokenABanks[i];

      const depositTx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank,
          tokenAccount: user.tokenAAccount,
          amount: depositAmount,
        })
      );

      await processBankrunTransaction(bankrunContext, depositTx, [user.wallet]);
    }
  });

  it("(admin) Create admin account on kaminoGroup", async () => {
    const { accountInit } = await import("./utils/user-instructions");

    const accountKeypair = Keypair.generate();
    const tx = new Transaction().add(
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
  });

  it("(admin) Create regular USDC bank for borrowing", async () => {
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

    regularBank = bankKey;
  });

  it("(admin) Seed liquidity in USDC bank", async () => {
    const { depositIx } = await import("./utils/user-instructions");
    const { createMintToInstruction } = await import("@solana/spl-token");

    const depositAmount = new BN(100_000 * 10 ** ecosystem.usdcDecimals);

    const tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        10_000_000 * 10 ** ecosystem.usdcDecimals
      ),
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: groupAdmin.accounts.get(USER_ACCOUNT),
        bank: regularBank,
        tokenAccount: groupAdmin.usdcAccount,
        amount: depositAmount,
      })
    );

    await processBankrunTransaction(bankrunContext, tx, [
      globalProgramAdmin.wallet,
      groupAdmin.wallet,
    ]);
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

    // Add all 9 Kamino banks, oracles, and reserves (bank 8 will be used by liquidator test)
    for (let i = 0; i < NUM_KAMINO_BANKS_FOR_TESTING; i++) {
      allAddresses.push(kaminoBanks[i]);
      allAddresses.push(oracles.tokenAOracle.publicKey);
      allAddresses.push(kaminoReserves[i]);
    }

    // Add 7 regular TOKEN_A banks and oracle
    for (let i = 0; i < NUM_REGULAR_TOKEN_A_BANKS; i++) {
      allAddresses.push(regularTokenABanks[i]);
      allAddresses.push(oracles.tokenAOracle.publicKey);
    }

    // Add the regular USDC bank and oracle
    allAddresses.push(regularBank);
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

  it("(user 0) Borrow USDC from regular bank using LUT", async () => {
    const { borrowIx, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");

    // Refresh oracles after slot warp
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const borrowAmount = new BN(1000 * 10 ** ecosystem.usdcDecimals);

    // Batch refresh first 8 Kamino reserves instruction
    const reserveAccounts: {
      pubkey: PublicKey;
      isSigner: boolean;
      isWritable: boolean;
    }[] = [];
    for (let i = 0; i < 8; i++) {
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
      .refreshReservesBatch(true) // skip_price_updates = true (oracles already refreshed)
      .remainingAccounts(reserveAccounts)
      .instruction();

    // Build remaining accounts: all active positions (8 Kamino + 7 regular TOKEN_A) + the USDC bank
    const remainingAccounts: PublicKey[][] = [];
    for (let i = 0; i < 8; i++) {
      remainingAccounts.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }
    for (let i = 0; i < NUM_REGULAR_TOKEN_A_BANKS; i++) {
      remainingAccounts.push([
        regularTokenABanks[i],
        oracles.tokenAOracle.publicKey,
      ]);
    }
    // Include the borrow bank (USDC) as well
    remainingAccounts.push([regularBank, oracles.usdcOracle.publicKey]);

    // Create the borrow instruction
    const borrowInstruction = await borrowIx(user.mrgnBankrunProgram, {
      marginfiAccount: userAccount,
      bank: regularBank,
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

    // Create versioned transaction with LUT (compute budget + batch refresh + borrow)
    const messageV0 = new TransactionMessage({
      payerKey: user.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: [computeBudgetIx, batchRefreshIx, borrowInstruction],
    }).compileToV0Message([lut]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([user.wallet]);
    await banksClient.processTransaction(versionedTx);
  });

  it("(admin) Make user 0 unhealthy by increasing USDC bank liability ratio", async () => {
    const { configureBank } = await import("./utils/group-instructions");
    const { blankBankConfigOptRaw } = await import(
      "./utils/types"
    );
    const { bigNumberToWrappedI80F48 } = await import("@mrgnlabs/mrgn-common");
    const { healthPulse, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");

    let config = blankBankConfigOptRaw();
    config.liabilityWeightInit = bigNumberToWrappedI80F48(1.7); // 170%
    config.liabilityWeightMaint = bigNumberToWrappedI80F48(1.6); // 160%

    let tx = new Transaction().add(
      await configureBank(groupAdmin.mrgnBankrunProgram, {
        bank: regularBank,
        bankConfigOpt: config,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);

    // Health pulse to update cache and verify user is unhealthy (using LUT)
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const positionAccounts: PublicKey[][] = [];
    for (let i = 0; i < 8; i++) {
      positionAccounts.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }
    positionAccounts.push([regularBank, oracles.usdcOracle.publicKey]);

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

  it("(user 1) Create marginfi account and deposit collateral", async () => {
    const { accountInit } = await import("./utils/user-instructions");
    const { depositIx } = await import("./utils/user-instructions");

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

    // Deposit some USDC as collateral for liquidation (liability bank)
    const depositAmountUsdc = new BN(10_000 * 10 ** ecosystem.usdcDecimals);
    tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: accountKeypair.publicKey,
        bank: regularBank,
        tokenAccount: user.usdcAccount,
        amount: depositAmountUsdc,
      })
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    // Deposit small amount of TOKEN_A into the asset bank (kaminoBanks[0]) to receive liquidated collateral
    const { makeKaminoDepositIx } = await import("./utils/kamino-instructions");
    const { simpleRefreshReserve, simpleRefreshObligation } = await import(
      "./utils/kamino-utils"
    );
    const depositAmountTokenA = new BN(1 * 10 ** ecosystem.tokenADecimals);

    // Derive the obligation for bank 0
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

  it("(user 1) Liquidate user 0 using LUT", async () => {
    const { liquidateIx, composeRemainingAccounts } = await import(
      "./utils/user-instructions"
    );
    const { getBankrunBlockhash } = await import("./utils/spl-staking-utils");
    const { dumpBankrunLogs } = await import("./utils/tools");

    const liquidator = users[1];
    const liquidatorAccount = liquidator.accounts.get(USER_ACCOUNT);
    const liqorAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    dumpAccBalances(liqorAcc);
    const liquidatee = users[0];
    const liquidateeAccount = liquidatee.accounts.get(USER_ACCOUNT);
    const liqeeAcc = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    dumpAccBalances(liqeeAcc);
    const liabIndex = liqeeAcc.lendingAccount.balances.findIndex((balance) =>
      balance.bankPk.equals(regularBank)
    );
    const liabBefore = wrappedI80F48toBigNumber(
      liqeeAcc.lendingAccount.balances[liabIndex].liabilityShares
    );

    // Pick one Kamino bank to liquidate (bank 0)
    const assetBankKey = kaminoBanks[0];
    const assetReserve = kaminoReserves[0];
    const liquidateAmount = new BN(5 * 10 ** ecosystem.tokenADecimals);

    // Build remaining accounts: asset oracle + reserve + liability oracle + liquidator accounts + liquidatee accounts
    const remainingForLiq: PublicKey[] = [
      oracles.tokenAOracle.publicKey, // asset oracle
      assetReserve, // asset reserve
      oracles.usdcOracle.publicKey, // liability oracle
    ];

    // Liquidator positions (user 1 has USDC in regularBank + TOKEN_A in kaminoBanks[0])
    const liquidatorPositions: PublicKey[][] = [
      [kaminoBanks[0], oracles.tokenAOracle.publicKey, kaminoReserves[0]],
      [regularBank, oracles.usdcOracle.publicKey],
    ];

    // Liquidatee positions (user 0 has 8 Kamino banks + 7 regular TOKEN_A banks + regularBank borrow)
    const liquidateePositions: PublicKey[][] = [];

    // Add 8 Kamino positions
    for (let i = 0; i < 8; i++) {
      liquidateePositions.push([
        kaminoBanks[i],
        oracles.tokenAOracle.publicKey,
        kaminoReserves[i],
      ]);
    }

    // Add 7 regular TOKEN_A positions
    for (let i = 0; i < NUM_REGULAR_TOKEN_A_BANKS; i++) {
      liquidateePositions.push([
        regularTokenABanks[i],
        oracles.tokenAOracle.publicKey,
      ]);
    }

    // Add USDC borrow position
    liquidateePositions.push([regularBank, oracles.usdcOracle.publicKey]);

    const liquidatorAccounts = composeRemainingAccounts(liquidatorPositions);
    remainingForLiq.push(...liquidatorAccounts);
    const liquidateeAccounts = composeRemainingAccounts(liquidateePositions);
    remainingForLiq.push(...liquidateeAccounts);

    const liquidateInstruction = await liquidateIx(
      liquidator.mrgnBankrunProgram,
      {
        assetBankKey: assetBankKey,
        liabilityBankKey: regularBank,
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
    const lut = new AddressLookupTableAccount({
      key: lutAddress,
      state: lutState,
    });

    // Add compute budget
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_400_000,
    });

    const reserveAccounts: {
      pubkey: PublicKey;
      isSigner: boolean;
      isWritable: boolean;
    }[] = [];
    for (let i = 0; i < 8; i++) {
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
      .refreshReservesBatch(true) // skip_price_updates = true (oracles already refreshed)
      .remainingAccounts(reserveAccounts)
      .instruction();

    // Create versioned transaction with LUT
    const messageV0 = new TransactionMessage({
      payerKey: liquidator.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions: [computeBudgetIx, batchRefreshIx, liquidateInstruction],
    }).compileToV0Message([lut]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([liquidator.wallet]);

    // Use tryProcessTransaction to capture logs on failure
    const result = await banksClient.tryProcessTransaction(versionedTx);
    if (result.result) {
      dumpBankrunLogs(result);
      throw new Error("Liquidation transaction failed");
    }
    if (verbose) {
      dumpBankrunLogs(result);
    }

    const liqorAccAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidatorAccount
    );
    dumpAccBalances(liqorAccAfter);

    // NOTE: seized $50 token A (5 tokens), repaid $45.5278104191 USDC
    /*
     * Liquidator fee = 2.5%
     * Insurance fee = 2.5%
     * Confidence interval = 2.12% (1% confidence * 2.12 = 2.12%)
     *
     * Token A is worth $10 with conf $0.212 (worth $9.788 low, $10.212 high)
     * USDC is worth $1 with conf $0.0212 (worth $0.9788 low, $1.0212 high)
     *
     * Liquidator must pay
     *  value of A minus liquidator fee (low bias within the confidence interval): 5 * (1 - 0.025) * 9.788 = $47.71
     *  USDC equivalent (high bias): 47.71 / 1.0212 = $46.7195456326 (~46.71 * 10^6 native)
     *
     * Liquidatee receives
     *  value of A minus (liquidator fee + insurance) (low bias): 5 * (1 - 0.025 - 0.025) * 9.788 = $46.493
     *  USDC equivalent (high bias): 46.493 / 1.0212 = $45.5278104191 (~45.52 & 10^6 native)
     *
     */
    const liqeeAccAfter = await bankrunProgram.account.marginfiAccount.fetch(
      liquidateeAccount
    );
    dumpAccBalances(liqeeAccAfter);
    const liabAfter = wrappedI80F48toBigNumber(
      liqeeAccAfter.lendingAccount.balances[liabIndex].liabilityShares
    );
    // Note: here we are relying on USDC = $1 and shares = tokens (no interest accrued), normally we
    // have to multiply shares * exchange rate and then by the token price.
    assert.approximately(
      Number(liabBefore) - Number(liabAfter),
      45.5278104191 * 10 ** 6,
      100
    );
  });

});
