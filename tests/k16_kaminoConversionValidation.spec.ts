import { BN, Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Transaction,
  SystemProgram,
  Keypair,
  AccountMeta,
  TransactionInstruction,
} from "@solana/web3.js";
import { assert } from "chai";
import { Clock } from "solana-bankrun";

import {
  banksClient,
  bankRunProvider,
  bankrunContext,
  bankrunProgram,
  globalProgramAdmin,
  klendBankrunProgram,
  kaminoAccounts,
  users,
  oracles,
  ecosystem,
  farmAccounts,
  // Keys
  MARKET,
  KAMINO_USDC_BANK,
  KAMINO_TOKEN_A_BANK,
  USDC_RESERVE,
  TOKEN_A_RESERVE,
  A_FARM_STATE,
  A_OBLIGATION_USER_STATE,
  kaminoGroup,
} from "./rootHooks";

import {
  simpleRefreshReserve,
  simpleRefreshObligation,
  estimateCollateralFromDeposit,
  getLiquidityExchangeRate,
} from "./utils/kamino-utils";
import {
  makeKaminoWithdrawIx,
  makeKaminoDepositIx,
} from "./utils/kamino-instructions";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { processBankrunTransaction } from "./utils/tools";
import { getTokenBalance, assertBNApproximately } from "./utils/genericTests";
import { createMintToInstruction } from "@solana/spl-token";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { Reserve } from "@kamino-finance/klend-sdk";
import Decimal from "decimal.js";
import {
  accountInit,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import { Marginfi } from "target/types/marginfi";

// Constants, not sure why we need these but we'll see
const SCALE_1E9 = new BN(10).pow(new BN(9)); // scale exchangeRate to 1e9 like sl09
const CONF_BPS = new BN(212); // 2.12% of RAW oracle price, matching d13/sl09
const BPS_DEN = new BN(10_000);
const PRECISION_DECIMALS = 9; // Use 9 decimals as baseline for precision

/**
 * Lower-biased price (PriceBias::Low), matching d13/sl09:
 *   lowerPrice = adjustedPrice - (rawPrice * 2.12%)
 * Note: haircut is taken from RAW price (not exchange-rate adjusted) in the reference tests.
 */
function lowerBiasedAdjustedPrice(
  rawOraclePrice: BN,
  exchangeRateDecimal: Decimal
): BN {
  const exchangeRateScaled = new BN(
    exchangeRateDecimal.mul(SCALE_1E9.toString()).floor().toString()
  );

  const adjustedScaled = rawOraclePrice.mul(exchangeRateScaled);
  const confScaled = adjustedScaled.mul(CONF_BPS).div(BPS_DEN);
  const resultScaled = adjustedScaled.sub(confScaled);

  return resultScaled.div(SCALE_1E9);
}

/**
 * Expected USD value:
 *   expectedUsd = assetShares * lowerPrice / 10^tokenDecimals
 * Consistent with sl09/d13 expected value formulations.
 */
function expectedUsdValue(
  assetShares: BN,
  lowerPrice: BN,
  tokenDecimals: number
): BN {
  return assetShares.mul(lowerPrice).div(new BN(10).pow(new BN(tokenDecimals)));
}

async function fetchReserve(reserve: PublicKey): Promise<Reserve> {
  return (await klendBankrunProgram.account.reserve.fetch(reserve)) as Reserve;
}

/**
 * Create a pulse health instruction to update the health cache
 */
export const makePulseHealthIx = async (
  program: Program<Marginfi>,
  marginfiAccount: PublicKey,
  remainingAccounts: AccountMeta[] = []
): Promise<TransactionInstruction> => {
  return program.methods
    .lendingAccountPulseHealth()
    .accounts({
      marginfiAccount,
    })
    .remainingAccounts(remainingAccounts)
    .instruction();
};

describe("k16: Kamino Conversion Validation", () => {
  let usdcBank: PublicKey;
  let tokenABank: PublicKey;
  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;
  let usdcObligation: PublicKey;
  let tokenAObligation: PublicKey;
  let user0Account: PublicKey;
  let user1Account: PublicKey;

  let user0InitialCTokens: BN;
  let user1InitialCTokens: BN;
  let user0DepositAmount: BN;
  let user1DepositAmount: BN;

  before(async () => {
    usdcBank = kaminoAccounts.get(KAMINO_USDC_BANK)!;
    tokenABank = kaminoAccounts.get(KAMINO_TOKEN_A_BANK)!;
    usdcReserve = kaminoAccounts.get(USDC_RESERVE)!;
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE)!;
    usdcObligation = kaminoAccounts.get(`${usdcBank.toString()}_OBLIGATION`)!;
    tokenAObligation = kaminoAccounts.get(
      `${tokenABank.toString()}_OBLIGATION`
    )!;
  });

  it("Create empty marginfi accounts for testing", async () => {
    for (let i = 0; i < 2; i++) {
      const accountKeypair = Keypair.generate();
      const initAccountIx = await accountInit(users[i].mrgnBankrunProgram, {
        marginfiGroup: kaminoGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: users[i].wallet.publicKey,
        feePayer: users[i].wallet.publicKey,
      });

      const tx = new Transaction().add(initAccountIx);
      await processBankrunTransaction(
        bankrunContext,
        tx,
        [users[i].wallet, accountKeypair],
        false,
        true
      );

      users[i].accounts.set("k16_account", accountKeypair.publicKey);
      if (i == 0) user0Account = accountKeypair.publicKey;
      else user1Account = accountKeypair.publicKey;
    }
  });

  it("Deposts tokens and validates initial cToken conversion", async () => {
    const fundTx = new Transaction()
      .add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          users[0].usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          1000 * 10 ** ecosystem.usdcDecimals
        )
      )
      .add(
        createMintToInstruction(
          ecosystem.tokenAMint.publicKey,
          users[1].tokenAAccount,
          globalProgramAdmin.wallet.publicKey,
          100 * 10 ** ecosystem.tokenADecimals
        )
      );
    await processBankrunTransaction(
      bankrunContext,
      fundTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    user0DepositAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    const usdcReserveBefore = await fetchReserve(usdcReserve);
    const expectedUsdcCTokens = estimateCollateralFromDeposit(
      usdcReserveBefore,
      user0DepositAmount
    );

    const refreshUsdcIx = await simpleRefreshReserve(
      klendBankrunProgram,
      usdcReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.usdcOracle.publicKey
    );

    const refreshUsdcObligationIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      usdcObligation,
      [usdcReserve]
    );

    const depositIx0 = await makeKaminoDepositIx(
      users[0].mrgnBankrunProgram,
      {
        marginfiAccount: user0Account,
        bank: usdcBank,
        signerTokenAccount: users[0].usdcAccount,
        lendingMarket: kaminoAccounts.get(MARKET)!,
        reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        obligationFarmUserState: null,
        reserveFarmState: null,
      },
      user0DepositAmount
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshUsdcIx, refreshUsdcObligationIx, depositIx0),
      [users[0].wallet],
      false,
      true
    );

    const marginfiAccount0 = await bankrunProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const balance0 = marginfiAccount0.lendingAccount.balances.find(
      (b) => b.active == 1 && b.bankPk.equals(usdcBank)
    );
    user0InitialCTokens = new BN(
      wrappedI80F48toBigNumber(balance0.assetShares).toString()
    );

    assertBNApproximately(user0InitialCTokens, expectedUsdcCTokens, new BN(1));

    user1DepositAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);

    // Refresh the reserve first to update interest accrual, then fetch fresh state
    const refreshTokenAIx = await simpleRefreshReserve(
      klendBankrunProgram,
      tokenAReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.tokenAOracle.publicKey
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshTokenAIx),
      [users[1].wallet]
    );

    // Now fetch the fresh (post-refresh) state to calculate expected cTokens
    const tokenAReserveBefore = await fetchReserve(tokenAReserve);
    const expectedTokenACTokens = estimateCollateralFromDeposit(
      tokenAReserveBefore,
      user1DepositAmount
    );

    const refreshTokenAObligationIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      tokenAObligation,
      [tokenAReserve]
    );

    const depositIx1 = await makeKaminoDepositIx(
      users[1].mrgnBankrunProgram,
      {
        marginfiAccount: user1Account,
        bank: tokenABank,
        signerTokenAccount: users[1].tokenAAccount,
        lendingMarket: kaminoAccounts.get(MARKET)!,
        reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        obligationFarmUserState: farmAccounts.get(A_OBLIGATION_USER_STATE),
        reserveFarmState: farmAccounts.get(A_FARM_STATE),
      },
      user1DepositAmount
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        refreshTokenAObligationIx,
        depositIx1
      ),
      [users[1].wallet],
      false,
      true
    );

    const marginfiAccount1 = await bankrunProgram.account.marginfiAccount.fetch(
      user1Account
    );
    const balance1 = marginfiAccount1.lendingAccount.balances.find(
      (b) => b.active == 1 && b.bankPk.equals(tokenABank)
    );
    user1InitialCTokens = new BN(
      wrappedI80F48toBigNumber(balance1.assetShares).toString()
    );
    // ??? This sometimes fails due to clock issues
    assertBNApproximately(
      user1InitialCTokens,
      expectedTokenACTokens,
      new BN(1)
    );
  });

  it("Advances time by 30 days and tracks interest accrual", async () => {
    const usdcReserveBefore = await fetchReserve(usdcReserve);
    const tokenAReserveBefore = await fetchReserve(tokenAReserve);

    const usdcExchangeRateBefore = getLiquidityExchangeRate(usdcReserveBefore);
    const tokenAExchangeRateBefore =
      getLiquidityExchangeRate(tokenAReserveBefore);

    const currentClock = await banksClient.getClock();
    const currentSlot = Number(currentClock.slot);
    const currentTimestamp = Number(currentClock.unixTimestamp);

    const newSlot = currentSlot + 1;
    const newTimestamp = currentTimestamp + 30 * 86_400;

    const newClock = new Clock(
      BigInt(newSlot),
      0n,
      currentClock.epoch,
      0n,
      BigInt(newTimestamp)
    );

    bankrunContext.setClock(newClock);

    const dummyTx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        toPubkey: globalProgramAdmin.wallet.publicKey,
        lamports: 1,
      })
    );
    await processBankrunTransaction(
      bankrunContext,
      dummyTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const refreshIx0 = await simpleRefreshReserve(
      klendBankrunProgram,
      usdcReserve,
      kaminoAccounts.get(MARKET),
      oracles.usdcOracle.publicKey
    );
    const refreshIx1 = await simpleRefreshReserve(
      klendBankrunProgram,
      tokenAReserve,
      kaminoAccounts.get(MARKET),
      oracles.tokenAOracle.publicKey
    );
    const refreshTx = new Transaction().add(refreshIx0, refreshIx1);
    await processBankrunTransaction(
      bankrunContext,
      refreshTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    const usdcReserveAfter = await fetchReserve(usdcReserve);
    const tokenAReserveAfter = await fetchReserve(tokenAReserve);

    const usdcExchangeRateAfter = getLiquidityExchangeRate(usdcReserveAfter);
    const tokenAExchangeRateAfter =
      getLiquidityExchangeRate(tokenAReserveAfter);

    assert.ok(
      usdcExchangeRateAfter.gt(usdcExchangeRateBefore),
      `USDC exchange rate should increase with interest. Before: ${usdcExchangeRateBefore.toString()}, After: ${usdcExchangeRateAfter.toString()}`
    );
    assert.ok(
      tokenAExchangeRateAfter.gt(tokenAExchangeRateBefore),
      `Token A exchange rate should increase with interest. Before: ${tokenAExchangeRateBefore.toString()}, After: ${tokenAExchangeRateAfter.toString()}`
    );
  });

  it("Validates health check valuations for both users match calculated values", async () => {
    // Refresh reserves before health pulse to ensure fresh oracle data
    const refreshUsdcBeforeHealthIx = await simpleRefreshReserve(
      klendBankrunProgram,
      usdcReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.usdcOracle.publicKey
    );

    const refreshUsdcObligationBeforeHealthIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      usdcObligation,
      [usdcReserve]
    );

    const pulseHealthIx0 = await makePulseHealthIx(
      users[0].mrgnBankrunProgram,
      user0Account,
      [
        { pubkey: usdcBank, isSigner: false, isWritable: false },
        {
          pubkey: oracles.usdcOracle.publicKey,
          isSigner: false,
          isWritable: false,
        },
        {
          pubkey: usdcReserve,
          isSigner: false,
          isWritable: false,
        },
      ]
    );
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        refreshUsdcBeforeHealthIx,
        refreshUsdcObligationBeforeHealthIx,
        pulseHealthIx0
      ),
      [users[0].wallet],
      false,
      true
    );

    const accAfter0 = await bankrunProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const healthCahce0 = accAfter0.healthCache;
    const assetValue0 = wrappedI80F48toBigNumber(healthCahce0.assetValue);
    const actualUser0UsdValue = new BN(
      Math.floor(assetValue0.toNumber() * 10 ** ecosystem.usdcDecimals)
    );

    // Refresh TokenA reserves before health pulse
    const refreshTokenABeforeHealthIx = await simpleRefreshReserve(
      klendBankrunProgram,
      tokenAReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.tokenAOracle.publicKey
    );

    const refreshTokenAObligationBeforeHealthIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      tokenAObligation,
      [tokenAReserve]
    );

    const pulseHealthIx1 = await makePulseHealthIx(
      users[1].mrgnBankrunProgram,
      user1Account,
      [
        { pubkey: tokenABank, isSigner: false, isWritable: false },
        {
          pubkey: oracles.tokenAOracle.publicKey,
          isSigner: false,
          isWritable: false,
        },
        {
          pubkey: tokenAReserve,
          isSigner: false,
          isWritable: false,
        },
      ]
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        refreshTokenABeforeHealthIx,
        refreshTokenAObligationBeforeHealthIx,
        pulseHealthIx1
      ),
      [users[1].wallet],
      false,
      true
    );
    const accAfter1 = await bankrunProgram.account.marginfiAccount.fetch(
      user1Account
    );
    const healthCache1 = accAfter1.healthCache;
    const assetValue1 = wrappedI80F48toBigNumber(healthCache1.assetValue);
    const actualUser1UsdValue = new BN(
      Math.floor(assetValue1.toNumber() * 10 ** ecosystem.usdcDecimals)
    );

    const usdcBankData = await bankrunProgram.account.bank.fetch(usdcBank);
    const tokenABankData = await bankrunProgram.account.bank.fetch(tokenABank);

    const usdcAssetWeight = wrappedI80F48toBigNumber(
      usdcBankData.config.assetWeightInit
    );
    const tokenAAssetWeight = wrappedI80F48toBigNumber(
      tokenABankData.config.assetWeightInit
    );

    const usdcReserveAfter = await fetchReserve(usdcReserve);
    const tokenAReserveAfter = await fetchReserve(tokenAReserve);

    const exUSDC = getLiquidityExchangeRate(usdcReserveAfter);
    const exA = getLiquidityExchangeRate(tokenAReserveAfter);

    // Raw oracle prices are quoted in quote-atomic (use USDC decimals)
    const rawUSDC = new BN(oracles.usdcPrice * 10 ** ecosystem.usdcDecimals);
    const rawA = new BN(oracles.tokenAPrice * 10 ** ecosystem.usdcDecimals);

    const lowUSDC = lowerBiasedAdjustedPrice(rawUSDC, exUSDC);
    const lowA = lowerBiasedAdjustedPrice(rawA, exA);

    const expectedUser0UsdValue = expectedUsdValue(
      user0InitialCTokens,
      lowUSDC,
      ecosystem.usdcDecimals
    );

    const expectedUser1UsdValue = expectedUsdValue(
      user1InitialCTokens,
      lowA,
      ecosystem.tokenADecimals
    );

    // === SIMPLIFIED DEBUG - FOCUS ON THE THREE KEY VALUES ===
    console.log("=== HEALTH PULSE vs CALCULATED VALUES ===");

    console.log("1. ASSET VALUES FROM HEALTH PULSE:");
    console.log(
      "   User0 (USDC) asset value from health pulse:",
      actualUser0UsdValue.toString()
    );
    console.log(
      "   User1 (TokenA) asset value from health pulse:",
      actualUser1UsdValue.toString()
    );

    console.log("2. ASSET VALUES FROM OUR CALCULATIONS:");
    console.log(
      "   User0 (USDC) calculated value:",
      expectedUser0UsdValue.toString()
    );
    console.log(
      "   User1 (TokenA) calculated value:",
      expectedUser1UsdValue.toString()
    );

    console.log("3. ASSET WEIGHT VALUES:");
    console.log("   USDC asset weight:", usdcAssetWeight.toString());
    console.log("   TokenA asset weight:", tokenAAssetWeight.toString());

    console.log("================================================");

    const expectedUser0WeightedValue = expectedUser0UsdValue
      .mul(
        new BN(
          Math.floor(usdcAssetWeight.toNumber() * 10 ** PRECISION_DECIMALS)
        )
      )
      .div(SCALE_1E9);
    const expectedUser1WeightedValue = expectedUser1UsdValue
      .mul(
        new BN(
          Math.floor(tokenAAssetWeight.toNumber() * 10 ** PRECISION_DECIMALS)
        )
      )
      .div(SCALE_1E9);

    const usdcTolerance = new BN(300); // Working on fixing USDC rounding precision issues
    const tokenATolerance = new BN(10);

    assertBNApproximately(
      actualUser0UsdValue,
      expectedUser0WeightedValue,
      usdcTolerance
    );
    assertBNApproximately(
      actualUser1UsdValue,
      expectedUser1WeightedValue,
      tokenATolerance
    );
  });

  it("USDC withdrawal test - half withdraw with BN(1) tolerance", async () => {
    const withdrawAmount = user0InitialCTokens.div(new BN(2));

    const usdcReserveBeforeWithdraw = await fetchReserve(usdcReserve);

    const exchangeRate = getLiquidityExchangeRate(usdcReserveBeforeWithdraw);
    const expectedTokens = new BN(
      Math.floor(withdrawAmount.toNumber() * exchangeRate.toNumber())
    );

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      users[0].usdcAccount
    );

    const refreshUsdcIx = await simpleRefreshReserve(
      klendBankrunProgram,
      usdcReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.usdcOracle.publicKey
    );

    const refreshUsdcObligationIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      usdcObligation,
      [usdcReserve]
    );

    const withdrawIx = await makeKaminoWithdrawIx(
      users[0].mrgnBankrunProgram,
      {
        marginfiAccount: user0Account,
        authority: users[0].wallet.publicKey,
        bank: usdcBank,
        destinationTokenAccount: users[0].usdcAccount,
        lendingMarket: kaminoAccounts.get(MARKET)!,
        reserveLiquidityMint: ecosystem.usdcMint.publicKey,
        obligationFarmUserState: null,
        reserveFarmState: null,
      },
      {
        amount: withdrawAmount,
        isFinalWithdrawal: false,
        remaining: composeRemainingAccounts([
          [usdcBank, oracles.usdcOracle.publicKey, usdcReserve],
        ]),
      }
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(refreshUsdcIx, refreshUsdcObligationIx, withdrawIx),
      [users[0].wallet],
      false,
      true
    );

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      users[0].usdcAccount
    );
    const actualTokensReceived = new BN(balanceAfter - balanceBefore);

    assertBNApproximately(actualTokensReceived, expectedTokens, new BN(1));
  });

  it("TokenA withdrawal test - full withdraw with BN(1) tolerance", async () => {
    const withdrawAmount = user1InitialCTokens;

    const tokenAReserveBeforeWithdraw = await fetchReserve(tokenAReserve);

    const exchangeRate = getLiquidityExchangeRate(tokenAReserveBeforeWithdraw);
    const expectedTokens = new BN(
      Math.floor(withdrawAmount.toNumber() * exchangeRate.toNumber())
    );

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      users[1].tokenAAccount
    );

    const refreshTokenAIx = await simpleRefreshReserve(
      klendBankrunProgram,
      tokenAReserve,
      kaminoAccounts.get(MARKET)!,
      oracles.tokenAOracle.publicKey
    );

    const refreshTokenAObligationIx = await simpleRefreshObligation(
      klendBankrunProgram,
      kaminoAccounts.get(MARKET)!,
      tokenAObligation,
      [tokenAReserve]
    );

    const withdrawIx = await makeKaminoWithdrawIx(
      users[1].mrgnBankrunProgram,
      {
        marginfiAccount: user1Account,
        authority: users[1].wallet.publicKey,
        bank: tokenABank,
        destinationTokenAccount: users[1].tokenAAccount,
        lendingMarket: kaminoAccounts.get(MARKET)!,
        reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        obligationFarmUserState: farmAccounts.get(A_OBLIGATION_USER_STATE),
        reserveFarmState: farmAccounts.get(A_FARM_STATE),
      },
      {
        amount: new BN(0),
        isFinalWithdrawal: true,
        remaining: composeRemainingAccounts([
          [tokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
        ]),
      }
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        refreshTokenAIx,
        refreshTokenAObligationIx,
        withdrawIx
      ),
      [users[1].wallet],
      false,
      true
    );

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      users[1].tokenAAccount
    );
    const actualTokensReceived = new BN(balanceAfter - balanceBefore);

    // Sanity check: tokens received should be greater than deposit amount due to interest
    assert.ok(
      actualTokensReceived.gt(user1DepositAmount),
      `Tokens received (${actualTokensReceived.toString()}) should be greater than original deposit (${user1DepositAmount.toString()}) due to interest accrual`
    );

    assertBNApproximately(actualTokensReceived, expectedTokens, new BN(1));
  });
});
