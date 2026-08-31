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
  armOrderInterestIx,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  placeOrderIx,
} from "../../utils/user-instructions";
import { deriveOrderPda } from "../../utils/pdas";
import { LST_ATA, USER_ACCOUNT } from "../../utils/mocks";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { getBankrunBlockhash } from "../../utils/tools";
import { I80F48_SCALE, mulI80, toI80Scaled } from "../../utils/bn-utils";

const I80F48_ONE = I80F48_SCALE;

/**
 * The leveraged staking loop the orders guide leads with: lend an LST, borrow SOL. Staked is the
 * only non-integration asset tag with a multiplier that is not 1, and `validate_asset_tags` lets it
 * pair with nothing but SOL, so this is the one shape that exercises a staked lend leg at all.
 *
 * Nothing here moves the clock. A first arming has no age gate, and the anchor it writes is the
 * whole point: it must carry the pool's NAV-per-LST, not the bank share value alone.
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

    // A fresh account rather than whatever s01-s09 left on this user, so the health check here
    // depends only on the LST position this spec opens.
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

  it("anchors the staked lend leg on the pool's NAV per LST, not the share value", async () => {
    const bankKeys = [validators[0].bank, solBank];
    const placeTx = new Transaction().add(
      await placeOrderIx(bankrunProgram, {
        marginfiAccount: userAccount,
        authority: user.wallet.publicKey,
        feePayer: user.wallet.publicKey,
        bankKeys,
        trigger: {
          stopLoss: { threshold: bigNumberToWrappedI80F48(0.01), maxSlippage },
        },
        interest: {
          windowSeconds: null,
          patienceSeconds: null,
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

    // Both legs go in writable: arming accrues them before reading their share indices.
    const remaining = composeRemainingAccounts([
      stakedLegAccounts(),
      [solBank, oracles.wsolOracle.publicKey],
    ]).map((pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: pubkey.equals(validators[0].bank) || pubkey.equals(solBank),
    }));

    const armTx = new Transaction().add(
      await armOrderInterestIx(bankrunProgram, {
        marginfiAccount: userAccount,
        group: stakedMarginfiGroup.publicKey,
        order,
        remaining,
      }),
    );
    armTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    armTx.sign(user.wallet);
    await banksClient.processTransaction(armTx);

    const fetched = await bankrunProgram.account.order.fetch(order);
    const stakedBank = await bankrunProgram.account.bank.fetch(
      validators[0].bank,
    );
    // The anchor is the bank's share value scaled by the pool's NAV per LST, which the bank cached
    // the last time an instruction priced it.
    const multiplier = toI80Scaled(stakedBank.cache.priceMultiplier);
    assert.notEqual(multiplier, I80F48_ONE, "a staked pool prices away from 1");
    assert.equal(
      toI80Scaled(fetched.interestAnchorAssetIndex),
      mulI80(toI80Scaled(stakedBank.assetShareValue), multiplier),
    );

    // The SOL borrow leg is native, so its anchor is exactly that bank's liability share value.
    const sol = await bankrunProgram.account.bank.fetch(solBank);
    assert.equal(
      toI80Scaled(fetched.interestAnchorDebtIndex),
      toI80Scaled(sol.liabilityShareValue),
    );
  });
});
