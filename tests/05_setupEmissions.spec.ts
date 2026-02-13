import { BN, Program } from "@coral-xyz/anchor";
import { BankrunProvider } from "anchor-bankrun";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  groupConfigure,
  setupEmissions,
  updateEmissions,
} from "./utils/group-instructions";
import { Marginfi } from "../target/types/marginfi";
import {
  bankKeypairUsdc,
  bankrunContext,
  bankrunProgram,
  bankRunProvider,
  ecosystem,
  groupAdmin,
  marginfiGroup,
  users,
  verbose,
} from "./rootHooks";
import {
  assertBNEqual,
  assertI80F48Approx,
  assertKeyDefault,
  assertKeysEqual,
  getTokenBalance,
} from "./utils/genericTests";
import { assert } from "chai";
import {
  EMISSIONS_FLAG_BORROW_ACTIVE,
  EMISSIONS_FLAG_LENDING_ACTIVE,
  CLOSE_ENABLED_FLAG,
} from "./utils/types";
import { createMintToInstruction } from "@solana/spl-token";
import { deriveEmissionsAuth, deriveEmissionsTokenAccount } from "./utils/pdas";

let program: Program<Marginfi>;
let mintAuthority: PublicKey;
let provider: BankrunProvider;

describe("Lending pool set up emissions", () => {
  before(() => {
    provider = bankRunProvider;
    program = bankrunProgram;
    mintAuthority = bankrunContext.payer.publicKey;
  });

  const emissionRate = new BN(500_000 * 10 ** ecosystem.tokenBDecimals);
  const totalEmissions = new BN(1_000_000 * 10 ** ecosystem.tokenBDecimals);

  it("(admin) Set user 1 as the emissions admin - happy path", async () => {
    const groupBefore = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assertKeyDefault(groupBefore.delegateEmissionsAdmin);
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await groupConfigure(groupAdmin.mrgnProgram, {
          newEmissionsAdmin: users[1].wallet.publicKey,
          marginfiGroup: marginfiGroup.publicKey,
        })
      )
    );
    const groupAfter = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assertKeysEqual(
      groupAfter.delegateEmissionsAdmin,
      users[1].wallet.publicKey
    );
  });

  it("Mint token B to the emissions admin for funding emissions", async () => {
    const emissionsAdmin = users[1];
    let tx: Transaction = new Transaction();
    tx.add(
      createMintToInstruction(
        ecosystem.tokenBMint.publicKey,
        emissionsAdmin.tokenBAccount,
        mintAuthority,
        BigInt(100_000_000) * BigInt(10 ** ecosystem.tokenBDecimals)
      )
    );
    await provider.sendAndConfirm(tx);
  });

  it("(user 1) Set up to token B emissions on (USDC) bank - happy path", async () => {
    const emissionsAdmin = users[1];
    const adminBBefore = await getTokenBalance(
      provider,
      emissionsAdmin.tokenBAccount
    );
    const [emissionsAccKey] = deriveEmissionsTokenAccount(
      program.programId,
      bankKeypairUsdc.publicKey,
      ecosystem.tokenBMint.publicKey
    );
    // Note: an uninitialized account that does nothing...
    const [emissionsAuthKey] = deriveEmissionsAuth(
      program.programId,
      bankKeypairUsdc.publicKey,
      ecosystem.tokenBMint.publicKey
    );

    // Snapshot flags before to verify non-emission flags are preserved
    const bankBefore = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const flagsBefore = bankBefore.flags.toNumber();
    const emissionBitsMask = EMISSIONS_FLAG_BORROW_ACTIVE | EMISSIONS_FLAG_LENDING_ACTIVE;
    const nonEmissionFlagsBefore = flagsBefore & ~emissionBitsMask;

    await emissionsAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await setupEmissions(emissionsAdmin.mrgnProgram, {
          bank: bankKeypairUsdc.publicKey,
          emissionsMint: ecosystem.tokenBMint.publicKey,
          fundingAccount: emissionsAdmin.tokenBAccount,
          emissionsFlags: new BN(
            EMISSIONS_FLAG_BORROW_ACTIVE + EMISSIONS_FLAG_LENDING_ACTIVE
          ),
          emissionsRate: emissionRate,
          totalEmissions: totalEmissions,
        })
      )
    );

    if (verbose) {
      console.log("Started token B borrow/lending emissions on USDC bank");
    }

    const [bank, adminBAfter, emissionsAccAfter] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      getTokenBalance(provider, emissionsAdmin.tokenBAccount),
      getTokenBalance(provider, emissionsAccKey),
    ]);

    const flagsAfter = bank.flags.toNumber();
    const nonEmissionFlagsAfter = flagsAfter & ~emissionBitsMask;

    assertKeysEqual(bank.emissionsMint, ecosystem.tokenBMint.publicKey);
    assertBNEqual(bank.emissionsRate, emissionRate);
    assertI80F48Approx(bank.emissionsRemaining, totalEmissions);

    // Emission bits should be set
    assert.equal(
      flagsAfter & emissionBitsMask,
      emissionBitsMask,
      "emission flags (borrow + lending) should be set"
    );
    // All non-emission flags must survive unchanged (e.g. CLOSE_ENABLED_FLAG, FREEZE_SETTINGS)
    assert.equal(
      nonEmissionFlagsAfter,
      nonEmissionFlagsBefore,
      `non-emission flags changed! before: 0b${nonEmissionFlagsBefore.toString(2)}, after: 0b${nonEmissionFlagsAfter.toString(2)}`
    );
    // Sanity: CLOSE_ENABLED_FLAG should still be on (set at bank creation)
    assert.notEqual(
      flagsAfter & CLOSE_ENABLED_FLAG,
      0,
      "CLOSE_ENABLED_FLAG must survive emissions setup"
    );

    assert.equal(adminBBefore - adminBAfter, totalEmissions.toNumber());
    assert.equal(emissionsAccAfter, totalEmissions.toNumber());
  });

  it("(user 1) Add more token B emissions on (USDC) bank - happy path", async () => {
    const emissionsAdmin = users[1];
    const [emissionsAccKey] = deriveEmissionsTokenAccount(
      program.programId,
      bankKeypairUsdc.publicKey,
      ecosystem.tokenBMint.publicKey
    );

    // Snapshot flags before to verify non-emission flags are preserved
    const bankBefore = await program.account.bank.fetch(bankKeypairUsdc.publicKey);
    const flagsBefore = bankBefore.flags.toNumber();
    const emissionBitsMask = EMISSIONS_FLAG_BORROW_ACTIVE | EMISSIONS_FLAG_LENDING_ACTIVE;
    const nonEmissionFlagsBefore = flagsBefore & ~emissionBitsMask;

    const [adminBBefore, emissionsAccBefore] = await Promise.all([
      getTokenBalance(provider, emissionsAdmin.tokenBAccount),
      getTokenBalance(provider, emissionsAccKey),
    ]);

    // Note: emissionsFlags is null here — no flag change requested
    await emissionsAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await updateEmissions(emissionsAdmin.mrgnProgram, {
          bank: bankKeypairUsdc.publicKey,
          emissionsMint: ecosystem.tokenBMint.publicKey,
          fundingAccount: emissionsAdmin.tokenBAccount,
          emissionsFlags: null,
          emissionsRate: null,
          additionalEmissions: totalEmissions,
        })
      )
    );

    const [bank, adminBAfter, emissionsAccAfter] = await Promise.all([
      program.account.bank.fetch(bankKeypairUsdc.publicKey),
      getTokenBalance(provider, emissionsAdmin.tokenBAccount),
      getTokenBalance(provider, emissionsAccKey),
    ]);

    const flagsAfter = bank.flags.toNumber();
    const nonEmissionFlagsAfter = flagsAfter & ~emissionBitsMask;

    assertKeysEqual(bank.emissionsMint, ecosystem.tokenBMint.publicKey);
    assertBNEqual(bank.emissionsRate, emissionRate);
    assertI80F48Approx(bank.emissionsRemaining, totalEmissions.muln(2));

    // Emission bits should still be set from the previous test
    assert.equal(
      flagsAfter & emissionBitsMask,
      emissionBitsMask,
      "emission flags (borrow + lending) should still be set"
    );
    // All non-emission flags must survive unchanged
    assert.equal(
      nonEmissionFlagsAfter,
      nonEmissionFlagsBefore,
      `non-emission flags changed! before: 0b${nonEmissionFlagsBefore.toString(2)}, after: 0b${nonEmissionFlagsAfter.toString(2)}`
    );
    // Sanity: CLOSE_ENABLED_FLAG should still be on
    assert.notEqual(
      flagsAfter & CLOSE_ENABLED_FLAG,
      0,
      "CLOSE_ENABLED_FLAG must survive emissions update"
    );

    assert.equal(adminBBefore - adminBAfter, totalEmissions.toNumber());
    assert.equal(
      emissionsAccAfter,
      emissionsAccBefore + totalEmissions.toNumber()
    );
  });
});
