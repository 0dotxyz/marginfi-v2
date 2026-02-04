import { PublicKey, SYSVAR_RENT_PUBKEY, Transaction } from "@solana/web3.js";
import {
  kaminoAccounts,
  MARKET,
  users,
  verbose,
  bankrunContext,
  klendBankrunProgram,
  ecosystem,
  globalProgramAdmin,
} from "./rootHooks";
import { deriveObligation, deriveUserMetadata } from "./utils/pdas";
import { SYSTEM_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/native/system";
import { KAMINO_METADATA, KAMINO_OBLIGATION } from "./utils/mocks";
import { processBankrunTransaction } from "./utils/tools";
import { ProgramTestContext } from "solana-bankrun";
import { createMintToInstruction } from "@solana/spl-token";
import { InitObligationArgs } from "@kamino-finance/klend-sdk/dist/idl_codegen/types";

let ctx: ProgramTestContext;

describe("k02: Init Kamino user", () => {
  before(async () => {
    ctx = bankrunContext;
  });

  it("Fund user USDC/Token A token accounts", async () => {
    let tx = new Transaction();

    for (let i = 0; i < users.length; i++) {
      tx.add(
        createMintToInstruction(
          ecosystem.tokenAMint.publicKey,
          users[i].tokenAAccount,
          globalProgramAdmin.wallet.publicKey,
          1_000_000 * 10 ** ecosystem.tokenADecimals
        )
      );
      tx.add(
        createMintToInstruction(
          ecosystem.usdcMint.publicKey,
          users[i].usdcAccount,
          globalProgramAdmin.wallet.publicKey,
          100_000_000 * 10 ** ecosystem.usdcDecimals
        )
      );
    }

    await processBankrunTransaction(ctx, tx, [globalProgramAdmin.wallet]);
  });

  it("(user 0/1/2) Init user metadata - happy path", async () => {
    await initUserMetadata(0);
    await initUserMetadata(1);
    await initUserMetadata(2);
  });

  it("(user 0/1/2) Init Kamino obligation - happy path", async () => {
    const initObligationArgs = new InitObligationArgs({ tag: 0, id: 0 });
    await initObligation(0, initObligationArgs);
    await initObligation(1, initObligationArgs);
    await initObligation(2, initObligationArgs);
  });

  async function initUserMetadata(userIndex: number) {
    const user = users[userIndex];

    // We can simply use an empty address for testing
    const dummyLookupTableAddress = new PublicKey(Buffer.alloc(32));

    const [metadataKey] = deriveUserMetadata(
      klendBankrunProgram.programId,
      user.wallet.publicKey
    );

    let tx = new Transaction().add(
      await klendBankrunProgram.methods
        .initUserMetadata(dummyLookupTableAddress)
        .accounts({
          owner: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
          userMetadata: metadataKey,
          referrerUserMetadata: null,
          rent: SYSVAR_RENT_PUBKEY,
          systemProgram: SYSTEM_PROGRAM_ID,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [user.wallet]);
    if (verbose) {
      console.log(`user ${userIndex} metadata: ${metadataKey}`);
    }
    user.accounts.set(KAMINO_METADATA, metadataKey);
  }

  async function initObligation(userIndex: number, args: InitObligationArgs) {
    const user = users[userIndex];

    const [obligationKey] = deriveObligation(
      klendBankrunProgram.programId,
      args.tag,
      args.id,
      user.wallet.publicKey,
      kaminoAccounts.get(MARKET),
      PublicKey.default,
      PublicKey.default
    );

    let tx = new Transaction().add(
      await klendBankrunProgram.methods
        .initObligation(args)
        .accounts({
          obligationOwner: user.wallet.publicKey,
          feePayer: user.wallet.publicKey,
          obligation: obligationKey,
          lendingMarket: kaminoAccounts.get(MARKET),
          // Note: seed requirements for non-zero tag vary, see the ix for details
          seed1Account: args.tag === 0 ? PublicKey.default : PublicKey.unique(),
          seed2Account: args.tag === 0 ? PublicKey.default : PublicKey.unique(),
          ownerUserMetadata: user.accounts.get(KAMINO_METADATA),
          rent: SYSVAR_RENT_PUBKEY,
          systemProgram: SYSTEM_PROGRAM_ID,
        })
        .instruction()
    );

    await processBankrunTransaction(ctx, tx, [user.wallet]);
    if (verbose) {
      console.log(`user ${userIndex} obligation: ${obligationKey}`);
    }
    user.accounts.set(KAMINO_OBLIGATION, obligationKey);
  }
});
