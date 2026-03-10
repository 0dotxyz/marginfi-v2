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
  ecosystem,
} from "./rootHooks";
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
import { assertBankrunTxFailed, getTokenBalance } from "./utils/genericTests";
import {
  deriveLendingMarketAuthority,
  deriveReserveCollateralMint,
  deriveReserveCollateralSupply,
  deriveReserveLiquiditySupply,
} from "./utils/pdas";
import { KLEND_PROGRAM_ID } from "./utils/types";

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

    const [lendingMarketAuthority] = deriveLendingMarketAuthority(
      KLEND_PROGRAM_ID,
      market,
    );
    const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveCollateralMint] = deriveReserveCollateralMint(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveDestinationDepositCollateral] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
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
        oracles.usdcOracle.publicKey,
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
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          reserveLiquiditySupply,
          reserveCollateralMint,
          reserveDestinationDepositCollateral,
          userSourceLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );

    await processBankrunTransaction(ctx, tx, [user.wallet]);

    const [userAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, reserveLiquiditySupply),
    ]);
    assert.equal(userBefore - userAfter, depositAmt);
    assert.equal(vaultAfter - vaultBefore, depositAmt);
  });

  it("(user 0) Deposit 0 USDC to Kamino reserve - should fail", async () => {
    const user = users[0];
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    const [lendingMarketAuthority] = deriveLendingMarketAuthority(
      KLEND_PROGRAM_ID,
      market,
    );
    const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveCollateralMint] = deriveReserveCollateralMint(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveDestinationDepositCollateral] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );

    const obligation = user.accounts.get(KAMINO_OBLIGATION);

    const tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await klendBankrunProgram.methods
        .depositReserveLiquidityAndObligationCollateral(new BN(0))
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          reserve: usdcReserve,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          reserveLiquiditySupply,
          reserveCollateralMint,
          reserveDestinationDepositCollateral,
          userSourceLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
    );

    const result = await processBankrunTransaction(ctx, tx, [user.wallet], true);
    // Kamino Error Code: InvalidAmount. Error Number: 6003.
    assertBankrunTxFailed(result, 6003);
  });

  const withdrawAmt: number = 10_000;
  it("(user 0) Withdraw 10% of USDC from Kamino reserve - happy path", async () => {
    const user = users[0];
    const market = kaminoAccounts.get(MARKET);
    const usdcReserve = kaminoAccounts.get(USDC_RESERVE);

    const [lendingMarketAuthority] = deriveLendingMarketAuthority(
      KLEND_PROGRAM_ID,
      market,
    );
    const [reserveLiquiditySupply] = deriveReserveLiquiditySupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveCollateralMint] = deriveReserveCollateralMint(
      KLEND_PROGRAM_ID,
      usdcReserve,
    );
    const [reserveSourceCollateral] = deriveReserveCollateralSupply(
      KLEND_PROGRAM_ID,
      usdcReserve,
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
        oracles.usdcOracle.publicKey,
      ),
      await simpleRefreshObligation(klendBankrunProgram, market, obligation, [
        usdcReserve,
      ]),
      await klendBankrunProgram.methods
        .withdrawObligationCollateralAndRedeemReserveCollateral(
          new BN(withdrawAmt),
        )
        .accounts({
          owner: user.wallet.publicKey,
          obligation: obligation,
          lendingMarket: market,
          lendingMarketAuthority: lendingMarketAuthority,
          withdrawReserve: usdcReserve,
          reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          reserveSourceCollateral,
          reserveCollateralMint,
          reserveLiquiditySupply: reserveLiquiditySupply,
          userDestinationLiquidity: user.usdcAccount,
          placeholderUserDestinationCollateral: null,
          collateralTokenProgram: TOKEN_PROGRAM_ID,
          liquidityTokenProgram: TOKEN_PROGRAM_ID,
          instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
        })
        .instruction(),
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
