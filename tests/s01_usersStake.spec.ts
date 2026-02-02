import { BN } from "@coral-xyz/anchor";
import {
  LAMPORTS_PER_SOL,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankRunProvider,
  users,
  validators,
  verbose,
  banksClient,
} from "./rootHooks";
import {
  createStakeAccount,
  delegateStake,
  getEpochAndSlot,
  getStakeAccount,
  getStakeActivation,
} from "./utils/stake-utils";
import {
  assertBNEqual,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { u64MAX_BN } from "./utils/types";
import { getAssociatedTokenAddressSync } from "@mrgnlabs/mrgn-common";
import {
  depositToSinglePoolIxes,
  getBankrunBlockhash,
} from "./utils/spl-staking-utils";
import { assert } from "chai";
import { LST_ATA, LST_ATA_v1, STAKE_ACC, STAKE_ACC_v1 } from "./utils/mocks";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { oracles } from "./rootHooks";

describe("User stakes some native and creates an account", () => {
  /** Users's validator 0 stake account */
  let user0StakeAccount: PublicKey;
  const stake = 10;

  before(async () => {
    // Refresh oracles to ensure they're up to date
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("(user 0) Create user stake account and stake to validator", async () => {
    let { createTx, stakeAccountKeypair } = createStakeAccount(
      users[0],
      stake * LAMPORTS_PER_SOL
    );
    // Note: bankrunContext.lastBlockhash only works if non-bankrun tests didn't run previously
    createTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    createTx.sign(users[0].wallet, stakeAccountKeypair);
    await banksClient.processTransaction(createTx);
    user0StakeAccount = stakeAccountKeypair.publicKey;

    if (verbose) {
      console.log("Create stake account: " + user0StakeAccount);
      console.log(
        " Stake: " +
          stake +
          " SOL (" +
          (stake * LAMPORTS_PER_SOL).toLocaleString() +
          " in native)"
      );
    }
    users[0].accounts.set("v0_stakeAcc", user0StakeAccount);

    let delegateTx = delegateStake(
      users[0],
      user0StakeAccount,
      validators[0].voteAccount
    );
    delegateTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    delegateTx.sign(users[0].wallet);
    await banksClient.processTransaction(delegateTx);

    if (verbose) {
      console.log("user 0 delegated to " + validators[0].voteAccount);
    }

    let { epoch, slot } = await getEpochAndSlot(banksClient);
    const stakeAccountInfo = await bankRunProvider.connection.getAccountInfo(
      user0StakeAccount
    );
    const stakeAccBefore = getStakeAccount(stakeAccountInfo.data);
    const meta = stakeAccBefore.meta;
    const delegation = stakeAccBefore.stake.delegation;
    const rent = new BN(meta.rentExemptReserve.toString());

    assertKeysEqual(delegation.voterPubkey, validators[0].voteAccount);
    assertBNEqual(
      new BN(delegation.stake.toString()),
      new BN(10 * LAMPORTS_PER_SOL).sub(rent)
    );
    assertBNEqual(new BN(delegation.activationEpoch.toString()), epoch);
    assertBNEqual(new BN(delegation.deactivationEpoch.toString()), u64MAX_BN);

    const stakeStatusBefore = await getStakeActivation(
      bankRunProvider.connection,
      user0StakeAccount,
      epoch
    );
    if (verbose) {
      console.log("It is now epoch: " + epoch + " slot " + slot);
      console.log(
        "Stake active: " +
          stakeStatusBefore.active.toLocaleString() +
          " inactive " +
          stakeStatusBefore.inactive.toLocaleString() +
          " status: " +
          stakeStatusBefore.status
      );
    }
  });

  it("(user 1/2/3) Stakes and delegates too", async () => {
    await stakeAndDelegateForUser(1, stake, 0);
    await stakeAndDelegateForUser(2, stake, 0);
    await stakeAndDelegateForUser(3, stake, 0);

    // repeat for validator 1 stake
    await stakeAndDelegateForUser(1, stake, 1);
    await stakeAndDelegateForUser(2, stake, 1);
    await stakeAndDelegateForUser(3, stake, 1);
  });

  const stakeAndDelegateForUser = async (
    userIndex: number,
    stakeAmount: number,
    validatorIndex: 0 | 1
  ) => {
    const user = users[userIndex];
    const stakeAccKey = validatorIndex === 0 ? STAKE_ACC : STAKE_ACC_v1;
    const voteAccount = validators[validatorIndex].voteAccount;

    let { createTx, stakeAccountKeypair } = createStakeAccount(
      user,
      stakeAmount * LAMPORTS_PER_SOL
    );
    createTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    createTx.sign(user.wallet, stakeAccountKeypair);
    await banksClient.processTransaction(createTx);
    user.accounts.set(stakeAccKey, stakeAccountKeypair.publicKey);

    let delegateTx = delegateStake(
      user,
      stakeAccountKeypair.publicKey,
      voteAccount
    );
    delegateTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    delegateTx.sign(user.wallet);
    await banksClient.processTransaction(delegateTx);
  };

  it("Advance the epoch", async () => {
    let { epoch: epochBefore, slot: _slotBefore } = await getEpochAndSlot(
      banksClient
    );
    bankrunContext.warpToEpoch(BigInt(epochBefore + 1));

    let { epoch: epochAfterWarp, slot: slotAfterWarp } = await getEpochAndSlot(
      banksClient
    );
    if (verbose) {
      console.log(
        "Warped to epoch: " + epochAfterWarp + " slot " + slotAfterWarp
      );
    }

    const stakeStatusAfter = await getStakeActivation(
      bankRunProvider.connection,
      user0StakeAccount,
      epochAfterWarp
    );
    if (verbose) {
      console.log(
        "Stake active: " +
          stakeStatusAfter.active.toLocaleString() +
          " inactive " +
          stakeStatusAfter.inactive.toLocaleString() +
          " status: " +
          stakeStatusAfter.status
      );
      console.log("");
    }

    // Advance a few slots to end the rewards period

    // NOTE: ALL STAKE PROGRAM IXES ARE DISABLED DURING THE REWARDS PERIOD. THIS MUST OCCUR OR THE
    // STAKE PROGRAM CANNOT RUN

    if (verbose) {
      console.log("Now stalling for a few slots to end the rewards period...");
    }
    for (let i = 0; i < 3; i++) {
      bankrunContext.warpToSlot(BigInt(i + slotAfterWarp + 1));
    }

    let { epoch, slot } = await getEpochAndSlot(banksClient);
    if (verbose) {
      console.log("It is now epoch: " + epoch + " slot " + slot);
    }
  });

  it("(user 0) Deposits " + stake + "stake to the v0 LST pool", async () => {
    const userStakeAccount = users[0].accounts.get(STAKE_ACC);
    // Note: use `findPoolMintAddress(SINGLE_POOL_PROGRAM_ID, splPool);` if mint is not known.
    const lstAta = getAssociatedTokenAddressSync(
      validators[0].splMint,
      users[0].wallet.publicKey
    );
    users[0].accounts.set(LST_ATA, lstAta);

    // Note: user stake account exists before, but is closed after
    // Here we note the balance of the stake account prior
    const stakeAccountInfo = await bankRunProvider.connection.getAccountInfo(
      userStakeAccount
    );
    const stakeAccBefore = getStakeAccount(stakeAccountInfo.data);
    const rent = new BN(stakeAccBefore.meta.rentExemptReserve.toString());
    const delegationBefore = Number(
      stakeAccBefore.stake.delegation.stake.toString()
    );
    assertBNEqual(
      new BN(delegationBefore),
      new BN(10 * LAMPORTS_PER_SOL).sub(rent)
    );

    // The spl stake pool account is already infused with 1 SOL at init
    const splStakeInfoBefore = await bankRunProvider.connection.getAccountInfo(
      validators[0].splSolPool
    );
    const splStakePoolBefore = getStakeAccount(splStakeInfoBefore.data);
    const delegationSplPoolBefore = new BN(
      splStakePoolBefore.stake.delegation.stake.toString()
    );
    if (verbose) {
      console.log("pool stake before: " + delegationSplPoolBefore.toString());
    }

    // Create lst ata, transfer authority, execute the deposit
    let tx = new Transaction();
    const ixes = await depositToSinglePoolIxes(
      bankRunProvider.connection,
      users[0].wallet.publicKey,
      validators[0].splPool,
      userStakeAccount,
      verbose
    );
    tx.add(...ixes);
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(users[0].wallet);
    await banksClient.processTransaction(tx);

    // The stake account no longer exists
    try {
      const accountInfo = await bankRunProvider.connection.getAccountInfo(
        userStakeAccount
      );
      assert.ok(
        accountInfo === null,
        "The account should not exist, but it does."
      );
    } catch (err) {
      assert.ok(true, "The account does not exist.");
    }

    const [lstAfter, splStakePoolInfo] = await Promise.all([
      getTokenBalance(bankRunProvider, lstAta),
      bankRunProvider.connection.getAccountInfo(validators[0].splSolPool),
    ]);
    if (verbose) {
      console.log("lst after: " + lstAfter.toLocaleString());
    }
    // LST tokens are issued 1:1 with stake because there has been zero appreciation
    // Also note that LST tokens use the same decimals.
    assert.equal(lstAfter, delegationBefore);

    const splStakePool = getStakeAccount(splStakePoolInfo.data);
    const delegationSplPoolAfter = new BN(
      splStakePool.stake.delegation.stake.toString()
    );
    if (verbose) {
      console.log("pool stake after: " + delegationSplPoolAfter.toString());
    }
    // The stake pool gained all of the stake that was held in the user stake acc
    assertBNEqual(
      delegationSplPoolAfter.sub(delegationSplPoolBefore),
      delegationBefore
    );
  });

  it(
    "(user 1/2/3) deposits " + stake + " to the v0 stake pool too",
    async () => {
      await depositForUser(1, 0);
      await depositForUser(2, 0);
      await depositForUser(3, 0);
      await depositForUser(1, 1);
      await depositForUser(2, 1);
      await depositForUser(3, 1);
    }
  );

  const depositForUser = async (userIndex: number, validatorIndex: 0 | 1) => {
    const user = users[userIndex];

    // Choose the correct keys and validator based on validatorIndex
    const stakeAccKey = validatorIndex === 0 ? STAKE_ACC : STAKE_ACC_v1;
    const lstAtaKey = validatorIndex === 0 ? LST_ATA : LST_ATA_v1;
    const { splPool, splMint } = validators[validatorIndex];

    const tx = new Transaction();
    const ixes = await depositToSinglePoolIxes(
      bankRunProvider.connection,
      user.wallet.publicKey,
      splPool,
      user.accounts.get(stakeAccKey),
      verbose
    );
    tx.add(...ixes);
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const lstAta = getAssociatedTokenAddressSync(
      splMint,
      user.wallet.publicKey
    );
    user.accounts.set(lstAtaKey, lstAta);
  };
});
