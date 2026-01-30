import { BN } from "@coral-xyz/anchor";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  ecosystem,
  kaminoAccounts,
  KAMINO_USDC_BANK,
  kaminoGroup,
  MARKET,
  oracles,
  USDC_RESERVE,
  users,
  verbose,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  bankRunProvider,
  banksClient,
} from "./rootHooks";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
  wrappedU68F60toBigNumber,
} from "./utils/kamino-utils";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";
import { MockUser, USER_ACCOUNT_K } from "./utils/mocks";
import { omitPadding, processBankrunTransaction } from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { makeKaminoDepositIx } from "./utils/kamino-instructions";
import { ProgramTestContext } from "solana-bankrun";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  assertBNEqual,
  assertU68F60Approx,
  assertI68F60Equal,
  assertI80F48Approx,
  assertI80F48Equal,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { KLEND_PROGRAM_ID } from "./utils/types";
import {
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
} from "./utils/pdas";
import { getEpochAndSlot } from "./utils/stake-utils";
import { BalanceRaw } from "@mrgnlabs/marginfi-client-v2";

let ctx: ProgramTestContext;
let bank: PublicKey;
let market: PublicKey;
let usdcReserve: PublicKey;
let usdcBankObligation: PublicKey;
let reserveLiquiditySupply: PublicKey;
let reserveCollateralSupply: PublicKey;

describe("k06: Kamino Deposit Tests", () => {
  before(async () => {
    ctx = bankrunContext;
    bank = kaminoAccounts.get(KAMINO_USDC_BANK);
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const bankKey = bank.toString();
    usdcBankObligation = kaminoAccounts.get(`${bankKey}_OBLIGATION`);

    [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.usdcMint.publicKey
    );
    [reserveCollateralSupply] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      market,
      ecosystem.usdcMint.publicKey
    );

    // Refresh oracles to ensure they have current timestamps
    // This prevents stale oracle issues when other test suites run first
    await refreshPullOraclesBankrun(oracles, ctx, banksClient);
  });

  async function executeDeposit(
    user: MockUser,
    amount: BN,
    userLabel: string
  ): Promise<void> {
    const marginfiAccount = user.accounts.get(USER_ACCOUNT_K);
    /** Without decimals, e.g. 1.1 USDC = 1.1 */
    const amtFloat = amount.toNumber() / 10 ** ecosystem.usdcDecimals;

    if (verbose) {
      console.log(
        `Deposit for user ${userLabel} Account: ${marginfiAccount.toString()}`
      );
    }

    const [
      obBefore,
      resBefore,
      bankBefore,
      userAccBefore,
      liqVaultBefore,
      colVaultBefore,
      userUsdcBefore,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.usdcAccount),
    ]);

    const balanceMaybe = userAccBefore.lendingAccount.balances.find(
      (b) => b.bankPk.equals(bank) && b.active === 1
    );
    const balanceAmtBefore = balanceMaybe
      ? wrappedI80F48toBigNumber(balanceMaybe.assetShares).toNumber()
      : 0;
    if (verbose) {
      if (balanceAmtBefore > 0) {
        console.log("Deposit into existing position with: " + balanceAmtBefore);
      } else {
        console.log("Deposit into new position");
      }
    }

    // console.log("🕑  State before:");
    // console.log("  • Obligation:");
    // // console.log(JSON.stringify(omitPadding(obBefore.deposits), null, 2));
    // // console.log(JSON.stringify(omitPadding(obBefore.lastUpdate), null, 2));
    // console.log("  • Reserve:");
    // // console.log(JSON.stringify(omitPadding(resBefore.collateral), null, 2));
    // console.log(`  • Liquidity Vault:  ${liqVaultBefore.toLocaleString()}`);
    // console.log(`  • Collateral Vault: ${colVaultBefore.toLocaleString()}`);
    // console.log(`  • User  USDC:       ${userUsdcBefore.toLocaleString()}`);

    const { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      // pass the USDC reserve since it's now part of the obligation
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        usdcBankObligation,
        [usdcReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.usdcAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        },
        amount
      )
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [
      obAfter,
      resAfter,
      bankAfter,
      userAccAfter,
      liqVaultAfter,
      colVaultAfter,
      userUsdcAfter,
    ] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
      bankrunProgram.account.bank.fetch(bank),
      bankrunProgram.account.marginfiAccount.fetch(marginfiAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
      getTokenBalance(bankRunProvider, reserveCollateralSupply),
      getTokenBalance(bankRunProvider, user.usdcAccount),
    ]);

    const userUsdcChange = userUsdcBefore - userUsdcAfter;
    if (verbose) {
      console.log(` Deposited ${amount} USDC actual change ${userUsdcChange}`);
    }
    assertBNEqual(amount, userUsdcChange);
    assertBNEqual(amount, liqVaultAfter - liqVaultBefore);
    // Note: The ratio between collateral and liquidity is 1:1 here!
    assertBNEqual(amount, colVaultAfter - colVaultBefore);

    const balancesAfter = userAccAfter.lendingAccount.balances;
    const balanceAfter: BalanceRaw = balancesAfter.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    assert.equal(balanceAfter.active, 1);
    assertI80F48Approx(
      balanceAfter.assetShares,
      // Note: Here collateral and liquidity are 1:1, so we can use amount, but this is actually
      // tracking collateral token!
      balanceAmtBefore + amount.toNumber()
    );
    assertBNEqual(
      resAfter.liquidity.availableAmount,
      resBefore.liquidity.availableAmount.add(amount)
    );
    assertI68F60Equal(resAfter.liquidity.borrowedAmountSf, 0);

    // Assert bank updated as expected
    const sharesBefore = wrappedI80F48toBigNumber(
      bankBefore.totalAssetShares
    ).toNumber();
    // No interest accumulates on Kamino banks, so the asset share value is always 1, and the
    // relationship between collateral tokens and shares is always 1:1
    assertI80F48Approx(
      bankAfter.totalAssetShares,
      sharesBefore + amount.toNumber()
    );
    assertI80F48Equal(bankAfter.assetShareValue, 1);
    assertI80F48Equal(bankAfter.collectedInsuranceFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedGroupFeesOutstanding, 0);
    assertI80F48Equal(bankAfter.collectedProgramFeesOutstanding, 0);

    // Assert obligation/reserve deposit state recorded as expected
    const depositBefore = obBefore.deposits[0];
    const depositAfter = obAfter.deposits[0];
    assertKeysEqual(depositAfter.depositReserve, usdcReserve);
    assertBNEqual(
      depositAfter.depositedAmount,
      // Note: Here collateral and liquidity are 1:1, so we can use amount, but this is actually
      // tracking collateral token!
      depositBefore.depositedAmount.add(amount)
    );

    // Oracle update assertions
    assert.equal(obAfter.lastUpdate.slot.toNumber(), slot);
    assert.equal(resAfter.lastUpdate.slot.toNumber(), slot);
    /*
        const PRICE_LOADED =        0b_0000_0001; // 1
        const PRICE_AGE_CHECKED =   0b_0000_0010; // 2
        const TWAP_CHECKED =        0b_0000_0100; // 4 
        const TWAP_AGE_CHECKED =    0b_0000_1000; // 8
        const HEURISTIC_CHECKED =   0b_0001_0000; // 16
        const PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32
    */
    // All flags clear = 32 + 16 + 8 + 4 + 2 + 1
    assert.equal(obAfter.lastUpdate.priceStatus, 63);
    assert.equal(resAfter.lastUpdate.priceStatus, 63);
    // Note: Deposit triggers `obligation.last_update.mark_stale()`, setting stale to true (1).
    assert.equal(obAfter.lastUpdate.stale, 1);
    // Note: Deposit also triggers `deposit_reserve.last_update.mark_stale()`
    assert.equal(resAfter.lastUpdate.stale, 1);

    // Note: Some values don't update until the NEXT refresh!
    let refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        usdcBankObligation,
        [usdcReserve]
      )
    );
    await processBankrunTransaction(ctx, refreshTx, [user.wallet]);

    // Market price float data doesn't update until the NEXT refresh
    const [obAfterRefresh, resAfterRefresh] = await Promise.all([
      klendBankrunProgram.account.obligation.fetch(usdcBankObligation),
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
    ]);
    const depositAfterRefresh = obAfterRefresh.deposits[0];
    const marketValueBefore = wrappedU68F60toBigNumber(
      depositBefore.marketValueSf
    ).toNumber();
    const marketValueAfter = wrappedU68F60toBigNumber(
      depositAfterRefresh.marketValueSf
    ).toNumber();
    const expectedDiff = amtFloat * oracles.usdcPrice;
    const expectedValue = marketValueBefore + expectedDiff;
    assert.approximately(
      marketValueAfter,
      expectedValue,
      marketValueAfter * 0.00001
    );
    // Note: the reserve, in a variable with the same name, records the token PRICE...
    assert.approximately(
      wrappedU68F60toBigNumber(
        resAfterRefresh.liquidity.marketPriceSf
      ).toNumber(),
      oracles.usdcPrice,
      oracles.usdcPrice * 0.00001
    );

    // After another refresh, the stale flag is removed and the oracle data can be used again. Note:
    // This forces multiple transactions in the same tx that consume oracle data (e.g. multiple
    // deposits) to refresh again after each tx is done.
    assert.equal(obAfterRefresh.lastUpdate.stale, 0);
    assert.equal(resAfterRefresh.lastUpdate.stale, 0);
  }

  const deposit1 = 1_000_000 * 10 ** ecosystem.usdcDecimals;
  it("(user 0) Deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[0], new BN(deposit1), "0");
  });

  const deposit2 = 200 * 10 ** ecosystem.usdcDecimals;
  it("(user 0) Second deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[0], new BN(deposit2), "0");
  });

  const deposit3 = 150 * 10 ** ecosystem.usdcDecimals;
  it("(user 1) Deposit to Kamino via Marginfi - happy path", async () => {
    await executeDeposit(users[1], new BN(deposit3), "1");
  });
});
