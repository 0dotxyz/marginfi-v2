import { BN } from "@coral-xyz/anchor";
import { ComputeBudgetProgram, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  globalFeeWallet,
  groupAdmin,
  oracles,
  riskAdmin,
  users,
} from "../../rootHooks";
import {
  adminCloseBankruptAccount,
  groupConfigure,
  handleBankruptcy,
  setFixedPrice,
} from "../../utils/group-instructions";
import { ACCOUNT_DISABLED, MAX_BALANCES } from "../../utils/types";
import {
  borrowIx,
  composeRemainingAccounts,
  depositIx,
} from "../../utils/user-instructions";
import { processBankrunTransaction } from "../../utils/tools";
import { refreshPullOraclesBankrun } from "../../utils/bankrun-oracles";
import { genericMultiBankTestSetup } from "../../genericSetups";

const startingSeed: number = 1700;
const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_1234000000M7");
const USER_ACCOUNT_THROWAWAY = "throwaway_account_m07";

let banks: PublicKey[] = [];
let throwawayGroup: Keypair;

/**
 * Worst case for `admin_close_bankrupt_account`: a bankrupted account with all 16 balances must
 * close in one transaction without a lookup table.
 */
describe("m07: Close a bankrupted account with 16 balances", () => {
  it("init group, init banks, and fund banks", async () => {
    const result = await genericMultiBankTestSetup(
      MAX_BALANCES,
      USER_ACCOUNT_THROWAWAY,
      groupBuff,
      startingSeed,
    );
    banks = result.banks;
    throwawayGroup = result.throwawayGroup;
  });

  it("Refresh oracles", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("(admin) Sets the risk admin", async () => {
    const tx = new Transaction().add(
      await groupConfigure(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        newRiskAdmin: riskAdmin.wallet.publicKey,
      }),
    );
    await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
  });

  it("(admin) Seeds liquidity in all banks", async () => {
    const userAccount = groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY);
    const amount = new BN(100 * 10 ** ecosystem.lstAlphaDecimals);
    // About the max per TX without LUTs
    const depositsPerTx = 5;
    for (let i = 0; i < banks.length; i += depositsPerTx) {
      const tx = new Transaction();
      for (const bank of banks.slice(i, i + depositsPerTx)) {
        tx.add(
          await depositIx(groupAdmin.mrgnBankrunProgram, {
            marginfiAccount: userAccount,
            bank,
            tokenAccount: groupAdmin.lstAlphaAccount,
            amount,
            depositUpToLimit: false,
          }),
        );
      }
      await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
    }
  });

  it("(user 0) Deposits into banks[0] and borrows from the other 15", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const depositTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[0],
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(100 * 10 ** ecosystem.lstAlphaDecimals),
        depositUpToLimit: false,
      }),
    );
    await processBankrunTransaction(bankrunContext, depositTx, [user.wallet]);

    for (let i = 1; i < banks.length; i++) {
      const remaining = banks
        .slice(0, i + 1)
        .map((bank) => [bank, oracles.pythPullLst.publicKey]);
      const tx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: banks[i],
          tokenAccount: user.lstAlphaAccount,
          remaining: composeRemainingAccounts(remaining),
          amount: new BN(1 * 10 ** ecosystem.lstAlphaDecimals),
        }),
      );
      await processBankrunTransaction(bankrunContext, tx, [user.wallet]);
    }
  });

  it("(admin) Prices the collateral near zero, so user 0 is insolvent", async () => {
    // Fixed prices everywhere also mean health checks need no oracle accounts, only banks.
    for (let i = 0; i < banks.length; i += 4) {
      const tx = new Transaction();
      for (const bank of banks.slice(i, i + 4)) {
        tx.add(
          await setFixedPrice(groupAdmin.mrgnBankrunProgram, {
            bank,
            price: bank.equals(banks[0]) ? 0.000001 : oracles.lstAlphaPrice,
          }),
        );
      }
      await processBankrunTransaction(bankrunContext, tx, [groupAdmin.wallet]);
    }
  });

  it("(risk admin) Bankrupts all 15 debt legs of user 0", async () => {
    const userAccount = users[0].accounts.get(USER_ACCOUNT_THROWAWAY);
    for (const bank of banks.slice(1)) {
      const tx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
        await handleBankruptcy(riskAdmin.mrgnBankrunProgram, {
          signer: riskAdmin.wallet.publicKey,
          marginfiAccount: userAccount,
          bank,
          remaining: composeRemainingAccounts(banks.map((b) => [b])),
        }),
      );
      await processBankrunTransaction(bankrunContext, tx, [riskAdmin.wallet]);
    }

    const acc = await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    assert.equal(
      acc.accountFlags.toNumber() & ACCOUNT_DISABLED,
      ACCOUNT_DISABLED,
    );
  });

  it("(risk admin) Closes user 0 with all 16 balances in one transaction", async () => {
    const userAccount = users[0].accounts.get(USER_ACCOUNT_THROWAWAY);
    const accBefore =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const balances = accBefore.lendingAccount.balances.filter(
      (b) => b.active != 0,
    );
    assert.equal(balances.length, MAX_BALANCES);

    const tx = new Transaction().add(
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      await adminCloseBankruptAccount(riskAdmin.mrgnBankrunProgram, {
        riskAdmin: riskAdmin.wallet.publicKey,
        marginfiAccount: userAccount,
        globalFeeWallet,
        banks: balances.map((b) => b.bankPk),
      }),
    );
    const result = await processBankrunTransaction(bankrunContext, tx, [
      riskAdmin.wallet,
    ]);
    console.log(`*m07 close: serialized tx = ${tx.serialize().length} bytes`);
    console.log(`*m07 close: compute units consumed = ${result.computeUnitsConsumed}`);

    assert.isNull(await banksClient.getAccount(userAccount));
  });
});
