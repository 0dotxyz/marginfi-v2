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
import { accountInit } from "./utils/user-instructions";
import { processBankrunTransaction } from "./utils/tools";
import { assertBankrunTxFailed, getTokenBalance } from "./utils/genericTests";
import { ASSET_TAG_JUPLEND } from "./utils/types";

import { ensureJuplendPoolForMint } from "./utils/juplend/juplend-bankrun-builder";
import { defaultJuplendBankConfig } from "./utils/juplend/juplend-utils";
import {
  deriveJuplendMrgnAddresses,
  makeAddJuplendBankIx,
  makeJuplendDepositIx,
  makeJuplendInitPositionIx,
} from "./utils/juplend/juplend-test-env";

/** deterministic 32 bytes */
const GROUP_SEED_JL08 = Buffer.from("JUPLEND_ACTIVATION_GATE_SEED_001");

describe("jl08: JupLend - bank activation gating (paused -> operational)", () => {
  const group = Keypair.fromSeed(GROUP_SEED_JL08);

  const BANK_SEED = new BN(77);
  const SEED_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC (6 decimals)
  const USER_DEPOSIT_AMOUNT = new BN(1_000_000); // 1 USDC

  // NOTE: users array is populated by root hooks, so we must access it after hooks run
  let userA: typeof users[0];

  let pool: Awaited<ReturnType<typeof ensureJuplendPoolForMint>>;

  let userMarginfiAccount: Keypair;
  let juplendBank: PublicKey;
  let liquidityVaultAuthority: PublicKey;
  let liquidityVault: PublicKey;
  let fTokenVault: PublicKey;

  before(async () => {
    // Access users after root hooks have populated the array
    userA = users[0];

    pool = await ensureJuplendPoolForMint({
      admin: groupAdmin.wallet,
      mint: ecosystem.usdcMint.publicKey,
      symbol: "jlUSDC",
      mintAuthority: globalProgramAdmin.wallet,
    });

    // Initialize a fresh marginfi group for this test
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await groupInitialize(groupAdmin.mrgnBankrunProgram, {
          marginfiGroup: group.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      ),
      [groupAdmin.wallet, group],
      false,
      true
    );

    // Create a marginfi account for userA
    userMarginfiAccount = Keypair.generate();
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await accountInit(userA.mrgnBankrunProgram!, {
          marginfiGroup: group.publicKey,
          marginfiAccount: userMarginfiAccount.publicKey,
          authority: userA.wallet.publicKey,
          feePayer: userA.wallet.publicKey,
        })
      ),
      [userA.wallet, userMarginfiAccount],
      false,
      true
    );

    // Add a new Juplend bank, which should start paused.
    const derived = deriveJuplendMrgnAddresses({
      mrgnProgramId: bankrunProgram.programId,
      group: group.publicKey,
      bankMint: ecosystem.usdcMint.publicKey,
      bankSeed: BANK_SEED,
      fTokenMint: pool.fTokenMint,
      tokenProgram: pool.tokenProgram,
    });

    juplendBank = derived.bank;
    liquidityVaultAuthority = derived.liquidityVaultAuthority;
    liquidityVault = derived.liquidityVault;
    fTokenVault = derived.fTokenVault;

    const config = defaultJuplendBankConfig(
      oracles.usdcOracle.publicKey,
      ecosystem.usdcDecimals
    );

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(
        await makeAddJuplendBankIx(groupAdmin.mrgnBankrunProgram!, {
          group: group.publicKey,
          admin: groupAdmin.wallet.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.usdcMint.publicKey,
          bankSeed: BANK_SEED,
          oracle: oracles.usdcOracle.publicKey,
          integrationAcc1: pool.lending,
          fTokenMint: pool.fTokenMint,
          config,
          tokenProgram: pool.tokenProgram,
        })
      ),
      [groupAdmin.wallet],
      false,
      true
    );
  });

  it("deposits are blocked until init_position activates bank", async () => {
    const bankBefore = await bankrunProgram.account.bank.fetch(juplendBank);

    assert.equal(bankBefore.config.assetTag, ASSET_TAG_JUPLEND);
    assert.ok(
      Object.keys(bankBefore.config.operationalState).includes("paused"),
      "expected new juplend bank to start paused"
    );

    // fToken vault is now a PDA created during add_pool (not a permissionless ATA),
    // so the old attack vector of creating the ATA early no longer applies.

    // Deposit should fail with MarginfiError::BankPaused (6016).
    const depositIx = await makeJuplendDepositIx(userA.mrgnBankrunProgram!, {
      group: group.publicKey,
      marginfiAccount: userMarginfiAccount.publicKey,
      authority: userA.wallet.publicKey,
      signerTokenAccount: userA.usdcAccount,

      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      amount: USER_DEPOSIT_AMOUNT,
      tokenProgram: pool.tokenProgram,
    });

    const failed = await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      true,
      false
    );
    assertBankrunTxFailed(failed, 6016);

    // Now activate the bank (creates protocol accounts + seed deposit).
    const initIx = await makeJuplendInitPositionIx(groupAdmin.mrgnBankrunProgram!, {
      feePayer: groupAdmin.wallet.publicKey,
      signerTokenAccount: groupAdmin.usdcAccount,

      bank: juplendBank,
      liquidityVaultAuthority,
      liquidityVault,
      fTokenVault,

      mint: ecosystem.usdcMint.publicKey,
      pool,
      seedDepositAmount: SEED_DEPOSIT_AMOUNT,
      tokenProgram: pool.tokenProgram,
    });

    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(initIx),
      [groupAdmin.wallet],
      false,
      true
    );

    const bankAfterInit = await bankrunProgram.account.bank.fetch(juplendBank);
    assert.ok(
      Object.keys(bankAfterInit.config.operationalState).includes("operational"),
      "expected bank to become operational after init_position"
    );

    // Deposit should now succeed.
    const usdcBefore = await getTokenBalance(bankRunProvider, userA.usdcAccount);
    await processBankrunTransaction(
      bankrunContext,
      new Transaction().add(depositIx),
      [userA.wallet],
      false,
      true
    );
    const usdcAfter = await getTokenBalance(bankRunProvider, userA.usdcAccount);

    assert.equal(
      usdcBefore - usdcAfter,
      USER_DEPOSIT_AMOUNT.toNumber(),
      "user USDC should decrease by deposit amount after activation"
    );
  });
});
