import { ensureMultiSuiteIntegrationsSetup } from "./utils/multi-limits-setup";

describe("m00: Multi-suite setup", () => {
  it("initializes Drift/Kamino prerequisites for m03/m04", async () => {
    await ensureMultiSuiteIntegrationsSetup();
  });
});
