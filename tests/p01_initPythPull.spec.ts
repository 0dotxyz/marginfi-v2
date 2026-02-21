import { BN, Wallet, getProvider, AnchorProvider } from "@coral-xyz/anchor";
import { AccountMeta, Keypair, Transaction } from "@solana/web3.js";
import {
  groupAdmin,
  bankrunContext,
  banksClient,
  bankrunProgram,
  verbose,
  ecosystem,
  oracles,
  PYTH_ORACLE_FEED_SAMPLE,
  users,
} from "./rootHooks";
import { assertI80F48Approx, assertKeysEqual } from "./utils/genericTests";
import { addBankWithSeed, groupInitialize } from "./utils/group-instructions";
import { assert } from "chai";
import {
  defaultBankConfig,
  ASSET_TAG_DEFAULT,
  ORACLE_SETUP_PYTH_PUSH,
  HEALTH_CACHE_ENGINE_OK,
  HEALTH_CACHE_HEALTHY,
  CONF_INTERVAL_MULTIPLE,
  HEALTH_CACHE_ORACLE_OK,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";
import { deriveBankWithSeed } from "./utils/pdas";
import { createMintToInstruction } from "@solana/spl-token";
import { USER_ACCOUNT } from "./utils/mocks";
import { accountInit, depositIx, healthPulse } from "./utils/user-instructions";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { bytesToF64, getBankrunBlockhash, processBankrunTransaction } from "./utils/tools";

const GROUP_SEED = Buffer.from("MARGINFI_GROUP_SEED_000000000pxx");
const throwawayGroup = Keypair.fromSeed(GROUP_SEED);
const BANK_SEED = 42;

describe("Pyth pull oracles in localnet", () => {
  it("(admin) Init group - happy path", async () => {
    let tx = new Transaction();

    tx.add(
      await groupInitialize(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
      }),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet, throwawayGroup);
    await banksClient.processTransaction(tx);

    let group = await bankrunProgram.account.marginfiGroup.fetch(
      throwawayGroup.publicKey,
    );
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
    if (verbose) {
      console.log("*init group: " + throwawayGroup.publicKey);
      console.log(" group admin: " + group.admin);
    }
  });

  it("(admin) Add 'LST' bank with mainnet pyth pull oracles (Jup's Sol oracle)", async () => {
    let setConfig = defaultBankConfig();
    const seed = new BN(BANK_SEED);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed,
    );
    const oracleMeta: AccountMeta = {
      pubkey: PYTH_ORACLE_FEED_SAMPLE,
      isSigner: false,
      isWritable: false,
    };

    let tx = new Transaction();
    tx.add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.lstAlphaMint.publicKey,
        config: setConfig,
        seed,
      }),
      await groupAdmin.mrgnProgram.methods
        .lendingPoolConfigureBankOracle(
          ORACLE_SETUP_PYTH_PUSH,
          PYTH_ORACLE_FEED_SAMPLE,
        )
        .accountsPartial({
          group: throwawayGroup.publicKey,
          bank: bankKey,
          admin: groupAdmin.wallet.publicKey,
        })
        .remainingAccounts([oracleMeta])
        .instruction(),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.tryProcessTransaction(tx);

    if (verbose) {
      console.log("*init LST bank " + bankKey);
    }

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(bank.config.assetTag, ASSET_TAG_DEFAULT);
  });

  it("(admin) Add 'LST' bank with mock pyth pull oracles", async () => {
    let setConfig = defaultBankConfig();
    const seed = new BN(BANK_SEED + 1);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed,
    );
    const oracleMeta: AccountMeta = {
      pubkey: oracles.pythPullLst.publicKey,
      isSigner: false,
      isWritable: false,
    };

    let tx = new Transaction();
    tx.add(
      await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
        marginfiGroup: throwawayGroup.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.lstAlphaMint.publicKey,
        config: setConfig,
        seed: seed,
      }),
      await groupAdmin.mrgnProgram.methods
        .lendingPoolConfigureBankOracle(
          ORACLE_SETUP_PYTH_PUSH,
          oracles.pythPullLst.publicKey,
        )
        .accountsPartial({
          group: throwawayGroup.publicKey,
          bank: bankKey,
          admin: groupAdmin.wallet.publicKey,
        })
        .remainingAccounts([oracleMeta])
        .instruction(),
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    await banksClient.processTransaction(tx);

    if (verbose) {
      console.log("*init WSOL bank " + bankKey);
    }

    const bank = await bankrunProgram.account.bank.fetch(bankKey);
    assert.equal(bank.config.assetTag, ASSET_TAG_DEFAULT);
  });

  it("(user 3) optional account setup", async () => {
    const user = users[3];

    // Init mrgn account if needed.
    if (!user.accounts.has(USER_ACCOUNT)) {
      const userAccKeypair = Keypair.generate();
      const userAccount = userAccKeypair.publicKey;
      user.accounts.set(USER_ACCOUNT, userAccount);

      let accountInitTx: Transaction = new Transaction();
      accountInitTx.add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: userAccount,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        }),
      );
      accountInitTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      accountInitTx.sign(user.wallet, userAccKeypair);
      await banksClient.processTransaction(accountInitTx);
    }
    // Fund the user
    const provider = getProvider() as AnchorProvider;
    const wallet = provider.wallet as Wallet;
    let fundUserTx = new Transaction();
    fundUserTx.add(
      createMintToInstruction(
        ecosystem.lstAlphaMint.publicKey,
        user.lstAlphaAccount,
        wallet.publicKey,
        20 * 10 ** ecosystem.lstAlphaDecimals,
      ),
    );
    fundUserTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    fundUserTx.sign(wallet.payer);
    await banksClient.processTransaction(fundUserTx);
  });

  it("(user 3) deposit into Pyth pull bank", async () => {
    const user = users[3];
    const depositAmount = 2;
    const userAcc = user.accounts.get(USER_ACCOUNT);

    const seed = new BN(BANK_SEED + 1);
    const [bankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.lstAlphaMint.publicKey,
      seed,
    );

    let depositTx = new Transaction().add(
      await depositIx(user.mrgnBankrunProgram, {
        marginfiAccount: userAcc,
        bank: bankKey,
        tokenAccount: user.lstAlphaAccount,
        amount: new BN(depositAmount * 10 ** ecosystem.lstAlphaDecimals),
        depositUpToLimit: false,
      }),
      // Pulse to view cache state and read prices
      await healthPulse(user.mrgnBankrunProgram, {
        marginfiAccount: userAcc,
        remaining: [bankKey, oracles.pythPullLst.publicKey],
      }),
    );

    await processBankrunTransaction(
      bankrunContext,
      depositTx,
      [user.wallet],
      false,
      true,
    );

    const acc = await bankrunProgram.account.marginfiAccount.fetch(userAcc);
    const cache = acc.healthCache;
    if (verbose) {
      console.log(
        "Shares: " +
          wrappedI80F48toBigNumber(
            acc.lendingAccount.balances[0].assetShares,
          ).toNumber(),
      );
      console.log("price actual: " + bytesToF64(cache.prices[0]));
      console.log(
        "assets actual: " +
          wrappedI80F48toBigNumber(cache.assetValue).toNumber(),
      );
    }

    assertI80F48Approx(
      acc.lendingAccount.balances[0].assetShares,
      depositAmount * 10 ** ecosystem.lstAlphaDecimals,
    );
    assert.equal(
      cache.flags,
      HEALTH_CACHE_HEALTHY + HEALTH_CACHE_ENGINE_OK + HEALTH_CACHE_ORACLE_OK,
    );
    const priceExpected =
      oracles.lstAlphaPrice *
      (1 - ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE);
    assert.approximately(bytesToF64(cache.prices[0]), priceExpected, 0.0001);
    assertI80F48Approx(cache.assetValue, priceExpected * depositAmount);
  });
});
