import { assert } from "chai";
import {
  Keypair,
  Transaction,
  SystemProgram,
  PublicKey,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createInitializeAccountInstruction,
} from "@solana/spl-token";
import {
  bankrunContext,
  ecosystem,
  users,
  solendAccounts,
  SOLEND_MARKET,
  SOLEND_USDC_RESERVE,
  SOLEND_TOKEN_A_RESERVE,
  SOLEND_USDC_LIQUIDITY_SUPPLY,
  SOLEND_USDC_COLLATERAL_MINT,
  SOLEND_USDC_COLLATERAL_SUPPLY,
  SOLEND_USDC_FEE_RECEIVER,
  SOLEND_TOKEN_A_LIQUIDITY_SUPPLY,
  SOLEND_TOKEN_A_COLLATERAL_MINT,
  SOLEND_TOKEN_A_COLLATERAL_SUPPLY,
  SOLEND_TOKEN_A_FEE_RECEIVER,
  bankRunProvider,
  oracles,
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { TOKEN_ACCOUNT_SIZE } from "./utils/types";
import {
  makeSolendRefreshReserveIx,
  makeSolendRefreshObligationIx,
  makeSolendDepositReserveLiquidityAndObligationCollateralIx,
  makeSolendBorrowObligationLiquidityIx,
} from "./utils/solend-sdk";
import { SOLEND_NULL_PUBKEY } from "./utils/solend-utils";
import { getTokenBalance } from "./utils/genericTests";
import { MockUser } from "./utils/mocks";
import BN from "bn.js";

describe("sl04: Solend - Deposits and Borrows", () => {
  let userA: MockUser;
  let userB: MockUser;

  let userACTokenA: Keypair;
  let userBCUsdc: Keypair;

  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;
  const USER_A_DEPOSIT_AMOUNT = new BN(50_000 * 10 ** ecosystem.tokenADecimals); // 50k Token A = $500k
  const USER_A_BORROW_AMOUNT = new BN(10_000 * 10 ** ecosystem.usdcDecimals); // 10k USDC = $10k
  const USER_B_DEPOSIT_AMOUNT = new BN(50_000 * 10 ** ecosystem.usdcDecimals); // 50k USDC = $50k
  const USER_B_BORROW_AMOUNT = new BN(1_000 * 10 ** ecosystem.tokenADecimals); // 1k Token A = $10k

  before(async () => {
    userA = users[0];
    userB = users[1];

    usdcReserve = solendAccounts.get(SOLEND_USDC_RESERVE);
    tokenAReserve = solendAccounts.get(SOLEND_TOKEN_A_RESERVE);
  });

  it("(user A) Create collateral token account and deposit Token A", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);
    const userAObligation = solendAccounts.get("user_a_obligation");

    const tokenACollateralMint = solendAccounts.get(
      SOLEND_TOKEN_A_COLLATERAL_MINT
    );
    const tokenALiquiditySupply = solendAccounts.get(
      SOLEND_TOKEN_A_LIQUIDITY_SUPPLY
    );
    const tokenACollateralSupply = solendAccounts.get(
      SOLEND_TOKEN_A_COLLATERAL_SUPPLY
    );

    userACTokenA = Keypair.generate();

    const createCollateralAccountIx = SystemProgram.createAccount({
      fromPubkey: userA.wallet.publicKey,
      newAccountPubkey: userACTokenA.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const initCollateralAccountIx = createInitializeAccountInstruction(
      userACTokenA.publicKey,
      tokenACollateralMint,
      userA.wallet.publicKey,
      TOKEN_PROGRAM_ID
    );

    const depositIx =
      makeSolendDepositReserveLiquidityAndObligationCollateralIx(
        {
          sourceLiquidity: userA.tokenAAccount,
          sourceCollateral: userACTokenA.publicKey,
          destinationCollateral: tokenACollateralSupply,
          reserve: tokenAReserve,
          reserveLiquiditySupply: tokenALiquiditySupply,
          reserveCollateralMint: tokenACollateralMint,
          lendingMarket: solendMarket,
          obligation: userAObligation,
          obligationOwner: userA.wallet.publicKey,
          pythOracle: oracles.tokenAOracle.publicKey,
          switchboardOracle: oracles.tokenAOracle.publicKey,
          userTransferAuthority: userA.wallet.publicKey,
        },
        {
          liquidityAmount: USER_A_DEPOSIT_AMOUNT,
        }
      );

    const userTokenBefore = await getTokenBalance(
      bankRunProvider,
      userA.tokenAAccount
    );

    const tx = new Transaction().add(
      createCollateralAccountIx,
      initCollateralAccountIx,
      depositIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet, userACTokenA],
      false,
      true
    );

    const userTokenAfter = await getTokenBalance(
      bankRunProvider,
      userA.tokenAAccount
    );

    assert.equal(
      new BN(userTokenBefore).sub(new BN(userTokenAfter)).toString(),
      USER_A_DEPOSIT_AMOUNT.toString()
    );

    userA.accounts.set("solend_ctokena", userACTokenA.publicKey);
  });

  it("(user B) Create collateral token account and deposit USDC", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);
    const userBObligation = solendAccounts.get("user_b_obligation");

    const usdcCollateralMint = solendAccounts.get(SOLEND_USDC_COLLATERAL_MINT);
    const usdcLiquiditySupply = solendAccounts.get(
      SOLEND_USDC_LIQUIDITY_SUPPLY
    );
    const usdcCollateralSupply = solendAccounts.get(
      SOLEND_USDC_COLLATERAL_SUPPLY
    );

    userBCUsdc = Keypair.generate();

    const createCollateralAccountIx = SystemProgram.createAccount({
      fromPubkey: userB.wallet.publicKey,
      newAccountPubkey: userBCUsdc.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const initCollateralAccountIx = createInitializeAccountInstruction(
      userBCUsdc.publicKey,
      usdcCollateralMint,
      userB.wallet.publicKey,
      TOKEN_PROGRAM_ID
    );

    const depositIx =
      makeSolendDepositReserveLiquidityAndObligationCollateralIx(
        {
          sourceLiquidity: userB.usdcAccount,
          sourceCollateral: userBCUsdc.publicKey,
          destinationCollateral: usdcCollateralSupply,
          reserve: usdcReserve,
          reserveLiquiditySupply: usdcLiquiditySupply,
          reserveCollateralMint: usdcCollateralMint,
          lendingMarket: solendMarket,
          obligation: userBObligation,
          obligationOwner: userB.wallet.publicKey,
          pythOracle: oracles.usdcOracle.publicKey,
          switchboardOracle: oracles.usdcOracle.publicKey,
          userTransferAuthority: userB.wallet.publicKey,
        },
        {
          liquidityAmount: USER_B_DEPOSIT_AMOUNT,
        }
      );

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      userB.usdcAccount
    );

    const tx = new Transaction().add(
      createCollateralAccountIx,
      initCollateralAccountIx,
      depositIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userB.wallet, userBCUsdc],
      false,
      true
    );

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      userB.usdcAccount
    );

    assert.equal(
      new BN(userUsdcBefore).sub(new BN(userUsdcAfter)).toString(),
      USER_B_DEPOSIT_AMOUNT.toString()
    );

    userB.accounts.set("solend_cusdc", userBCUsdc.publicKey);
  });

  it("(user A) Borrow USDC against Token A collateral", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);
    const userAObligation = solendAccounts.get("user_a_obligation");

    const usdcLiquiditySupply = solendAccounts.get(
      SOLEND_USDC_LIQUIDITY_SUPPLY
    );
    const usdcFeeReceiver = solendAccounts.get(SOLEND_USDC_FEE_RECEIVER);

    const refreshDepositReserveIx = makeSolendRefreshReserveIx({
      reserve: tokenAReserve,
      pythOracle: oracles.tokenAOracle.publicKey,
      switchboardOracle: SOLEND_NULL_PUBKEY,
    });

    const refreshBorrowReserveIx = makeSolendRefreshReserveIx({
      reserve: usdcReserve,
      pythOracle: oracles.usdcOracle.publicKey,
      switchboardOracle: SOLEND_NULL_PUBKEY,
    });

    const refreshObligationIx = makeSolendRefreshObligationIx({
      obligation: userAObligation,
      depositReserves: [tokenAReserve],
      borrowReserves: [],
    });

    const borrowIx = makeSolendBorrowObligationLiquidityIx(
      {
        sourceLiquidity: usdcLiquiditySupply,
        destinationLiquidity: userA.usdcAccount,
        borrowReserve: usdcReserve,
        borrowReserveLiquidityFeeReceiver: usdcFeeReceiver,
        obligation: userAObligation,
        lendingMarket: solendMarket,
        obligationOwner: userA.wallet.publicKey,
        depositReserves: [tokenAReserve],
      },
      {
        liquidityAmount: USER_A_BORROW_AMOUNT,
      }
    );

    const userUsdcBefore = await getTokenBalance(
      bankRunProvider,
      userA.usdcAccount
    );

    const tx = new Transaction().add(
      refreshDepositReserveIx,
      refreshBorrowReserveIx,
      refreshObligationIx,
      borrowIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userA.wallet],
      false,
      true
    );

    const userUsdcAfter = await getTokenBalance(
      bankRunProvider,
      userA.usdcAccount
    );
    const borrowedAmount = new BN(userUsdcAfter).sub(new BN(userUsdcBefore));

    assert.equal(borrowedAmount.toString(), USER_A_BORROW_AMOUNT.toString());
  });

  it("(user B) Borrow Token A against USDC collateral", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);
    const userBObligation = solendAccounts.get("user_b_obligation");

    const tokenALiquiditySupply = solendAccounts.get(
      SOLEND_TOKEN_A_LIQUIDITY_SUPPLY
    );
    const tokenAFeeReceiver = solendAccounts.get(SOLEND_TOKEN_A_FEE_RECEIVER);

    const refreshDepositReserveIx = makeSolendRefreshReserveIx({
      reserve: usdcReserve,
      pythOracle: oracles.usdcOracle.publicKey,
      switchboardOracle: SOLEND_NULL_PUBKEY,
    });

    const refreshBorrowReserveIx = makeSolendRefreshReserveIx({
      reserve: tokenAReserve,
      pythOracle: oracles.tokenAOracle.publicKey,
      switchboardOracle: SOLEND_NULL_PUBKEY,
    });

    const refreshObligationIx = makeSolendRefreshObligationIx({
      obligation: userBObligation,
      depositReserves: [usdcReserve],
      borrowReserves: [],
    });

    const borrowIx = makeSolendBorrowObligationLiquidityIx(
      {
        sourceLiquidity: tokenALiquiditySupply,
        destinationLiquidity: userB.tokenAAccount,
        borrowReserve: tokenAReserve,
        borrowReserveLiquidityFeeReceiver: tokenAFeeReceiver,
        obligation: userBObligation,
        lendingMarket: solendMarket,
        obligationOwner: userB.wallet.publicKey,
        depositReserves: [usdcReserve],
      },
      {
        liquidityAmount: USER_B_BORROW_AMOUNT,
      }
    );

    const userTokenABefore = await getTokenBalance(
      bankRunProvider,
      userB.tokenAAccount
    );

    const tx = new Transaction().add(
      refreshDepositReserveIx,
      refreshBorrowReserveIx,
      refreshObligationIx,
      borrowIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx,
      [userB.wallet],
      false,
      true
    );

    const userTokenAAfter = await getTokenBalance(
      bankRunProvider,
      userB.tokenAAccount
    );
    const borrowedAmount = new BN(userTokenAAfter).sub(
      new BN(userTokenABefore)
    );

    assert.equal(borrowedAmount.toString(), USER_B_BORROW_AMOUNT.toString());
  });
});
