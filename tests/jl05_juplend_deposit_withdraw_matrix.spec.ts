import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import { accountInit, composeRemainingAccounts } from "./utils/user-instructions";

import { ensureJuplendPoolForMint, ensureJuplendClaimAccount } from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
  makeJuplendWithdrawIx,
} from "./utils/juplend/juplend-test-env";
import {
  decodeJuplendLendingState,
  expectedSharesForDeposit,
  expectedSharesForWithdraw,
  EXCHANGE_PRICE_PRECISION,
} from "./utils/juplend/juplend-state";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import { wrappedI80F48toBigNumber } from "@mrgnlabs/mrgn-common";

/** deterministic (32 bytes) */
const JUPLEND_GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000005");

function i80f48ToBigInt(x: any): bigint {
  // wrappedI80F48toBigNumber returns a BigNumber
  return BigInt(wrappedI80F48toBigNumber(x).toFixed(0));
}

async function fetchTokenExchangePrice(lending: PublicKey): Promise<bigint> {
  const info = await bankRunProvider.connection.getAccountInfo(lending);
  assert.ok(info, "missing lending state");
  const st = decodeJuplendLendingState(info!.data);
  return st.tokenExchangePrice;
}

async function fetchMarginfiAssetShares(
  program: any,
  marginfiAccount: PublicKey,
  bankPk: PublicKey
): Promise<bigint> {
  const acc = await program.account.marginfiAccount.fetch(marginfiAccount);
  const bal = (acc.lendingAccount.balances as any[]).find(
    (b) => b.active && (b.bankPk as PublicKey).equals(bankPk)
  );
  if (!bal) return 0n;
  return i80f48ToBigInt(bal.assetShares);
}

describe("jl05: JupLend - deposit/withdraw matrix (bankrun)", () => {
  const juplendGroup = Keypair.fromSeed(JUPLEND_GROUP_SEED);

  const BANK_SEED = new BN(5);
  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];

  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;

  let juplendBank: PublicKey;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;
  let claimAccount: PublicKey;

  let fTokenVaultBaseline: bigint;

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    // 1) Ensure underlying Juplend pool exists
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // 2) Init group
    const initGroupIx = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initGroupIx),
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );

    // 3) Add bank
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      fTokenMint: pool.fTokenMint,
    });

    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;
    claimAccount = derived.claimAccount;

    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    const addBankIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      oracle: oracles.usdcOracle.publicKey,
      integrationAcc1: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(addBankIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // 4) Activate bank (create fToken ATA + seed deposit)
    const initPosIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: groupAdmin.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initPosIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Create the claim account for liquidity_vault_authority (required for withdraw)
    await ensureJuplendClaimAccount({
      payer: groupAdmin.wallet,
      user: liquidityVaultAuthority,
      mint: ecosystem.usdcMint.publicKey,
    });

    fTokenVaultBaseline = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    assert.isTrue(fTokenVaultBaseline > 0n, "expected non-zero seed fToken balance");
  });

  it("matrix: deposit+withdraw in the same transaction nets out cleanly", async () => {
    const userAcc = Keypair.generate();
    const initIx = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [userA.wallet, userAcc],
      false,
      true
    );

    const depositAmt = new BN(10_000_000); // 10 USDC

    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

    const usdcBefore = BigInt(await getTokenBalance(bankRunProvider, userA.usdcAccount));
    const fTokenBefore = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userA.usdcAccount,

      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: depositAmt,
    });

    // Use withdrawAll to avoid rounding issues:
    // - Deposit uses floor(amount * 1e12 / price) for shares
    // - Withdraw uses ceil(amount * 1e12 / price) for shares
    // So withdraw(depositAmt) would try to burn more shares than we have.
    const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      destinationTokenAccount: userA.usdcAccount,

      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      claimAccount,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: new BN(0),
      withdrawAll: true,
      remainingAccounts: [],  // Position closes, no remaining needed
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx, withdrawIx),
      [userA.wallet],
      false,
      true
    );

    const usdcAfter = BigInt(await getTokenBalance(bankRunProvider, userA.usdcAccount));
    const fTokenAfter = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesAfter = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    assert.equal(sharesAfter, 0n, "expected no remaining shares after withdraw_all");
    // Due to floor rounding on both deposit (shares) and withdraw (underlying),
    // user may lose at most 1 unit of underlying.
    const maxRoundingLoss = 1n;
    assert.isTrue(
      usdcBefore - usdcAfter <= maxRoundingLoss,
      `expected at most 1 unit rounding loss, got ${usdcBefore - usdcAfter}`
    );
    assert.equal(fTokenAfter, fTokenBefore, "expected no net fToken vault delta");
  });

  it("matrix: deposit → withdraw(partial) → deposit → withdraw(all shares)", async () => {
    // fresh marginfi account
    const userAcc = Keypair.generate();
    const initIx = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [userA.wallet, userAcc],
      false,
      true
    );

    const deposit1 = new BN(25_000_000); // 25 USDC
    const withdraw1 = new BN(10_000_000); // 10 USDC
    const deposit2 = new BN(5_000_000); // 5 USDC

    // -----------------------------
    // deposit #1
    // -----------------------------
    const fTokenBefore1 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesBefore1 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    const depositIx1 = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userA.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: deposit1,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx1),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfter1 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesAfter1 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    const minted1 = fTokenAfter1 - fTokenBefore1;
    const price1 = await fetchTokenExchangePrice(pool.lending);
    const expectedMint1 = expectedSharesForDeposit(BigInt(deposit1.toString()), price1);

    assert.equal(minted1.toString(), expectedMint1.toString(), "minted shares mismatch (deposit1)");
    assert.equal(
      (sharesAfter1 - sharesBefore1).toString(),
      expectedMint1.toString(),
      "marginfi shares mismatch (deposit1)"
    );

    // -----------------------------
    // withdraw #1 (partial)
    // -----------------------------
    const fTokenBeforeW1 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesBeforeW1 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );
    const userUsdcBeforeW1 = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

    const withdrawIx1 = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      destinationTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      underlyingOracle: oracles.usdcOracle.publicKey,
      pool,
      fTokenVault,
      claimAccount,
      amount: withdraw1,
      remainingAccounts: remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawIx1),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfterW1 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesAfterW1 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );
    const userUsdcAfterW1 = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const burned1 = fTokenBeforeW1 - fTokenAfterW1;
    const priceW1 = await fetchTokenExchangePrice(pool.lending);
    const expectedBurn1 = expectedSharesForWithdraw(BigInt(withdraw1.toString()), priceW1);

    assert.equal(
      (userUsdcAfterW1 - userUsdcBeforeW1).toString(),
      withdraw1.toString(),
      "underlying amount mismatch (withdraw1)"
    );
    assert.equal(burned1.toString(), expectedBurn1.toString(), "burned shares mismatch (withdraw1)");
    assert.equal(
      (sharesBeforeW1 - sharesAfterW1).toString(),
      expectedBurn1.toString(),
      "marginfi shares mismatch (withdraw1)"
    );

    // -----------------------------
    // deposit #2
    // -----------------------------
    const fTokenBefore2 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesBefore2 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    const depositIx2 = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userA.usdcAccount,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: deposit2,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx2),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfter2 = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesAfter2 = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    const minted2 = fTokenAfter2 - fTokenBefore2;
    const price2 = await fetchTokenExchangePrice(pool.lending);
    const expectedMint2 = expectedSharesForDeposit(BigInt(deposit2.toString()), price2);

    assert.equal(minted2.toString(), expectedMint2.toString(), "minted shares mismatch (deposit2)");
    assert.equal(
      (sharesAfter2 - sharesBefore2).toString(),
      expectedMint2.toString(),
      "marginfi shares mismatch (deposit2)"
    );

    // -----------------------------
    // withdraw ALL: convert remaining shares to underlying (floor) and redeem
    // -----------------------------
    const sharesBeforeAll = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );

    // NOTE: marginfi `juplend_withdraw` always calls `update_rate` internally.
    // To compute the *exact* expected underlying for withdraw_all, we fetch the exchange
    // price after the tx (it is updated in-tx) and use that for the preview math.

    const fTokenBeforeAll = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const userUsdcBeforeAll = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const withdrawAllIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      bank: juplendBank,
      destinationTokenAccount: userA.usdcAccount,
      liquidityVaultAuthority,
      liquidityVault,
      mint: ecosystem.usdcMint.publicKey,
      underlyingOracle: oracles.usdcOracle.publicKey,
      pool,
      fTokenVault,
      claimAccount,
      // withdraw_all ignores `amount` and withdraws the full position.
      amount: new BN(0),
      withdrawAll: true,
      remainingAccounts: remaining,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawAllIx),
      [userA.wallet],
      false,
      true
    );

    const fTokenAfterAll = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    const sharesAfterAll = await fetchMarginfiAssetShares(
      userA.mrgnBankrunProgram!,
      userAcc.publicKey,
      juplendBank
    );
    const userUsdcAfterAll = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    const priceAfterAll = await fetchTokenExchangePrice(pool.lending);
    // redeemable underlying = floor(shares * price / 1e12)
    const redeemableUnderlying = (sharesBeforeAll * priceAfterAll) / EXCHANGE_PRICE_PRECISION;
    assert.isTrue(redeemableUnderlying > 0n, "expected redeemable underlying > 0");

    // Underlying exactness
    assert.equal(
      (userUsdcAfterAll - userUsdcBeforeAll).toString(),
      redeemableUnderlying.toString(),
      "underlying amount mismatch (withdrawAll)"
    );

    // Share accounting: withdrawAll should burn ALL remaining shares
    const burnedAll = fTokenBeforeAll - fTokenAfterAll;
    assert.equal(burnedAll.toString(), sharesBeforeAll.toString(), "burned != sharesBeforeAll");
    assert.equal(sharesAfterAll.toString(), "0", "expected user shares to be 0 after withdrawAll");

    // Bank-level fToken vault should return to baseline (seed deposit only)
    assert.equal(
      fTokenAfterAll.toString(),
      fTokenVaultBaseline.toString(),
      "expected fToken vault back at seed baseline"
    );
  });

  it("matrix: repeated deposit+withdraw-all cycles do not drift", async () => {
    const userAcc = Keypair.generate();
    const initIx = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userAcc.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [userA.wallet, userAcc],
      false,
      true
    );

    const remaining = composeRemainingAccounts([
      juplendHealthRemainingAccounts(
        juplendBank,
        oracles.usdcOracle.publicKey,
        pool.lending
      ),
    ]);

    // Do 3 cycles of: deposit 1 USDC, then redeem all shares.
    for (let i = 0; i < 3; i++) {
      const depositAmt = new BN(1_000_000);

      const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
        group: juplendGroup.publicKey,
        marginfiAccount: userAcc.publicKey,
        authority: userA.wallet.publicKey,
        signerTokenAccount: userA.usdcAccount,
        bank: juplendBank,
        liquidityVaultAuthority,
        liquidityVault,
        fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        amount: depositAmt,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(depositIx),
        [userA.wallet],
        false,
        true
      );

      const shares = await fetchMarginfiAssetShares(
        userA.mrgnBankrunProgram!,
        userAcc.publicKey,
        juplendBank
      );
      assert.isTrue(shares > 0n, "expected >0 shares after deposit");

      const price = await fetchTokenExchangePrice(pool.lending);
      const redeemable = (shares * price) / EXCHANGE_PRICE_PRECISION;
      assert.isTrue(redeemable > 0n, "expected redeemable > 0");

      const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
        group: juplendGroup.publicKey,
        marginfiAccount: userAcc.publicKey,
        authority: userA.wallet.publicKey,
        bank: juplendBank,
        destinationTokenAccount: userA.usdcAccount,
        liquidityVaultAuthority,
        liquidityVault,
        mint: ecosystem.usdcMint.publicKey,
        underlyingOracle: oracles.usdcOracle.publicKey,
        pool,
        fTokenVault,
        claimAccount,
        amount: new BN(redeemable.toString()),
        remainingAccounts: remaining,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(withdrawIx),
        [userA.wallet],
        false,
        true
      );

      const sharesAfter = await fetchMarginfiAssetShares(
        userA.mrgnBankrunProgram!,
        userAcc.publicKey,
        juplendBank
      );
      assert.equal(sharesAfter.toString(), "0", "expected shares=0 after withdraw-all cycle");
    }

    // Bank-level fToken vault should still be baseline seed deposit
    const fTokenFinal = BigInt(await getTokenBalance(bankRunProvider, fTokenVault));
    assert.equal(
      fTokenFinal.toString(),
      fTokenVaultBaseline.toString(),
      "expected fToken vault baseline after cycles"
    );
  });
});
