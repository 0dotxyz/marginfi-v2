import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { groupConfigure } from "./utils/group-instructions";
import { Marginfi } from "../target/types/marginfi";
import { bankrunProgram, groupAdmin, marginfiGroup } from "./rootHooks";
import { assertKeysEqual } from "./utils/genericTests";

let program: Program<Marginfi>;

describe("Config group", () => {
  before(() => {
    program = bankrunProgram;
  });

  it("(admin) Config group - no change", async () => {
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await groupConfigure(groupAdmin.mrgnProgram, {
          newAdmin: null,
          marginfiGroup: marginfiGroup.publicKey,
        })
      )
    );

    let group = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
  });

  it("(admin) Config group - set new admin", async () => {
    let newAdmin = Keypair.generate();
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await groupConfigure(groupAdmin.mrgnProgram, {
          newAdmin: newAdmin.publicKey,
          marginfiGroup: marginfiGroup.publicKey,
        })
      )
    );

    let group = await program.account.marginfiGroup.fetch(
      marginfiGroup.publicKey
    );
    assertKeysEqual(group.admin, newAdmin.publicKey);

    // Restore original
    await groupAdmin.mrgnProgram.provider.sendAndConfirm!(
      new Transaction().add(
        await groupAdmin.mrgnProgram.methods
          .marginfiGroupConfigure(
            groupAdmin.wallet.publicKey,
            PublicKey.default,
            PublicKey.default,
            PublicKey.default,
            PublicKey.default,
            PublicKey.default,
            PublicKey.default,
            null,
            null
          )
          .accountsPartial({
            marginfiGroup: marginfiGroup.publicKey,
            admin: newAdmin.publicKey,
          })
          .instruction()

        // Note: Fails because admin is incorrectly implied, since it's now newAdmin, and we have
        // groupAdmin signing for the above instead:

        // await groupConfigure(groupAdmin.mrgnProgram, {
        //   newAdmin: groupAdmin.wallet.publicKey,
        //   marginfiGroup: marginfiGroup.publicKey,
        // })
      ),
      [newAdmin]
    );

    group = await program.account.marginfiGroup.fetch(marginfiGroup.publicKey);
    assertKeysEqual(group.admin, groupAdmin.wallet.publicKey);
  });
});
