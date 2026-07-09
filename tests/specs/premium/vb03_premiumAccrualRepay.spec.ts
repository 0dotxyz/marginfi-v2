import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { assert } from "chai";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  oracles,
  premiumGroup,
  PREMIUM_SEED,
  users,
  verbose,
} from "../../rootHooks";
import { USER_ACCOUNT_VB } from "../../utils/mocks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  repayIx,
} from "../../utils/user-instructions";
import {
  collectBankPremiumFees,
  editFeeStateV2Premium,
} from "../../utils/premium-instructions";
import { assertI80F48Approx, getTokenBalance } from "../../utils/genericTests";
import { deriveBankWithSeed } from "../../utils/pdas";
import {
  advanceBankrunClock,
  getBankrunBlockhash,
  processBankrunTransaction,
} from "../../utils/tools";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";

const USDC_SEED = new BN(PREMIUM_SEED);
const SOL_SEED = new BN(PREMIUM_SEED + 1);
const YEAR = 365 * 24 * 60 * 60;

let usdcBank: PublicKey;
let solBank: PublicKey;
let borrowerAccount: PublicKey;

const usdcNative = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
const solNative = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);
const NATIVE_USDC = 10 ** ecosystem.usdcDecimals;

const fetchUsdcBalance = async () => {
  const account =
    await bankrunProgram.account.marginfiAccount.fetch(borrowerAccount);
  return account.lendingAccount.balances.find(
    (b: any) => b.active && b.bankPk.equals(usdcBank),
  );
};

const collectedPremium = async (): Promise<number> => {
  const bank = await bankrunProgram.account.bank.fetch(usdcBank);
  return wrappedI80F48toBigNumber(bank.collectedPremiumOutstanding).toNumber();
};

const remainingForBorrower = () =>
  composeRemainingAccounts([
    [solBank, oracles.wsolOracle.publicKey],
    [usdcBank, oracles.usdcOracle.publicKey],
  ]);

describe("vb03: Premium accrual, repay settlement, sweep", () => {
  let user: (typeof users)[number];

  before(async () => {
    user = users[3];
    [usdcBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      premiumGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      USDC_SEED,
    );
    [solBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      premiumGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      SOL_SEED,
    );
  });

  it("(user 3) borrows 1000 USDC against SOL collateral", async () => {
    const kp = Keypair.generate();
    borrowerAccount = kp.publicKey;
    user.accounts.set(USER_ACCOUNT_VB, kp.publicKey);
    let tx = new Transaction().add(
      await accountInit(user.mrgnBankrunProgram, {
        marginfiGroup: premiumGroup.publicKey,
        marginfiAccount: kp.publicKey,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet, kp);
    await processBankrunTransaction(bankrunContext, tx, [user.wallet, kp]);

    tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: solNative(20),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: remainingForBorrower(),
        amount: usdcNative(1000),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
  });

  it("warp 1 year + pulse materializes ~10 USDC premium (1000 x 1%)", async () => {
    await advanceBankrunClock(bankrunContext, YEAR);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const tx = new Transaction().add(
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        group: premiumGroup.publicKey,
        remaining: remainingForBorrower(),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const balance = await fetchUsdcBalance();
    const outstanding = wrappedI80F48toBigNumber(
      balance.premiumOutstanding,
    ).toNumber();
    if (verbose) console.log("*materialized premium: " + outstanding / NATIVE_USDC + " USDC");
    assertI80F48Approx(balance.premiumOutstanding, 10 * NATIVE_USDC, 50);
    // Realized-only: nothing on the bank counter until tokens arrive.
    assert.approximately(await collectedPremium(), 0, 1);
  });

  it("partial repay settles premium first (debt shares unchanged)", async () => {
    const before = await fetchUsdcBalance();
    const sharesBefore = wrappedI80F48toBigNumber(before.liabilityShares);

    const tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: remainingForBorrower(),
        amount: usdcNative(4),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const after = await fetchUsdcBalance();
    // Zero-interest bank: all 4 USDC went to premium, principal (shares) is untouched.
    assertI80F48Approx(after.liabilityShares, before.liabilityShares, 0.0001);
    assert.approximately(
      wrappedI80F48toBigNumber(after.premiumOutstanding).toNumber(),
      6 * NATIVE_USDC,
      50,
    );
    // The 4 USDC premium leg is realized on the bank, pending sweep.
    assert.approximately(await collectedPremium(), 4 * NATIVE_USDC, 100);
    assert.approximately(
      sharesBefore.toNumber(),
      wrappedI80F48toBigNumber(after.liabilityShares).toNumber(),
      1,
    );
  });

  it("repay_all closes the balance and books the remaining premium", async () => {
    const tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: borrowerAccount,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [solBank, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0),
        repayAll: true,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const balance = await fetchUsdcBalance();
    assert.isUndefined(balance, "USDC liability balance should be closed");
    // Total realized premium across both repays ~= 10 USDC.
    assert.approximately(await collectedPremium(), 10 * NATIVE_USDC, 100);
  });

  it("sweep transfers realized premium to the premium wallet ATA", async () => {
    // Point the premium wallet at a wallet we control and create its canonical USDC ATA.
    const premiumWallet = Keypair.generate();
    const premiumAta = getAssociatedTokenAddressSync(
      ecosystem.usdcMint.publicKey,
      premiumWallet.publicKey,
    );
    const payer = bankrunContext.payer;
    let tx = new Transaction().add(
      await editFeeStateV2Premium(globalProgramAdmin.mrgnBankrunProgram, {
        admin: globalProgramAdmin.wallet.publicKey,
        premiumWallet: premiumWallet.publicKey,
      }),
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        premiumAta,
        premiumWallet.publicKey,
        ecosystem.usdcMint.publicKey,
      ),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(payer, globalProgramAdmin.wallet);
    await banksClient.processTransaction(tx);

    tx = new Transaction().add(
      await collectBankPremiumFees(users[0].mrgnBankrunProgram, {
        bank: usdcBank,
        premiumAta,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [users[0].wallet]);

    const swept = Number(
      await getTokenBalance(bankrunProgram.provider, premiumAta),
    );
    if (verbose) console.log("*swept to premium wallet: " + swept / NATIVE_USDC + " USDC");
    assert.approximately(swept, 10 * NATIVE_USDC, 100);
    // Whole-native-unit sweep: only sub-unit dust remains on the bank counter.
    assert.isBelow(await collectedPremium(), 1);

    // A second sweep with no whole units realized moves nothing.
    tx = new Transaction().add(
      await collectBankPremiumFees(users[0].mrgnBankrunProgram, {
        bank: usdcBank,
        premiumAta,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [users[0].wallet]);
    const sweptAfter = Number(
      await getTokenBalance(bankrunProgram.provider, premiumAta),
    );
    assert.equal(sweptAfter, swept, "second sweep should move nothing");
  });
});
