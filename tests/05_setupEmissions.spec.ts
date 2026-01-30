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

    assertKeysEqual(bank.emissionsMint, ecosystem.tokenBMint.publicKey);
    assertBNEqual(bank.emissionsRate, emissionRate);
    assertI80F48Approx(bank.emissionsRemaining, totalEmissions);
    assertBNEqual(
      bank.flags,
      new BN(EMISSIONS_FLAG_BORROW_ACTIVE + EMISSIONS_FLAG_LENDING_ACTIVE)
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
    const [adminBBefore, emissionsAccBefore] = await Promise.all([
      getTokenBalance(provider, emissionsAdmin.tokenBAccount),
      getTokenBalance(provider, emissionsAccKey),
    ]);

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

    assertKeysEqual(bank.emissionsMint, ecosystem.tokenBMint.publicKey);
    assertBNEqual(bank.emissionsRate, emissionRate);
    assertI80F48Approx(bank.emissionsRemaining, totalEmissions.muln(2));
    assertBNEqual(
      bank.flags,
      new BN(EMISSIONS_FLAG_BORROW_ACTIVE + EMISSIONS_FLAG_LENDING_ACTIVE)
    );
    assert.equal(adminBBefore - adminBAfter, totalEmissions.toNumber());
    assert.equal(
      emissionsAccAfter,
      emissionsAccBefore + totalEmissions.toNumber()
    );
  });
});
