// Acceptance — Flow C step 2 (the brain grows): a multi-period Stripe fixture
// makes end-of-month synthesis flag a spend spike, observable over the brain
// MCP. The anomaly→learning *suppression* (Flow C steps 3–4) needs no CLI entry
// point and stays covered by the Rust unit tests (brain/synthesis suppression);
// this closes the detection half end-to-end.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { callTool, freshHome, haveBin, run, type Env } from "./helpers";

const d = haveBin ? describe : describe.skip;

// 2026-04 baseline (low) then a 2026-05 spike — ratio far over the 1.3 threshold.
const TWO_PERIOD_CSV = `team,period,gross,net,discount_tier,credits,employee_salary
eng,2026-04,10000,8000,Enterprise (Tier 3),2000,240000
ops,2026-04,8000,7000,Enterprise (Tier 3),1000,210000
eng,2026-05,100000,78000,Enterprise (Tier 3),22000,240000
ops,2026-05,30000,26000,Enterprise (Tier 3),4000,210000
`;

d("Flow C — the brain detects a spend spike", () => {
  let env: Env;
  beforeAll(() => {
    env = freshHome();
    // drop a multi-period fixture the Stripe connector will read on ingest
    const dir = join(env.CONTEXTFUL_HOME!, "fixtures", "stripe");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "finance.csv"), TWO_PERIOD_CSV);
    run(env, ["ctl", "seed"]);
    run(env, ["ingest", "--source", "stripe"]); // synthesis runs anomaly detection
  });

  it("detect_anomalies surfaces the spike for an authorized caller", async () => {
    const r = await callTool(env, "agent:cto/1", "brain.detect_anomalies", {
      view: "stripe/spend_by_team",
    });
    const sc = r.structuredContent;
    expect(sc.status).toBe("ok");
    expect(sc.rows.length).toBeGreaterThanOrEqual(1);
    const spike = sc.rows[0];
    expect(spike.period).toBe("2026-05");
    expect(spike.observed).toBeGreaterThan(spike.baseline);
  });
});
