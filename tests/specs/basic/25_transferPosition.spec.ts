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
  borrowIx,
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
  let collateralBank: PublicKey;
  let debtBank: PublicKey;
  let feeWallet: PublicKey;

  before(async () => {
    const setup = await genericMultiBankTestSetup(
      2,
      accountName,
      groupSeed,
      9_000,
    );
    group = setup.throwawayGroup.publicKey;
    [collateralBank, debtBank] = setup.banks;
    const groupAcc = await bankrunProgram.account.marginfiGroup.fetch(group);
    feeWallet = groupAcc.feeStateCache.globalFeeWallet;
  });

  const account = (user: MockUser) => user.accounts.get(accountName);

  const lamports = async (key: PublicKey) =>
    BigInt((await banksClient.getAccount(key))!.lamports);

  const send = async (tx: Transaction, ...signers: MockUser[]) => {
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(...signers.map((u) => u.wallet));
    return banksClient.tryProcessTransaction(tx);
  };

  /** `[bank, oracle]` per active balance (plus `extraBank`), in the on-chain descending order */
  const observation = async (accountPk: PublicKey, extraBank?: PublicKey) => {
    const acc = await bankrunProgram.account.marginfiAccount.fetch(accountPk);
    const banks = acc.lendingAccount.balances
      .filter((b) => b.active === 1)
      .map((b) => b.bankPk);
    if (extraBank && !banks.some((b) => b.equals(extraBank))) {
      banks.push(extraBank);
    }
    const groups = await Promise.all(
      banks.map(async (b) => {
        const collateralBankcc = await bankrunProgram.account.bank.fetch(b);
        return [b, collateralBankcc.config.oracleKeys[0]];
      }),
    );
    return composeRemainingAccounts(groups);
  };

  const deposit = async (user: MockUser, bank: PublicKey, amount: BN) => {
    const tx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: account(user),
        bank,
        tokenAccount: user.lstAlphaAccount,
        amount,
        depositUpToLimit: false,
      }),
    );
    const result = await send(tx, user);
    assert.isNull(result.result);
  };

  const borrow = async (user: MockUser, bank: PublicKey, amount: BN) => {
    const tx = new Transaction().add(
      await borrowIx(user.mrgnBankrunProgram, {
        marginfiAccount: account(user),
        bank,
        tokenAccount: user.lstAlphaAccount,
        remaining: await observation(account(user), bank),
        amount,
      }),
    );
    const result = await send(tx, user);
    assert.isNull(result.result);
  };

  /** The source authority signs and pays the fee; `consenting` signs only to accept debt. */
  const transfer = async (
    source: MockUser,
    destination: MockUser,
    bank: PublicKey,
    amount: BN,
    consenting?: MockUser,
  ) => {
    const tx = new Transaction().add(
      await lendingAccountTransferPositionIx(bankrunProgram, {
        group,
        sourceMarginfiAccount: account(source),
        destinationMarginfiAccount: account(destination),
        authority: source.wallet.publicKey,
        destinationAuthority: consenting ? consenting.wallet.publicKey : null,
        feePayer: source.wallet.publicKey,
        bank,
        globalFeeWallet: feeWallet,
        transferAmount: amount,
        sourceRemaining: await observation(account(source)),
        destinationRemaining: await observation(account(destination), bank),
      }),
    );
    return consenting ? send(tx, source, consenting) : send(tx, source);
  };

  const balance = async (user: MockUser, bank: PublicKey) => {
    const acc = await bankrunProgram.account.marginfiAccount.fetch(account(user));
    const found = acc.lendingAccount.balances.find(
      (b) => b.active === 1 && b.bankPk.equals(bank),
    );
    assert.isDefined(found);
    return found;
  };

  const setPositionTransferFlags = async (
    user: MockUser,
    disableReceive: boolean,
  ) => {
    const tx = new Transaction().add(
      await user.mrgnBankrunProgram.methods
        .marginfiAccountSetPositionTransferFlags(disableReceive)
        .accounts({
          marginfiAccount: account(user),
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
          marginfiAccount: account(user),
          admin: groupAdmin.wallet.publicKey,
        })
        .instruction(),
    );
    const result = await send(tx, groupAdmin);
    assert.isNull(result.result);
  };

  it("(user 0 -> user 1) moves half a position on the sender's signature; the sender pays the default fee", async () => {
    const depositAmount = new BN(100).mul(lst);
    const transferAmount = depositAmount.divn(2);
    await deposit(users[0], collateralBank, depositAmount);

    const feeWalletBefore = await lamports(feeWallet);
    const receiverBefore = await lamports(users[1].wallet.publicKey);
    const result = await transfer(users[0], users[1], collateralBank, transferAmount);
    assert.isNull(result.result);

    // No borrows exist in this bank, so one share is exactly one native unit.
    assert.equal(
      toI80Scaled((await balance(users[0], collateralBank)).assetShares),
      nativeToI80Scaled(depositAmount.sub(transferAmount)),
    );
    assert.equal(
      toI80Scaled((await balance(users[1], collateralBank)).assetShares),
      nativeToI80Scaled(transferAmount),
    );
    assert.equal(
      await lamports(feeWallet),
      feeWalletBefore + DEFAULT_POSITION_TRANSFER_FEE,
    );
    assert.equal(await lamports(users[1].wallet.publicKey), receiverBefore);
  });

  it("(user 0 -> user 0) rejects a transfer to the same account", async () => {
    const result = await transfer(users[0], users[0], collateralBank, lst);
    // PositionTransferIdenticalAccounts
    assertBankrunTxFailed(result, 6903);
  });

  it("(user 0 -> user 2) rejects a frozen destination", async () => {
    await setFreeze(users[2], true);
    const result = await transfer(users[0], users[2], collateralBank, lst);
    // AccountFrozen
    assertBankrunTxFailed(result, 6103);
    await setFreeze(users[2], false);
  });

  it("(user 0 -> user 1) rejects a destination that disabled receiving", async () => {
    await setPositionTransferFlags(users[1], true);
    const result = await transfer(users[0], users[1], collateralBank, lst);
    // PositionTransferReceiveDisabled
    assertBankrunTxFailed(result, 6900);
    await setPositionTransferFlags(users[1], false);
  });

  it("(user 0 -> user 1) rejects a transfer below the minimum value", async () => {
    const result = await transfer(users[0], users[1], collateralBank, new BN(1_000));
    // InvalidPositionTransferAmount
    assertBankrunTxFailed(result, 6901);
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
    const sourceBefore = (await balance(users[0], collateralBank)).assetShares;
    const destinationBefore = (await balance(users[1], collateralBank)).assetShares;
    const feeWalletBefore = await lamports(feeWallet);
    const result = await transfer(users[0], users[1], collateralBank, transferAmount);
    assert.isNull(result.result);

    assert.equal(
      toI80Scaled((await balance(users[0], collateralBank)).assetShares),
      toI80Scaled(sourceBefore) - nativeToI80Scaled(transferAmount),
    );
    assert.equal(
      toI80Scaled((await balance(users[1], collateralBank)).assetShares),
      toI80Scaled(destinationBefore) + nativeToI80Scaled(transferAmount),
    );
    assert.equal(
      await lamports(feeWallet),
      feeWalletBefore + BigInt(FEE_LAMPORTS),
    );

    // Back to the program default for the rest of the suite.
    await setFee(0);
  });

  it("(user 0 -> user 2) moves debt only with the receiver's signature", async () => {
    await deposit(users[1], debtBank, new BN(100).mul(lst));
    const borrowAmount = new BN(10).mul(lst);
    await borrow(users[0], debtBank, borrowAmount);
    await deposit(users[2], collateralBank, new BN(100).mul(lst));

    const debtAmount = new BN(5).mul(lst);
    const refused = await transfer(users[0], users[2], debtBank, debtAmount);
    // PositionTransferDebtConsentRequired
    assertBankrunTxFailed(refused, 6904);

    const result = await transfer(users[0], users[2], debtBank, debtAmount, users[2]);
    assert.isNull(result.result);

    // The clock has not moved since the borrow, so one liability share is one native unit.
    assert.equal(
      toI80Scaled((await balance(users[0], debtBank)).liabilityShares),
      nativeToI80Scaled(borrowAmount.sub(debtAmount)),
    );
    assert.equal(
      toI80Scaled((await balance(users[2], debtBank)).liabilityShares),
      nativeToI80Scaled(debtAmount),
    );
  });
});
