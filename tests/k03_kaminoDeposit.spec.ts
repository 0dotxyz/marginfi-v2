import { BN } from "@coral-xyz/anchor";
import {
  kaminoAccounts,
  MARKET,
  oracles,
  USDC_RESERVE,
  users,
  bankrunContext,
  bankRunProvider,
  klendBankrunProgram,
} from "./rootHooks";
import { lendingMarketAuthPda, Reserve } from "@kamino-finance/klend-sdk";
import { SYSVAR_INSTRUCTIONS_PUBKEY, Transaction } from "@solana/web3.js";
import { KAMINO_OBLIGATION } from "./utils/mocks";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import { processBankrunTransaction } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";
import { assert } from "chai";
import { getTokenBalance } from "./utils/genericTests";

let ctx: ProgramTestContext;

describe("k03: Deposit to Kamino reserve", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  const depositAmt: number = 100_000;
  it("(user 0) Deposit USDC to Kamino reserve - happy path", async () => {
    const user = users[0];
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(usdcReserve)).data
    );
    const liquidityMint = reserveAcc.liquidity.mintPubkey;
    const reserveLiquiditySupply = reserveAcc.liquidity.supplyVault;
    const collateralMint = reserveAcc.collateral.mintPubkey;
    const collateralVault = reserveAcc.collateral.supplyVault;

    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId
    );
    const obligation = user.accounts.get(KAMINO_OBLIGATION);
    const [userBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
    ]);

    let tx = new Transaction();
    tx.add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation),
      await klendBankrunProgram.methods
        .depositReserveLiquidityAndObligationCollateral(new BN(depositAmt))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          reserve: usdcReserve,
          reserveLiquidityMint: liquidityMint,
          reserveLiquiditySupply: reserveLiquiditySupply,
          reserveCollateralMint: collateralMint,
          reserveDestinationDepositCollateral: collateralVault,
          userSourceLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [userAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
    ]);
    assert.equal(userBefore - userAfter, depositAmt);
    assert.equal(vaultAfter - vaultBefore, depositAmt);
  });

  const withdrawAmt: number = 10_000;
  it("(user 0) Withdraw 10% of USDC from Kamino reserve - happy path", async () => {
    const user = users[0];
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    const reserveAcc: Reserve = Reserve.decode(
      (await bankRunProvider.connection.getAccountInfo(usdcReserve)).data
    );
    const liquidityMint = reserveAcc.liquidity.mintPubkey;
    const reserveLiquiditySupply = reserveAcc.liquidity.supplyVault;
    const collateralMint = reserveAcc.collateral.mintPubkey;
    const collateralVault = reserveAcc.collateral.supplyVault;

    const [lendingMarketAuthority] = lendingMarketAuthPda(
      market,
      klendBankrunProgram.programId
    );
    const obligation = user.accounts.get(KAMINO_OBLIGATION);
    const [userBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
    ]);

    let tx = new Transaction();
    tx.add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await klendBankrunProgram.methods
        .withdrawObligationCollateralAndRedeemReserveCollateral(
          new BN(withdrawAmt)
        )
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          withdrawReserve: usdcReserve,
          reserveLiquidityMint: liquidityMint,
          reserveSourceCollateral: collateralVault,
          reserveCollateralMint: collateralMint,
          reserveLiquiditySupply: reserveLiquiditySupply,
          userDestinationLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [userAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
    ]);
    assert.equal(userAfter - userBefore, withdrawAmt);
    assert.equal(vaultBefore - vaultAfter, withdrawAmt);
  });
});
