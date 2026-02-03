import { BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  Transaction,
  Keypair,
  SystemProgram,
  ComputeBudgetProgram,
} from "@solana/web3.js";
import {
  ecosystem,
  solendAccounts,
  solendGroup,
  users,
  bankrunContext,
  bankrunProgram,
  oracles,
  banksClient,
  globalProgramAdmin,
  bankRunProvider,
  SOLEND_MARKET,
  SOLEND_USDC_BANK,
  SOLEND_TOKEN_A_BANK,
  SOLEND_USDC_RESERVE,
  SOLEND_TOKEN_A_RESERVE,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { getTokenBalance, assertBNApproximately } from "./utils/genericTests";
import { accountInit } from "./utils/user-instructions";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import BigNumber from "bignumber.js";
import { Clock } from "solana-bankrun";
import assert from "assert";
import { createMintToInstruction } from "@solana/spl-token";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  makeSolendDepositIx,
  makeSolendWithdrawIx,
} from "./utils/solend-instructions";
import { makeSolendRefreshReserveIx } from "./utils/solend-sdk";
import { composeRemainingAccounts } from "./utils/user-instructions";
import { parseReserve } from "./utils/solend-sdk/state/reserve";
import { makePulseHealthIx } from "./utils/drift-utils";
import { SOLEND_NULL_PUBKEY } from "./utils/solend-utils";

const WAD = new BN(10).pow(new BN(18));

describe("sl09: cToken Conversion Math Validation", () => {
  let user0Account: PublicKey;
  let user1Account: PublicKey;
  let usdcBank: PublicKey;
  let tokenABank: PublicKey;
  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;

  let user0InitialCTokens: BN;
  let user1InitialCTokens: BN;
  let user0DepositAmount: BN;
  let user1DepositAmount: BN;

  before(async () => {
    usdcBank = solendAccounts.get(SOLEND_USDC_BANK)!;
    tokenABank = solendAccounts.get(SOLEND_TOKEN_A_BANK)!;
    usdcReserve = solendAccounts.get(SOLEND_USDC_RESERVE)!;
    tokenAReserve = solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!;
  });

  it("Creates temporary-seed marginfi accounts for testing", async () => {
    for (let i = 0; i < 2; i++) {
      const tempSeed = Buffer.alloc(32);
      tempSeed.write(`sl09_test_user_${i}_`.padEnd(32, "0"));
      const accountKeypair = Keypair.fromSeed(tempSeed);

      const initAccountIx = await accountInit(users[i].mrgnBankrunProgram, {
        marginfiGroup: solendGroup.publicKey,
        marginfiAccount: accountKeypair.publicKey,
        authority: users[i].wallet.publicKey,
        feePayer: users[i].wallet.publicKey,
      });

      const tx = new Transaction().add(initAccountIx);
      await processBankrunTransaction(bankrunContext, tx, [
        users[i].wallet,
        accountKeypair,
      ]);

      users[i].accounts.set("sl09_temp_account", accountKeypair.publicKey);

      if (i === 0) user0Account = accountKeypair.publicKey;
      else user1Account = accountKeypair.publicKey;
    }
  });

  it("Deposits tokens and validates initial cToken conversion", async () => {
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
    await processBankrunTransaction(bankrunContext, fundTx, [
      globalProgramAdmin.wallet,
    ]);

    user0DepositAmount = new BN(100 * 10 ** ecosystem.usdcDecimals);

    // Refresh the USDC reserve first to update interest accrual and avoid stale reserve error
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        makeSolendRefreshReserveIx({
          reserve: usdcReserve,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      ),
      [users[0].wallet]
    );

    // Now fetch fresh state to calculate expected cTokens
    const usdcReserveBefore = await fetchAndParseReserve(usdcReserve);
    const expectedUsdcCTokens = calculateDepositCTokens(
      user0DepositAmount,
      usdcReserveBefore
    );

    const depositIx0 = await makeSolendDepositIx(
      users[0].mrgnBankrunProgram,
      {
        marginfiAccount: user0Account,
        bank: usdcBank,
        signerTokenAccount: users[0].usdcAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.usdcOracle.publicKey,
      },
      {
        amount: user0DepositAmount,
      }
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx0),
      [users[0].wallet]
    );

    const marginfiAccount0 = await bankrunProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const balance0 = marginfiAccount0.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(usdcBank)
    );
    user0InitialCTokens = new BN(
      wrappedI80F48toBigNumber(balance0.assetShares).toString()
    );

    assertBNApproximately(user0InitialCTokens, expectedUsdcCTokens, new BN(1));

    user1DepositAmount = new BN(10 * 10 ** ecosystem.tokenADecimals);

    // Refresh the Token A reserve first to update interest accrual and avoid stale reserve error
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        makeSolendRefreshReserveIx({
          reserve: tokenAReserve,
          pythOracle: oracles.tokenAOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      ),
      [users[1].wallet]
    );

    // Now fetch fresh state to calculate expected cTokens
    const tokenAReserveBefore = await fetchAndParseReserve(tokenAReserve);
    const expectedTokenACTokens = calculateDepositCTokens(
      user1DepositAmount,
      tokenAReserveBefore
    );

    const depositIx1 = await makeSolendDepositIx(
      users[1].mrgnBankrunProgram,
      {
        marginfiAccount: user1Account,
        bank: tokenABank,
        signerTokenAccount: users[1].tokenAAccount,
        lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
        pythPrice: oracles.tokenAOracle.publicKey,
      },
      {
        amount: user1DepositAmount,
      }
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx1),
      [users[1].wallet]
    );

    const marginfiAccount1 = await bankrunProgram.account.marginfiAccount.fetch(
      user1Account
    );
    const balance1 = marginfiAccount1.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(tokenABank)
    );
    user1InitialCTokens = new BN(
      wrappedI80F48toBigNumber(balance1.assetShares).toString()
    );

    assertBNApproximately(
      user1InitialCTokens,
      expectedTokenACTokens,
      new BN(1)
    );
  });

  it("Advances time by 30 days and verifies exchange rate changes", async () => {
    const usdcReserveBefore = await fetchAndParseReserve(usdcReserve);
    const tokenAReserveBefore = await fetchAndParseReserve(tokenAReserve);

    const usdcExchangeRateBefore = calculateExchangeRate(usdcReserveBefore);
    const tokenAExchangeRateBefore = calculateExchangeRate(tokenAReserveBefore);

    const currentClock = await banksClient.getClock();
    const currentSlot = Number(currentClock.slot);
    const currentTimestamp = Number(currentClock.unixTimestamp);

    const daysToAdvance = 30;
    const secondsToAdvance = daysToAdvance * 86400;
    const slotsToAdvance = secondsToAdvance * 0.4;
    const newSlot = currentSlot + slotsToAdvance;
    const newTimestamp = currentTimestamp + secondsToAdvance;

    const newClock = new Clock(
      BigInt(newSlot),
      0n,
      currentClock.epoch,
      0n,
      BigInt(newTimestamp)
    );

    bankrunContext.setClock(newClock);

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const refreshTx = new Transaction()
      .add(
        makeSolendRefreshReserveIx({
          reserve: usdcReserve,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      )
      .add(
        makeSolendRefreshReserveIx({
          reserve: tokenAReserve,
          pythOracle: oracles.tokenAOracle.publicKey,
          switchboardOracle: SOLEND_NULL_PUBKEY,
        })
      );

    await processBankrunTransaction(bankrunContext, refreshTx, [
      globalProgramAdmin.wallet,
    ]);

    const usdcReserveAfter = await fetchAndParseReserve(usdcReserve);
    const tokenAReserveAfter = await fetchAndParseReserve(tokenAReserve);

    const usdcExchangeRateAfter = calculateExchangeRate(usdcReserveAfter);
    const tokenAExchangeRateAfter = calculateExchangeRate(tokenAReserveAfter);

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
    const marginfiAccount0 = await bankrunProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const balance0 = marginfiAccount0.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(usdcBank)
    );
    const user0CTokens = new BN(
      wrappedI80F48toBigNumber(balance0.assetShares).toString()
    );
    const usdcReserve = await fetchAndParseReserve(
      solendAccounts.get(SOLEND_USDC_RESERVE)!
    );
    const usdcPrice = new BN(oracles.usdcPrice * 10 ** 6);
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
          pubkey: solendAccounts.get(SOLEND_USDC_RESERVE)!,
          isSigner: false,
          isWritable: false,
        },
      ]
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseHealthIx0),
      [users[0].wallet],
      false,
      true
    );
    const accAfter0 = await bankrunProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const healthCache0 = accAfter0.healthCache;
    const assetValue0 = wrappedI80F48toBigNumber(healthCache0.assetValue);
    const actualUser0UsdValue = new BN(
      Math.floor(assetValue0.toNumber() * 10 ** 6)
    );

    const marginfiAccount1 = await bankrunProgram.account.marginfiAccount.fetch(
      user1Account
    );
    const balance1 = marginfiAccount1.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(tokenABank)
    );
    const user1CTokens = new BN(
      wrappedI80F48toBigNumber(balance1.assetShares).toString()
    );
    const tokenAReserve = await fetchAndParseReserve(
      solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!
    );
    const tokenAPrice = new BN(oracles.tokenAPrice * 10 ** 6);
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
          pubkey: solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!,
          isSigner: false,
          isWritable: false,
        },
      ]
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(pulseHealthIx1),
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
      Math.floor(assetValue1.toNumber() * 10 ** 6)
    );

    const usdcBankData = await bankrunProgram.account.bank.fetch(usdcBank);
    const tokenABankData = await bankrunProgram.account.bank.fetch(tokenABank);

    const usdcAssetWeight = wrappedI80F48toBigNumber(
      usdcBankData.config.assetWeightInit
    );
    const tokenAAssetWeight = wrappedI80F48toBigNumber(
      tokenABankData.config.assetWeightInit
    );

    const expectedUser0WeightedValue = calculateSolendExpectedValue(
      user0CTokens,
      usdcReserve,
      usdcPrice,
      ecosystem.usdcDecimals,
      usdcAssetWeight
    );

    const expectedUser1WeightedValue = calculateSolendExpectedValue(
      user1CTokens,
      tokenAReserve,
      tokenAPrice,
      ecosystem.tokenADecimals,
      tokenAAssetWeight
    );

    // Tolerance of 100 accounts for minor rounding differences between
    // TypeScript BigNumber and on-chain I80F48 fixed-point math
    const tolerance = new BN(100);
    assertBNApproximately(
      actualUser0UsdValue,
      expectedUser0WeightedValue,
      tolerance
    );
    assertBNApproximately(
      actualUser1UsdValue,
      expectedUser1WeightedValue,
      tolerance
    );
  });

  it("Validates partial withdrawal matches calculated token amount", async () => {
    const withdrawCTokens = user0InitialCTokens.div(new BN(2));

    const usdcReserve = await fetchAndParseReserve(
      solendAccounts.get(SOLEND_USDC_RESERVE)!
    );
    const expectedTokens = calculateWithdrawalTokens(
      withdrawCTokens,
      usdcReserve
    );

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      users[0].usdcAccount
    );

    const withdrawTx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
      await makeSolendWithdrawIx(
        users[0].mrgnBankrunProgram,
        {
          marginfiAccount: user0Account,
          bank: usdcBank,
          destinationTokenAccount: users[0].usdcAccount,
          lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
          pythPrice: oracles.usdcOracle.publicKey,
        },
        {
          amount: withdrawCTokens,
          withdrawAll: false,
          remaining: composeRemainingAccounts([
            [
              usdcBank,
              oracles.usdcOracle.publicKey,
              solendAccounts.get(SOLEND_USDC_RESERVE)!,
            ],
          ]),
        }
      )
    );

    await processBankrunTransaction(bankrunContext, withdrawTx, [
      users[0].wallet,
    ]);

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      users[0].usdcAccount
    );
    const actualTokensReceived = new BN(balanceAfter - balanceBefore);

    assertBNApproximately(actualTokensReceived, expectedTokens, new BN(1));
  });

  it("Validates full withdrawal and final exchange rate", async () => {
    const tokenAReserve = await fetchAndParseReserve(
      solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!
    );
    const expectedTokens = calculateWithdrawalTokens(
      user1InitialCTokens,
      tokenAReserve
    );

    const balanceBefore = await getTokenBalance(
      bankRunProvider,
      users[1].tokenAAccount
    );

    const withdrawTx = new Transaction().add(
      await makeSolendWithdrawIx(
        users[1].mrgnBankrunProgram,
        {
          marginfiAccount: user1Account,
          bank: tokenABank,
          destinationTokenAccount: users[1].tokenAAccount,
          lendingMarket: solendAccounts.get(SOLEND_MARKET)!,
          pythPrice: oracles.tokenAOracle.publicKey,
        },
        {
          amount: new BN(0),
          withdrawAll: true,
          remaining: composeRemainingAccounts([
            [
              tokenABank,
              oracles.tokenAOracle.publicKey,
              solendAccounts.get(SOLEND_TOKEN_A_RESERVE)!,
            ],
          ]),
        }
      )
    );

    await processBankrunTransaction(bankrunContext, withdrawTx, [
      users[1].wallet,
    ]);

    const balanceAfter = await getTokenBalance(
      bankRunProvider,
      users[1].tokenAAccount
    );
    const actualTokensReceived = new BN(balanceAfter - balanceBefore);

    assert.ok(
      actualTokensReceived.gt(user1DepositAmount),
      `Should receive more than deposited due to interest. Deposited: ${user1DepositAmount.toString()}, Received: ${actualTokensReceived.toString()}`
    );

    assertBNApproximately(actualTokensReceived, expectedTokens, new BN(1));
  });
});

/**
 * Fetches and parses a Solend reserve account
 */
async function fetchAndParseReserve(reservePubkey: PublicKey): Promise<any> {
  const reserveAccount = await bankrunContext.banksClient.getAccount(
    reservePubkey
  );
  if (!reserveAccount) {
    throw new Error(`Reserve account not found: ${reservePubkey.toString()}`);
  }

  const reserve = parseReserve(reservePubkey, {
    ...reserveAccount,
    data: Buffer.from(reserveAccount.data),
  });
  return reserve.info;
}

/**
 * Calculates the current exchange rate for a Solend reserve
 * Exchange rate = Total Liquidity / cToken Supply
 * Where Total Liquidity = Available Amount + Borrowed Amount - Protocol Fees
 */
function calculateExchangeRate(reserve: any): BN {
  const availableAmount = new BN(reserve.liquidity.availableAmount.toString());
  const borrowedAmountWads = new BN(
    reserve.liquidity.borrowedAmountWads.toString()
  );
  const borrowedAmount = borrowedAmountWads.div(WAD);
  const protocolFeesWads = new BN(
    reserve.liquidity.accumulatedProtocolFeesWads.toString()
  );
  const protocolFees = protocolFeesWads.div(WAD);

  const totalLiquidity = availableAmount.add(borrowedAmount).sub(protocolFees);

  const cTokenSupply = new BN(reserve.collateral.mintTotalSupply.toString());

  if (cTokenSupply.isZero()) {
    return new BN(1);
  }

  return totalLiquidity.mul(new BN(10).pow(new BN(9))).div(cTokenSupply);
}

/**
 * Calculates expected cTokens when depositing tokens
 * cTokens = depositAmount / exchangeRate
 */
function calculateDepositCTokens(depositAmount: BN, reserve: any): BN {
  const exchangeRate = calculateExchangeRate(reserve);

  return depositAmount.mul(new BN(10).pow(new BN(9))).div(exchangeRate);
}

/**
 * Calculates expected tokens when withdrawing cTokens
 * tokens = cTokens * exchangeRate
 */
function calculateWithdrawalTokens(cTokenAmount: BN, reserve: any): BN {
  const exchangeRate = calculateExchangeRate(reserve);

  return cTokenAmount.mul(exchangeRate).div(new BN(10).pow(new BN(9)));
}

/**
 * Calculates the expected USD value for Solend positions in health checks
 * Following the exact pattern from d13 but adapting for Solend's exchange rate adjustment
 * @param assetWeight - Optional asset weight as BigNumber (e.g., 0.8 for 80%)
 */
function calculateSolendExpectedValue(
  cTokenAmount: BN,
  reserve: any,
  rawOraclePrice: BN,
  tokenDecimals: number,
  assetWeight?: BigNumber
): BN {
  const exchangeRate = calculateExchangeRate(reserve);

  const adjustedOraclePrice = rawOraclePrice
    .mul(exchangeRate)
    .div(new BN(10).pow(new BN(9)));

  // Apply confidence interval to adjusted price (not raw price)
  const confidenceInterval = adjustedOraclePrice
    .mul(new BN(212))
    .div(new BN(10000));
  const lowerPrice = adjustedOraclePrice.sub(confidenceInterval);

  let expectedValue = cTokenAmount
    .mul(lowerPrice)
    .div(new BN(10).pow(new BN(tokenDecimals)));

  if (assetWeight) {
    const WEIGHT_SCALE = new BN("10000000000");
    const weightScaled = new BN(
      assetWeight.times(WEIGHT_SCALE.toString()).toFixed(0)
    );
    expectedValue = expectedValue.mul(weightScaled).div(WEIGHT_SCALE);
  }

  return expectedValue;
}
