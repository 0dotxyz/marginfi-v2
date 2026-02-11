import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createMintToInstruction } from "@solana/spl-token";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  banksClient,
  ecosystem,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
  verbose,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { accountInit, composeRemainingAccounts, healthPulse } from "./utils/user-instructions";
import { processBankrunTransaction } from "./utils/tools";
import { getBankrunBlockhash } from "./utils/spl-staking-utils";
import { assertBankrunTxFailed, getTokenBalance } from "./utils/genericTests";
import { USER_ACCOUNT } from "./utils/mocks";
import { refreshPullOraclesBankrun } from "./utils/bankrun-oracles";

import { ensureJuplendPoolForMint, ensureJuplendClaimAccount } from "./utils/juplend/juplend-bankrun-builder";
import { juplendUpdateRateIx } from "./utils/juplend/juplend-instructions";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
  makeJuplendWithdrawIx,
} from "./utils/juplend/juplend-test-env";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import { HEALTH_CACHE_HEALTHY } from "./utils/types";

/** deterministic 32 bytes */
const THROWAWAY_GROUP_SEED_JL07 = Buffer.from(
  "JUPLEND_POS_LIMIT_SEED_000001___"
);

describe("jl07: JupLend integration position limit (8 positions)", () => {
  const throwawayGroup = Keypair.fromSeed(THROWAWAY_GROUP_SEED_JL07);
  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let user: typeof users[0];
  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;

  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC
  const USER_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC

  const JUplend_SEEDS: BN[] = Array.from({ length: 9 }, (_, i) => new BN(100 + i));
  const juplendBanks: {
    seed: BN;
    bank: any;
    liquidityVaultAuthority: any;
    liquidityVault: any;
    fTokenVault: any;
    claimAccount: any;
  }[] = [];

  // Address Lookup Table for large transactions (withdraw with 8 positions)
  let lutAddress: PublicKey;

  before(async () => {
    // Access users after root hooks have populated the array
    user = users[0];

    // 1) Ensure JupLend pool exists for USDC
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // 2) Init a throwaway group
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      ),
      [groupAdmin.wallet, throwawayGroup],
      false,
      true
    );

    // 3) Fund groupAdmin + user with USDC
    const mintUsdcTx = new Transaction();
    const mintAmount = new BN(1_000 * 10 ** ecosystem.usdcDecimals).toNumber();
    for (const u of [groupAdmin, user]) {
      mintUsdcTx.add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          u.usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          mintAmount
        )
      );
    }
    await processBankrunTransaction(
      bankrunContext,
      mintUsdcTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // 4) Add + activate 9 Juplend banks (same mint, different bank_seed)
    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    for (const seed of JUplend_SEEDS) {
      const derived = deriveJuplendMrgnAddresses({
        mrgnProgramId: bankrunProgram.programId,
        group: throwawayGroup.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: seed,
        fTokenMint: pool.fTokenMint,
        tokenProgram: pool.tokenProgram,
      });

      juplendBanks.push({
        seed,
        bank: derived.bank,
        liquidityVaultAuthority: derived.liquidityVaultAuthority,
        liquidityVault: derived.liquidityVault,
        fTokenVault: derived.fTokenVault,
        claimAccount: derived.claimAccount,
      });

      const addIx = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram, {
        group: throwawayGroup.publicKey,
        admin: groupAdmin.wallet.publicKey,
        feePayer: groupAdmin.wallet.publicKey,
        bankMint: ecosystem.usdcMint.publicKey,
        bankSeed: seed,
        oracle: oracles.usdcOracle.publicKey,
        integrationAcc1: pool.lending,
        fTokenMint: pool.fTokenMint,
        config,
        tokenProgram: pool.tokenProgram,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(addIx),
        [groupAdmin.wallet],
        false,
        true
      );

      const initPosIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram, {
        feePayer: groupAdmin.wallet.publicKey,
        signerTokenAccount: groupAdmin.usdcAccount,
        bank: derived.bank,
        liquidityVaultAuthority: derived.liquidityVaultAuthority,
        liquidityVault: derived.liquidityVault,
        fTokenVault: derived.fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        seedDepositAmount: SEED_DEPOSIT_AMOUNT,
        tokenProgram: pool.tokenProgram,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(initPosIx),
        [groupAdmin.wallet],
        false,
        true
      );

      // Create claim account for this bank's liquidity_vault_authority (required for withdraw)
      await ensureJuplendClaimAccount({
        payer: groupAdmin.wallet,
        user: derived.liquidityVaultAuthority,
        mint: ecosystem.usdcMint.publicKey,
      });
    }

    // 5) Create Address Lookup Table for large transactions (withdraw with 8 positions)
    // Collect all addresses that will be used in health checks
    const lutAddresses: PublicKey[] = [
      throwawayGroup.publicKey,
      bankrunProgram.programId,
      oracles.usdcOracle.publicKey,
      pool.lending,
      ecosystem.usdcMint.publicKey,
      ...juplendBanks.flatMap((b) => [
        b.bank,
        b.liquidityVaultAuthority,
        b.liquidityVault,
        b.fTokenVault,
        b.claimAccount,
      ]),
    ];

    // Create the LUT
    const recentSlot = Number(await banksClient.getSlot());
    const [createLutIx, createdLutAddress] = AddressLookupTableProgram.createLookupTable({
      authority: groupAdmin.wallet.publicKey,
      payer: groupAdmin.wallet.publicKey,
      recentSlot: recentSlot - 1,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(createLutIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Extend the LUT in chunks of 20 (max per tx)
    const chunkSize = 20;
    for (let i = 0; i < lutAddresses.length; i += chunkSize) {
      const chunk = lutAddresses.slice(i, i + chunkSize);
      const extendLutIx = AddressLookupTableProgram.extendLookupTable({
        authority: groupAdmin.wallet.publicKey,
        payer: groupAdmin.wallet.publicKey,
        lookupTable: createdLutAddress,
        addresses: chunk,
      });
      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(extendLutIx),
        [groupAdmin.wallet],
        false,
        true
      );
    }

    lutAddress = createdLutAddress;

    // Warp slots forward to activate the LUT (required before use)
    const slotsToAdvance = 60 * 0.4; // ~24 slots
    const currentSlot = Number(await banksClient.getSlot());
    bankrunContext.warpToSlot(BigInt(currentSlot + slotsToAdvance));
  });

  it("opens 8 Juplend positions, fails on 9th, then succeeds after closing one", async () => {
    // 1) init marginfi account
    const accountKeypair = Keypair.generate();
    user.accounts.set(USER_ACCOUNT, accountKeypair.publicKey);

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountInit(user.mrgnBankrunProgram, {
          marginfiGroup: throwawayGroup.publicKey,
          marginfiAccount: accountKeypair.publicKey,
          authority: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
        })
      ),
      [user.wallet, accountKeypair],
      false,
      true
    );

    const startingUsdc = await getTokenBalance(bankRunProvider, user.usdcAccount);

    // 2) deposit into first 8 banks (open 8 integration positions)
    for (let i = 0; i < 8; i++) {
      const b = juplendBanks[i];
      const ix = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
        group: throwawayGroup.publicKey,
        marginfiAccount: user.accounts.get(USER_ACCOUNT),
        authority: user.wallet.publicKey,
        signerTokenAccount: user.usdcAccount,
        bank: b.bank,
        liquidityVaultAuthority: b.liquidityVaultAuthority,
        liquidityVault: b.liquidityVault,
        fTokenVault: b.fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        amount: USER_DEPOSIT_AMOUNT,
        tokenProgram: pool.tokenProgram,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [user.wallet],
        false,
        true
      );
    }

    
    // 2b) Health pulse should succeed with 8 Juplend positions,
    // as long as we update_rate in the SAME tx (strict same-slot freshness).
    await refreshPullOraclesBankrun(oracles, bankrunContext, banksClient);

    const remainingForPulse = composeRemainingAccounts(
      juplendBanks.slice(0, 8).map((b) =>
        juplendHealthRemainingAccounts(
          b.bank,
          oracles.usdcOracle.publicKey,
          pool.lending
        )
      )
    );

    const updateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    // Health pulse with 8 positions needs extra compute budget
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 600_000,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        computeBudgetIx,
        updateRateIx,
        await healthPulse(user.mrgnBankrunProgram!, {
          marginfiAccount: user.accounts.get(USER_ACCOUNT),
          remaining: remainingForPulse,
        })
      ),
      [user.wallet],
      false,
      true
    );

    const afterPulse = await user.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      user.accounts.get(USER_ACCOUNT)
    );
    assert.ok(
      afterPulse.healthCache.flags & HEALTH_CACHE_HEALTHY,
      "expected account to be healthy after pulse with 8 Juplend positions"
    );

// 3) 9th deposit should fail with IntegrationPositionLimitExceeded (6212)
    {
      const b = juplendBanks[8];
      const ix = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
        group: throwawayGroup.publicKey,
        marginfiAccount: user.accounts.get(USER_ACCOUNT),
        authority: user.wallet.publicKey,
        signerTokenAccount: user.usdcAccount,
        bank: b.bank,
        liquidityVaultAuthority: b.liquidityVaultAuthority,
        liquidityVault: b.liquidityVault,
        fTokenVault: b.fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        amount: USER_DEPOSIT_AMOUNT,
        tokenProgram: pool.tokenProgram,
      });

      const result = await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [user.wallet],
        true,
        false
      );

      assertBankrunTxFailed(result, 6212);
    }

    // 4) Close one Juplend position (withdraw_all) and retry deposit into 9th
    const bankToClose = juplendBanks[0];

    // For withdraw_all, the position will be closed so we exclude it from remaining accounts.
    // Only include banks 1-7 (the positions that will remain active after withdraw_all).
    const remaining = composeRemainingAccounts(
      juplendBanks.slice(1, 8).map((b) =>
        juplendHealthRemainingAccounts(
          b.bank,
          oracles.usdcOracle.publicKey,
          pool.lending
        )
      )
    );

    const withdrawIx = await makeJuplendWithdrawIx(user.mrgnBankrunProgram!, {
      group: throwawayGroup.publicKey,
      marginfiAccount: user.accounts.get(USER_ACCOUNT),
      authority: user.wallet.publicKey,
      destinationTokenAccount: user.usdcAccount,
      bank: bankToClose.bank,
      liquidityVaultAuthority: bankToClose.liquidityVaultAuthority,
      liquidityVault: bankToClose.liquidityVault,
      fTokenVault: bankToClose.fTokenVault,
      claimAccount: bankToClose.claimAccount,
      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: new BN(0),
      withdrawAll: true,
      remainingAccounts: remaining,
      tokenProgram: pool.tokenProgram,
    });

    // Withdraw with 8 positions exceeds legacy tx size limit.
    // Use versioned transaction with Address Lookup Table (LUT) to compress account refs.
    const lutRaw = await banksClient.getAccount(lutAddress);
    assert.ok(lutRaw, "LUT account should exist");
    const lutState = AddressLookupTableAccount.deserialize(lutRaw.data);
    const lut = new AddressLookupTableAccount({
      key: lutAddress,
      state: lutState,
    });

    // Need update_rate for fresh lending state during health check
    const withdrawUpdateRateIx = juplendUpdateRateIx(
      {
        lending: pool.lending,
        mint: ecosystem.usdcMint.publicKey,
        fTokenMint: pool.fTokenMint,
        supplyTokenReservesLiquidity: pool.tokenReserve,
        rewardsRateModel: pool.lendingRewardsRateModel,
      },
      pool.lendingProgram
    );

    const blockhash = await getBankrunBlockhash(bankrunContext);
    const messageV0 = new TransactionMessage({
      payerKey: user.wallet.publicKey,
      recentBlockhash: blockhash,
      instructions: [computeBudgetIx, withdrawUpdateRateIx, withdrawIx],
    }).compileToV0Message([lut]);

    const versionedTx = new VersionedTransaction(messageV0);
    versionedTx.sign([user.wallet]);

    const result = await banksClient.tryProcessTransaction(versionedTx);
    if (result.result) {
      // Transaction failed - dump logs for debugging
      console.log("[jl07] Withdraw transaction failed:", result.result);
      console.log("[jl07] Logs:", result.meta?.logMessages?.join("\n"));
      throw new Error(`Withdraw failed: ${result.result}`);
    }

    // Verify position is fully closed (withdraw_all sets close_balance: true)
    const mfiAccAfter = await user.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      user.accounts.get(USER_ACCOUNT)
    );
    const balAfter = (mfiAccAfter.lendingAccount.balances as any[]).find(
      (b) => b.active && (b.bankPk as any as PublicKey).equals(bankToClose.bank)
    );
    assert.isUndefined(balAfter, "expected position to be fully closed after withdraw_all");

    // 5) Now 9th deposit should succeed (we freed up a slot)
    {
      const b = juplendBanks[8];
      const ix = await makeJuplendDepositIx(user.mrgnBankrunProgram!, {
        group: throwawayGroup.publicKey,
        marginfiAccount: user.accounts.get(USER_ACCOUNT),
        authority: user.wallet.publicKey,
        signerTokenAccount: user.usdcAccount,
        bank: b.bank,
        liquidityVaultAuthority: b.liquidityVaultAuthority,
        liquidityVault: b.liquidityVault,
        fTokenVault: b.fTokenVault,
        mint: ecosystem.usdcMint.publicKey,
        pool,
        amount: USER_DEPOSIT_AMOUNT,
        tokenProgram: pool.tokenProgram,
      });

      await processBankrunTransaction(
        bankrunContext,
        new Transaction().add(ix),
        [user.wallet],
        false,
        true
      );
    }

    // Verify we now have 8 active positions (7 original + 1 new from 9th bank)
    const mfiAccFinal = await user.mrgnBankrunProgram!.account.marginfiAccount.fetch(
      user.accounts.get(USER_ACCOUNT)
    );
    const activePositions = (mfiAccFinal.lendingAccount.balances as any[]).filter(
      (b) => b.active
    ).length;
    assert.equal(activePositions, 8, "expected 8 active positions after closing one and opening another");

    const endingUsdc = await getTokenBalance(bankRunProvider, user.usdcAccount);
    if (verbose) {
      console.log(`USDC start=${startingUsdc} end=${endingUsdc}`);
    }
    assert.isTrue(
      endingUsdc < startingUsdc,
      "expected user to have spent some USDC across deposits"
    );
  });
});
