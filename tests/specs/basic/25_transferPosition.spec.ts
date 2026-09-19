import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  users,
} from "../../rootHooks";
import {
  composeRemainingAccounts,
  depositIx,
  lendingAccountTransferPositionIx,
} from "../../utils/user-instructions";
import { editGlobalFeeState } from "../../utils/group-instructions";
import { getBankrunBlockhash } from "../../utils/tools";
import { genericMultiBankTestSetup } from "../../genericSetups";
import { assertBankrunTxFailed } from "../../utils/genericTests";
import { nativeToI80Scaled, toI80Scaled } from "../../utils/bn-utils";
import { MockUser } from "../../utils/mocks";

/** Program default, charged while `FeeState.position_transfer_fee` is 0 */
const DEFAULT_POSITION_TRANSFER_FEE = 500_000n;

describe("Position transfer", () => {
  const accountName = "transfer_position_acc";
  const groupSeed = Buffer.alloc(32);
  groupSeed.write("TRANSFER_POSITION_GRP", 0);
  const lst = new BN(10).pow(new BN(ecosystem.lstAlphaDecimals));

  let group: PublicKey;
  let bank: PublicKey;
  let feeWallet: PublicKey;

  before(async () => {
    const setup = await genericMultiBankTestSetup(
      1,
      accountName,
      groupSeed,
      9_000,
    );
    group = setup.throwawayGroup.publicKey;
    bank = setup.banks[0];
    const groupAcc = await bankrunProgram.account.marginfiGroup.fetch(group);
    feeWallet = groupAcc.feeStateCache.globalFeeWallet;
  });

  const lamports = async (key: PublicKey) =>
    BigInt((await banksClient.getAccount(key))!.lamports);

  const send = async (tx: Transaction, ...signers: MockUser[]) => {
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(...signers.map((u) => u.wallet));
    return banksClient.tryProcessTransaction(tx);
  };

  const deposit = async (user: MockUser, amount: BN) => {
    const tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: user.accounts.get(accountName),
        bank,
        tokenAccount: user.lstAlphaAccount,
        amount,
        depositUpToLimit: false,
      }),
    );
    const result = await send(tx, user);
    assert.isNull(result.result);
  };

  /** `[bank, oracle]` per active balance (plus `extraBank`), in the on-chain descending order */
  const observation = async (account: PublicKey, extraBank?: PublicKey) => {
    const acc = await bankrunProgram.account.marginfiAccount.fetch(account);
    const banks = acc.lendingAccount.balances
      .filter((b) => b.active === 1)
      .map((b) => b.bankPk);
    if (extraBank && !banks.some((b) => b.equals(extraBank))) {
      banks.push(extraBank);
    }
    const groups = await Promise.all(
      banks.map(async (b) => {
        const bankAcc = await bankrunProgram.account.bank.fetch(b);
        return [b, bankAcc.config.oracleKeys[0]];
      }),
    );
    return composeRemainingAccounts(groups);
  };

  const transfer = async (
    source: MockUser,
    destination: MockUser,
    amount: BN,
    destinationAuthority: MockUser = destination,
  ) => {
    const sourceAccount = source.accounts.get(accountName);
    const destinationAccount = destination.accounts.get(accountName);
    const tx = new Transaction().add(
      await lendingAccountTransferPositionIx(bankrunProgram, {
        group,
        sourceMarginfiAccount: sourceAccount,
        destinationMarginfiAccount: destinationAccount,
        authority: source.wallet.publicKey,
        destinationAuthority: destinationAuthority.wallet.publicKey,
        bank,
        globalFeeWallet: feeWallet,
        transferAmount: amount,
        sourceRemaining: await observation(sourceAccount),
        destinationRemaining: await observation(destinationAccount, bank),
      }),
    );
    return send(tx, source, destinationAuthority);
  };

  const shares = async (user: MockUser) => {
    const acc = await bankrunProgram.account.marginfiAccount.fetch(
      user.accounts.get(accountName),
    );
    const balance = acc.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isDefined(balance);
    return balance.assetShares;
  };

  const setPositionTransferFlags = async (
    user: MockUser,
    disableSend: boolean | null,
    disableReceive: boolean | null,
  ) => {
    const tx = new Transaction().add(
      await user.mrgnBankrunProgram.methods
        .marginfiAccountSetPositionTransferFlags(disableSend, disableReceive)
        .accounts({
          marginfiAccount: user.accounts.get(accountName),
          authority: user.wallet.publicKey,
        })
        .instruction(),
    );
    const result = await send(tx, user);
    assert.isNull(result.result);
  };

  const setFreeze = async (user: MockUser, frozen: boolean) => {
    const tx = new Transaction().add(
      await groupAdmin.mrgnBankrunProgram.methods
        .marginfiAccountSetFreeze(frozen)
        .accounts({
          group,
          marginfiAccount: user.accounts.get(accountName),
          admin: groupAdmin.wallet.publicKey,
        })
        .instruction(),
    );
    const result = await send(tx, groupAdmin);
    assert.isNull(result.result);
  };

  it("(user 0 -> user 1) moves half a position; the receiver pays the default fee", async () => {
    const depositAmount = new BN(100).mul(lst);
    const transferAmount = depositAmount.divn(2);
    await deposit(users[0], depositAmount);

    const feeWalletBefore = await lamports(feeWallet);
    const receiverBefore = await lamports(users[1].wallet.publicKey);
    const result = await transfer(users[0], users[1], transferAmount);
    assert.isNull(result.result);

    // No borrows exist in this bank, so one share is exactly one native unit.
    assert.equal(
      toI80Scaled(await shares(users[0])),
      nativeToI80Scaled(depositAmount.sub(transferAmount)),
    );
    assert.equal(toI80Scaled(await shares(users[1])), nativeToI80Scaled(transferAmount));
    assert.equal(
      await lamports(feeWallet),
      feeWalletBefore + DEFAULT_POSITION_TRANSFER_FEE,
    );
    assert.equal(
      await lamports(users[1].wallet.publicKey),
      receiverBefore - DEFAULT_POSITION_TRANSFER_FEE,
    );
  });

  it("(user 0 -> user 0) rejects a transfer to the same account", async () => {
    const result = await transfer(users[0], users[0], lst);
    // PositionTransferIdenticalAccounts
    assertBankrunTxFailed(result, 6904);
  });

  it("(user 0 -> user 1) rejects a destination authority that does not own the destination", async () => {
    const result = await transfer(users[0], users[1], lst, users[0]);
    // Unauthorized
    assertBankrunTxFailed(result, 6042);
  });

  it("(user 0 -> user 2) rejects a frozen destination", async () => {
    await setFreeze(users[2], true);
    const result = await transfer(users[0], users[2], lst);
    // AccountFrozen
    assertBankrunTxFailed(result, 6103);
    await setFreeze(users[2], false);
  });

  it("(user 0 -> user 1) rejects a source that disabled sending", async () => {
    await setPositionTransferFlags(users[0], true, null);
    const result = await transfer(users[0], users[1], lst);
    // PositionTransferSendDisabled
    assertBankrunTxFailed(result, 6901);
    await setPositionTransferFlags(users[0], false, null);
  });

  it("(user 0 -> user 1) rejects a destination that disabled receiving", async () => {
    await setPositionTransferFlags(users[1], null, true);
    const result = await transfer(users[0], users[1], lst);
    // PositionTransferDisabled
    assertBankrunTxFailed(result, 6900);
    await setPositionTransferFlags(users[1], null, false);
  });

  it("(user 0 -> user 1) rejects a transfer below the minimum value", async () => {
    const result = await transfer(users[0], users[1], new BN(1_000));
    // InvalidPositionTransferAmount
    assertBankrunTxFailed(result, 6902);
  });

  it("(fee admin) configures a position-transfer fee, which is then charged", async () => {
    const FEE_LAMPORTS = 7_000_000;
    const setFee = async (fee: number) => {
      const tx = new Transaction().add(
        await editGlobalFeeState(globalProgramAdmin.mrgnBankrunProgram, {
          admin: globalProgramAdmin.wallet.publicKey,
          positionTransferFee: fee,
        }),
      );
      const result = await send(tx, globalProgramAdmin);
      assert.isNull(result.result);
    };
    await setFee(FEE_LAMPORTS);

    const transferAmount = new BN(10).mul(lst);
    const sourceBefore = await shares(users[0]);
    const destinationBefore = await shares(users[1]);
    const feeWalletBefore = await lamports(feeWallet);
    const result = await transfer(users[0], users[1], transferAmount);
    assert.isNull(result.result);

    assert.equal(
      toI80Scaled(await shares(users[0])),
      toI80Scaled(sourceBefore) - nativeToI80Scaled(transferAmount),
    );
    assert.equal(
      toI80Scaled(await shares(users[1])),
      toI80Scaled(destinationBefore) + nativeToI80Scaled(transferAmount),
    );
    assert.equal(
      await lamports(feeWallet),
      feeWalletBefore + BigInt(FEE_LAMPORTS),
    );

    // Back to the program default for the rest of the suite.
    await setFee(0);
  });
});
