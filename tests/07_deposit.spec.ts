import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { AccountMeta, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { Clock } from "solana-bankrun";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";
import BigNumber from "bignumber.js";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairA,
  bankKeypairSol,
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
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
  expectFailedTxWithError,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  composeRemainingAccountsByBalances,
  accountInit,
  borrowIx,
  composeRemainingAccounts,
  depositIx,
  withdrawIx,
} from "./utils/user-instructions";
import { USER_ACCOUNT } from "./utils/mocks";
import { createMintToInstruction } from "@solana/spl-token";
import { deriveBankWithSeed, deriveLiquidityVault } from "./utils/pdas";
import { addBankWithSeed, groupInitialize } from "./utils/group-instructions";
import {
  defaultBankConfig,
  ORACLE_SETUP_PYTH_PUSH,
  u64MAX_BN,
} from "./utils/types";
import { getBankrunBlockhash, getBankrunTime } from "./utils/tools";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

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
    const remainingUser1 = composeRemainingAccounts(
      [
        [bankKey, oracles.tokenAOracle.publicKey],
        [bankKeypairUsdc.publicKey, oracles.usdcOracle.publicKey],
      ].filter((group) => !group[0].equals(bankKey))
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
    const remainingUser0 = composeRemainingAccounts(
      [
        [bankKey, oracles.tokenAOracle.publicKey],
        [bankKeypairA.publicKey, oracles.tokenAOracle.publicKey],
      ].filter((group) => !group[0].equals(bankKey))
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

describe("Deposit up to limit with accrued interest", () => {
  const GROUP_SEED = Buffer.from("MRGN_DEPOSIT_LIMIT_CLK_TEST_0000");
  const USDC_BANK_SEED = new BN(29_000);
  const TOKEN_A_BANK_SEED = new BN(29_001);
  const UA = "z00_acc";

  const DEPOSIT_LIMIT_NATIVE = new BN(2_000 * 10 ** 6);
  const LENDER_DEPOSIT_NATIVE = new BN(600 * 10 ** 6);
  const BORROW_AMOUNT_NATIVE = new BN(400 * 10 ** 6);
  const THIRTY_DAYS_SECS = 30 * 24 * 60 * 60;

  let throwawayGroup: Keypair;
  let usdcBankKey: PublicKey;
  let clockBeforeTest: Clock;
  let tokenABankKey: PublicKey;
  // Pre-accrual remaining capacity captured before the clock is advanced
  let staleCapacityNative: BigNumber;

  before(async () => {
    throwawayGroup = Keypair.fromSeed(GROUP_SEED);

    // Init throwaway group
    {
      const tx = new Transaction().add(
        await groupInitialize(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet, throwawayGroup);
      await banksClient.processTransaction(tx);
    }

    // USDC bank with deposit limit and borrow enabled to drive interest accrual
    [usdcBankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.usdcMint.publicKey,
      USDC_BANK_SEED
    );
    {
      const config = defaultBankConfig();
      config.depositLimit = DEPOSIT_LIMIT_NATIVE;
      config.borrowLimit = new BN(2_000 * 10 ** 6);
      const oracleIx = await groupAdmin.mrgnBankrunProgram.methods
        .lendingPoolConfigureBankOracle(
          ORACLE_SETUP_PYTH_PUSH,
          oracles.usdcOracle.publicKey
        )
        .accountsPartial({
          group: throwawayGroup.publicKey,
          bank: usdcBankKey,
          admin: groupAdmin.wallet.publicKey,
        })
        .remainingAccounts([
          {
            pubkey: oracles.usdcOracle.publicKey,
            isSigner: false,
            isWritable: false,
          } as AccountMeta,
        ])
        .instruction();
      const tx = new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          config,
          seed: USDC_BANK_SEED,
        }),
        oracleIx
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      await banksClient.processTransaction(tx);
    }

    // Token-A bank used as collateral only
    [tokenABankKey] = deriveBankWithSeed(
      bankrunProgram.programId,
      throwawayGroup.publicKey,
      ecosystem.tokenAMint.publicKey,
      TOKEN_A_BANK_SEED
    );
    {
      const config = defaultBankConfig();
      config.depositLimit = new BN(100_000 * 10 ** ecosystem.tokenADecimals);
      const oracleIx = await groupAdmin.mrgnBankrunProgram.methods
        .lendingPoolConfigureBankOracle(
          ORACLE_SETUP_PYTH_PUSH,
          oracles.tokenAOracle.publicKey
        )
        .accountsPartial({
          group: throwawayGroup.publicKey,
          bank: tokenABankKey,
          admin: groupAdmin.wallet.publicKey,
        })
        .remainingAccounts([
          {
            pubkey: oracles.tokenAOracle.publicKey,
            isSigner: false,
            isWritable: false,
          } as AccountMeta,
        ])
        .instruction();
      const tx = new Transaction().add(
        await addBankWithSeed(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.tokenAMint.publicKey,
          config,
          seed: TOKEN_A_BANK_SEED,
        }),
        oracleIx
      );
      tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      tx.sign(groupAdmin.wallet);
      await banksClient.processTransaction(tx);
    }

    // Init user accounts and mint tokens (users 0, 1, 2)
    const payer = bankrunContext.payer;
    for (let i = 0; i <= 2; i++) {
      const user = users[i];
      const kp = Keypair.generate();
      user.accounts.set(UA, kp.publicKey);
      const accountTx = new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: kp.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      );
      accountTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      accountTx.sign(user.wallet, kp);
      await banksClient.processTransaction(accountTx);

      const mintTx = new Transaction().add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          user.usdcAccount,
          payer.publicKey,
          5_000 * 10 ** ecosystem.usdcDecimals
        ),
        createMintToInstruction(
          ecosystem.tokenAMint.publicKey,
          user.tokenAAccount,
          payer.publicKey,
          5_000 * 10 ** ecosystem.tokenADecimals
        )
      );
      mintTx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
      mintTx.sign(payer);
      await banksClient.processTransaction(mintTx);
    }

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    // User 0 deposits USDC (lender)
    await users[0].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(users[0].mrgnProgram, {
          marginfiAccount: users[0].accounts.get(UA),
          bank: usdcBankKey,
          tokenAccount: users[0].usdcAccount,
          amount: LENDER_DEPOSIT_NATIVE,
          depositUpToLimit: false,
        })
      )
    );

    // User 1 deposits token-A collateral then borrows USDC
    await users[1].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(users[1].mrgnProgram, {
          marginfiAccount: users[1].accounts.get(UA),
          bank: tokenABankKey,
          tokenAccount: users[1].tokenAAccount,
          amount: new BN(2_000 * 10 ** ecosystem.tokenADecimals),
          depositUpToLimit: false,
        })
      )
    );
    await users[1].mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await borrowIx(users[1].mrgnProgram, {
          marginfiAccount: users[1].accounts.get(UA),
          bank: usdcBankKey,
          tokenAccount: users[1].usdcAccount,
          remaining: composeRemainingAccounts([
            [tokenABankKey, oracles.tokenAOracle.publicKey],
            [usdcBankKey, oracles.usdcOracle.publicKey],
          ]),
          amount: BORROW_AMOUNT_NATIVE,
        })
      )
    );

    // Capture the remaining deposit capacity before advancing the clock.
    const bankBefore = await bankrunProgram.account.bank.fetch(usdcBankKey);
    const currentAssets = wrappedI80F48toBigNumber(bankBefore.totalAssetShares).multipliedBy(
      wrappedI80F48toBigNumber(bankBefore.assetShareValue)
    );
    staleCapacityNative = new BigNumber(bankBefore.config.depositLimit.toString())
      .minus(currentAssets)
      .minus(1)
      .integerValue(BigNumber.ROUND_FLOOR);
    assert.ok(staleCapacityNative.isGreaterThan(0));

    // Advance the bankrun clock by ~30 days so interest accrues
    const currentClock = await banksClient.getClock();
    clockBeforeTest = currentClock;
    bankrunContext.setClock(
      new Clock(
        currentClock.slot,
        currentClock.epochStartTimestamp,
        currentClock.epoch,
        currentClock.leaderScheduleEpoch,
        currentClock.unixTimestamp + BigInt(THIRTY_DAYS_SECS)
      )
    );

    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);
  });

  it("depositing the pre-accrual remaining capacity without deposit_up_to_limit fails", async () => {
    const user = users[2];
    await expectFailedTxWithError(
      async () => {
        await user.mrgnProgram.provider.sendAndConfirm(
          new Transaction().add(
            await depositIx(user.mrgnProgram, {
              marginfiAccount: user.accounts.get(UA),
              bank: usdcBankKey,
              tokenAccount: user.usdcAccount,
              amount: new BN(staleCapacityNative.toFixed(0)),
              depositUpToLimit: false,
            })
          )
        );
      },
      "BankAssetCapacityExceeded",
      6003
    );
  });

  it("deposit_up_to_limit=true succeeds and deposits less than the pre-accrual capacity", async () => {
    const user = users[2];
    const usdcBefore = await getTokenBalance(bankRunProvider, user.usdcAccount);

    await user.mrgnProgram.provider.sendAndConfirm(
      new Transaction().add(
        await depositIx(user.mrgnProgram, {
          marginfiAccount: user.accounts.get(UA),
          bank: usdcBankKey,
          tokenAccount: user.usdcAccount,
          amount: u64MAX_BN,
          depositUpToLimit: true,
        })
      )
    );
    const usdcAfter = await getTokenBalance(bankRunProvider, user.usdcAccount);
    const actualDeposited = usdcBefore - usdcAfter;

    assert.ok(actualDeposited > 0, "expected a non-zero deposit");
    assert.ok(
      new BigNumber(actualDeposited).isLessThan(staleCapacityNative),
      `actual deposit ${actualDeposited} must be < pre-accrual capacity ${staleCapacityNative}`
    );

    // Total assets must remain strictly below the deposit limit
    const bankFinal = await bankrunProgram.account.bank.fetch(usdcBankKey);
    const totalAssets = wrappedI80F48toBigNumber(bankFinal.totalAssetShares).multipliedBy(
      wrappedI80F48toBigNumber(bankFinal.assetShareValue)
    );
    assert.ok(
      totalAssets.isLessThan(new BigNumber(bankFinal.config.depositLimit.toString())),
      "total assets must remain below deposit limit"
    );

    if (verbose) {
      console.log(
        `deposited: ${actualDeposited}, pre-accrual capacity was: ${staleCapacityNative.toFixed(0)}`
      );
    }
  });

  after(async () => {
    // Restore the clock so tests that run after this suite are unaffected.
    bankrunContext.setClock(clockBeforeTest);
  });
});
