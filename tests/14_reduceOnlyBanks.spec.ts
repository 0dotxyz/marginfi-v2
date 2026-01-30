import { BN, Program } from "@coral-xyz/anchor";
import { configureBank } from "./utils/group-instructions";
import { Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunProgram,
  ecosystem,
  groupAdmin,
  marginfiGroup,
  oracles,
  users,
} from "./rootHooks";
import { expectFailedTxWithError } from "./utils/genericTests";
import { assert } from "chai";
import {
  CONF_INTERVAL_MULTIPLE,
  defaultBankConfigOptRaw,
  ORACLE_CONF_INTERVAL,
} from "./utils/types";
import {
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  healthPulse,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import { dummyTx } from "./utils/bankrunConnection";

describe("Reduce-Only Bank Tests", () => {
  let program: Program<Marginfi>;

  before(() => {
    program = bankrunProgram;
  });

  before("Initialize user accounts", async () => {
    const user0AccountKeypair = Keypair.generate();
    users[0].accounts.set(USER_ACCOUNT, user0AccountKeypair.publicKey);
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(program, {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: user0AccountKeypair.publicKey,
          authority: users[0].wallet.publicKey,
          feePayer: users[0].wallet.publicKey,
        })
      ),
      [user0AccountKeypair]
    );

    const user1AccountKeypair = Keypair.generate();
    users[1].accounts.set(USER_ACCOUNT, user1AccountKeypair.publicKey);
    await users[1].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await accountInit(program, {
          marginfiGroup: marginfiGroup.publicKey,
          marginfiAccount: user1AccountKeypair.publicKey,
          authority: users[1].wallet.publicKey,
          feePayer: users[1].wallet.publicKey,
        })
      ),
      [user1AccountKeypair]
    );
  });

  it("(admin) Set bank to ReduceOnly, then restore to Operational - verifies state changes", async () => {
    const bankKey = bankKeypairSol.publicKey;

    try {
      // Set SOL bank to ReduceOnly
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                reduceOnly: undefined,
              },
            },
          })
        )
      );

      let bank = await program.account.bank.fetch(bankKey);
      assert.deepEqual(bank.config.operationalState, { reduceOnly: {} });

      // Restore to Operational
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                operational: undefined,
              },
            },
          })
        )
      );
    } finally {
      // Ensure cleanup even if test fails
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          dummyTx(groupAdmin.wallet.publicKey, users[1].wallet.publicKey),
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                operational: undefined,
              },
            },
          })
        )
      );
    }
  });

  it("(user 0) ReduceOnly collateral is worthless for new loans - should fail with RiskEngineInitRejected", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const depositAmountTokenA = 0.5;
    const depositAmountTokenA_native = new BN(
      depositAmountTokenA * 10 ** ecosystem.tokenADecimals
    );

    try {
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await depositIx(user.mrgnProgram, {
            marginfiAccount: userAccount,
            bank: bankKeypairA.publicKey,
            tokenAccount: user.tokenAAccount,
            amount: depositAmountTokenA_native,
            depositUpToLimit: false,
          })
        )
      );

      // Health pulse BEFORE configuring bank to ReduceOnly
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await healthPulse(user.mrgnProgram, {
            marginfiAccount: userAccount,
            remaining: composeRemainingAccounts([
              [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
            ]),
          })
        )
      );

      const accBefore = await program.account.marginfiAccount.fetch(
        userAccount
      );
      const cacheBefore = accBefore.healthCache;
      const assetValueBefore = wrappedI80F48toBigNumber(cacheBefore.assetValue);
      const assetValueMaintBefore = wrappedI80F48toBigNumber(
        cacheBefore.assetValueMaint
      );

      const confidence = ORACLE_CONF_INTERVAL * CONF_INTERVAL_MULTIPLE;
      const adjustedAssetPrice = oracles.tokenAPrice * (1.0 - confidence);
      const bankA = await program.account.bank.fetch(bankKeypairA.publicKey);
      const assetWeightInit = wrappedI80F48toBigNumber(
        bankA.config.assetWeightInit
      ).toNumber();
      const expectedAssetValue =
        depositAmountTokenA * adjustedAssetPrice * assetWeightInit;

      // Verify assetValue matches expected calculation before ReduceOnly
      assert.approximately(
        assetValueBefore.toNumber(),
        expectedAssetValue,
        0.01,
        "Asset value should match expected calculation before ReduceOnly"
      );

      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairA.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                reduceOnly: undefined,
              },
            },
          })
        )
      );

      // Health pulse AFTER configuring bank to ReduceOnly
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          dummyTx(user.wallet.publicKey, users[1].wallet.publicKey),
          await healthPulse(user.mrgnProgram, {
            marginfiAccount: userAccount,
            remaining: composeRemainingAccounts([
              [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
            ]),
          })
        )
      );

      const accAfter = await program.account.marginfiAccount.fetch(userAccount);
      const cacheAfter = accAfter.healthCache;
      const assetValueAfter = wrappedI80F48toBigNumber(cacheAfter.assetValue);
      const assetValueMaintAfter = wrappedI80F48toBigNumber(
        cacheAfter.assetValueMaint
      );

      // Verify assetValue drops to 0 after ReduceOnly
      assert.equal(
        assetValueAfter.toNumber(),
        0,
        "Asset value should be 0 after ReduceOnly"
      );

      // Verify assetValueMaint is unchanged
      assert.equal(
        assetValueMaintBefore.toNumber(),
        assetValueMaintAfter.toNumber(),
        "Asset value (maint) should remain unchanged"
      );

      // User tries to borrow USDC using ReduceOnly Token A as collateral
      const borrowAmountUsdc = 10;
      const borrowAmountUsdc_native = new BN(
        borrowAmountUsdc * 10 ** ecosystem.usdcDecimals
      );

      await expectFailedTxWithError(
        async () => {
          await user.mrgnProgram.provider.sendAndConfirm!(
            new Transaction().add(
              await borrowIx(user.mrgnProgram, {
                marginfiAccount: userAccount,
                bank: bankKeypairUsdc.publicKey,
                tokenAccount: user.usdcAccount,
                remaining: composeRemainingAccounts([
                  [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
                  [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
                ]),
                amount: borrowAmountUsdc_native,
              })
            )
          );
        },
        "RiskEngineInitRejected",
        6006
      );
    } finally {
      // Restore Token A bank to Operational for cleanup
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairA.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                operational: undefined,
              },
            },
          })
        )
      );

      // Withdraw all Token A
      try {
        await user.mrgnProgram.provider.sendAndConfirm!(
          new Transaction().add(
            await withdrawIx(user.mrgnProgram, {
              marginfiAccount: userAccount,
              bank: bankKeypairA.publicKey,
              tokenAccount: user.tokenAAccount,
              remaining: [oracles.tokenAOracle.publicKey],
              amount: depositAmountTokenA_native,
              withdrawAll: true,
            })
          )
        );
      } catch (e) {
        console.log("Cleanup withdrawal failed: ", e);
      }
    }
  });

  it("(user 1) ReduceOnly collateral maintains worth for existing loans but fails for new borrows", async () => {
    const user = users[1];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const depositAmountTokenA = 0.5;
    const depositAmountTokenA_native = new BN(
      depositAmountTokenA * 10 ** ecosystem.tokenADecimals
    );

    try {
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await depositIx(user.mrgnProgram, {
            marginfiAccount: userAccount,
            bank: bankKeypairA.publicKey,
            tokenAccount: user.tokenAAccount,
            amount: depositAmountTokenA_native,
            depositUpToLimit: false,
          })
        )
      );

      const borrowAmountUsdc = 1;
      const borrowAmountUsdc_native = new BN(
        borrowAmountUsdc * 10 ** ecosystem.usdcDecimals
      );

      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await borrowIx(user.mrgnProgram, {
            marginfiAccount: userAccount,
            bank: bankKeypairUsdc.publicKey,
            tokenAccount: user.usdcAccount,
            remaining: composeRemainingAccounts([
              [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
              [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
            ]),
            amount: borrowAmountUsdc_native,
          })
        )
      );

      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairA.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                reduceOnly: undefined,
              },
            },
          })
        )
      );

      await expectFailedTxWithError(
        async () => {
          await user.mrgnProgram.provider.sendAndConfirm!(
            new Transaction().add(
              await borrowIx(user.mrgnProgram, {
                marginfiAccount: userAccount,
                bank: bankKeypairUsdc.publicKey,
                tokenAccount: user.usdcAccount,
                remaining: composeRemainingAccounts([
                  [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
                  [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
                ]),
                amount: borrowAmountUsdc_native,
              })
            )
          );
        },
        "RiskEngineInitRejected",
        6006
      );

      const userAccData = await program.account.marginfiAccount.fetch(
        userAccount
      );
      const balances = userAccData.lendingAccount.balances;

      const tokenABalanceIdx = balances.findIndex((b) =>
        b.bankPk.equals(bankKeypairA.publicKey)
      );
      assert.notEqual(tokenABalanceIdx, -1, "Token A balance should exist");
      assert.equal(
        balances[tokenABalanceIdx].active,
        1,
        "Token A balance should be active"
      );

      const usdcBalanceIdx = balances.findIndex((b) =>
        b.bankPk.equals(bankKeypairUsdc.publicKey)
      );
      assert.notEqual(usdcBalanceIdx, -1, "USDC balance should exist");
      assert.equal(
        balances[usdcBalanceIdx].active,
        1,
        "USDC balance should be active"
      );
    } finally {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairA.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                operational: undefined,
              },
            },
          })
        )
      );
    }
  });

  it("(user 0) Can't borrow using only ReduceOnly collateral but position remains healthy", async () => {
    const user = users[0];
    const userAccount = user.accounts.get(USER_ACCOUNT);

    const depositAmountSol = 0.05; // 0.05 SOL * $150 = $7.50
    const depositAmountSol_native = new BN(
      depositAmountSol * 10 ** ecosystem.wsolDecimals
    );
    const depositAmountTokenA = 0.2; // 0.2 Token A * $10 = $2
    const depositAmountTokenA_native = new BN(
      depositAmountTokenA * 10 ** ecosystem.tokenADecimals
    );

    try {
      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await depositIx(user.mrgnProgram, {
            marginfiAccount: userAccount,
            bank: bankKeypairSol.publicKey,
            tokenAccount: user.wsolAccount,
            amount: depositAmountSol_native,
            depositUpToLimit: false,
          })
        )
      );

      await user.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await depositIx(user.mrgnProgram, {
            marginfiAccount: userAccount,
            bank: bankKeypairA.publicKey,
            tokenAccount: user.tokenAAccount,
            amount: depositAmountTokenA_native,
            depositUpToLimit: false,
          })
        )
      );

      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairSol.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                reduceOnly: undefined,
              },
            },
          })
        )
      );

      const borrowAmountUsdc = 5;
      const borrowAmountUsdc_native = new BN(
        borrowAmountUsdc * 10 ** ecosystem.usdcDecimals
      );

      await expectFailedTxWithError(
        async () => {
          await user.mrgnProgram.provider.sendAndConfirm!(
            new Transaction().add(
              await borrowIx(user.mrgnProgram, {
                marginfiAccount: userAccount,
                bank: bankKeypairUsdc.publicKey,
                tokenAccount: user.usdcAccount,
                remaining: composeRemainingAccounts([
                  [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
                  [bankKeypairSol.publicKey, oracles.wsolOracle.publicKey],
                  [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
                ]),
                amount: borrowAmountUsdc_native,
              })
            )
          );
        },
        "RiskEngineInitRejected",
        6006
      );

      // Verify account has both balances
      const userAccData = await program.account.marginfiAccount.fetch(
        userAccount
      );
      const balances = userAccData.lendingAccount.balances;

      // Should have SOL (ReduceOnly) and Token A (Operational)
      const solBalanceIdx = balances.findIndex((b) =>
        b.bankPk.equals(bankKeypairSol.publicKey)
      );
      const tokenABalanceIdx = balances.findIndex((b) =>
        b.bankPk.equals(bankKeypairA.publicKey)
      );

      assert.notEqual(solBalanceIdx, -1, "SOL balance should exist");
      assert.notEqual(tokenABalanceIdx, -1, "Token A balance should exist");
    } finally {
      await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBank(groupAdmin.mrgnProgram, {
            bank: bankKeypairSol.publicKey,
            bankConfigOpt: {
              ...defaultBankConfigOptRaw(),
              operationalState: {
                operational: undefined,
              },
            },
          })
        )
      );
    }
  });
});
