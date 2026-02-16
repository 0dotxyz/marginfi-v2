import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { AccountMeta, PublicKey, Transaction } from "@solana/web3.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  marginfiGroup,
  oracles,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNApproximately,
  assertI80F48Approx,
  assertI80F48Equal,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  composeRemainingAccounts,
  composeRemainingAccountsByBalances,
  depositIx,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { createMintToInstruction } from "@solana/spl-token";
import { deriveBankWithSeed, deriveLiquidityVault } from "./utils/pdas";
import { addBankWithSeed } from "./utils/group-instructions";
import {
  defaultBankConfig,
  ORACLE_SETUP_PYTH_PUSH,
  u64MAX_BN,
} from "./utils/types";
import { getBankrunTime } from "./utils/tools";

let program: Program<Marginfi>;
let mintAuthority: PublicKey;
let provider: BankrunProvider;

describe("Deposit funds", () => {
  before(() => {
    provider = bankRunProvider;
    program = bankrunProgram;
    // Use bankrun payer as mint authority (same as when mints were created)
    mintAuthority = bankrunContext.payer.publicKey;
  });
  const depositAmountA = 2;
  const depositAmountA_native = new BN(
    depositAmountA * 10 ** ecosystem.tokenADecimals
  );

  const depositAmountUsdc = 100;
  const depositAmountUsdc_native = new BN(
    depositAmountUsdc * 10 ** ecosystem.usdcDecimals
  );

  const depositAmountSol = 10;
  const depositAmountSol_native = new BN(
    depositAmountSol * 10 ** ecosystem.wsolDecimals
  );

  it("(Fund user 0 and user 1 USDC/Token A/SOL token accounts", async () => {
    let tx = new Transaction();
    for (let i = 0; i < users.length; i++) {
      tx.add(
        createMintToInstruction(
          ecosystem.tokenAMint.publicKey,
          users[i].tokenAAccount,
          mintAuthority,
          100 * 10 ** ecosystem.tokenADecimals
        )
      );
      tx.add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          users[i].usdcAccount,
          mintAuthority,
          10000 * 10 ** ecosystem.usdcDecimals
        )
      );
      tx.add(
        createMintToInstruction(
          ecosystem.wsolMint.publicKey,
          users[i].wsolAccount,
          mintAuthority,
          10000 * 10 ** ecosystem.wsolDecimals
        )
      );
    }
    // Use provider which has payer as wallet (the mint authority)
    await provider.sendAndConfirm(tx);
  });

  it("(user 0) deposit token A to bank - happy path", async () => {
    const user = users[0];
    const [bankLiquidityVault] = deriveLiquidityVault(
      program.programId,
      bankKeypairA.publicKey
    );
    const [userABefore, vaultABefore] = await Promise.all([
      getTokenBalance(provider, user.tokenAAccount),
      getTokenBalance(provider, bankLiquidityVault),
    ]);
    if (verbose) {
      console.log("user 0 A before: " + userABefore.toLocaleString());
      console.log("vault A before:  " + vaultABefore.toLocaleString());
    }

    const user0Account = user.accounts.get(USER_ACCOUNT);

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: user0Account,
          bank: bankKeypairA.publicKey,
          tokenAccount: user.tokenAAccount,
          amount: depositAmountA_native,
          depositUpToLimit: false,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bankKeypairA.publicKey);
    assert.equal(bankAfter.lendingPositionCount, 1);

    const userAcc = await program.account.marginfiAccount.fetch(user0Account);
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    // Note: The first deposit issues shares 1:1 and the shares use the same decimals
    assertI80F48Approx(balances[0].assetShares, depositAmountA_native);
    assertI80F48Equal(balances[0].liabilityShares, 0);
    assertI80F48Equal(balances[0].emissionsOutstanding, 0);

    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(balances[0].lastUpdate, now, 2);
    assertBNApproximately(userAcc.lastUpdate, now, 2);

    const [userAAfter, vaultAAfter] = await Promise.all([
      getTokenBalance(provider, user.tokenAAccount),
      getTokenBalance(provider, bankLiquidityVault),
    ]);
    if (verbose) {
      console.log("user 0 A after: " + userAAfter.toLocaleString());
      console.log("vault A after:  " + vaultAAfter.toLocaleString());
    }
    assert.equal(userABefore - depositAmountA_native.toNumber(), userAAfter);
    assert.equal(vaultABefore + depositAmountA_native.toNumber(), vaultAAfter);
  });

  it("(user 1) deposit USDC to bank - happy path", async () => {
    const user = users[1];
    const userUsdcBefore = await getTokenBalance(provider, user.usdcAccount);
    if (verbose) {
      console.log("user 1 USDC before: " + userUsdcBefore.toLocaleString());
    }

    const user1Account = user.accounts.get(USER_ACCOUNT);

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: user1Account,
          bank: bankKeypairUsdc.publicKey,
          tokenAccount: user.usdcAccount,
          amount: depositAmountUsdc_native,
          depositUpToLimit: false,
        })
      )
    );

    const bankAfter = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    assert.equal(bankAfter.lendingPositionCount, 1);

    const userAcc = await program.account.marginfiAccount.fetch(user1Account);
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    // Note: The first deposit issues shares 1:1 and the shares use the same decimals
    assertI80F48Approx(balances[0].assetShares, depositAmountUsdc_native);
    assertI80F48Equal(balances[0].liabilityShares, 0);
    assertI80F48Equal(balances[0].emissionsOutstanding, 0);

    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(balances[0].lastUpdate, now, 2);
    assertBNApproximately(userAcc.lastUpdate, now, 2);

    const userUsdcAfter = await getTokenBalance(provider, user.usdcAccount);
    if (verbose) {
      console.log("user 1 USDC after: " + userUsdcAfter.toLocaleString());
    }
    assert.equal(
      userUsdcBefore - depositAmountUsdc_native.toNumber(),
      userUsdcAfter
    );
  });

  it("(user 1) deposit up to limit - happy path", async () => {
    const depositAmount0 = 500;
    const depositLimit = 10000;

    // Init a dummy bank for this test...
    let config = defaultBankConfig();
    config.depositLimit = new BN(10_000);
    const seed = new BN(0);
    const [bankKey] = deriveBankWithSeed(
      program.programId,
      marginfiGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      seed
    );
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnProgram, {
          marginfiGroup: marginfiGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          // globalFeeWallet: globalFeeWallet,
          config: config,
          seed: seed,
        }),
        await program.methods
          .lendingPoolConfigureBankOracle(
            ORACLE_SETUP_PYTH_PUSH,
            oracles.tokenAOracle.publicKey
          )
          .accountsPartial({
            group: marginfiGroup.publicKey,
            bank: bankKey,
            admin: groupAdmin.wallet.publicKey,
          })
          .remainingAccounts([
            {
              pubkey: oracles.tokenAOracle.publicKey,
              isSigner: false,
              isWritable: false,
            } as AccountMeta,
          ])
          .instruction()
      )
    );

    // User 0 deposits a small amount of funds...
    const user0Account = users[0].accounts.get(USER_ACCOUNT);
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(users[0].mrgnProgram, {
          marginfiAccount: user0Account,
          bank: bankKey,
          tokenAccount: users[0].tokenAAccount,
          amount: new BN(depositAmount0),
          depositUpToLimit: false,
        })
      )
    );

    let bankAfter = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfter.lendingPositionCount, 1);

    // And now user user 1 attempts to deposit up to the deposit cap
    const user = users[1];
    const userTokenABefore = await getTokenBalance(
      provider,
      user.tokenAAccount
    );
    if (verbose) {
      console.log(
        "user 1 Token A before: " + userTokenABefore.toLocaleString()
      );
    }

    const user1Account = user.accounts.get(USER_ACCOUNT);
    const userAccBefore = await program.account.marginfiAccount.fetch(
      user1Account
    );
    const balancesBefore = userAccBefore.lendingAccount.balances;
    assert.equal(balancesBefore[0].active, 1);
    assert.equal(balancesBefore[1].active, 0);

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: user1Account,
          bank: bankKey,
          tokenAccount: user.tokenAAccount,
          // NOTE: Pass u64::MAX to go up to the deposit limit regardless of amount, or pass some
          // smaller amount to clamp to that amount (the actual amount deposited is always
          // min(amount, deposit_amt_up_to_cap))
          amount: u64MAX_BN,
          depositUpToLimit: true,
        })
      )
    );

    bankAfter = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfter.lendingPositionCount, 2);

    const userTokenAAfter = await getTokenBalance(provider, user.tokenAAccount);
    if (verbose) {
      console.log("user 1 Token A after: " + userTokenAAfter.toLocaleString());
    }
    // Note: We are always 1 token short of the deposit limit, because an internal check performs a
    // < instead of a <= when validating the deposit limit
    const expected = depositLimit - depositAmount0 - 1;
    assert.equal(
      userTokenABefore - userTokenAAfter,
      depositLimit - depositAmount0 - 1
    );

    const userAcc = await program.account.marginfiAccount.fetch(user1Account);
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[0].active, 1);
    assert.equal(balances[1].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const depositIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(bankKey)
    );
    assertI80F48Approx(balances[depositIndex].assetShares, expected);
    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(balances[depositIndex].lastUpdate, now, 2);
    assertBNApproximately(userAcc.lastUpdate, now, 2);

    // withdraw amounts to restore to previous state...

    // For withdrawAll, include all active balances, including the closing bank.
    const remainingUser1 = composeRemainingAccountsByBalances(
      userAcc.lendingAccount.balances,
      [
        [bankKey, oracles.tokenAOracle.publicKey],
        [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
      ],
      bankKey
    );
    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(user.mrgnProgram, {
          marginfiAccount: user1Account,
          bank: bankKey,
          tokenAccount: user.tokenAAccount,
          remaining: remainingUser1,
          amount: new BN(1), // doesn't matter when withdrawing all...
          withdrawAll: true,
        })
      )
    );

    // For withdrawAll, include all active balances, including the closing bank.
    const user0Acc = await users[0].mrgnProgram.account.marginfiAccount.fetch(
      user0Account
    );
    const remainingUser0 = composeRemainingAccountsByBalances(
      user0Acc.lendingAccount.balances,
      [
        [bankKey, oracles.tokenAOracle.publicKey],
        [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
      ],
      bankKey
    );
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await withdrawIx(users[0].mrgnProgram, {
          marginfiAccount: user0Account,
          bank: bankKey,
          tokenAccount: users[0].tokenAAccount,
          remaining: remainingUser0,
          amount: new BN(1), // doesn't matter when withdrawing all...
          withdrawAll: true,
        })
      )
    );

    bankAfter = await program.account.bank.fetch(bankKey);
    assert.equal(bankAfter.lendingPositionCount, 0);
  });

  it("(user 1) deposit SOL to bank - happy path", async () => {
    const user = users[1];
    const userSolBefore = await getTokenBalance(provider, user.wsolAccount);
    if (verbose) {
      console.log("user 1 SOL before: " + userSolBefore.toLocaleString());
    }

    const user1Account = user.accounts.get(USER_ACCOUNT);

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: user1Account,
          bank: bankKeypairSol.publicKey,
          tokenAccount: user.wsolAccount,
          amount: depositAmountSol_native,
          depositUpToLimit: false,
        })
      )
    );

    const userAcc = await program.account.marginfiAccount.fetch(user1Account);
    const balances = userAcc.lendingAccount.balances;
    assert.equal(balances[1].active, 1);

    // Note: the newly added balance may NOT be the last one in the list, due to sorting, so we have to find its position first
    const depositIndex = balances.findIndex((balance) =>
      balance.bankPk.equals(bankKeypairSol.publicKey)
    );

    // Note: The first deposit issues shares 1:1 and the shares use the same decimals
    assertI80F48Approx(
      balances[depositIndex].assetShares,
      depositAmountSol_native
    );
    assertI80F48Equal(balances[depositIndex].liabilityShares, 0);
    assertI80F48Equal(balances[depositIndex].emissionsOutstanding, 0);

    let now = await getBankrunTime(bankrunContext);
    assertBNApproximately(balances[depositIndex].lastUpdate, now, 2);
    assertBNApproximately(userAcc.lastUpdate, now, 2);

    const userSolAfter = await getTokenBalance(provider, user.wsolAccount);
    if (verbose) {
      console.log("user 1 SOL after: " + userSolAfter.toLocaleString());
    }
    assert.equal(
      userSolBefore - depositAmountSol_native.toNumber(),
      userSolAfter
    );
  });
});
