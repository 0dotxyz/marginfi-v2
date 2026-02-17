import { BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import {
  createAssociatedTokenAccountInstruction,
  createInitializeMint2Instruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  getMintLen,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import {
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  globalProgramAdmin,
  groupAdmin,
  oracles,
  users,
} from "./rootHooks";

import { groupInitialize } from "./utils/group-instructions";
import { getTokenBalance } from "./utils/genericTests";
import { processBankrunTransaction } from "./utils/tools";
import { accountInit } from "./utils/user-instructions";

import {
  ensureJuplendPoolForMint,
  ensureJuplendClaimAccount,
} from "./utils/juplend/juplend-bankrun-builder";
import {
  deriveJuplendMrgnAddresses,
  juplendHealthRemainingAccounts,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
  makeJuplendWithdrawIx,
} from "./utils/juplend/juplend-test-env";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";

/**
 * jl11: JupLend Token-2022 Smoke Test
 *
 * Tests that JupLend deposit/withdraw works correctly with Token-2022 mints.
 * Creates a basic Token-2022 mint (no extensions) and verifies core operations.
 */
describe("jl11: JupLend - Token-2022 smoke test", () => {
  // Deterministic seeds
  const GROUP_SEED = Buffer.from("JUPLEND_GROUP_SEED_0000000000011");
  const TOKEN2022_MINT_SEED = Buffer.from("TOKEN2022_MINT_SEED_000000000000");

  const BANK_SEED = new BN(11);
  const TOKEN_DECIMALS = 6;
  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 token
  const USER_DEPOSIT_AMOUNT = new BN(100_000_000); // 100 tokens
  const USER_WITHDRAW_AMOUNT = new BN(50_000_000); // 50 tokens

  const juplendGroup = Keypair.fromSeed(GROUP_SEED);
  const token2022Mint = Keypair.fromSeed(TOKEN2022_MINT_SEED);

  let userA: (typeof users)[0];
  let userToken2022Ata: PublicKey;
  let adminToken2022Ata: PublicKey;
  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;

  let userMarginfiAccount: Keypair;
  let juplendBank: PublicKey;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;
  let claimAccount: PublicKey;

  before(async () => {
    userA = users[0];

    // =========================================================================
    // Step 1: Create Token-2022 mint
    // =========================================================================
    const mintLen = getMintLen([]);
    const rent = await bankRunProvider.connection.getMinimumBalanceForRentExemption(mintLen);

    const createMintTx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: globalProgramAdmin.wallet.publicKey,
        newAccountPubkey: token2022Mint.publicKey,
        space: mintLen,
        lamports: rent,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeMint2Instruction(
        token2022Mint.publicKey,
        TOKEN_DECIMALS,
        globalProgramAdmin.wallet.publicKey, // mint authority
        globalProgramAdmin.wallet.publicKey, // freeze authority
        TOKEN_2022_PROGRAM_ID
      )
    );

    await processBankrunTransaction(
      bankrunContext,
      createMintTx,
      [globalProgramAdmin.wallet, token2022Mint],
      false,
      true
    );

    // =========================================================================
    // Step 2: Create ATAs for admin and user
    // =========================================================================
    adminToken2022Ata = getAssociatedTokenAddressSync(
      token2022Mint.publicKey,
      groupAdmin.wallet.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );

    userToken2022Ata = getAssociatedTokenAddressSync(
      token2022Mint.publicKey,
      userA.wallet.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );

    const createAtasTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        globalProgramAdmin.wallet.publicKey,
        adminToken2022Ata,
        groupAdmin.wallet.publicKey,
        token2022Mint.publicKey,
        TOKEN_2022_PROGRAM_ID
      ),
      createAssociatedTokenAccountInstruction(
        globalProgramAdmin.wallet.publicKey,
        userToken2022Ata,
        userA.wallet.publicKey,
        token2022Mint.publicKey,
        TOKEN_2022_PROGRAM_ID
      )
    );

    await processBankrunTransaction(
      bankrunContext,
      createAtasTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // =========================================================================
    // Step 3: Mint tokens to admin (for seed deposit) and user (for testing)
    // =========================================================================
    const mintToTx = new Transaction().add(
      createMintToInstruction(
        token2022Mint.publicKey,
        adminToken2022Ata,
        globalProgramAdmin.wallet.publicKey,
        10_000_000_000, // 10,000 tokens for admin
        [],
        TOKEN_2022_PROGRAM_ID
      ),
      createMintToInstruction(
        token2022Mint.publicKey,
        userToken2022Ata,
        globalProgramAdmin.wallet.publicKey,
        10_000_000_000, // 10,000 tokens for user
        [],
        TOKEN_2022_PROGRAM_ID
      )
    );

    await processBankrunTransaction(
      bankrunContext,
      mintToTx,
      [globalProgramAdmin.wallet],
      false,
      true
    );

    // =========================================================================
    // Step 4: Create JupLend pool for Token-2022 mint
    // =========================================================================
    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: token2022Mint.publicKey,
      symbol: "jlT22",
      mintAuthority: globalProgramAdmin.wallet,
    });
  });

  // ===========================================================================
  // SETUP TESTS
  // ===========================================================================

  it("setup: init group", async () => {
    const ix = await groupInitialize(groupAdmin.mrgnBankrunProgram, {
      marginfiGroup: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [groupAdmin.wallet, juplendGroup],
      false,
      true
    );
  });

  it("setup: init user marginfi account", async () => {
    userMarginfiAccount = Keypair.generate();
    const ix = await accountInit(userA.mrgnBankrunProgram!, {
      marginfiGroup: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      feePayer: userA.wallet.publicKey,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [userA.wallet, userMarginfiAccount],
      false,
      true
    );
  });

  it("setup: add Token-2022 JupLend bank", async () => {
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: juplendGroup.publicKey,
      bankMint: token2022Mint.publicKey,
      bankSeed: BANK_SEED,
      fTokenMint: pool.fTokenMint,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;
    claimAccount = derived.claimAccount;

    // Use tokenA oracle for price (price doesn't matter for smoke test)
    const config = defaultJuplendBankConfig(oracles.tokenAOracle.publicKey, TOKEN_DECIMALS);

    const ix = await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      admin: groupAdmin.wallet.publicKey,
      feePayer: groupAdmin.wallet.publicKey,
      bankMint: token2022Mint.publicKey,
      bankSeed: BANK_SEED,
      oracle: oracles.tokenAOracle.publicKey,
      jupLendingState: pool.lending,
      fTokenMint: pool.fTokenMint,
      config,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(ix),
      [groupAdmin.wallet],
      false,
      true
    );
  });

  it("setup: init position (seed deposit)", async () => {
    const initIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram!, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: adminToken2022Ata,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      mint: token2022Mint.publicKey,
      pool,
      fTokenVault,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [groupAdmin.wallet],
      false,
      true
    );

    // Create claim account for withdraws
    await ensureJuplendClaimAccount({
      payer: groupAdmin.wallet,
      user: liquidityVaultAuthority,
      mint: token2022Mint.publicKey,
    });
  });

  // ===========================================================================
  // DEPOSIT/WITHDRAW TESTS
  // ===========================================================================

  it("deposit Token-2022 tokens", async () => {
    const userBalBefore = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));

    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userToken2022Ata,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,
      mint: token2022Mint.publicKey,
      pool,
      amount: USER_DEPOSIT_AMOUNT,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );

    const userBalAfter = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));
    const deposited = userBalBefore - userBalAfter;

    assert.equal(
      deposited.toString(),
      USER_DEPOSIT_AMOUNT.toString(),
      "user should have deposited correct amount"
    );
  });

  it("partial withdraw Token-2022 tokens", async () => {
    const userBalBefore = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));

    // Partial withdraws need health remaining accounts (bank, oracle, lending state)
    const healthAccounts = juplendHealthRemainingAccounts(
      juplendBank,
      oracles.tokenAOracle.publicKey,
      pool.lending
    );

    const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      destinationTokenAccount: userToken2022Ata,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      mint: token2022Mint.publicKey,
      underlyingOracle: oracles.tokenAOracle.publicKey,
      pool,
      fTokenVault,
      claimAccount,
      amount: USER_WITHDRAW_AMOUNT,
      withdrawAll: false,
      remainingAccounts: healthAccounts,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawIx),
      [userA.wallet],
      false,
      true
    );

    const userBalAfter = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));
    const withdrawn = userBalAfter - userBalBefore;

    assert.equal(
      withdrawn.toString(),
      USER_WITHDRAW_AMOUNT.toString(),
      "user should have withdrawn correct amount"
    );
  });

  it("withdrawAll remaining Token-2022 tokens", async () => {
    const userBalBefore = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));

    const withdrawIx = await makeJuplendWithdrawIx(userA.mrgnBankrunProgram!, {
      group: juplendGroup.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      destinationTokenAccount: userToken2022Ata,
      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      mint: token2022Mint.publicKey,
      underlyingOracle: oracles.tokenAOracle.publicKey,
      pool,
      fTokenVault,
      claimAccount,
      amount: new BN(0),
      withdrawAll: true,
      remainingAccounts: [], // Position closes, no remaining accounts needed
      tokenProgram: TOKEN_2022_PROGRAM_ID,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(withdrawIx),
      [userA.wallet],
      false,
      true
    );

    const userBalAfter = BigInt(await getTokenBalance(bankRunProvider, userToken2022Ata));
    const withdrawn = userBalAfter - userBalBefore;

    // Should have withdrawn approximately the remaining balance (50 tokens minus any dust)
    // At 1:1 exchange rate, should be exactly 50 tokens
    const expectedRemaining = BigInt(USER_DEPOSIT_AMOUNT.toString()) - BigInt(USER_WITHDRAW_AMOUNT.toString());
    assert.equal(
      withdrawn.toString(),
      expectedRemaining.toString(),
      "withdrawAll should return remaining balance"
    );
  });
});
