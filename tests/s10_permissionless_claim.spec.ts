/*
 * Here we test the permissionless withdrawal of bank fees. This is unrelated to staked collateral
 * and could execute in any test suite that has earned some fees.
 */
import { Keypair, Transaction } from "@solana/web3.js";
import {
  stakedBankKeypairSol,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalFeeWallet,
  groupAdmin,
  users,
} from "./rootHooks";
import {
  assertBankrunTxFailed,
  assertKeyDefault,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { getAssociatedTokenAddressSync } from "@mrgnlabs/mrgn-common";
import { u64MAX_BN } from "./utils/types";
import {
  collectBankFees,
  updateBankFeesDestinationAccount,
  withdrawFeesPermissionless,
} from "./utils/group-instructions";
import { createAssociatedTokenAccountIdempotentInstruction } from "@solana/spl-token";
import { deriveFeeVault } from "./utils/pdas";

let bankKeypairSol: Keypair;

describe("Set up permissionless fee claiming", () => {
  before(() => {
    bankKeypairSol = stakedBankKeypairSol;
  });

  /// Some wallet the admin wants to use to claim. This could also be their own wallet, user can
  /// pick arbitrarily.
  const externalWallet: Keypair = Keypair.generate();
  const wsolAta = getAssociatedTokenAddressSync(
    ecosystem.wsolMint.publicKey,
    externalWallet.publicKey
  );

  it("(user 0) tries to set a bogus claim destination - should fail", async () => {
    const user = users[0];

    let tx = new Transaction().add(
      await updateBankFeesDestinationAccount(user.mrgnBankrunProgram, {
        bank: bankKeypairSol.publicKey,
        destination: user.wsolAccount, // sneaky sneaky
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    let result = await banksClient.tryProcessTransaction(tx);
    // dumpBankrunLogs(result);

    // 6042 = Unauthorized (program uses more specific error than generic has_one)
    assertBankrunTxFailed(result, "0x179a");
  });

  it("(admin) set a claim destination - happy path", async () => {
    const admin = groupAdmin;

    let bankBefore = await bankrunProgram.account.bank.fetch(
      bankKeypairSol.publicKey
    );
    assertKeyDefault(bankBefore.feesDestinationAccount);

    let tx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        admin.wallet.publicKey,
        wsolAta,
        externalWallet.publicKey,
        ecosystem.wsolMint.publicKey
      ),
      await updateBankFeesDestinationAccount(admin.mrgnBankrunProgram, {
        bank: bankKeypairSol.publicKey,
        destination: wsolAta,
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(admin.wallet);
    await banksClient.processTransaction(tx);

    let bankAfter = await bankrunProgram.account.bank.fetch(
      bankKeypairSol.publicKey
    );
    assertKeysEqual(bankAfter.feesDestinationAccount, wsolAta);
  });

  it("(user 0 - permissionless) collect and withdraw fees - happy path", async () => {
    const user = users[0];
    // Note: for program fees, the global fee ata must always be provided when collecting
    const globalFeeAta = getAssociatedTokenAddressSync(
      ecosystem.wsolMint.publicKey,
      globalFeeWallet
    );

    let tx = new Transaction().add(
      createAssociatedTokenAccountIdempotentInstruction(
        user.wallet.publicKey,
        globalFeeAta,
        globalFeeWallet,
        ecosystem.wsolMint.publicKey
      ),
      await collectBankFees(user.mrgnBankrunProgram, {
        bank: bankKeypairSol.publicKey,
        feeAta: globalFeeAta,
      }),
      await withdrawFeesPermissionless(user.mrgnBankrunProgram, {
        bank: bankKeypairSol.publicKey,
        amount: u64MAX_BN, // withdraw all...
      })
    );

    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    let feeAccBalance = await getTokenBalance(bankRunProvider, wsolAta);
    // We don't really care how much was earned, if there's a non-zero number here then collection
    // was a success.
    assert.isAtLeast(feeAccBalance, 1);

    const [feeVault] = deriveFeeVault(
      bankrunProgram.programId,
      bankKeypairSol.publicKey
    );
    let feeVaultBalance = await getTokenBalance(bankRunProvider, feeVault);
    // The fee vault should be empty now.
    assert.equal(feeVaultBalance, 0);
  });
});
