import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  oracles,
  premiumGroup,
  PREMIUM_SEED,
  users,
  verbose,
} from "./rootHooks";
import { USER_ACCOUNT_VB } from "./utils/mocks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
} from "./utils/user-instructions";
import { deriveBankWithSeed } from "./utils/pdas";
import { getBankrunBlockhash, processBankrunTransaction } from "./utils/tools";
import { u32ToPremiumRate } from "./utils/premium-instructions";

const USDC_SEED = new BN(PREMIUM_SEED);
const SOL_SEED = new BN(PREMIUM_SEED + 1);
const SOL_UNTAGGED_SEED = new BN(PREMIUM_SEED + 2);

let usdcBank: PublicKey;
let solBank: PublicKey;
let solUntaggedBank: PublicKey;

/** Snapshot APR (as a fraction) of the account's USDC liability balance. */
const usdcSnapshotFraction = async (marginfiAccount: PublicKey): Promise<number> => {
  const account =
    await bankrunProgram.account.marginfiAccount.fetch(marginfiAccount);
  const balance = account.lendingAccount.balances.find(
    (b: any) => b.active && b.bankPk.equals(usdcBank),
  );
  assert.ok(balance, "expected an active USDC liability balance");
  return u32ToPremiumRate(balance.premiumRateSnapshot);
};

/** Init a fresh marginfi account for a user on the premium group. */
const initAccount = async (userIndex: number): Promise<PublicKey> => {
  const user = users[userIndex];
  const kp = Keypair.generate();
  user.accounts.set(USER_ACCOUNT_VB, kp.publicKey);
  const tx = new Transaction().add(
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
  return kp.publicKey;
};

describe("vb02: Premium snapshot on borrow", () => {
  const solNative = (n: number) => new BN(n * 10 ** ecosystem.wsolDecimals);
  const usdcNative = (n: number) => new BN(n * 10 ** ecosystem.usdcDecimals);
  const borrowAmount = usdcNative(100);

  before(async () => {
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
    [solUntaggedBank] = deriveBankWithSeed(
      bankrunProgram.programId,
      premiumGroup.publicKey,
      ecosystem.wsolMint.publicKey,
      SOL_UNTAGGED_SEED,
    );
  });

  it("(admin) seeds USDC liquidity for borrowers", async () => {
    const kp = Keypair.generate();
    groupAdmin.accounts.set(USER_ACCOUNT_VB, kp.publicKey);
    let tx = new Transaction().add(
      await accountInit(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: premiumGroup.publicKey,
        marginfiAccount: kp.publicKey,
        authority: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, kp);
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet, kp]);

    tx = new Transaction().add(
      await depositIx(groupAdmin.mrgnBankrunProgram, {
        marginfiAccount: kp.publicKey,
        bank: usdcBank,
        tokenAccount: groupAdmin.usdcAccount,
        amount: usdcNative(500_000),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(user 0) single tagged collateral -> snapshot == pair rate (1%)", async () => {
    const user = users[0];
    const account = await initAccount(0);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: solNative(10),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [solBank, oracles.wsolOracle.publicKey],
          [usdcBank, oracles.usdcOracle.publicKey],
        ]),
        amount: borrowAmount,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const rate = await usdcSnapshotFraction(account);
    if (verbose) console.log("*single-collateral snapshot: " + rate * 100 + "%");
    assert.approximately(rate, 0.01, 0.00005);
  });

  it("(user 1) mixed collateral (tagged + untagged, equal USD) -> weighted 0.5%", async () => {
    const user = users[1];
    const account = await initAccount(1);

    // Equal token amounts of the SAME mint/oracle => equal collateral USD => 50/50 weighting.
    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        amount: solNative(10),
      }),
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solUntaggedBank,
        tokenAccount: user.wsolAccount,
        amount: solNative(10),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [solBank, oracles.wsolOracle.publicKey],
          [solUntaggedBank, oracles.wsolOracle.publicKey],
          [usdcBank, oracles.usdcOracle.publicKey],
        ]),
        amount: borrowAmount,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const rate = await usdcSnapshotFraction(account);
    if (verbose) console.log("*mixed-collateral snapshot: " + rate * 100 + "%");
    // (0.5 * 1%) + (0.5 * 0%) = 0.5%
    assert.approximately(rate, 0.005, 0.00005);
  });

  it("(user 2) untagged collateral only (missing pair) -> snapshot 0", async () => {
    const user = users[2];
    const account = await initAccount(2);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: solUntaggedBank,
        tokenAccount: user.wsolAccount,
        amount: solNative(10),
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account,
        bank: usdcBank,
        tokenAccount: user.usdcAccount,
        remaining: composeRemainingAccounts([
          [solUntaggedBank, oracles.wsolOracle.publicKey],
          [usdcBank, oracles.usdcOracle.publicKey],
        ]),
        amount: borrowAmount,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [user.wallet]);

    const rate = await usdcSnapshotFraction(account);
    if (verbose) console.log("*untagged-collateral snapshot: " + rate * 100 + "%");
    assert.equal(rate, 0);
  });
});
