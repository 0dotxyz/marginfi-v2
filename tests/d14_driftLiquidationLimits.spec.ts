import { assert } from "chai";
import {
  setupDriftLiqEnv,
  DriftLiqEnv,
  runLiquidationScenario,
  ScenarioResult,
  PASSING_DRIFT_COUNT,
} from "./utils/drift-liq-helpers";

describe("d14: Drift liquidation limits", () => {
  let env: DriftLiqEnv;

  before(async function () {
    this.timeout(300_000);

    // Create all 31 banks + LUT once
    env = await setupDriftLiqEnv();

    // Verify setup (PASSING_DRIFT_COUNT + 1 drift banks for the failing test)
    assert.equal(
      env.driftBanks.length,
      PASSING_DRIFT_COUNT + 1,
      `Should have ${PASSING_DRIFT_COUNT + 1} drift banks`
    );
    assert.equal(env.regularBanks.length, 15, "Should have 15 regular banks");
    assert.ok(env.liabBank, "Should have 1 liab bank");
    assert.ok(env.lutAddress, "Should have LUT address");
  });

  it("tests liquidation across drift/regular combinations", async function () {
    this.timeout(600_000); // 10 minutes for all scenarios

    const results: ScenarioResult[] = [];

    // Test combinations: driftCount from 1 to PASSING_DRIFT_COUNT, regularCount = 15 - driftCount
    // Total positions = driftCount + regularCount + 1 borrow = 16
    for (let driftCount = 1; driftCount <= PASSING_DRIFT_COUNT; driftCount++) {
      const regularCount = 15 - driftCount;
      const result = await runLiquidationScenario(env, {
        driftCount,
        regularCount,
        scenarioIndex: driftCount - 1,
      });
      results.push(result);
    }

    // Print summary table
    console.log("\n" + "=".repeat(80));
    console.log("📊 LIQUIDATION LIMITS SUMMARY");
    console.log("=".repeat(80));
    console.log(
      "Drift | Regular | Borrow | Total | Status     | CU Used    | Error"
    );
    console.log("-".repeat(80));

    for (const r of results) {
      const status = r.success ? "SUCCESS" : "FAILED";
      const cu = r.cuUsed
        ? r.cuUsed.toString().padStart(10)
        : "N/A".padStart(10);
      const error = r.errorType || "-";
      const total = r.driftCount + r.regularCount + 1;
      console.log(
        `${r.driftCount.toString().padStart(5)} | ` +
          `${r.regularCount.toString().padStart(7)} | ` +
          `${"1".padStart(6)} | ` +
          `${total.toString().padStart(5)} | ` +
          `${status.padEnd(10)} | ` +
          `${cu} | ` +
          `${error}`
      );
    }
    console.log("=".repeat(80));

    // Verify we ran all scenarios
    assert.equal(
      results.length,
      PASSING_DRIFT_COUNT,
      `Should have run ${PASSING_DRIFT_COUNT} scenarios`
    );
  });

  it("fails with too many drift positions due to active bank limit", async function () {
    this.timeout(120_000);

    const driftCount = PASSING_DRIFT_COUNT + 1;
    const regularCount = 15 - driftCount;

    try {
      await runLiquidationScenario(env, {
        driftCount,
        regularCount,
        scenarioIndex: 99,
      });
      assert.fail("Should have thrown IntegrationPositionLimitExceeded");
    } catch (e: any) {
      // 0x1844 = 6212 = IntegrationPositionLimitExceeded
      assert.ok(
        e.message.includes("0x1844"),
        `Expected error 0x1844 (IntegrationPositionLimitExceeded), got: ${e.message}`
      );
    }
  });
});
