import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { Transaction, AccountMeta, PublicKey } from "@solana/web3.js";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  groupAdmin,
  users,
  oracles,
} from "./rootHooks";
import {
  depositIx,
  composeRemainingAccounts,
  lendingAccountTransferPositionIx,
} from "./utils/user-instructions";
import { getBankrunBlockhash } from "./utils/tools";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { genericMultiBankTestSetup } from "./genericSetups";
import { ASSET_TAG_KAMINO, ASSET_TAG_DRIFT, ASSET_TAG_SOLEND, ASSET_TAG_JUPLEND, ASSET_TAG_STAKED } from "./utils/types";

describe("Position Transfer Tests", () => {
  let groupSeedBase = "TRANSFER_TEST_GRP_";
  let startingSeed = 9_000;
  let setupTest: { banks: any[]; throwawayGroup: any };
  let accountName = "transfer_test_acc";

  const createGroupBuffer = (suffix: number) => {
    let buf = Buffer.alloc(32);
    let str = groupSeedBase + suffix.toString().padEnd(14, "0");
    buf.write(str, 0);
    return buf;
  };

  const getTransferRemainingAccounts = async (bankPk: PublicKey): Promise<AccountMeta[]> => {
    const bank = await bankrunProgram.account.bank.fetch(bankPk);
    const assetTag = bank.config.assetTag;
    
    const group: PublicKey[] = [bankPk, bank.config.oracleKeys[0]];

    if (
      assetTag === ASSET_TAG_KAMINO ||
      assetTag === ASSET_TAG_DRIFT ||
      assetTag === ASSET_TAG_SOLEND ||
      assetTag === ASSET_TAG_JUPLEND
    ) {
      group.push(bank.config.oracleKeys[1]);
    }

    if (assetTag === ASSET_TAG_STAKED) {
      group.push(
        bank.config.oracleKeys[1],
        bank.config.oracleKeys[2],
        bank.config.oracleKeys[3],
      );
    }

    const composed = composeRemainingAccounts([group]);
    return composed.map((pubkey) => ({
      pubkey,
      isSigner: false,
      isWritable: false,
    }));
  };

  const setupTransfer = async (
    sourceUser: any,
    destUser: any,
    depositAmount: BN = new BN(100_000_000_000),
    transferAmount: BN = new BN(50_000_000_000),
  ) => {
    const program = bankrunProgram;
    const bankPk = setupTest.banks[0];
    const userAccountPk = sourceUser.accounts.get(accountName)!;
    const user2AccountPk = destUser.accounts.get(accountName)!;

    const depositIxData = await depositIx(sourceUser.mrgnBankrunProgram, {
      marginfiAccount: userAccountPk,
      bank: bankPk,
      tokenAccount: sourceUser.lstAlphaAccount,
      amount: depositAmount,
      depositUpToLimit: false,
    });

    const depositTx = new Transaction().add(depositIxData);
    depositTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    depositTx.sign(sourceUser.wallet);
    await banksClient.processTransaction(depositTx);

    const remainingAccounts = await getTransferRemainingAccounts(bankPk);
    const marginfiGroupAccount = await program.account.marginfiGroup.fetch(setupTest.throwawayGroup.publicKey);
    const feeWallet = marginfiGroupAccount.feeStateCache.globalFeeWallet;

    return {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    };
  };

  before(async () => {
    setupTest = await genericMultiBankTestSetup(
      1,
      accountName,
      createGroupBuffer(0),
      startingSeed
    );
  });

  it("Should transfer position between accounts successfully", async () => {
    const testUser = users[0];
    const testUser2 = users[1];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    await banksClient.processTransaction(transferTx);

    const userAccountAfter = await program.account.marginfiAccount.fetch(
      userAccountPk
    );
    const user2AccountAfter = await program.account.marginfiAccount.fetch(
      user2AccountPk
    );

    assert.isTrue(
      userAccountAfter.lendingAccount.balances.some(
        (b: any) =>
          b.bankPk.equals(bankPk) &&
          wrappedI80F48toBigNumber(b.assetShares).gt(0)
      ),
      "Source should still have balance after transfer"
    );

    assert.isTrue(
      user2AccountAfter.lendingAccount.balances.some(
        (b: any) =>
          b.bankPk.equals(bankPk) &&
          wrappedI80F48toBigNumber(b.assetShares).gt(0)
      ),
      "Destination should have received balance"
    );
  });

  it("Should reject transfer to same account", async () => {
    const testUser = users[2];

    const {
      program,
      bankPk,
      userAccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: userAccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet);
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(result.result, "Should fail with PositionTransferIdenticalAccounts");
  });

  it("Should reject transfer with wrong destination authority", async () => {
    const testUser = users[3];
    const testUser2 = users[4];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const transferIxWithWrongAuth = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIxWithWrongAuth);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet);
    
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(
      result.result,
      "Should fail when destination authority is not the account authority"
    );
  });

  it("Should reject transfer without destination signature", async () => {
    const testUser = users[5];
    const testUser2 = users[6];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet);
    
    let failed = false;
    try {
      await banksClient.tryProcessTransaction(transferTx);
    } catch (e: any) {
      if (e.message.includes("Signature verification failed")) {
        failed = true;
      }
    }
    
    assert.isTrue(
      failed,
      "Should fail without destination authority signature at transaction serialization"
    );
  });

  it("Should reject transfer when destination is frozen", async () => {
    const testUser = users[7];
    const testUser2 = users[8];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const freezeIx = await program.methods
      .marginfiAccountSetFreeze(true)
      .accounts({
        marginfiAccount: user2AccountPk,
        admin: groupAdmin.wallet.publicKey,
      })
      .instruction();

    const freezeTx = new Transaction().add(freezeIx);
    freezeTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    freezeTx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(freezeTx);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(result.result, "Should fail when destination is frozen");
  });

  it("Should reject transfer when source has TRANSFER_SEND_DISABLED flag", async () => {
    const testUser = users[9];
    const testUser2 = users[10];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const flagsIx = await program.methods
      .marginfiAccountSetTransferFlags(false, true)
      .accounts({
        marginfiAccount: userAccountPk,
        authority: testUser.wallet.publicKey,
      })
      .instruction();

    const flagsTx = new Transaction().add(flagsIx);
    flagsTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    flagsTx.sign(testUser.wallet);
    await banksClient.processTransaction(flagsTx);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(
      result.result,
      "Should fail when source has TRANSFER_SEND_DISABLED"
    );
  });

  it("Should reject transfer when destination has TRANSFER_RECEIVE_DISABLED flag", async () => {
    const testUser = users[11];
    const testUser2 = users[12];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const flagsIx = await program.methods
      .marginfiAccountSetTransferFlags(true, false)
      .accounts({
        marginfiAccount: user2AccountPk,
        authority: testUser2.wallet.publicKey,
      })
      .instruction();

    const flagsTx = new Transaction().add(flagsIx);
    flagsTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    flagsTx.sign(testUser2.wallet);
    await banksClient.processTransaction(flagsTx);

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(
      result.result,
      "Should fail when destination has TRANSFER_RECEIVE_DISABLED"
    );
  });

  it("Should reject transfer below minimum amount", async () => {
    const testUser = users[13];
    const testUser2 = users[14];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      feeWallet,
    } = await setupTransfer(testUser, testUser2, new BN(100_000_000_000), new BN(1_000));

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount: new BN(1_000),
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    const result = await banksClient.tryProcessTransaction(transferTx);
    assert.isNotNull(result.result, "Should fail with amount below minimum");
  });  it("Should collect protocol fee on transfer", async () => {
    const testUser = users[14];
    const testUser2 = users[15];

    const {
      program,
      bankPk,
      userAccountPk,
      user2AccountPk,
      remainingAccounts,
      transferAmount,
      feeWallet,
    } = await setupTransfer(testUser, testUser2);

    const feeWalletBalanceBefore = await banksClient.getAccount(feeWallet);
    const feeBalanceBefore = feeWalletBalanceBefore?.lamports ?? 0;

    const transferIx = await lendingAccountTransferPositionIx(program, {
      sourceMarginfiAccount: userAccountPk,
      destinationMarginfiAccount: user2AccountPk,
      bank: bankPk,
      marginfiGroup: setupTest.throwawayGroup.publicKey,
      authority: testUser.wallet.publicKey,
      destinationAuthority: testUser2.wallet.publicKey,
      transferAmount,
      globalFeeWallet: feeWallet,
      remainingAccounts,
    });

    const transferTx = new Transaction().add(transferIx);
    transferTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    transferTx.sign(testUser.wallet, testUser2.wallet);
    await banksClient.processTransaction(transferTx);

    const feeWalletBalanceAfter = await banksClient.getAccount(feeWallet);
    const feeBalanceAfter = feeWalletBalanceAfter?.lamports ?? 0;

    assert.isTrue(
      feeBalanceAfter > feeBalanceBefore,
      "Fee wallet should have received protocol fee"
    );
  });
});




