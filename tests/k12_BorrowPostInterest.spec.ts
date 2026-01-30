import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  ecosystem,
  groupAdmin,
  kaminoAccounts,
  MARKET,
  oracles,
  users,
  verbose,
  bankrunContext,
  bankrunProgram,
  klendBankrunProgram,
  THROWAWAY_GROUP_SEED_K10,
  USDC_RESERVE,
  TOKEN_A_RESERVE,
} from "./rootHooks";
import {
  borrowIx,
  healthPulse,
  composeRemainingAccounts,
} from "./utils/user-instructions";
import {
  deriveBankWithSeed,
  deriveBaseObligation,
  deriveLiquidityVaultAuthority,
} from "./utils/pdas";
import {
  logHealthCache,
  processBankrunTransaction as processBankrunTx,
} from "./utils/tools";
import { assert } from "chai";
import {
  assertBankrunTxFailed,
  assertI80F48Approx,
} from "./utils/genericTests";
import {
  defaultKaminoBankConfig,
  simpleRefreshObligation,
  simpleRefreshReserve,
} from "./utils/kamino-utils";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import {
  makeAddKaminoBankIx,
  makeInitObligationIx,
  makeKaminoDepositIx,
} from "./utils/kamino-instructions";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";
import { BalanceRaw } from "@mrgnlabs/marginfi-client-v2";

describe("k12: Borrow Tests (Recycles mrgn banks from k10)", () => {
  const startingSeed = 6;
  const throwawayGroup = Keypair.fromSeed(THROWAWAY_GROUP_SEED_K10);
  const USER_ACCOUNT_THROWAWAY = "throwaway_account_k";
  let banks: PublicKey[] = [];
  let usdcReserve: PublicKey;
  let tokenAReserve: PublicKey;
  let kaminoUsdcBank: PublicKey;
  let kaminoUsdcObligation: PublicKey;
  let kaminoTokenABank: PublicKey;
  let kaminoTokenAObligation: PublicKey;
  let mrgnID: PublicKey;

  before(async () => {
    // Re-derive the seeded banks using the known starting seed
    const numBanks = 2; // same as in k10 setup
    for (let i = 0; i < numBanks; i++) {
      const [bankPk] = deriveBankWithSeed(
        bankrunProgram.programId,
        throwawayGroup.publicKey,
        ecosystem.lstAlphaMint.publicKey,
        new BN(startingSeed).addn(i)
      );
      banks.push(bankPk);
    }
    mrgnID = bankrunProgram.programId;
    [kaminoUsdcBank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      new BN(startingSeed).addn(1)
    );
    usdcReserve = kaminoAccounts.get(USDC_RESERVE);
    tokenAReserve = kaminoAccounts.get(TOKEN_A_RESERVE);
    const market = kaminoAccounts.get(MARKET);
    [kaminoUsdcObligation] = deriveBaseObligation(
      deriveLiquidityVaultAuthority(
        bankrunProgram.programId,
        kaminoUsdcBank
      )[0],
      market
    );
  });

  it("(admin) init kamino token A bank", async () => {
    const market = kaminoAccounts.get(MARKET);
    const seed = new BN(startingSeed).addn(1);

    let config = defaultKaminoBankConfig(
      oracles.tokenAOracle.publicKey
    );
    config.assetWeightInit = bigNumberToWrappedI80F48(0.8);
    config.assetWeightMaint = bigNumberToWrappedI80F48(0.9);

    [kaminoTokenABank] = deriveBankWithSeed(
      mrgnID,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );

    let tx = new Transaction().add(
      await makeAddKaminoBankIx(
        groupAdmin.mrgnBankrunProgram,
        {
          group: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          integrationAcc1: tokenAReserve,
          kaminoMarket: market,
          oracle: oracles.tokenAOracle.publicKey,
        },
        {
          config: config,
          seed: seed,
        }
      )
    );
    await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);

    [kaminoTokenAObligation] = deriveBaseObligation(
      deriveLiquidityVaultAuthority(
        bankrunProgram.programId,
        kaminoTokenABank
      )[0],
      market
    );

    tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 2_000_000 }),
      await makeInitObligationIx(groupAdmin.mrgnBankrunProgram, {
        feePayer: groupAdmin.wallet.publicKey,
        bank: kaminoTokenABank,
        signerTokenAccount: groupAdmin.tokenAAccount,
        lendingMarket: market,
        reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        pythOracle: oracles.tokenAOracle.publicKey,
      })
    );
    await processBankrunTx(bankrunContext, tx, [groupAdmin.wallet]);
  });

  /*
    Kamino sets reserve "stale" flag after a deposit, which typically requires another refresh to
    unset, this means you can't deposit twice in one tx (unless you refresh before/after the deposit
    ix) - a limitation that isn't found in other marginfi assets. 

    Two different deposits (into different kamino banks) in the same tx is fine, but in practice you
    would need a LUT to be able to pack a tx large enough to contain two deposits and the needed
    refresh instructions.

    Note: we DO IGNORE the kamino stale flag for staleness checks in the margin risk engine, which
    allows us to treat a reserve as valid even if a deposit occured in the same tx, e.g. in a
    deposit/borrow tx (provided it wasn't actually stale due to oracle issues)
   */
  it("(user 2) Attempts to deposit twice in one tx - blocked by Kamino", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmountUsdc = new BN(200 * 10 ** ecosystem.usdcDecimals);
    const market = kaminoAccounts.get(MARKET);

    let result = await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          market,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          kaminoUsdcObligation,
          [usdcReserve]
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoUsdcBank,
            signerTokenAccount: user.usdcAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          depositAmountUsdc
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoUsdcBank,
            signerTokenAccount: user.usdcAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          depositAmountUsdc
        )
      ),
      [user.wallet],
      true
    );
    // ReserveStale
    assertBankrunTxFailed(result, 6009);
  });

  it("(user 2) Deposits into USDC bank", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmountUsdc = new BN(200 * 10 ** ecosystem.usdcDecimals);
    const market = kaminoAccounts.get(MARKET);

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          market,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          kaminoUsdcObligation,
          [usdcReserve]
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoUsdcBank,
            signerTokenAccount: user.usdcAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.usdcMint.publicKey,
          },
          depositAmountUsdc
        ),
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          remaining: composeRemainingAccounts([
            [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          ]),
        })
      ),
      [user.wallet]
    );

    const acc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const cache = acc.healthCache;
    const depVal =
      (depositAmountUsdc.toNumber() / 10 ** ecosystem.usdcDecimals) *
      oracles.usdcPrice;
    const depValWithConf =
      depVal - depVal * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
    if (verbose) {
      console.log("expected value (w/ confidence): " + depValWithConf);
      console.log(
        "actual value:               " +
          wrappedI80F48toBigNumber(cache.assetValueEquity).toString()
      );
    }
    // Note: interest has accumulated, so we expect this position should be worth a few % more.
    const t = depValWithConf * 0.01;

    assertI80F48Approx(cache.assetValueEquity, depValWithConf, t);
    // Note: Default asset weights for Kamino banks is also 1
    assertI80F48Approx(cache.assetValue, depValWithConf, t);
    assertI80F48Approx(cache.assetValueMaint, depValWithConf, t);
    // TODO repeat the above for the token A test below
  });

  it("(user 2) Deposit without refreshing - fails for staleness", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmountTokenA = new BN(5 * 10 ** ecosystem.tokenADecimals);
    const market = kaminoAccounts.get(MARKET);

    let result = await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoTokenABank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          },
          depositAmountTokenA
        )
      ),
      [user.wallet],
      true
    );
    // ReserveStale
    assertBankrunTxFailed(result, 6009);

    result = await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          tokenAReserve,
          market,
          oracles.tokenAOracle.publicKey
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoTokenABank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          },
          depositAmountTokenA
        )
      ),
      [user.wallet],
      true
    );
    // ObligationStale.
    assertBankrunTxFailed(result, 6017);
  });

  /*
    It's notable that Kamino requires a refresh with each deposit, unlike marginfi where no risk
    engine check (and therefore no refresh equivalent) occurs on deposits.
  */
  it("(user 2) Deposits into token A bank - happy path", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const depositAmountTokenA = new BN(5 * 10 ** ecosystem.tokenADecimals);
    const market = kaminoAccounts.get(MARKET);

    await processBankrunTx(
      bankrunContext,
      new Transaction().add(
        await simpleRefreshReserve(
          klendBankrunProgram,
          tokenAReserve,
          market,
          oracles.tokenAOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          kaminoTokenAObligation,
          [tokenAReserve]
        ),
        // Note: it doesn't matter for deposit that the USDC obligation is stale here, only the
        // deposit bank's obligation matters. It also doesn't matter that the USDC reserve is stale:
        // only the token A reserve needs to be up-to-date. However, if USDC was stale here, then
        // health pulse could show it with a value of zero.
        await simpleRefreshReserve(
          klendBankrunProgram,
          usdcReserve,
          market,
          oracles.usdcOracle.publicKey
        ),
        await simpleRefreshObligation(
          klendBankrunProgram,
          market,
          kaminoUsdcObligation,
          [usdcReserve]
        ),
        await makeKaminoDepositIx(
          user.mrgnBankrunProgram,
          {
            marginfiAccount: userAccount,
            bank: kaminoTokenABank,
            signerTokenAccount: user.tokenAAccount,
            lendingMarket: market,
            reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
          },
          depositAmountTokenA
        ),
        // Pulse so the next test has an up-to-date cache for the "before" state.
        await healthPulse(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          remaining: composeRemainingAccounts([
            [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
            [kaminoTokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
          ]),
        })
      ),
      [user.wallet]
    );
  });

  it("(user 2) Borrows from bank[0] - kamino is valued as expected", async () => {
    const user = users[2];
    const bank = banks[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);
    const borrowAmount = new BN(0.5 * 10 ** ecosystem.lstAlphaDecimals);

    const [accBefore, _bankBefore] = await Promise.all([
      bankrunProgram.account.marginfiAccount.fetch(userAccount),
      bankrunProgram.account.bank.fetch(bank),
    ]);
    const balBefore = accBefore.lendingAccount.balances.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    const owedBefore = balBefore
      ? wrappedI80F48toBigNumber(balBefore.liabilityShares).toNumber()
      : 0;
    logHealthCache("user cache before: ", accBefore.healthCache);

    let tx = new Transaction().add(
      // TODO show that this fails when reserves are stale
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank,
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [kaminoTokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
          [bank, oracles.pythPullLst.publicKey],
        ]),
        amount: borrowAmount,
      }),
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [kaminoTokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
          [bank, oracles.pythPullLst.publicKey],
        ]),
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);

    const [accAfter, bankAfter] = await Promise.all([
      bankrunProgram.account.marginfiAccount.fetch(userAccount),
      bankrunProgram.account.bank.fetch(bank),
    ]);
    logHealthCache("user cache after: ", accAfter.healthCache);
    const balAfter = accAfter.lendingAccount.balances.find(
      (b: BalanceRaw) => b.bankPk.equals(bank) && b.active === 1
    );
    const owedAfter = wrappedI80F48toBigNumber(
      balAfter.liabilityShares
    ).toNumber();
    const originationFee = wrappedI80F48toBigNumber(
      bankAfter.config.interestRateConfig.protocolOriginationFee
    ).toNumber();
    const actual = owedAfter - owedBefore;
    const expected =
      borrowAmount.toNumber() + borrowAmount.toNumber() * originationFee;
    assert.equal(actual, expected);

    // TODO assert health cache info, etc
  });

  /*
    As with regular margin banks, a deposit and borrow in the same tx is possible.
   */
  it("(user 2) Composed deposit-borrow example - happy path", async () => {
    const user = users[2];
    const bank = banks[0];
    const market = kaminoAccounts.get(MARKET);
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    let tx = new Transaction().add(
      await simpleRefreshReserve(
        klendBankrunProgram,
        tokenAReserve,
        market,
        oracles.tokenAOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        kaminoTokenAObligation,
        [tokenAReserve]
      ),
      await simpleRefreshReserve(
        klendBankrunProgram,
        usdcReserve,
        market,
        oracles.usdcOracle.publicKey
      ),
      await simpleRefreshObligation(
        klendBankrunProgram,
        market,
        kaminoUsdcObligation,
        [usdcReserve]
      ),
      await makeKaminoDepositIx(
        user.mrgnBankrunProgram,
        {
          marginfiAccount: userAccount,
          bank: kaminoTokenABank,
          signerTokenAccount: user.tokenAAccount,
          lendingMarket: market,
          reserveLiquidityMint: ecosystem.tokenAMint.publicKey,
        },
        new BN(0.0001 * 10 ** ecosystem.tokenADecimals)
      ),
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank,
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [kaminoUsdcBank, oracles.usdcOracle.publicKey, usdcReserve],
          [kaminoTokenABank, oracles.tokenAOracle.publicKey, tokenAReserve],
          [bank, oracles.pythPullLst.publicKey],
        ]),
        // some nominal amount
        amount: new BN(0.00001 * 10 ** ecosystem.lstAlphaDecimals),
      })
    );
    await processBankrunTx(bankrunContext, tx, [user.wallet]);
  });
});
