import { describe, it, expect } from "vitest";
import {
  attenuate,
  delegateTo,
  effectiveCapability,
  mint,
  NoRootAuthority,
} from "./access";
import { brainQuery } from "./brain";
import {
  approveRequest,
  routeRequest,
  SalaryInvariantViolation,
} from "./requests";
import {
  CFO_ENVELOPE,
  CFO_ROOT,
  CONTROL_PLANE_ROOT,
  ctoAgentCapability,
  cfoCapability,
  DATASETS,
  engAgentCapability,
  FINANCE_PRIVATE,
  FLOW_A_REQUEST,
  FLOW_B_REQUEST,
  SPEND_BY_TEAM,
} from "./scenario";

describe("attenuation can only narrow (caps(child) ⊆ caps(parent))", () => {
  it("removes a field and never lets a later block re-add it", () => {
    const parent = cfoCapability();
    const child = attenuate(parent, { by: "cfo", denyFields: ["employee_salary"] });
    const grandchild = attenuate(child, { by: "cto", allowFields: ["employee_salary", "credits"] });

    const parentFields = effectiveCapability(parent)!.fields;
    const childFields = effectiveCapability(child)!.fields;
    const grandchildFields = effectiveCapability(grandchild)!.fields;

    expect(parentFields.has("employee_salary")).toBe(true);
    expect(childFields.has("employee_salary")).toBe(false);
    // allowFields on a denied field cannot resurrect it — subset is preserved.
    expect(grandchildFields.has("employee_salary")).toBe(false);
    expect([...grandchildFields].every((f) => parentFields.has(f))).toBe(true);
  });

  it("intersects row scopes — never widens rows", () => {
    const parent = mint(CFO_ROOT, "cfo", {
      ops: ["query"],
      view: SPEND_BY_TEAM,
      fields: ["team", "gross"],
      rows: [{ field: "team", in: ["eng", "ops", "sales"] }],
    });
    const child = attenuate(parent, { by: "cfo", rows: [{ field: "team", in: ["ops", "sales", "finance"] }] });
    const rows = effectiveCapability(child)!.rows.find((r) => r.field === "team")!.in;
    expect(rows.sort()).toEqual(["ops", "sales"]); // ∩, not ∪
  });
});

describe("no capability super-root", () => {
  it("control-plane root cannot mint finance_private authority", () => {
    expect(() =>
      mint(CONTROL_PLANE_ROOT, "agent:eng/1", { ops: ["query"], view: FINANCE_PRIVATE, fields: ["employee_salary"] }),
    ).toThrow(NoRootAuthority);
  });
});

describe("brain query is capability-filtered", () => {
  it("CTO agent sees team spend but finance_private is denied (no grant)", () => {
    const cap = ctoAgentCapability();
    const ok = brainQuery(cap, DATASETS, { view: SPEND_BY_TEAM, fields: ["gross", "net"] });
    expect(ok.ok).toBe(true);

    const denied = brainQuery(cap, DATASETS, { view: FINANCE_PRIVATE, fields: ["credits"] });
    expect(denied.ok).toBe(false);
    if (!denied.ok) expect(denied.reason).toBe("no_grant");
  });

  it("engineering agent only sees its own team rows", () => {
    const res = brainQuery(engAgentCapability(), DATASETS, { view: SPEND_BY_TEAM, fields: ["gross"] });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.rows).toHaveLength(1);
      expect(res.rows[0].team).toBe("eng");
    }
  });
});

describe("Flow A — request → approve → scoped pull", () => {
  it("CFO approval delegates a salary-free, credit-aware token to the CTO agent", () => {
    const route = routeRequest(FLOW_A_REQUEST, CFO_ENVELOPE);
    expect(route.decision).toBe("escalate"); // finance_private isn't auto-approved

    const granted = approveRequest(cfoCapability(), FLOW_A_REQUEST);
    expect(granted.holder).toBe(FLOW_A_REQUEST.requester);

    const res = brainQuery(granted, DATASETS, { view: FINANCE_PRIVATE, fields: ["gross", "credits", "discount_tier"] });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.fields).toEqual(expect.arrayContaining(["credits", "discount_tier"]));
      // salary must not leak even though the CFO token carries it
      const salaryRetry = brainQuery(granted, DATASETS, { view: FINANCE_PRIVATE, fields: ["employee_salary"] });
      expect(salaryRetry.ok).toBe(true);
      if (salaryRetry.ok) {
        expect(salaryRetry.redacted).toContain("employee_salary");
        expect(salaryRetry.rows.some((r) => "employee_salary" in r)).toBe(false);
      }
    }
  });
});

describe("Flow B — the salary invariant (negative test)", () => {
  it("routing offers NO approval path for employee_salary", () => {
    const route = routeRequest(FLOW_B_REQUEST, CFO_ENVELOPE);
    expect(route.decision).toBe("forbidden");
  });

  it("even a direct approval mint refuses to grant salary (defense in depth)", () => {
    expect(() => approveRequest(cfoCapability(), FLOW_B_REQUEST)).toThrow(SalaryInvariantViolation);
  });

  it("an engineering agent has no token and no path that yields salary", () => {
    const cap = engAgentCapability();
    const direct = brainQuery(cap, DATASETS, { view: FINANCE_PRIVATE, fields: ["employee_salary"] });
    expect(direct.ok).toBe(false); // no grant at all for finance_private

    // delegating from the engineering agent can only narrow what it has (spend_by_team),
    // so it can never produce finance_private/employee_salary authority.
    const delegated = delegateTo(cap, "agent:eng/2", { by: "agent:eng/1" });
    const eff = effectiveCapability(delegated)!;
    expect(eff.fields.has("employee_salary")).toBe(false);
  });
});
