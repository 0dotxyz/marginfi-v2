import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { bigNumberToWrappedI80F48 } from "@mrgnlabs/mrgn-common";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  oracles,
  stakedBankKeypairSol,
  stakedMarginfiGroup,
  users,
  validators,
} from "../../rootHooks";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  placeInterestOrderIx,
  pulseBankPrice,
} from "../../utils/user-instructions";
import { deriveOrderPda } from "../../utils/pdas";
import { LST_ATA, USER_ACCOUNT } from "../../utils/mocks";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { advanceBankrunClock, getBankrunBlockhash } from "../../utils/tools";
import {
  bnToBigIntSafe,
  I80F48_SCALE,
  mulI80,
  toI80Scaled,
} from "../../utils/bn-utils";

const I80F48_ONE = I80F48_SCALE;
/** Matches `BANK_RATE_READING_SPACING_SECONDS`. */
const READING_SPACING = 10_800;

/**
 * Staked is the only non-integration asset tag whose multiplier is not 1, and `validate_asset_tags`
 * pairs it with nothing but SOL, so an LST lend against a SOL borrow is the one shape that
 * exercises a staked lend leg. Placing the order needs no history; the one clock step here is the
 * reading spacing the last test has to clear.
 */
describe("Interest trigger on a staked lend leg", () => {
  const solBank = stakedBankKeypairSol.publicKey;
  const U32_MAX = 0xffff_ffff;
  const maxSlippage = Math.floor((100 / 10_000) * U32_MAX);

  /** The staked leg needs its LST mint, SOL pool and on-ramp alongside the oracle. */
  const stakedLegAccounts = () => [
    validators[0].bank,
    oracles.wsolOracle.publicKey, // the staked bank prices off the wsol oracle too
    validators[0].splMint,
    validators[0].splSolPool,
    validators[0].splOnRampPool,
  ];

  // rootHooks populates `users` in its own hook, so these cannot be read at module load.
  let user: (typeof users)[number];
  let userAccount: PublicKey;

  before(async () => {
    user = users[3];
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // A fresh account, so the health check here depends only on the LST position this spec opens
    // and not on what s01-s09 left on this user.
    const kp = Keypair.generate();
    userAccount = kp.publicKey;
    const initTx = new Transaction().add(
      await accountInit(bankrunProgram, {
        marginfiGroup: stakedMarginfiGroup.publicKey,
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
      }),
    );
    initTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    initTx.sign(user.wallet, kp);
    await banksClient.processTransaction(initTx);
  });

  it("(user 3) re-opens an LST lend against a SOL borrow", async () => {
    const depositTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[0].bank,
        tokenAccount: user.accounts.get(LST_ATA),
        amount: new BN(1 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    depositTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    depositTx.sign(user.wallet);
    await banksClient.processTransaction(depositTx);

    const borrowTx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: solBank,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          stakedLegAccounts(),
          [solBank, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.05 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    borrowTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    borrowTx.sign(user.wallet);
    await banksClient.processTransaction(borrowTx);
  });

  it("places an interest order on the staked pair with no accounts beyond a plain order", async () => {
    const bankKeys = [validators[0].bank, solBank];
    const placeTx = new Transaction().add(
      await placeInterestOrderIx(bankrunProgram, {
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
        bankKeys,
        trigger: {
          stopLoss: { threshold: bigNumberToWrappedI80F48(1), maxSlippage: 0 },
        },
        interest: {
          windowSeconds: null,
          exitBudgetSeconds: null,
          minNegativeApr: null,
        },
      }),
    );
    placeTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    placeTx.sign(user.wallet);
    await banksClient.processTransaction(placeTx);

    const [order] = deriveOrderPda(
      bankrunProgram.programId,
      userAccount,
      bankKeys,
    );
    const fetched = await bankrunProgram.account.order.fetch(order);
    assert.equal(fetched.interestFlags, 1);
  });

  it("reads the staked lend leg at the pool's NAV per LST, not the share value", async () => {
    // Earlier specs priced this bank at the current clock, and a bank records at most one reading
    // per spacing, so step past it and republish the oracles at the new time.
    await advanceBankrunClock(bankrunContext, READING_SPACING);
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Pricing the bank takes its reading; the staked leg prices off its pool accounts too.
    const pulseTx = new Transaction().add(
      await pulseBankPrice(bankrunProgram, {
        bank: validators[0].bank,
        remaining: stakedLegAccounts().slice(1),
      }),
    );
    pulseTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    pulseTx.sign(user.wallet);
    await banksClient.processTransaction(pulseTx);

    const stakedBank = await bankrunProgram.account.bank.fetch(
      validators[0].bank,
    );
    const multiplier = toI80Scaled(stakedBank.cache.priceMultiplier);
    assert.notEqual(multiplier, I80F48_ONE, "a staked pool prices away from 1");
    const newest = stakedBank.rateReadings.reduce((a: any, b: any) =>
      b.timestamp.gt(a.timestamp) ? b : a,
    );
    // A reading keeps an index's I80F48 bits above the lowest 16.
    const encoded = (scaled: bigint) => scaled >> 16n;
    assert.equal(
      bnToBigIntSafe(newest.assetIndex),
      encoded(mulI80(toI80Scaled(stakedBank.assetShareValue), multiplier)),
    );
    assert.equal(
      bnToBigIntSafe(newest.debtIndex),
      encoded(mulI80(toI80Scaled(stakedBank.liabilityShareValue), multiplier)),
    );
  });
});
