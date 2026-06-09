// Contextful · the Q3 FinOps demo scenario (specs/00-overview.md §2–4).
//
// Concrete personas, root keys, datasets, initial tokens, and the two
// reference requests that drive Flow A (request → approve → scoped pull) and
// Flow B (the salary invariant). Everything the web console needs is here.

import { mint, view, type Capability, type Principal, type RootKey, type View } from "./access";
import type { Dataset } from "./brain";
import type { AccessRequest, Envelope } from "./requests";

// ---- Views ----------------------------------------------------------------

export const SPEND_BY_TEAM: View = view("stripe", "spend_by_team");
export const FINANCE_PRIVATE: View = view("stripe", "finance_private");

// ---- Principals -----------------------------------------------------------

export const CFO: Principal = { kind: "human", id: "cfo", name: "Dana (CFO)", role: "finance" };
export const CTO_AGENT: Principal = { kind: "agent", id: "agent:cto/1", name: "CTO's agent", owner: "cto" };
export const ENG_AGENT: Principal = { kind: "agent", id: "agent:eng/1", name: "Engineering agent", owner: "eng" };

export const PRINCIPALS: Principal[] = [CTO_AGENT, ENG_AGENT, CFO];

/** Short tag for presence dots / labels. */
export const tag = (p: Principal): string =>
  p.id === "cfo" ? "CF" : p.id.startsWith("agent:cto") ? "CT" : p.id.startsWith("agent:eng") ? "EN" : "◆";

// ---- Root keys (no super-root) --------------------------------------------

/** CFO owns the finance resource root — sole minter of finance_private authority. */
export const CFO_ROOT: RootKey = {
  id: "cfo",
  owner: "cfo",
  views: [FINANCE_PRIVATE, SPEND_BY_TEAM],
};

/** Control-plane root: identity/membership only — holds NO data views by design. */
export const CONTROL_PLANE_ROOT: RootKey = { id: "control-plane", owner: "control-plane", views: [] };

// ---- Datasets (Kaggle-derived mock, single period) ------------------------

const PERIOD = "2026-05";

export const DATASETS: Dataset[] = [
  {
    view: SPEND_BY_TEAM,
    columns: ["team", "period", "gross", "net"],
    rows: [
      { team: "eng", period: PERIOD, gross: 100_000, net: 100_000 },
      { team: "ops", period: PERIOD, gross: 30_000, net: 30_000 },
      { team: "sales", period: PERIOD, gross: 20_000, net: 20_000 },
      { team: "finance", period: PERIOD, gross: 15_000, net: 15_000 },
    ],
  },
  {
    view: FINANCE_PRIVATE,
    columns: ["team", "period", "gross", "net", "discount_tier", "credits", "employee_salary"],
    rows: [
      { team: "eng", period: PERIOD, gross: 100_000, net: 78_000, discount_tier: "Enterprise (Tier 3)", credits: 22_000, employee_salary: 240_000 },
      { team: "ops", period: PERIOD, gross: 30_000, net: 26_000, discount_tier: "Enterprise (Tier 3)", credits: 4_000, employee_salary: 210_000 },
      { team: "sales", period: PERIOD, gross: 20_000, net: 17_000, discount_tier: "Enterprise (Tier 3)", credits: 3_000, employee_salary: 200_000 },
      { team: "finance", period: PERIOD, gross: 15_000, net: 13_000, discount_tier: "Enterprise (Tier 3)", credits: 2_000, employee_salary: 230_000 },
    ],
  },
];

// ---- Initial capabilities -------------------------------------------------

const ALL_TEAMS = ["eng", "ops", "sales", "finance"];

/** The CFO holds the full finance root token (sole salary authority). */
export const cfoCapability = (): Capability =>
  mint(CFO_ROOT, CFO.id, {
    ops: ["query", "read"],
    view: FINANCE_PRIVATE,
    fields: ["team", "period", "gross", "net", "discount_tier", "credits", "employee_salary"],
  });

/** CTO's agent: team-level spend only (no finance_private) until Flow A grants it. */
export const ctoAgentCapability = (): Capability =>
  mint(CFO_ROOT, CTO_AGENT.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
  });

/** Engineering agent: usage view, own team rows only. Never any salary path. */
export const engAgentCapability = (): Capability =>
  mint(CFO_ROOT, ENG_AGENT.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
    rows: [{ field: "team", in: ["eng"] }],
  });

export const initialCapability = (principalId: string): Capability => {
  switch (principalId) {
    case CFO.id:
      return cfoCapability();
    case CTO_AGENT.id:
      return ctoAgentCapability();
    case ENG_AGENT.id:
      return engAgentCapability();
    default:
      throw new Error(`no initial capability for ${principalId}`);
  }
};

// ---- Auto-mode envelope ----------------------------------------------------

export const CFO_ENVELOPE: Envelope = {
  owner: "cfo",
  autoApprove: [{ view: SPEND_BY_TEAM, maxTtl: "7d" }],
  neverDelegate: ["employee_salary"],
};

// ---- Reference requests ----------------------------------------------------

/** Flow A: CTO's agent needs credit-adjusted spend. Approvable (no salary). */
export const FLOW_A_REQUEST: AccessRequest = {
  id: "req-flow-a",
  requester: CTO_AGENT.id,
  view: FINANCE_PRIVATE,
  fields: ["gross", "credits", "discount_tier"],
  rowScope: [{ field: "team", in: ALL_TEAMS }],
  reason: "To judge whether this month's Claude spend is net-justified after credits.",
  doc: "finops",
  ttl: "7d",
};

/** Flow B: Engineering agent reaches for salary. No approval path exists. */
export const FLOW_B_REQUEST: AccessRequest = {
  id: "req-flow-b",
  requester: ENG_AGENT.id,
  view: FINANCE_PRIVATE,
  fields: ["employee_salary"],
  reason: "Wants to benchmark team comp against spend.",
  doc: "finops",
  ttl: "7d",
};
