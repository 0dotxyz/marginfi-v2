import { Program } from "@coral-xyz/anchor";
import { Keypair, Transaction } from "@solana/web3.js";
import { Marginfi } from "../../../target/types/marginfi";
import {
  addBank,
  configureBankOracleScope,
  groupInitialize,
} from "../../utils/group-instructions";
import { pulseBankPrice } from "../../utils/user-instructions";
import {
  bankrunContext,
  bankrunProgram,
  ecosystem,
  groupAdmin,
} from "../../rootHooks";
import { assertI80F48Approx } from "../../utils/genericTests";
import { defaultBankConfig } from "../../utils/types";
import { getBankrunTime } from "../../utils/tools";
import { assert } from "chai";
import * as snapshot from "../../fixtures/scope_mrgn_feed.json";
import { makeScopePrices, setScopeFeed } from "../../utils/scope-utils";

/**
 * Replays a real snapshot of the marginfi scope feed (mainnet `OraclePrices`) through the
 * adapter. Unlike 23, the numbers here are not invented: they are the exact `value`/`exp`
 * pairs the live feed carries for every entry we intend to point a bank at, covering the
 * SplStake rates, the MultiplicationChain USD quotes, the capped hyUSD entry and the
 * DiscountToMaturity PTs. Timestamps are re-stamped at test time so only the price math is
 * under test.
 */

type SnapshotEntry = {
  index: number;
  label: string;
  value: string;
  exp: number;
  expectedPrice: number;
};

const entries: SnapshotEntry[] = snapshot.entries;

const snapshotGroup = Keypair.generate();
const snapshotBank = Keypair.generate();
const feed = Keypair.generate().publicKey;

let program: Program<Marginfi>;

/** value / 10^exp, computed independently of the fixture's rounded `expectedPrice`. */
const decode = (e: SnapshotEntry) => Number(BigInt(e.value)) / 10 ** e.exp;

describe("Scope oracle (live feed snapshot)", () => {
  const writeSnapshot = (timestamp: number) => {
    const data = makeScopePrices(
      entries.map((e) => ({
        index: e.index,
        value: BigInt(e.value),
        exp: BigInt(e.exp),
        timestamp,
      }))
    );
    setScopeFeed(bankrunContext, feed, data);
  };

  before(async () => {
    program = bankrunProgram;
    const admin = groupAdmin.mrgnProgram;

    writeSnapshot(await getBankrunTime(bankrunContext));

    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await groupInitialize(admin, {
          marginfiGroup: snapshotGroup.publicKey,
          admin: groupAdmin.wallet.publicKey,
        })
      ),
      [snapshotGroup]
    );
    await admin.provider.sendAndConfirm!(
      new Transaction().add(
        await addBank(admin, {
          marginfiGroup: snapshotGroup.publicKey,
          feePayer: groupAdmin.wallet.publicKey,
          bankMint: ecosystem.wsolMint.publicKey,
          bank: snapshotBank.publicKey,
          config: defaultBankConfig(),
        })
      ),
      [snapshotBank]
    );
  });

  it("the snapshot is the shape we expect", () => {
    assert.isAbove(entries.length, 40, "snapshot looks truncated");
    for (const e of entries) {
      assert.isBelow(
        e.exp,
        24,
        `${e.label}: exp outside the power-of-ten table`
      );
      assert.isAbove(BigInt(e.value) > 0n ? 1 : 0, 0, `${e.label}: zero value`);
      assert.approximately(
        decode(e),
        e.expectedPrice,
        Math.max(1e-9, e.expectedPrice * 1e-9),
        `${e.label}: fixture expectedPrice disagrees with value/10^exp`
      );
    }
    // Exponents actually exercised by the live feed. If this set grows, the adapter is
    // seeing a shape this suite has not covered.
    const exps = [...new Set(entries.map((e) => e.exp))].sort((a, b) => a - b);
    assert.deepEqual(exps, [8, 9, 15, 17]);
  });

  for (const e of entries) {
    it(`prices entry ${e.index} — ${e.label}`, async () => {
      const admin = groupAdmin.mrgnProgram;
      await admin.provider.sendAndConfirm!(
        new Transaction().add(
          await configureBankOracleScope(admin, {
            bank: snapshotBank.publicKey,
            oracle: feed,
            entryIndex: e.index,
          })
        )
      );
      await admin.provider.sendAndConfirm!(
        new Transaction().add(
          await pulseBankPrice(admin, {
            bank: snapshotBank.publicKey,
            remaining: [feed],
          })
        )
      );

      const bank = await program.account.bank.fetch(snapshotBank.publicKey);
      const expected = decode(e);
      assertI80F48Approx(
        bank.cache.lastOraclePrice,
        expected,
        Math.max(1e-8, expected * 1e-9)
      );
    });
  }
});
