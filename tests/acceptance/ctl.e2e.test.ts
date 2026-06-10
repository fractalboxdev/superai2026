// Acceptance — the control plane (spec 07 §3) via the `ctl` CLI: seed, show,
// mint, revoke, grant. Asserts the no-super-root catalog and the salary
// invariant at the grant boundary.

import { beforeAll, describe, expect, it } from "vitest";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { freshHome, haveBin, run, type Env } from "./helpers";

const d = haveBin ? describe : describe.skip;

d("control plane (ctl)", () => {
  let env: Env;
  beforeAll(() => {
    env = freshHome();
    run(env, ["ctl", "seed"]);
  });

  it("seed lays down principals, the no-super-root catalog, and envelopes", () => {
    const out = run(env, ["ctl", "show"]);
    expect(out).toContain("agent:cto/1");
    expect(out).toContain("cfo");
    expect(out).toContain("cfo owns [stripe/finance_private, stripe/spend_by_team]");
    expect(out).toContain("control-plane owns []"); // control plane holds NO data views
    expect(out).toContain("employee_salary"); // CFO envelope never_delegate
  });

  it("seed persists a token file per principal under caps/", () => {
    const home = env.CONTEXTFUL_HOME!;
    expect(existsSync(join(home, "caps", "agent_cto_1.json"))).toBe(true);
    expect(existsSync(join(home, "caps", "cfo.json"))).toBe(true);
  });

  it("mint re-issues a principal's initial token", () => {
    const out = run(env, ["ctl", "mint", "--principal", "agent:cto/1"]);
    expect(out).toMatch(/minted/i);
  });

  it("revoke records the principal in the revocation list", () => {
    run(env, ["ctl", "revoke", "--principal", "agent:eng/1"]);
    const out = run(env, ["ctl", "show"]);
    expect(out).toContain("revoked");
    expect(out).toContain("agent:eng/1");
  });

  it("grant approves a scoped, salary-free token", () => {
    const out = run(env, [
      "ctl",
      "grant",
      "--to",
      "agent:cto/1",
      "--view",
      "stripe/finance_private",
      "--fields",
      "gross,credits,discount_tier",
    ]);
    expect(out).toMatch(/granted agent:cto\/1/);
    expect(out).toMatch(/salary always denied/);
  });

  it("grant of employee_salary is refused (salary invariant)", () => {
    expect(() =>
      run(env, [
        "ctl",
        "grant",
        "--to",
        "agent:eng/1",
        "--view",
        "stripe/finance_private",
        "--fields",
        "employee_salary",
      ]),
    ).toThrow();
  });
});
