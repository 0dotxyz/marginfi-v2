import { Keypair, Transaction } from "@solana/web3.js";
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
import { processBankrunTransaction } from "./utils/tools";
import { assert } from "chai";
import { assertBNApproximately } from "./utils/genericTests";
import {
  integerPriceToFractionSf,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
// Note: there's some glitch in Kamino's lib based on a Raydium static init, it's currently patch-package hacked...
import { LendingMarket, Reserve } from "@kamino-finance/klend-sdk";
import { createMintToInstruction } from "@solana/spl-token";
import { ProgramTestContext } from "./utils/litesvm";

let ctx: ProgramTestContext;
import {
  createKaminoMarket,
  createReserve,
} from "./utils/kamino-reserve-setup";

describe("k01: Init Kamino instance", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  // Note: We use the same admins for Kamino as for mrgn, but in practice the Kamino program is
  // adminstrated by a different organization
  it("(admin) Init Kamino Market - happy path", async () => {
    const lendingMarket = await createKaminoMarket();
    kaminoAccounts.set(MARKET, lendingMarket);
    if (verbose) {
      console.log("Kamino market: " + lendingMarket);
    }

    const marketAcc: LendingMarket = LendingMarket.decode(
      (await bankRunProvider.connection.getAccountInfo(lendingMarket)).data
    );
    assert.equal(
      marketAcc.lendingMarketOwner.toString(),
      groupAdmin.wallet.publicKey.toString()
    );
  });

  it("(admin) create USDC reserve", async () => {
    // We need to mint some USDC to the admin's account first
    const tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.usdcMint.publicKey,
        groupAdmin.usdcAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.usdcDecimals
      )
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
      groupAdmin.usdcAccount
    );
  });

  it("(admin) create token A reserve", async () => {
    // We need to mint some Token A to the admin's account first
    const tx = new Transaction().add(
      createMintToInstruction(
        ecosystem.tokenAMint.publicKey,
        groupAdmin.tokenAAccount,
        globalProgramAdmin.wallet.publicKey,
        1000 * 10 ** ecosystem.tokenADecimals
      )
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
      groupAdmin.tokenAAccount
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
        oracles.usdcOracle.publicKey
      )
    );

    await processBankrunTransaction(ctx, tx, [users[0].wallet]);

    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(reserveKey)).data
    );

    // Note: prices are stored as scaled fraction (multiply price by 2^60)
    // E.g. the price is 10 so 10 * 2^60 ~= 1.15292e+19
    assertBNApproximately(
      reserveAcc.liquidity.marketPriceSf,
      integerPriceToFractionSf(oracles.usdcPrice),
      100_000
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
        oracles.tokenAOracle.publicKey
      )
    );
    await processBankrunTransaction(ctx, tx, [groupAdmin.wallet]);

    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(reserveKey)).data
    );

    // Note: prices are stored as scaled fraction (multiply price by 2^60)
    // E.g. the price is 10 so 10 * 2^60 ~= 1.15292e+19
    assertBNApproximately(
      reserveAcc.liquidity.marketPriceSf,
      integerPriceToFractionSf(oracles.tokenAPrice),
      100_000
    );
  });
});
