import { assert } from "chai";
import { Keypair, Transaction, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createMintToInstruction,
} from "@solana/spl-token";
import {
  bankrunContext,
  groupAdmin,
  globalProgramAdmin,
  ecosystem,
  oracles,
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
} from "./rootHooks";
import { processBankrunTransaction } from "./utils/tools";
import { TOKEN_ACCOUNT_SIZE } from "./utils/types";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import {
  makeSolendInitReserveIx,
  getDefaultReserveConfig,
  SOLEND_PROGRAM_ID,
} from "./utils/solend-sdk";
import {
  SOLEND_RESERVE_SIZE,
  SOLEND_USDC_RESERVE_SEED,
  SOLEND_TOKEN_A_RESERVE_SEED,
} from "./utils/solend-utils";

describe("sl02: Init Solend reserves", () => {
  const usdcReserve = Keypair.fromSeed(SOLEND_USDC_RESERVE_SEED);
  const tokenAReserve = Keypair.fromSeed(SOLEND_TOKEN_A_RESERVE_SEED);

  const usdcCollateralMint = Keypair.generate();
  const tokenACollateralMint = Keypair.generate();

  const usdcLiquiditySupply = Keypair.generate();
  const usdcCollateralSupply = Keypair.generate();
  const tokenALiquiditySupply = Keypair.generate();
  const tokenACollateralSupply = Keypair.generate();

  const usdcLiquidityFeeReceiver = Keypair.generate();
  const tokenALiquidityFeeReceiver = Keypair.generate();

  const usdcDestinationCollateral = Keypair.generate();
  const tokenADestinationCollateral = Keypair.generate();

  it("(admin) Initialize USDC reserve", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);

    await refreshPullOraclesBankrun(
      oracles,
      bankrunContext,
      bankrunContext.banksClient
    );

    const createReserveAccountIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcReserve.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          SOLEND_RESERVE_SIZE
        ),
      space: SOLEND_RESERVE_SIZE,
      programId: SOLEND_PROGRAM_ID,
    });

    const createCollateralMintIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcCollateralMint.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          MINT_SIZE
        ),
      space: MINT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createLiquiditySupplyIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcLiquiditySupply.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createCollateralSupplyIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcCollateralSupply.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createFeeReceiverIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcLiquidityFeeReceiver.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createDestinationCollateralIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: usdcDestinationCollateral.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const mintAmountUsdc = 1000 * 10 ** ecosystem.usdcDecimals;
    const mintToAdminIx = createMintToInstruction(
      ecosystem.usdcMint.publicKey,
      groupAdmin.usdcAccount,
      globalProgramAdmin.wallet.publicKey,
      mintAmountUsdc
    );

    const initReserveIx = makeSolendInitReserveIx(
      {
        reserve: usdcReserve.publicKey,
        liquidityMint: ecosystem.usdcMint.publicKey,
        liquiditySupply: usdcLiquiditySupply.publicKey,
        liquidityFeeReceiver: usdcLiquidityFeeReceiver.publicKey,
        collateralMint: usdcCollateralMint.publicKey,
        collateralSupply: usdcCollateralSupply.publicKey,
        pythOracle: oracles.usdcOracle.publicKey,
        lendingMarket: solendMarket,
        lendingMarketOwner: groupAdmin.wallet.publicKey,
        transferAuthority: groupAdmin.wallet.publicKey,
        sourceLiquidity: groupAdmin.usdcAccount,
        destinationCollateral: usdcDestinationCollateral.publicKey,
      },
      {
        initialLiquidityAmount: mintAmountUsdc,
      }
    );

    const tx1 = new Transaction().add(
      createReserveAccountIx,
      createCollateralMintIx,
      createLiquiditySupplyIx,
      createCollateralSupplyIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx1,
      [
        groupAdmin.wallet,
        usdcReserve,
        usdcCollateralMint,
        usdcLiquiditySupply,
        usdcCollateralSupply,
      ],
      false,
      true
    );

    const tx2 = new Transaction().add(
      createFeeReceiverIx,
      createDestinationCollateralIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx2,
      [groupAdmin.wallet, usdcLiquidityFeeReceiver, usdcDestinationCollateral],
      false,
      true
    );

    const tx3 = new Transaction().add(mintToAdminIx, initReserveIx);

    await processBankrunTransaction(
      bankrunContext,
      tx3,
      [globalProgramAdmin.wallet, groupAdmin.wallet],
      false,
      true
    );

    solendAccounts.set(SOLEND_USDC_RESERVE, usdcReserve.publicKey);
    solendAccounts.set(
      SOLEND_USDC_LIQUIDITY_SUPPLY,
      usdcLiquiditySupply.publicKey
    );
    solendAccounts.set(
      SOLEND_USDC_COLLATERAL_MINT,
      usdcCollateralMint.publicKey
    );
    solendAccounts.set(
      SOLEND_USDC_COLLATERAL_SUPPLY,
      usdcCollateralSupply.publicKey
    );
    solendAccounts.set(
      SOLEND_USDC_FEE_RECEIVER,
      usdcLiquidityFeeReceiver.publicKey
    );

    const reserveAccount = await bankrunContext.banksClient.getAccount(
      usdcReserve.publicKey
    );

    assert.isNotNull(reserveAccount);
    assert.equal(reserveAccount.owner.toString(), SOLEND_PROGRAM_ID.toString());
  });

  it("(admin) Initialize Token A reserve", async () => {
    const solendMarket = solendAccounts.get(SOLEND_MARKET);

    await refreshPullOraclesBankrun(
      oracles,
      bankrunContext,
      bankrunContext.banksClient
    );

    const createReserveAccountIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenAReserve.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          SOLEND_RESERVE_SIZE
        ),
      space: SOLEND_RESERVE_SIZE,
      programId: SOLEND_PROGRAM_ID,
    });

    const createCollateralMintIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenACollateralMint.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          MINT_SIZE
        ),
      space: MINT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createLiquiditySupplyIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenALiquiditySupply.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createCollateralSupplyIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenACollateralSupply.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createFeeReceiverIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenALiquidityFeeReceiver.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const createDestinationCollateralIx = SystemProgram.createAccount({
      fromPubkey: groupAdmin.wallet.publicKey,
      newAccountPubkey: tokenADestinationCollateral.publicKey,
      lamports:
        await bankRunProvider.connection.getMinimumBalanceForRentExemption(
          TOKEN_ACCOUNT_SIZE
        ),
      space: TOKEN_ACCOUNT_SIZE,
      programId: TOKEN_PROGRAM_ID,
    });

    const mintAmountTokenA = 1000 * 10 ** ecosystem.tokenADecimals;
    const mintToAdminIx = createMintToInstruction(
      ecosystem.tokenAMint.publicKey,
      groupAdmin.tokenAAccount,
      globalProgramAdmin.wallet.publicKey,
      mintAmountTokenA
    );

    const initReserveIx = makeSolendInitReserveIx(
      {
        reserve: tokenAReserve.publicKey,
        liquidityMint: ecosystem.tokenAMint.publicKey,
        liquiditySupply: tokenALiquiditySupply.publicKey,
        liquidityFeeReceiver: tokenALiquidityFeeReceiver.publicKey,
        collateralMint: tokenACollateralMint.publicKey,
        collateralSupply: tokenACollateralSupply.publicKey,
        pythOracle: oracles.tokenAOracle.publicKey,
        lendingMarket: solendMarket,
        lendingMarketOwner: groupAdmin.wallet.publicKey,
        transferAuthority: groupAdmin.wallet.publicKey,
        sourceLiquidity: groupAdmin.tokenAAccount,
        destinationCollateral: tokenADestinationCollateral.publicKey,
      },
      {
        config: getDefaultReserveConfig(
          tokenALiquidityFeeReceiver.publicKey,
          ecosystem.tokenADecimals
        ),
        initialLiquidityAmount: mintAmountTokenA,
      }
    );

    const tx1 = new Transaction().add(
      createReserveAccountIx,
      createCollateralMintIx,
      createLiquiditySupplyIx,
      createCollateralSupplyIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx1,
      [
        groupAdmin.wallet,
        tokenAReserve,
        tokenACollateralMint,
        tokenALiquiditySupply,
        tokenACollateralSupply,
      ],
      false,
      true
    );

    const tx2 = new Transaction().add(
      createFeeReceiverIx,
      createDestinationCollateralIx
    );

    await processBankrunTransaction(
      bankrunContext,
      tx2,
      [
        groupAdmin.wallet,
        tokenALiquidityFeeReceiver,
        tokenADestinationCollateral,
      ],
      false,
      true
    );

    const tx3 = new Transaction().add(mintToAdminIx, initReserveIx);

    await processBankrunTransaction(
      bankrunContext,
      tx3,
      [globalProgramAdmin.wallet, groupAdmin.wallet],
      false,
      true
    );

    solendAccounts.set(SOLEND_TOKEN_A_RESERVE, tokenAReserve.publicKey);
    solendAccounts.set(
      SOLEND_TOKEN_A_LIQUIDITY_SUPPLY,
      tokenALiquiditySupply.publicKey
    );
    solendAccounts.set(
      SOLEND_TOKEN_A_COLLATERAL_MINT,
      tokenACollateralMint.publicKey
    );
    solendAccounts.set(
      SOLEND_TOKEN_A_COLLATERAL_SUPPLY,
      tokenACollateralSupply.publicKey
    );
    solendAccounts.set(
      SOLEND_TOKEN_A_FEE_RECEIVER,
      tokenALiquidityFeeReceiver.publicKey
    );

    const reserveAccount = await bankrunContext.banksClient.getAccount(
      tokenAReserve.publicKey
    );

    assert.isNotNull(reserveAccount);
    assert.equal(reserveAccount.owner.toString(), SOLEND_PROGRAM_ID.toString());
  });
});
