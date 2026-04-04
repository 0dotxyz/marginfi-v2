import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  EMODE_INIT_RATE_LST_TO_LST,
  EMODE_INIT_RATE_SOL_TO_LST,
  EMODE_SEED,
  emodeGroup,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertI80F48Approx,
} from "./utils/genericTests";
import { CONF_INTERVAL_MULTIPLE, ORACLE_CONF_INTERVAL } from "./utils/types";
import { deriveBankWithSeed } from "./utils/pdas";
import {
  bigNumberToWrappedI80F48,
  wrappedI80F48toBigNumber,
} from "@mrgnlabs/mrgn-common";
import { USER_ACCOUNT_E } from "./utils/mocks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
} from "./utils/user-instructions";
import { groupConfigure } from "./utils/group-instructions";
import { bytesToF64, getBankrunBlockhash } from "./utils/tools";

// Banks are listed here in the sorted-by-public-keys order - the same used in the lending account balances
const seed = new BN(EMODE_SEED);
let usdcBank: PublicKey;
let lstBBank: PublicKey;
let stableBank: PublicKey;
let lstABank: PublicKey;
let solBank: PublicKey;

describe("Emode borrowing", () => {
  const confidence = ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;

  before(async () => {
    [usdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed
    );
    [lstBBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed.addn(1)
    );
    [lstABank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed
    );
    [solBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      seed
    );
    [stableBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      emodeGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      seed.addn(1)
    );

    // These suites cover classic eMode math; disable same-asset leverage explicitly so the
    // tests are not affected
    const tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        sameAssetEmodeInitLeverage: bigNumberToWrappedI80F48(1),
        sameAssetEmodeMaintLeverage: bigNumberToWrappedI80F48(1),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);
  });

  it("Initialize user accounts (if needed)", async () => {
    for (let i = 0; i < users.length; i++) {
      const userAccKeypair = Keypair.generate();
      const userAccount = userAccKeypair.publicKey;
      if (users[i].accounts.get(USER_ACCOUNT_E)) {
        if (verbose) {
          console.log("Skipped creating user " + i);
        }
        continue;
      } else {
        if (verbose) {
          console.log("user [" + i + "]: " + userAccount);
        }
        users[i].accounts.set(USER_ACCOUNT_E, userAccount);
      }

      let userinitTx: Transaction = new Transaction();
      userinitTx.add(
        await accountInit(users[i].mrgnBankrunProgram, {
          marginfiGroup: emodeGroup.publicKey,
          marginfiAccount: userAccount,
          authority: users[i].wallet.publicKey,
          feePayer: users[i].wallet.publicKey,
        })
      );
      userinitTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      userinitTx.sign(users[i].wallet, userAccKeypair);
      await banksClient.processTransaction(userinitTx);
    }

    if (groupAdmin.accounts.get(USER_ACCOUNT_E)) {
      console.log("Skipped creating admin account");
      return;
    }
    const userAccKeypair = Keypair.generate();
    const userAccount = userAccKeypair.publicKey;
    groupAdmin.accounts.set(USER_ACCOUNT_E, userAccount);

    let userinitTx: Transaction = new Transaction();
    userinitTx.add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: emodeGroup.publicKey,
        marginfiAccount: userAccount,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      })
    );
    userinitTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    userinitTx.sign(groupAdmin.wallet, userAccKeypair);
    await banksClient.processTransaction(userinitTx);
  });

  it("(admin) Seeds liquidity in the banks", async () => {
    const user = groupAdmin;
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        amount: new BN(100 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: stableBank,
        tokenAccount: user.usdcAccount,
        amount: new BN(100 * 10 ** ecosystem.usdcDecimals),
        depositUpToLimit: false,
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: new BN(100 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstABank,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(100 * 10 ** ecosystem.lstAlphaDecimals),
        depositUpToLimit: false,
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstBBank,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(100 * 10 ** ecosystem.lstAlphaDecimals),
        depositUpToLimit: false,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });

  /*
   * SOL is worth $150, and LST is worth $175. Against a 10 SOL position, worth $1500, with a
   * `EMODE_INIT_RATE_SOL_TO_LST` of 90% we expect to borrow .9 * 1500 / 175 ~= 7.71428571429 LST.
   *
   * Due to confidence adjustments the actual price of the LST liability is ~104% of $175 so:
   * - 0.9 * 1500 / (175 * 1.0424) ~= 7.3264992874 LST
   *
   * With the bank's default rate sans-emode (50%), we could only borrow ~4.28571428571 LST
   *
   * Note: the liability weight was assumed to be 1 in the calculations above, emode never modifies
   * liability weights.
   *
   * Note: To derive the conf bands above, the confidence of Pyth Pull is 1%,
   * and we apply a 2.12 constant multiplier, see health pulse printout for actual internal pricing.
   */
  it("(user 0) borrows LST A against SOL at a favorable rate - happy path", async () => {
    const user = users[0];
    const solDeposit = 10;
    const lstBorrow = 7.3;
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: new BN(solDeposit * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstABank,
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [solBank, oracles.wsolOracle.publicKey],
          [lstABank, oracles.pythPullLst.publicKey],
        ]),
        amount: new BN(lstBorrow * 10 ** ecosystem.lstAlphaDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    let userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = userAcc.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cacheAfter.assetValue);
    const liabValue = wrappedI80F48toBigNumber(cacheAfter.liabilityValue);
    if (verbose) {
      console.log("---user health state---");
      console.log("asset value: " + assetValue.toString());
      console.log("liab value: " + liabValue.toString());
      console.log("prices: ");
      for (let i = 0; i < cacheAfter.prices.length; i++) {
        const price = bytesToF64(cacheAfter.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
    }

    const assetsExpected =
      oracles.wsolPrice *
      (1.0 - confidence) *
      solDeposit *
      EMODE_INIT_RATE_SOL_TO_LST;
    assertI80F48Approx(cacheAfter.assetValue, assetsExpected);

    const liabsExpected =
      oracles.lstAlphaPrice *
      lstBorrow *
      (1 + ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE) *
      1; // Note: Liability weight 1 for banks in this test
    assertI80F48Approx(
      cacheAfter.liabilityValue,
      liabsExpected,
      liabsExpected * 0.0001
    );
  });

  // This illustrates a possible emode footgun: the user tries to borrow USDC, which would cause
  // them to lose the emode benefit from their LST borrow. Even this trivial borrow amount fails
  // because breaking the emode benefit would put this user significantly in bad health.
  it("(user 0) tries to borrow a trivial amount of USDC - fails, emode error", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [solBank, oracles.wsolOracle.publicKey],
          [lstABank, oracles.pythPullLst.publicKey],
          [usdcBank, oracles.usdcOracle.publicKey],
        ]),
        amount: new BN(0.000001 * 10 ** ecosystem.usdcDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // 6009 (RiskEngineInitRejected)
    assertBankrunTxFailed(result, "0x1779");
  });

  /*
   * Both LST are worth $175. With an LST to LST emode rate of 80%, we expect:
   * .8 * 1750 / 175 = 8 LST B borrowed
   *
   * And with the confidence adjustment:
   * - (0.8 * 1750 * (1 - 0.01 * 2.12)) / (175 * 1.0424) ~= 7.34919416731 LST
   */
  const lstADeposit = 10;
  const lstBBorrow = 7.3;
  it("(user 1) borrows LST B against LST A at a favorable rate - happy path", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstABank,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(lstADeposit * 10 ** ecosystem.lstAlphaDecimals),
        depositUpToLimit: false,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: lstBBank,
        tokenAccount: user.lstAlphaAccount,
        remaining: composeRemainingAccounts([
          [lstABank, oracles.pythPullLst.publicKey],
          [lstBBank, oracles.pythPullLst.publicKey],
        ]),
        amount: new BN(lstBBorrow * 10 ** ecosystem.lstAlphaDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    let userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = userAcc.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cacheAfter.assetValue);
    const liabValue = wrappedI80F48toBigNumber(cacheAfter.liabilityValue);
    if (verbose) {
      console.log("---user health state---");
      console.log("asset value: " + assetValue.toString());
      console.log("liab value: " + liabValue.toString());
      console.log("prices: ");
      for (let i = 0; i < cacheAfter.prices.length; i++) {
        const price = bytesToF64(cacheAfter.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
    }

    // (0.8 * 1750 * (1 - 0.01 * 2.12))
    const assetsExpected =
      oracles.lstAlphaPrice *
      lstADeposit *
      EMODE_INIT_RATE_LST_TO_LST *
      (1 - ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE);
    assertI80F48Approx(cacheAfter.assetValue, assetsExpected);

    const liabsExpected =
      oracles.lstAlphaPrice *
      lstBBorrow *
      (1 + ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE) *
      1; // Note: Liability weight 1 for banks in this test
    assertI80F48Approx(
      cacheAfter.liabilityValue,
      liabsExpected,
      liabsExpected * 0.0001
    );
  });

  // Here the user adds a SOL borrow, which is fine, because SOL has a better emode pairing with LST
  // A than LST B does. The lesser emode value (from the LST B borrow) is used for collateral
  // purposes, but the position doesn't blow up like with the earlier attempted USDC borrow, its
  // value doesn't change other than the trivial increase from the SOL borrow.
  it("(user 1) tries to also borrow a trivial amount of SOL - succeeds, no change", async () => {
    const user = users[1];
    const wsolBorrow = 0.000001;
    const userAccount = user.accounts.get(USER_ACCOUNT_E);

    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [lstABank, oracles.pythPullLst.publicKey],
          [lstBBank, oracles.pythPullLst.publicKey],
          [solBank, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.000001 * 10 ** ecosystem.wsolDecimals),
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    // Note: essentially no change
    let userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount
    );
    const cacheAfter = userAcc.healthCache;
    const assetValue = wrappedI80F48toBigNumber(cacheAfter.assetValue);
    const liabValue = wrappedI80F48toBigNumber(cacheAfter.liabilityValue);
    if (verbose) {
      console.log("---user health state---");
      console.log("asset value: " + assetValue.toString());
      console.log("liab value: " + liabValue.toString());
      console.log("prices: ");
      for (let i = 0; i < cacheAfter.prices.length; i++) {
        const price = bytesToF64(cacheAfter.prices[i]);
        if (price != 0) {
          console.log(" [" + i + "] " + price);
        }
      }
    }

    // (0.8 * 1750 * (1 - 0.01 * 2.12))
    const assetsExpected =
      oracles.lstAlphaPrice *
      lstADeposit *
      EMODE_INIT_RATE_LST_TO_LST *
      (1 - ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE);
    assertI80F48Approx(cacheAfter.assetValue, assetsExpected);

    const liabsExpected =
      oracles.lstAlphaPrice *
        lstBBorrow *
        (1 + ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE) *
        1 +
      wsolBorrow * oracles.wsolPrice; // Close enough for wsol value, the amount is trivial
    assertI80F48Approx(
      cacheAfter.liabilityValue,
      liabsExpected,
      liabsExpected * 0.001
    );
  });

  // TODO test against isolated bank (not yet supported in program)

  // TODO moar tests....
});
