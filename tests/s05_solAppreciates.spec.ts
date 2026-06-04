import { BN, Wallet } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  stakedBankKeypairSol,
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  oracles,
  users,
  validators,
  bankRunProvider,
  verbose,
  groupAdmin,
} from "./rootHooks";
import { assertBankrunTxFailed } from "./utils/genericTests";
import { assert } from "chai";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  pulseBankPrice,
} from "./utils/user-instructions";
import { LST_ATA, LST_ATA_v1, USER_ACCOUNT } from "./utils/mocks";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { getBankrunBlockhash } from "./utils/tools";
import { getEpochAndSlot } from "./utils/bankrunConnection";
import { getStakeAccount } from "./utils/stake-utils";
import { replenishPool } from "./utils/spl-staking-utils";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

let bankKeypairSol: Keypair;

describe("Borrow power grows as v0 Staked SOL gains value from appreciation", () => {
  before(() => {
    bankKeypairSol = stakedBankKeypairSol;
  });

  // User 2 has a validator 0 staked deposit [0] position with 1 LST token.
  // Users 0/1/2/3 deposited 10 SOL each, so v0 LST supply is 40 SOL.
  // The v0 pool NAV is 50 SOL: 40 SOL user deposits + 1 SOL initial pool stake
  // + 9 SOL supplied through the v0 onramp. We no longer subtract the initial
  // 1 SOL bootstrap stake, so the price is 50 / 40 = 1.25 SOL per LST.

  /** SOL to add to the validator as pretend-earned mev rewards */
  const stakeSolAppreciation = 30;
  const splPoolAppreciation = 40; // different to simplify the calculations
  let wallet: Wallet;

  before(async () => {
    // Refresh oracles to ensure they're up to date
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
    wallet = bankRunProvider.wallet;
  });

  it("(user 2) tries to borrow 1.3 SOL against 1 v0 STAKED - fails, not enough funds", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(1.3 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);

    // 6009 (Generic risk engine rejection)
    assertBankrunTxFailed(result, "0x1779");

    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 0);
  });

  // Note: there is also some natural appreciation here because a few epochs have elapsed...
  it(
    "v0 stake sol pool grows by " +
      stakeSolAppreciation +
      " SOL (e.g. MEV rewards) - LST price grows",
    async () => {
      let tx = new Transaction();
      tx.add(
        SystemProgram.transfer({
          fromPubkey: wallet.publicKey,
          toPubkey: validators[0].splSolPool,
          lamports: stakeSolAppreciation * LAMPORTS_PER_SOL,
        }),
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(wallet.payer);
      await banksClient.processTransaction(tx);

      const priceMultiplierAfterAppreciation = await fetchPriceMultiplier();
      assert.equal(priceMultiplierAfterAppreciation, 2.0); // (50 + 30) / 40 = 2
    },
  );

  it("(user 2 - attacker) ties to sneak in bad lst mint - should fail", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[1].splMint, // Bad mint
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.1 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);

    // 6009 (Generic risk engine rejection)
    assertBankrunTxFailed(result, "0x1779");
  });

  it("(user 2 - attacker) ties to sneak in bad sol pool - should fail", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[1].splSolPool,
            validators[0].splOnRampPool,
          ], // Bad pool
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.2 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);

    // 6009 (Generic risk engine rejection)
    assertBankrunTxFailed(result, "0x1779");
  });

  // Now the stake is worth enough now (1 LST = 2 SOL) and the user can borrow
  it("(user 2) borrows 1.3 SOL against their STAKED position - succeeds", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(1.3 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.tryProcessTransaction(tx);

    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const borrowIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(bankKeypairSol.publicKey),
    );
    assert.notEqual(borrowIndex, -1);
  });

  // Note: MEV rewards or other kickbacks to stakers often get admin-deposited directly to the
  // splPool just like this.
  it(
    "v0 pool grows by " +
      splPoolAppreciation +
      " SOL (MEV rewards) - LST price doesn't change",
    async () => {
      let tx = new Transaction();
      tx.add(
        SystemProgram.transfer({
          fromPubkey: wallet.publicKey,
          toPubkey: validators[0].splPool,
          lamports: splPoolAppreciation * LAMPORTS_PER_SOL,
        }),
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(wallet.payer);
      await banksClient.processTransaction(tx);

      const priceMultiplierAfterAppreciation = await fetchPriceMultiplier();
      assert.equal(priceMultiplierAfterAppreciation, 2.0); // still the same
    },
  );

  it("Realize income from MEV rewards - price grows again", async () => {
    const onRampPoolKey = validators[0].splOnRampPool;
    const onRampAccBefore = await bankRunProvider.connection.getAccountInfo(
      onRampPoolKey,
    );
    const onRampBefore = getStakeAccount(onRampAccBefore.data);
    const stakeBefore = onRampBefore.stake.delegation.stake.toString();
    if (verbose) {
      console.log("On ramp lamps: " + onRampAccBefore.lamports);
      console.log("On ramp stake: " + stakeBefore);
    }

    let { epoch: epochBeforeWarp, slot: _slotBeforeWarp } =
      await getEpochAndSlot(banksClient);
    bankrunContext.warpToEpoch(BigInt(epochBeforeWarp + 1));
    let { epoch: _epochAfterWarp, slot: slotAfterWarp } = await getEpochAndSlot(
      banksClient,
    );

    // Note: YES YOU NEED THIS BLOCK. Once the epoch ends there is a period before the "rewards"
    // period ends and some txes need to land before staking stuff works again.
    for (let i = 0; i < 3; i++) {
      bankrunContext.warpToSlot(BigInt(i + slotAfterWarp + 1));
      const dummyTx = new Transaction();
      dummyTx.add(
        SystemProgram.transfer({
          fromPubkey: users[0].wallet.publicKey,
          toPubkey: bankrunProgram.provider.publicKey,
          lamports: i,
        }),
      );
      dummyTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      dummyTx.sign(users[0].wallet);
      await banksClient.processTransaction(dummyTx);
    }

    let { epoch, slot } = await getEpochAndSlot(banksClient);
    if (verbose) {
      console.log("It is now epoch: " + epoch + " slot " + slot);
    }

    // Next, the replenish crank cycles free SOL into the "on ramp" pool
    let replenishTx = new Transaction().add(
      replenishPool(validators[0].voteAccount),
    );
    replenishTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    replenishTx.sign(wallet.payer); // pays the tx fee and rent
    await banksClient.processTransaction(replenishTx);

    const onRampAccAfter = await bankRunProvider.connection.getAccountInfo(
      onRampPoolKey,
    );
    const onRampAfter = getStakeAccount(onRampAccAfter.data);
    const stakeAfter = onRampAfter.stake.delegation.stake.toString();
    if (verbose) {
      console.log("On ramp lamps: " + onRampAccAfter.lamports);
      console.log("On ramp stake: " + stakeAfter);
    }

    const priceMultiplierAfterAppreciation = await fetchPriceMultiplier();
    assert.equal(priceMultiplierAfterAppreciation, 3.0); // (80 + 40) / 40 = 3
  });

  // Now the stake is worth enough and the user can borrow
  it("(user 2) borrows 1.6 SOL against their STAKED position - succeeds", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(1.6 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.tryProcessTransaction(tx);

    const userAcc = await bankrunProgram.account.marginfiAccount.fetch(
      userAccount,
    );
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const borrowIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(bankKeypairSol.publicKey),
    );
    assert.notEqual(borrowIndex, -1);
  });

  it("(user 2) deposits to another staked bank - should succeed", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    const userLstAta = user.accounts.get(LST_ATA_v1);

    let tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: validators[1].bank,
        tokenAccount: userLstAta,
        // some nominal amount...
        amount: new BN(0.000001 * 10 ** ecosystem.wsolDecimals),
        depositUpToLimit: false,
      }),
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });

  it("(user 2) borrows with two active positions - happy path", async () => {
    const user = users[2];
    const userAccount = user.accounts.get(USER_ACCOUNT);
    let tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: bankKeypairSol.publicKey,
        tokenAccount: user.wsolAccount,
        remaining: composeRemainingAccounts([
          [
            validators[0].bank,
            oracles.wsolOracle.publicKey,
            validators[0].splMint,
            validators[0].splSolPool,
            validators[0].splOnRampPool,
          ],
          [
            validators[1].bank,
            oracles.wsolOracle.publicKey,
            validators[1].splMint,
            validators[1].splSolPool,
            validators[1].splOnRampPool,
          ],
          [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
        ]),
        amount: new BN(0.00001 * 10 ** ecosystem.wsolDecimals),
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);
  });
});

const fetchPriceMultiplier = async () => {
  const pulseTx = new Transaction().add(
    await pulseBankPrice(groupAdmin.mrgnBankrunProgram, {
      bank: validators[0].bank,
      remaining: [
        oracles.wsolOracle.publicKey,
        validators[0].splMint,
        validators[0].splSolPool,
        validators[0].splOnRampPool,
      ],
    }),
  );
  pulseTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
  pulseTx.sign(groupAdmin.wallet);

  await banksClient.processTransaction(pulseTx);

  const bank = await bankrunProgram.account.bank.fetch(validators[0].bank);
  const priceWithOnRamp = wrappedI80F48toBigNumber(
    bank.cache.lastOraclePrice,
  ).toNumber();
  assert.equal(priceWithOnRamp, oracles.wsolPrice);

  return wrappedI80F48toBigNumber(bank.cache.priceMultiplier).toNumber();
};
