import {
  AccountMeta,
  AddressLookupTableProgram,
  BPF_LOADER_DEPRECATED_PROGRAM_ID,
  BPF_LOADER_PROGRAM_ID,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  kaminoAccounts,
  klendBankrunProgram,
  MARKET,
  oracles,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  users,
  verbose,
} from "./rootHooks";
import {
  createLookupTableForInstructions,
  getBankrunBlockhash,
  processBankrunTransaction,
} from "./utils/tools";
import { assert } from "chai";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assertBNApproximately, assertKeysEqual } from "./utils/genericTests";
import Decimal from "decimal.js";
import { Fraction } from "@kamino-finance/klend-sdk/dist/classes/fraction";
import {
  GLOBAL_CONFIG_SIZE,
  LENDING_MARKET_SIZE,
  RESERVE_SIZE,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
// Note: there's some glitch in Kamino's lib based on a Raydium static init, it's currently patch-package hacked...
import {
  LendingMarket,
  Reserve,
  MarketWithAddress,
  BorrowRateCurve,
  CurvePoint,
  BorrowRateCurveFields,
  PriceFeed,
  AssetReserveConfig,
  updateEntireReserveConfigIx,
  globalConfigPda,
} from "@kamino-finance/klend-sdk";
import { createMintToInstruction } from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";

let ctx: ProgramTestContext;
import { KLEND_PROGRAM_ID } from "./utils/types";
import {
  deriveFeeReceiver,
  deriveLendingMarketAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
} from "./utils/pdas";
import { Address, address } from "@solana/addresses";
import { createNoopSigner } from "@solana/kit";
import { dummyIx } from "./utils/bankrunConnection";

describe("k01: Init Kamino instance", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  // Note: We use the same admins for Kamino as for mrgn, but in practice the Kamino program is
  // adminstrated by a different organization
  it("(admin) Init Kamino Market - happy path", async () => {
    const usdcString = "USDC";
    const quoteCurrency = Array.from(usdcString.padEnd(32, "\0")).map((c) =>
      c.charCodeAt(0),
    );

    const lendingMarket = Keypair.generate();
    const [lendingMarketAuthority] = deriveLendingMarketAuthority(
      KLEND_PROGRAM_ID,
      lendingMarket.publicKey,
    );

    await ensureGlobalConfigExists();

    // const globalConfig = await globalConfigPda(
    //   address(klendBankrunProgram.programId.toString()),
    // );

    // const [programData] = PublicKey.findProgramAddressSync(
    //   [klendBankrunProgram.programId.toBuffer()],
    //   BPF_LOADER_PROGRAM_ID,
    // );

    let tx = new Transaction();
    tx.add(
      // // Init global config (FAILS due to BPF_LOADER_PROGRAM_ID, probably because it's deprecated)
      // await klendBankrunProgram.methods
      //   .initGlobalConfig()
      //   .accounts({
      //     payer: groupAdmin.wallet.publicKey,
      //     globalConfig,
      //     programData,
      //     systemProgram: SystemProgram.programId,
      //     rent: SYSVAR_RENT_PUBKEY,
      //   })
      //   .instruction()

      // Create a zeroed account that's large enough to hold the lending market
      SystemProgram.createAccount({
        fromPubkey: groupAdmin.wallet.publicKey,
        newAccountPubkey: lendingMarket.publicKey,
        space: LENDING_MARKET_SIZE + 8,
        lamports:
          await bankRunProvider.connection.getMinimumBalanceForRentExemption(
            LENDING_MARKET_SIZE + 8,
          ),
        programId: klendBankrunProgram.programId,
      }),
      // Init lending market
      await klendBankrunProgram.methods
        .initLendingMarket(quoteCurrency)
        .accounts({
          lendingMarketOwner: groupAdmin.wallet.publicKey,
          lendingMarket: lendingMarket.publicKey,
          lendingMarketAuthority: lendingMarketAuthority,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .instruction(),
    );

    await processBankrunTransaction(
      ctx,
      tx,
      [groupAdmin.wallet, lendingMarket],
      false,
      true,
    );
    kaminoAccounts.set(MARKET, lendingMarket.publicKey);
    if (verbose) {
      console.log("Kamino market: " + lendingMarket.publicKey);
    }

    const marketAcc: LendingMarket = LendingMarket.decode(
      (await bankRunProvider.connection.getAccountInfo(lendingMarket.publicKey))
        .data,
    );
    assert.equal(
      marketAcc.lendingMarketOwner.toString(),
      groupAdmin.wallet.publicKey.toString(),
    );
  });

  it("(admin) create USDC reserve", async () => {
    // We need to mint some USDC to the admin's account first
    const tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.usdcDecimals,
      ),
    );
    await processBankrunTransaction(ctx, tx, [globalProgramAdmin.wallet]);

    await createReserve(
      ecosystem.usdcMint.publicKey,
      USDC_RESERVE,
      ecosystem.usdcDecimals,
      // Note: Kamino performs zero oracle validation, it is happy to accept the mock program here
      // instead of Pyth, or any other spoof of Pyth with the same account structure, so be wary!
      // Using Pyth Pull oracle instead of legacy Pyth oracle
      oracles.usdcOracle.publicKey,
      groupAdmin.usdcAccount,
    );
  });

  it("(admin) create token A reserve", async () => {
    // We need to mint some Token A to the admin's account first
    const tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.tokenADecimals,
      ),
    );
    await processBankrunTransaction(ctx, tx, [globalProgramAdmin.wallet]);

    await createReserve(
      ecosystem.tokenAMint.publicKey,
      TOKEN_A_RESERVE,
      ecosystem.tokenADecimals,
      // Using Pyth Pull oracle instead of legacy Pyth oracle
      oracles.tokenAOracle.publicKey,
      groupAdmin.tokenAAccount,
    );
  });

  it("(user 0 - permissionless) refresh USDC reserve price with Pyth Pull oracle", async () => {
    let marketKey = kaminoAccounts.get(MARKET);
    let reserveKey = kaminoAccounts.get(USDC_RESERVE);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        reserveKey,
        marketKey,
        oracles.usdcOracle.publicKey,
      ),
    );

    await processBankrunTransaction(ctx, tx, [users[0].wallet]);

    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(reserveKey)).data,
    );

    // Note: prices are stored as scaled fraction (multiply price by 2^60)
    // E.g. the price is 10 so 10 * 2^60 ~= 1.15292e+19
    let expected = Fraction.fromDecimal(new Decimal(oracles.usdcPrice));
    assertBNApproximately(
      reserveAcc.liquidity.marketPriceSf,
      expected.valueSf,
      100_000,
    );
  });

  it("(admin - permissionless) refresh token A reserve price with Pyth Pull oracle", async () => {
    let marketKey = kaminoAccounts.get(MARKET);
    let reserveKey = kaminoAccounts.get(TOKEN_A_RESERVE);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        reserveKey,
        marketKey,
        oracles.tokenAOracle.publicKey,
      ),
    );
    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(reserveKey)).data,
    );

    // Note: prices are stored as scaled fraction (multiply price by 2^60)
    // E.g. the price is 10 so 10 * 2^60 ~= 1.15292e+19
    let expected = Fraction.fromDecimal(new Decimal(oracles.tokenAPrice));
    assertBNApproximately(
      reserveAcc.liquidity.marketPriceSf,
      expected.valueSf,
      100_000,
    );
  });

  async function createReserve(
    mint: PublicKey,
    reserveLabel: string,
    decimals: number,
    oracle: PublicKey,
    liquiditySource: PublicKey,
  ) {
    const reserve = Keypair.generate();
    const market = kaminoAccounts.get(MARKET);

    const [lendingMarketAuthority] = deriveLendingMarketAuthority(
      KLEND_PROGRAM_ID,
      market,
    );

    const [feeReceiver] = deriveFeeReceiver(
      KLEND_PROGRAM_ID,
      reserve.publicKey,
    );

    const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      reserve.publicKey,
    );

    const [reserveCollateralMint] = deriveReserveCollateralMint(
      KLEND_PROGRAM_ID,
      reserve.publicKey,
    );

    const [reserveCollateralSupply] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      reserve.publicKey,
    );

    assertKeysEqual(klendBankrunProgram.programId, KLEND_PROGRAM_ID);

    const tx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: groupAdmin.wallet.publicKey,
        newAccountPubkey: reserve.publicKey,
        space: RESERVE_SIZE + 8,
        lamports:
          await bankRunProvider.connection.getMinimumBalanceForRentExemption(
            RESERVE_SIZE + 8,
          ),
        programId: klendBankrunProgram.programId,
      }),
      await klendBankrunProgram.methods
        .initReserve()
        .accountsStrict({
          signer: groupAdmin.wallet.publicKey,
          lendingMarket: market,
          lendingMarketAuthority,
          reserve: reserve.publicKey,
          reserveLiquidityMint: mint,
          reserveLiquiditySupply,
          feeReceiver,
          reserveCollateralMint,
          reserveCollateralSupply,
          initialLiquiditySource: liquiditySource,
          rent: SYSVAR_RENT_PUBKEY,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .instruction(),
    );

    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet, reserve]);
    kaminoAccounts.set(reserveLabel, reserve.publicKey);

    if (verbose) {
      console.log("Kamino reserve " + reserveLabel + " " + reserve.publicKey);
    }

    const marketAcc: LendingMarket = LendingMarket.decode(
      (await bankRunProvider.connection.getAccountInfo(market)).data,
    );
    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(reserve.publicKey)).data,
    );
    assert.equal(reserveAcc.lendingMarket.toString(), market.toString());
    // Reserves start in an unconfigured "Hidden" state
    assert.equal(reserveAcc.config.status, 2);

    // Update the reserve to a sane operational state
    const marketWithAddress: MarketWithAddress = {
      address: address(market.toString()),
      state: marketAcc,
    };

    const borrowRateCurve = new BorrowRateCurve({
      points: [
        // At 0% utilization: 50% interest rate
        new CurvePoint({ utilizationRateBps: 0, borrowRateBps: 50000 }),
        // At 50% utilization: 100% interest rate
        new CurvePoint({ utilizationRateBps: 5000, borrowRateBps: 100000 }),
        // At 80% utilization: 500% interest rate
        new CurvePoint({ utilizationRateBps: 8000, borrowRateBps: 500000 }),
        // At 100% utilization: 1000% interest rate
        new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
        // Fill remaining points to complete the curve
        ...Array(7).fill(
          new CurvePoint({ utilizationRateBps: 10000, borrowRateBps: 1000000 }),
        ),
      ],
    } as BorrowRateCurveFields);
    const assetReserveConfigParams = {
      loanToValuePct: 75, // 75%
      liquidationThresholdPct: 85, // 85%
      borrowRateCurve,
      depositLimit: new Decimal(1_000_000_000),
      borrowLimit: new Decimal(1_000_000_000),
    };

    const priceFeed: PriceFeed = {
      pythPrice: address(oracle.toString()),
      // switchboardPrice: NULL_PUBKEY,
      // switchboardTwapPrice: NULL_PUBKEY,
      // scopePriceConfigAddress: NULL_PUBKEY,
      // scopeChain: [0, 65535, 65535, 65535],
      // scopeTwapChain: [52, 65535, 65535, 65535],
    };

    const assetReserveConfig = new AssetReserveConfig({
      mint: address(mint.toString()),
      mintTokenProgram: address(TOKEN_PROGRAM_ID.toString()),
      tokenName: reserveLabel,
      mintDecimals: decimals,
      priceFeed: priceFeed,
      ...assetReserveConfigParams,
    }).getReserveConfig();

    const addr = address(groupAdmin.wallet.publicKey.toString());
    const signer = createNoopSigner(addr);
    const updateReserveIx = await updateEntireReserveConfigIx(
      signer,
      marketWithAddress.address,
      address(reserve.publicKey.toString()),
      assetReserveConfig,
      address(klendBankrunProgram.programId.toString()),
    );

    const ix = toWeb3Ix(updateReserveIx as any);
    const lutAccount = await createLookupTableForInstructions(
      groupAdmin.wallet,
      [ix],
    );

    const instructions = [
      ComputeBudgetProgram.setComputeUnitLimit({
        units: 1_400_000,
      }),
      ix,
    ];

    const messageV0 = new TransactionMessage({
      payerKey: groupAdmin.wallet.publicKey,
      recentBlockhash: await getBankrunBlockhash(bankrunContext),
      instructions,
    }).compileToV0Message([lutAccount]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([groupAdmin.wallet]);
    await banksClient.processTransaction(versionedTx);
  }
});

type KitAccountMeta = {
  address: string;
  role?: number;
  isSigner?: boolean;
  isWritable?: boolean;
};

type KitInstruction = {
  programAddress: string;
  accounts: readonly KitAccountMeta[];
  data: Uint8Array;
};

function toWeb3Ix(ix: KitInstruction): TransactionInstruction {
  const keys: AccountMeta[] = ix.accounts.map((account) => {
    // Depending on the exact Kit shape, role may encode signer/writable.
    // If your generated client already exposes isSigner/isWritable, use those directly.
    return {
      pubkey: new PublicKey(account.address),
      isSigner: Boolean(account.isSigner),
      isWritable: Boolean(account.isWritable),
    };
  });

  return new TransactionInstruction({
    programId: new PublicKey(ix.programAddress),
    keys,
    data: Buffer.from(ix.data),
  });
}

import crypto from "crypto";

function anchorDiscriminator(name: string): Buffer {
  return crypto
    .createHash("sha256")
    .update(`account:${name}`)
    .digest()
    .subarray(0, 8);
}

async function ensureGlobalConfigExists() {
  const globalConfigAddr = await globalConfigPda(
    address(klendBankrunProgram.programId.toString()),
  );
  const globalConfigPk = new PublicKey(globalConfigAddr.toString());

  const existing = await bankRunProvider.connection.getAccountInfo(
    globalConfigPk,
  );
  if (existing) {
    return globalConfigPk;
  }

  const lamports =
    await bankRunProvider.connection.getMinimumBalanceForRentExemption(
      GLOBAL_CONFIG_SIZE,
    );

  const data = Buffer.alloc(GLOBAL_CONFIG_SIZE);
  anchorDiscriminator("GlobalConfig").copy(data, 0);

  bankrunContext.setAccount(globalConfigPk, {
    lamports,
    data,
    owner: klendBankrunProgram.programId,
    executable: false,
    rentEpoch: 0,
  });

  return globalConfigPk;
}
