import {
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankRunProvider,
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
  processBankrunTransaction,
} from "./utils/tools";
import { assert } from "chai";
import { assertBNApproximately } from "./utils/genericTests";
import Decimal from "decimal.js";
import { Fraction } from "@kamino-finance/klend-sdk/dist/classes/fraction";
import {
  LENDING_MARKET_SIZE,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
// Note: there's some glitch in Kamino's lib based on a Raydium static init, it's currently patch-package hacked...
import {
  LendingMarket,
  Reserve,
} from "@kamino-finance/klend-sdk";
import { createMintToInstruction } from "@solana/spl-token";
import { ProgramTestContext } from "solana-bankrun";

let ctx: ProgramTestContext;
import { KLEND_PROGRAM_ID } from "./utils/types";
import {
  deriveLendingMarketAuthority,
} from "./utils/pdas";
import { createReserve } from "./utils/kamino-instructions";

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

    let tx = new Transaction();
    tx.add(
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

    await processBankrunTransaction(ctx, tx, [
      groupAdmin.wallet,
      lendingMarket,
    ]);
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
      Keypair.generate(),
      kaminoAccounts.get(MARKET),
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
      Keypair.generate(),
      kaminoAccounts.get(MARKET),
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
});
