// Opt-in CU benchmark (NOT part of any CI slice — lives outside the tests/*.spec.ts globs).
// Run with: anchor run bench-emode
//
// Measures the on-chain compute cost of the two emode reconcile variants via the mocks
// `bench_reconcile_emode` ix, which brackets each call with sol_log_compute_units. The numbers it
// prints are the source for the CU tables in the reconcile_emode_configs* doc-comments
// (type-crate/src/types/emode.rs); re-run this to confirm or refresh them.
import { Transaction } from "@solana/web3.js";
import {
  bankrunContext,
  banksClient,
  groupAdmin,
  mocksBankrunProgram,
} from "../rootHooks";
import { getBankrunBlockhash } from "../utils/tools";
import { assert } from "chai";

describe("emode reconcile CU benchmark", () => {
  const measure = async (
    numConfigs: number
  ): Promise<{ onChain: number; classic: number }> => {
    const tx = new Transaction().add(
      await mocksBankrunProgram.methods
        .benchReconcileEmode(numConfigs)
        .accounts({ payer: groupAdmin.wallet.publicKey })
        .instruction()
    );
    tx.recentBlockhash = await getBankrunBlockhash(bankrunContext);
    tx.sign(groupAdmin.wallet);
    const meta = await banksClient.processTransaction(tx);

    // Each reconcile call is bracketed by a "X units remaining" log line. Four lines total:
    // [0]-[1] = fixed-buffer reconcile_emode_configs, [2]-[3] = reconcile_emode_configs_classic.
    const remaining = (meta.logMessages ?? [])
      .map((l: string) => l.match(/([\d,]+) units remaining/))
      .filter((m): m is RegExpMatchArray => m !== null)
      .map((m) => Number(m[1].replace(/,/g, "")));
    assert.equal(remaining.length, 4, "expected four compute-unit log lines");
    return {
      onChain: remaining[0] - remaining[1],
      classic: remaining[2] - remaining[3],
    };
  };

  it("prints CU for both reconcile variants (worst-case full configs)", async () => {
    console.log(
      "\n  N (configs) | fixed-buffer CU | classic CU | delta (classic - fixed)"
    );
    console.log(
      "  ------------|-----------------|------------|------------------------"
    );
    for (const n of [1, 2, 3, 5, 10]) {
      const { onChain, classic } = await measure(n);
      console.log(
        `  ${String(n).padEnd(11)} | ${String(onChain).padEnd(15)} | ${String(
          classic
        ).padEnd(10)} | ${classic - onChain}`
      );
      // Sanity only — both variants must actually run and consume CU.
      assert.isAbove(onChain, 0);
      assert.isAbove(classic, 0);
    }
    console.log("");
  });
});
