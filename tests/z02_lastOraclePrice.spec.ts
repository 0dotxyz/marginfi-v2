import { BN } from "@coral-xyz/anchor";
import {
  ComputeBudgetProgram,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  bankrunContext,
  bankrunProgram,
  banksClient,
  ecosystem,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";
import { genericMultiBankTestSetup } from "./genericSetups";
import {
  borrowIx,
  composeRemainingAccounts,
  composeRemainingAccountsByBalances,
  depositIx,
  repayIx,
  pulseBankPrice,
} from "./utils/user-instructions";
import { accrueInterest } from "./utils/group-instructions";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { assert } from "chai";
import {
  CONF_INTERVAL_MULTIPLE,
  ORACLE_CONF_INTERVAL,
  u64MAX_BN,
} from "./utils/types";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";
import { assertI80F48Approx, assertI80F48Equal } from "./utils/genericTests";

const readCacheFields = (cache: any) => {
  const price = cache?.lastOraclePrice ?? 0;
  const conf = cache?.lastOraclePriceConfidence ?? 0;
  return { price, conf };
};

/**
 * Bankrun tests that exercise the bank cache's last oracle price behavior:
 * - cache stays empty for non-price-based operations
 * - price-based instructions (borrow) stamp price & confidence
 * - pure interest accrual does not change last oracle price/confidence
 * - full repay (liabilities → 0) resets the cache
 */
describe("Bank cache last oracle price", () => {
  const startingSeed = 533;
  // Keypair::fromSeed requires exactly 32 bytes
  const groupBuff = Buffer.from("MARGINFI_GROUP_SEED_LAST_ORACLE0");
  const USER_ACCOUNT_THROWAWAY = "throwaway_last_oracle";

  let banks: PublicKey[] = [];
  let throwawayGroupPk: PublicKey;

  const seedAmount = new BN(1_000 * 10 ** ecosystem.lstAlphaDecimals);
  const collateralAmount = new BN(200 * 10 ** ecosystem.lstAlphaDecimals);
  const borrowAmount = new BN(50 * 10 ** ecosystem.lstAlphaDecimals);

  it("init group, banks, seed liquidity, and verify cache starts empty", async () => {
    const result = await genericMultiBankTestSetup(
      2,
      USER_ACCOUNT_THROWAWAY,
      groupBuff,
      startingSeed
    );
    banks = result.banks;
    throwawayGroupPk = result.throwawayGroup.publicKey;

    // Admin seeds both banks with liquidity (deposit does not use oracle price)
    const tx = new Transaction();
    for (const bank of banks) {
      tx.add(
        await depositIx(groupAdmin.mrgnBankrunProgram, {
          marginfiAccount: groupAdmin.accounts.get(USER_ACCOUNT_THROWAWAY),
          bank,
          tokenAccount: groupAdmin.lstAlphaAccount,
          amount: seedAmount,
          depositUpToLimit: false,
        })
      );
    }
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    const bank = await bankrunProgram.account.bank.fetch(banks[1]);
    const { price, conf } = readCacheFields(bank.cache);

    // No price-based instruction has run yet, cache should be zeroed
    assertI80F48Equal(price, 0);
    assertI80F48Equal(conf, 0);
  });

  it("(user 0) borrow stamps last oracle price/confidence", async () => {
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    // Deposit collateral into bank 0 (no oracle price used)
    {
      const tx = new Transaction().add(
        await depositIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: banks[0],
          tokenAccount: user.lstAlphaAccount,
          amount: collateralAmount,
          depositUpToLimit: false,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(user.wallet);
      await banksClient.processTransaction(tx);
    }

    const bankBefore = await bankrunProgram.account.bank.fetch(banks[1]);

    // Borrow from bank 1 to trigger a price-based share update + cache stamp
    {
      const remaining = composeRemainingAccounts([
        [banks[0], oracles.pythPullLst.publicKey],
        [banks[1], oracles.pythPullLst.publicKey],
      ]);
      const tx = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 300_000 }),
        await borrowIx(user.mrgnBankrunProgram, {
          marginfiAccount: userAccount,
          bank: banks[1],
          tokenAccount: user.lstAlphaAccount,
          remaining,
          amount: borrowAmount,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(user.wallet);
      await banksClient.processTransaction(tx);
    }

    const bankAfter = await bankrunProgram.account.bank.fetch(banks[1]);
    const { price, conf } = readCacheFields(bankAfter.cache);
    const expectedPrice = oracles.lstAlphaPrice;
    const expectedConf =
      expectedPrice * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;

    // Cache should record the last oracle price and confidence
    assertI80F48Approx(price, expectedPrice, 0.0001);
    assertI80F48Approx(conf, expectedConf, 0.01);
  });

  it("cached price tracks oracle price increases on subsequent price-based tx", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    // Bump the oracle price and refresh the on-chain oracle account
    const originalOraclePrice = oracles.lstAlphaPrice;
    const newOraclePrice = originalOraclePrice * 1.2;
    oracles.lstAlphaPrice = newOraclePrice;
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // Trigger a permissionless bank price pulse that will refresh the cache
    {
      const tx = new Transaction().add(
        await pulseBankPrice(user.mrgnBankrunProgram, {
          group: throwawayGroupPk,
          bank: banks[1],
          remaining: [oracles.pythPullLst.publicKey],
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(user.wallet);
      await banksClient.processTransaction(tx);
    }

    const bankAfter = await bankrunProgram.account.bank.fetch(banks[1]);
    const { price: updatedPrice, conf: updatedConf } = readCacheFields(
      bankAfter.cache
    );
    const expectedConf =
      newOraclePrice * ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;

    // Cached price should now reflect the higher oracle price
    assertI80F48Approx(updatedPrice, newOraclePrice, 0.0001);
    assertI80F48Approx(updatedConf, expectedConf, 0.01);

    // Restore oracle price for any subsequent tests
    oracles.lstAlphaPrice = originalOraclePrice;
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("accrue interest does not change last oracle price/confidence", async () => {
    const user = users[0];

    const bankBefore = await bankrunProgram.account.bank.fetch(banks[1]);
    const beforeFields = readCacheFields(bankBefore.cache);

    const tx = new Transaction().add(
      await accrueInterest(user.mrgnBankrunProgram, {
        bank: banks[1],
      }),
      // Dummy ix to keep bankrun happy
      SystemProgram.transfer({
        fromPubkey: user.wallet.publicKey,
        toPubkey: bankrunProgram.provider.publicKey,
        lamports: 41,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const bankAfter = await bankrunProgram.account.bank.fetch(banks[1]);
    const afterFields = readCacheFields(bankAfter.cache);

    // update_bank_cache was called with None, so price/confidence should be unchanged
    assertI80F48Equal(afterFields.price, beforeFields.price);
    assertI80F48Equal(afterFields.conf, beforeFields.conf);
  });

  it("(user 0) full repay resets bank cache, but not price, when liabs go to zero", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT_THROWAWAY);

    const bankBefore = await bankrunProgram.account.bank.fetch(banks[1]);
    const { price: priceBefore, conf: confBefore } = readCacheFields(
      bankBefore.cache
    ); // ensure cache is populated before repay

    // For repayAll, include all active balances, including the closing bank.
    const userAccBefore =
      await bankrunProgram.account.marginfiAccount.fetch(userAccount);
    const remaining = composeRemainingAccountsByBalances(
      userAccBefore.lendingAccount.balances,
      [
        [banks[1], oracles.pythPullLst.publicKey],
        [banks[0], oracles.pythPullLst.publicKey],
      ]
    );
    const tx = new Transaction().add(
      await repayIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAccount,
        bank: banks[1],
        tokenAccount: user.lstAlphaAccount,
        remaining,
        amount: u64MAX_BN,
        repayAll: true,
      })
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(user.wallet);
    await banksClient.processTransaction(tx);

    const bankAfter = await bankrunProgram.account.bank.fetch(banks[1]);
    const { price, conf } = readCacheFields(bankAfter.cache);

    // When liabilities (or assets) go to zero, resets rest of cache, not price
    assertI80F48Equal(price, priceBefore);
    assertI80F48Equal(conf, confBefore);
  });
});
