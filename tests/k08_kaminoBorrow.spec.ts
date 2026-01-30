// Note: We need this test to generated borrows on Kamino so the Mrgn-held user deposits will earn
// interest, we don't really care about the borrow logic itself.
import { BN } from "@coral-xyz/anchor";
import {
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  MARKET,
  TOKEN_A_RESERVE,
  USDC_RESERVE,
  users,
  bankrunContext,
  klendBankrunProgram,
  oracles,
  bankRunProvider,
  banksClient,
  verbose,
} from "./rootHooks";
import { lendingMarketAuthPda } from "@kamino-finance/klend-sdk";
import {
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  SystemProgram,
  PublicKey,
} from "@solana/web3.js";
import { KAMINO_OBLIGATION, MockUser } from "./utils/mocks";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
  wrappedU68F60toBigNumber,
} from "./utils/kamino-utils";
import { assert, expect } from "chai";
import { processBankrunTransaction } from "./utils/tools";
import { getTokenBalance } from "./utils/genericTests";
import { Clock, ProgramTestContext } from "solana-bankrun";
import { getEpochAndSlot } from "./utils/stake-utils";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

let ctx: ProgramTestContext;
let usdcReserve: PublicKey;
let tokenAReserve: PublicKey;
let market: PublicKey;

describe("k08: Borrow from Kamino reserve to simulate interest accrual", () => {
  before(async () => {
    ctx = bankrunContext;
    market = kaminoAccounts.get(MARKET);
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
  });

  async function getBalances(user: MockUser): Promise<{
    userUsdcBalance: number;
    userTokenABalance: number;
    usdcReserveSupplyBalance: number;
    tokenAReserveSupplyBalance: number;
  }> {
    const userUsdcAccount = user.usdcAccount;
    const userTokenAAccount = user.tokenAAccount;

    const [
      usdcReserveData,
      tokenAReserveData,
      userUsdcBalance,
      userTokenABalance,
    ] = await Promise.all([
      klendBankrunProgram.account.reserve.fetch(usdcReserve),
      klendBankrunProgram.account.reserve.fetch(tokenAReserve),
      getTokenBalance(bankRunProvider, userUsdcAccount),
      getTokenBalance(bankRunProvider, userTokenAAccount),
    ]);
    const usdcReserveSupply = usdcReserveData.liquidity.supplyVault;
    const tokenAReserveSupply = tokenAReserveData.liquidity.supplyVault;

    const [usdcReserveSupplyBalance, tokenAReserveSupplyBalance] =
      await Promise.all([
        getTokenBalance(bankRunProvider, usdcReserveSupply),
        getTokenBalance(bankRunProvider, tokenAReserveSupply),
      ]);

    return {
      userUsdcBalance,
      userTokenABalance,
      usdcReserveSupplyBalance,
      tokenAReserveSupplyBalance,
    };
  }

  it("(user 1) Should deposit Token A to Kamino as collateral", async () => {
    const user = users[1];
    const initialBalances = await getBalances(user);
    // Note: Kamino obligation direct obligation (not through marginfi)
    const obligation = user.accounts.get(KAMINO_OBLIGATION);
    const reserveData = await klendBankrunProgram.account.reserve.fetch(
      tokenAReserve,
    );
    const tokenALiquidityMint = reserveData.liquidity.mintPubkey;
    const tokenAReserveLiquiditySupply = reserveData.liquidity.supplyVault;
    const tokenACollateralMint = reserveData.collateral.mintPubkey;
    const tokenACollateralVault = reserveData.collateral.supplyVault;
    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId,
    );
    const depositAmount = 200_000 * 10 ** ecosystem.tokenADecimals;

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey,
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        obligation,
        [],
      ),
      await klendBankrunProgram.methods
        .depositReserveLiquidityAndObligationCollateral(new BN(depositAmount))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          reserve: tokenAReserve,
          reserveLiquidityMint: tokenALiquidityMint,
          reserveLiquiditySupply: tokenAReserveLiquiditySupply,
          reserveCollateralMint: tokenACollateralMint,
          reserveDestinationDepositCollateral: tokenACollateralVault,
          userSourceLiquidity: user.tokenAAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const postDepositBalances = await getBalances(user);
    const userTokenADiff =
      initialBalances.userTokenABalance - postDepositBalances.userTokenABalance;
    const reserveTokenADiff =
      postDepositBalances.tokenAReserveSupplyBalance -
      initialBalances.tokenAReserveSupplyBalance;
    const tolerance = depositAmount * 0.0001;
    assert.approximately(userTokenADiff, depositAmount, tolerance);
    assert.approximately(reserveTokenADiff, depositAmount, tolerance);
  });

  it("(user 1) Should borrow USDC against the Token A collateral", async () => {
    let user = users[1];
    const initialBalances = await getBalances(user);

    const borrowReserveData = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve,
    );
    const usdcFeesVault = borrowReserveData.liquidity.feeVault;
    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId,
    );
    // Note: Kamino obligation direct obligation (not through marginfi)
    const obligation = user.accounts.get(KAMINO_OBLIGATION);
    const usdcLiquidityMint = borrowReserveData.liquidity.mintPubkey;
    const usdcLiquiditySupply = borrowReserveData.liquidity.supplyVault;
    const borrowAmount = 800_000 * 10 ** ecosystem.usdcDecimals;

    // Update the bank's borrow limit
    const borrowLimit = new BN(1_000_000 * 10 ** ecosystem.usdcDecimals);
    const valueBuffer = Buffer.alloc(8);
    borrowLimit.toArrayLike(Buffer, "le", 8).copy(valueBuffer);

    let elevationTx = new Transaction().add(
      await klendBankrunProgram.methods
        // TODO what is code 45?
        .updateReserveConfig(new BN(45), valueBuffer, false)
        .accounts({
          lendingMarketOwner: groupAdmin.wallet.publicKey,
          reserve: usdcReserve,
          lendingMarket: market,
        })
        .instruction(),
    );
    await processBankrunTransaction(ctx, elevationTx, [groupAdmin.wallet]);

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey,
      ),
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        tokenAReserve,
      ]),
      await klendBankrunProgram.methods
        .borrowObligationLiquidity(new BN(borrowAmount))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          borrowReserve: usdcReserve,
          borrowReserveLiquidityMint: usdcLiquidityMint,
          reserveSourceLiquidity: usdcLiquiditySupply,
          userDestinationLiquidity: user.usdcAccount,
          borrowReserveLiquidityFeeReceiver: usdcFeesVault, // Fee receiver
          referrerTokenState: klendBankrunProgram.programId, // Referrer token state (using program id as a dummy)
          tokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const postBorrowBalances = await getBalances(user);
    const userUsdcDiff =
      postBorrowBalances.userUsdcBalance - initialBalances.userUsdcBalance;
    const reserveUsdcDiff =
      initialBalances.usdcReserveSupplyBalance -
      postBorrowBalances.usdcReserveSupplyBalance;
    const tolerance = borrowAmount * 0.00001;
    assert.approximately(userUsdcDiff, borrowAmount, tolerance);
    assert.approximately(reserveUsdcDiff, borrowAmount, tolerance);
    if (verbose) {
      console.log(
        "reserve supply before: " +
          initialBalances.usdcReserveSupplyBalance +
          " after " +
          postBorrowBalances.usdcReserveSupplyBalance,
      );
    }
  });

  it("(user 2) Borrows Token A against USDC collateral", async () => {
    let user = users[2];
    // Note: Kamino obligation direct obligation (not through marginfi)
    const obligation = user.accounts.get(KAMINO_OBLIGATION);
    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId,
    );
    const depositAmount = 80_000_000 * 10 ** ecosystem.usdcDecimals;
    const borrowAmount = 8_000 * 10 ** ecosystem.tokenADecimals;

    // Update the token A reserve's borrow limit
    const borrowLimit = new BN(1_000_000 * 10 ** ecosystem.usdcDecimals);
    const valueBuffer = Buffer.alloc(8);
    borrowLimit.toArrayLike(Buffer, "le", 8).copy(valueBuffer);
    let elevationTx = new Transaction().add(
      await klendBankrunProgram.methods
        // TODO what is code 45?
        .updateReserveConfig(new BN(45), valueBuffer, false)
        .accounts({
          lendingMarketOwner: groupAdmin.wallet.publicKey,
          reserve: tokenAReserve,
          lendingMarket: market,
        })
        .instruction(),
    );
    await processBankrunTransaction(ctx, elevationTx, [groupAdmin.wallet]);

    // Deposit some USDC first

    const usdcReserveData = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve,
    );
    const usdcLiquidityMint = usdcReserveData.liquidity.mintPubkey;
    const usdcLiquiditySupply = usdcReserveData.liquidity.supplyVault;
    const usdcCollateralMint = usdcReserveData.collateral.mintPubkey;
    const usdcCollateralVault = usdcReserveData.collateral.supplyVault;

    let depositTx = new Transaction();
    depositTx.add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation),
      await klendBankrunProgram.methods
        .depositReserveLiquidityAndObligationCollateral(new BN(depositAmount))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          reserve: usdcReserve,
          reserveLiquidityMint: usdcLiquidityMint,
          reserveLiquiditySupply: usdcLiquiditySupply,
          reserveCollateralMint: usdcCollateralMint,
          reserveDestinationDepositCollateral: usdcCollateralVault,
          userSourceLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );

    await processBankrunTransaction(ctx, depositTx, [user.wallet]);

    const borrowReserveData = await klendBankrunProgram.account.reserve.fetch(
      tokenAReserve,
    );
    const tokenALiquidityMint = borrowReserveData.liquidity.mintPubkey;
    const tokenALiquiditySupply = borrowReserveData.liquidity.supplyVault;
    const tokenAFeesVault = borrowReserveData.liquidity.feeVault;

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey,
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await klendBankrunProgram.methods
        .borrowObligationLiquidity(new BN(borrowAmount))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          borrowReserve: tokenAReserve,
          borrowReserveLiquidityMint: tokenALiquidityMint,
          reserveSourceLiquidity: tokenALiquiditySupply,
          userDestinationLiquidity: user.tokenAAccount,
          borrowReserveLiquidityFeeReceiver: tokenAFeesVault, // Fee receiver
          referrerTokenState: klendBankrunProgram.programId, // Referrer token state (using program id as a dummy)
          tokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );
    await processBankrunTransaction(ctx, tx, [user.wallet]);
  });

  it("Should accrue interest on USDC reserve by advancing the clock", async () => {
    const resBefore = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve,
    );
    const availableBefore = resBefore.liquidity.availableAmount.toNumber();
    const borrowedBefore = wrappedU68F60toBigNumber(
      resBefore.liquidity.borrowedAmountSf,
    );

    // Warp to 1 hour in the future (1 hour = 3600 seconds, at ~2.4s per slot = ~1500 slots)
    const clock = await banksClient.getClock();
    const timeTarget = clock.unixTimestamp + 3600n;
    const targetUnix = BigInt(timeTarget);
    const newClock = new Clock(
      0n, // slot
      0n, // epochStartTimestamp
      0n, // epoch
      0n, // leaderScheduleEpoch
      targetUnix,
    );
    bankrunContext.setClock(newClock);
    let { epoch: _epoch, slot } = await getEpochAndSlot(banksClient);
    const targetSlot = BigInt(slot + 1500);
    if (verbose) {
      console.log(
        `Warping slot ${targetSlot}  time ${timeTarget} (~1hr later)`,
      );
    }
    ctx.warpToSlot(targetSlot);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Capture the interest accrual from the time jump
    let refreshTx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
    );
    await processBankrunTransaction(
      ctx,
      refreshTx,
      [groupAdmin.wallet],
      false,
      true,
    );

    const resAfter = await klendBankrunProgram.account.reserve.fetch(
      usdcReserve,
    );
    const availableAfter = resAfter.liquidity.availableAmount.toNumber();
    const borrowedAfter = wrappedU68F60toBigNumber(
      resAfter.liquidity.borrowedAmountSf,
    );
    const borrowedDiff = Number(borrowedAfter.minus(borrowedBefore));

    if (verbose) {
      console.log(
        "borrowed amt before: " +
          borrowedBefore.toString() +
          " after " +
          borrowedAfter.toString() +
          " diff " +
          borrowedDiff,
      );
    }

    // TODO assert more specific interest increase...
    assert(borrowedAfter > borrowedBefore);
    // Note: available balance only changes in response to borrows, interest does not impact the actual
    // liquidity available for lending.
    // TODO assert this value does decrease during a borrow?
    assert.equal(availableAfter, availableBefore);
  });
});
