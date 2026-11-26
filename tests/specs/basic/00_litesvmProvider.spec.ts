import { utils } from "@coral-xyz/anchor";
import {
  SystemProgram,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { bankRunProvider, banksClient } from "../../rootHooks";

describe("LiteSVM Anchor provider", () => {
  it("sends a v0 transaction", async () => {
    const payer = bankRunProvider.wallet.publicKey;
    const before = await banksClient.getBalance(payer);
    const blockhash = (await bankRunProvider.connection.getLatestBlockhash())
      .blockhash;
    const tx = new VersionedTransaction(
      new TransactionMessage({
        payerKey: payer,
        recentBlockhash: blockhash,
        instructions: [
          SystemProgram.transfer({
            fromPubkey: payer,
            toPubkey: payer,
            lamports: 1,
          }),
        ],
      }).compileToV0Message()
    );

    const signature = await bankRunProvider.sendAndConfirm(tx);
    assert.lengthOf(utils.bytes.bs58.decode(signature), 64);
    assert.isTrue((await banksClient.getBalance(payer)) < before);
  });

  // TODO v1 support when it becomes available....

  // Note: use bankrunContext.banksClient.simulateTransaction to simulate instead
  it("reports that provider simulation is unsupported", async () => {
    const payer = bankRunProvider.wallet.publicKey;
    const ix = SystemProgram.transfer({
      fromPubkey: payer,
      toPubkey: payer,
      lamports: 1,
    });
    const blockhash = (await bankRunProvider.connection.getLatestBlockhash())
      .blockhash;
    const transactions = [
      new Transaction().add(ix),
      new VersionedTransaction(
        new TransactionMessage({
          payerKey: payer,
          recentBlockhash: blockhash,
          instructions: [ix],
        }).compileToV0Message()
      ),
    ];

    for (const tx of transactions) {
      try {
        await bankRunProvider.simulate(tx);
        assert.fail("Expected simulation to fail");
      } catch (err) {
        assert.include(
          (err as Error).message,
          "Anchor provider simulation is not supported; use BanksClient.simulateTransaction"
        );
      }
    }
  });
});
