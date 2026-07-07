import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  juplendAccounts,
  oracles,
  users,
} from "./rootHooks";
import { MockUser } from "./utils/mocks";
import { accountInit } from "./utils/user-instructions";
import {
  buildHealthRemainingAccounts,
  mintToTokenAccount,
  processBankrunTransaction,
} from "./utils/tools";
import {
  assertBNEqual,
  getTokenBalance,
  parseMarginfiEvents,
} from "./utils/genericTests";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { deriveLiquidityVaultAuthority } from "./utils/pdas";
import {
  deriveJuplendPoolKeys,
  findJuplendClaimAccountPda,
} from "./utils/juplend/juplend-pdas";
import { JUPLEND_STATE_KEYS } from "./utils/juplend/test-state";
import type { JuplendPoolKeys } from "./utils/juplend/types";
import {
  juplendIntegrationProtocolMetas,
  makeIntegrationDepositIx,
  makeIntegrationWithdrawIx,
} from "./utils/integration-instructions";

describe("jlr12: Unified integration deposit/withdraw (JupLend)", () => {
  const marginfiAccountKeypair = Keypair.generate();
  const marginfiAccount = marginfiAccountKeypair.publicKey;
  let depositAmount: BN;
  let user: MockUser;
  let bank: PublicKey;
  let pool: JuplendPoolKeys;
  let claimAccount: PublicKey;
  let liquidityVault: PublicKey;

  before(async () => {
    user = users[0];
    bank = juplendAccounts.get(JUPLEND_STATE_KEYS.jlr01BankUsdc);
    depositAmount = new BN(10 * 10 ** ecosystem.usdcDecimals);

    const bankAcc = await bankrunProgram.account.bank.fetch(bank);
    liquidityVault = bankAcc.liquidityVault;
    pool = deriveJuplendPoolKeys({ mint: bankAcc.mint });
    const [liquidityVaultAuthority] = deriveLiquidityVaultAuthority(
      bankrunProgram.programId,
      bank,
    );
    [claimAccount] = findJuplendClaimAccountPda(
      liquidityVaultAuthority,
      pool.mint,
    );

    await mintToTokenAccount(
      ecosystem.usdcMint.publicKey,
      user.usdcAccount,
      depositAmount,
    );
    const initIx = await accountInit(user.mrgnBankrunProgram, {
      marginfiGroup: bankAcc.group,
      marginfiAccount,
      authority: user.wallet.publicKey,
      feePayer: user.wallet.publicKey,
    });
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [user.wallet, marginfiAccountKeypair],
    );
  });

  it("(user 0) deposits to the JupLend bank via integration_deposit - happy path", async () => {
    const [userUsdcBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);

    const tx = new Transaction().add(
      await makeIntegrationDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          signerTokenAccount: user.usdcAccount,
        },
        depositAmount,
        { jupLend: {} },
        juplendIntegrationProtocolMetas(pool, "deposit"),
      ),
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);

    const depositEvent = parseMarginfiEvents(
      bankrunProgram,
      result.logMessages,
    ).find((e) => e.name === "lendingAccountDepositEvent");
    assert.isDefined(depositEvent, "Expected lendingAccountDepositEvent");
    assertBNEqual(depositEvent!.data.amount, depositAmount);

    const [userUsdcAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    assert.equal(userUsdcBefore - userUsdcAfter, depositAmount.toNumber());
    // Funds pass through the liquidity vault into the venue within the instruction
    assert.equal(vaultAfter, vaultBefore);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount,
    );
    const balance = acc.lendingAccount.balances.find(
      (b: any) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isDefined(balance, "Expected an active balance for the bank");
    // Shares are fTokens; the exchange rate is >= 1, so 0 < shares <= amount
    const shares = wrappedI80F48toBigNumber(balance.assetShares).toNumber();
    assert.isAbove(shares, 0);
    assert.isAtMost(shares, depositAmount.toNumber());
  });

  it("(user 0) withdraws all from the JupLend bank via integration_withdraw - happy path", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    const [userUsdcBefore, vaultBefore] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    const oracleKeys = await buildHealthRemainingAccounts(marginfiAccount);

    const tx = new Transaction().add(
      await makeIntegrationWithdrawIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount,
          bank,
          destinationTokenAccount: user.usdcAccount,
        },
        { amount: new BN(0), withdrawAll: true, oracleKeys },
        { jupLend: {} },
        juplendIntegrationProtocolMetas(pool, "withdraw", claimAccount),
      ),
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      user.wallet,
    ]);

    const withdrawEvent = parseMarginfiEvents(
      bankrunProgram,
      result.logMessages,
    ).find((e) => e.name === "lendingAccountWithdrawEvent");
    assert.isDefined(withdrawEvent, "Expected lendingAccountWithdrawEvent");
    assert.isTrue(withdrawEvent!.data.closeBalance);

    const [userUsdcAfter, vaultAfter] = await Promise.all([
      getTokenBalance(bankRunProvider, user.usdcAccount),
      getTokenBalance(bankRunProvider, liquidityVault),
    ]);
    // Tolerance covers venue rounding and interest accrued over the few slots since deposit
    assert.approximately(
      userUsdcAfter - userUsdcBefore,
      depositAmount.toNumber(),
      100,
    );
    // JupLend withdrawals route through the intermediary ATA; the vault is untouched
    assert.equal(vaultAfter, vaultBefore);

    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      marginfiAccount,
    );
    const balance = acc.lendingAccount.balances.find(
      (b: any) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isUndefined(balance, "Expected the balance to be closed");
  });
});
