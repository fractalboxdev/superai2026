// Contextful · the Q3 FinOps demo scenario (specs/00-overview.md §2–4).
//
// Concrete personas, root keys, datasets, initial tokens, and the two
// reference requests that drive Flow A (request → approve → scoped pull) and
// Flow B (the salary invariant). Everything the web console needs is here.

import { mint, view, viewEq, type Capability, type Principal, type RootKey, type View } from "./access";
import type { Dataset } from "./brain";
import type { AccessRequest, Envelope } from "./requests";

// ---- Views ----------------------------------------------------------------

export const SPEND_BY_TEAM: View = view("stripe", "spend_by_team");
export const FINANCE_PRIVATE: View = view("stripe", "finance_private");

// ---- Principals -----------------------------------------------------------

// The demo cast is always the Pied Piper team (HBO Silicon Valley), displayed
// as "Name (Role)". Principal ids and owners stay stable (`cfo`, `agent:cto/1`,
// …) — they are wire/CLI identifiers shared with crates/sync, the acceptance
// suite, and the registry's owner linkage.

// Humans (each owns the agents minted under their id) ...
export const CFO: Principal = { kind: "human", id: "cfo", name: "Monica (CFO)", role: "finance" };
export const CTO: Principal = { kind: "human", id: "cto", name: "Richard (CEO)", role: "engineering" };
export const ENG: Principal = { kind: "human", id: "eng", name: "Dinesh (Lead Engineer)", role: "engineering" };

// ... and their agents (agent:<owner>/<n>, no root authority of their own).
export const CFO_AGENT: Principal = { kind: "agent", id: "agent:cfo/1", name: "Monica (CFO)'s analyst agent", owner: "cfo" };
export const CTO_AGENT: Principal = { kind: "agent", id: "agent:cto/1", name: "Richard (CEO)'s agent", owner: "cto" };
export const ENG_AGENT: Principal = { kind: "agent", id: "agent:eng/1", name: "Dinesh (Lead Engineer)'s agent", owner: "eng" };

/** The demo console's cast (a subset of the registry, focused on the two flows). */
export const PRINCIPALS: Principal[] = [CTO_AGENT, ENG_AGENT, CFO];

/**
 * The full control-plane principal registry — every human and, owned by it,
 * the agents minted under its id. This is what the company directory ([03 §6.1])
 * enumerates; `PRINCIPALS` above is the narrower console cast.
 */
export const REGISTRY: Principal[] = [CFO, CFO_AGENT, CTO, CTO_AGENT, ENG, ENG_AGENT];

/** Humans in the registry (directory rows). */
export const humans = (): Extract<Principal, { kind: "human" }>[] =>
  REGISTRY.filter((p): p is Extract<Principal, { kind: "human" }> => p.kind === "human");

/** Agents a human owns (nested under each directory row). */
export const ownedAgents = (ownerId: string): Principal[] =>
  REGISTRY.filter((p): p is Extract<Principal, { kind: "agent" }> => p.kind === "agent" && p.owner === ownerId);

/** Look up a registry principal by id. */
export const principal = (id: string): Principal | undefined => REGISTRY.find((p) => p.id === id);

/** Short tag for presence dots / labels. */
export const tag = (p: Principal): string =>
  p.id === "cfo" || p.id.startsWith("agent:cfo")
    ? "MH"
    : p.id === "cto" || p.id.startsWith("agent:cto")
      ? "RH"
      : p.id === "eng" || p.id.startsWith("agent:eng")
        ? "DC"
        : "◆";

/** Stable presence color per owner family (matches the console's dot palette). */
export const principalColor = (id: string): string =>
  id === "cfo" || id.startsWith("agent:cfo")
    ? "var(--cf-sky-500)"
    : id === "cto" || id.startsWith("agent:cto")
      ? "var(--cf-indigo-500)"
      : "var(--cf-amber-500)";

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

/** Richard (CEO)'s agent: team-level spend only (no finance_private) until Flow A grants it. */
export const ctoAgentCapability = (): Capability =>
  mint(CFO_ROOT, CTO_AGENT.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
  });

/** Dinesh (Lead Engineer)'s agent: usage view, own team rows only. Never any salary path. */
export const engAgentCapability = (): Capability =>
  mint(CFO_ROOT, ENG_AGENT.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
    rows: [{ field: "team", in: ["eng"] }],
  });

/** CFO's analyst agent: a finance_private token already narrowed to drop salary. */
export const cfoAgentCapability = (): Capability =>
  mint(CFO_ROOT, CFO_AGENT.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
  });

/** CTO (human): team-level spend across all teams — the token they can narrow when delegating. */
export const ctoCapability = (): Capability =>
  mint(CFO_ROOT, CTO.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
  });

/** Eng lead (human): team-level spend, scoped to the eng + ops rows they own. */
export const engLeadCapability = (): Capability =>
  mint(CFO_ROOT, ENG.id, {
    ops: ["query", "read"],
    view: SPEND_BY_TEAM,
    fields: ["team", "period", "gross", "net"],
    rows: [{ field: "team", in: ["eng", "ops"] }],
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

/** Initial token for ANY registry principal (used to seed the web access-control UI). */
export const registryCapability = (principalId: string): Capability => {
  switch (principalId) {
    case CFO_AGENT.id:
      return cfoAgentCapability();
    case CTO.id:
      return ctoCapability();
    case ENG.id:
      return engLeadCapability();
    default:
      return initialCapability(principalId);
  }
};

/** Seed token map for the whole registry — one capability per principal. */
export const registryCapabilities = (): Record<string, Capability> =>
  Object.fromEntries(REGISTRY.map((p) => [p.id, registryCapability(p.id)]));

// ---- Auto-mode envelope ----------------------------------------------------

export const CFO_ENVELOPE: Envelope = {
  owner: "cfo",
  autoApprove: [{ view: SPEND_BY_TEAM, maxTtl: "7d" }],
  neverDelegate: ["employee_salary"],
};

// ---- Reference requests ----------------------------------------------------

/** Flow A: Richard (CEO)'s agent needs credit-adjusted spend. Approvable (no salary). */
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

/** Flow B: Dinesh (Lead Engineer)'s agent reaches for salary. No approval path exists. */
export const FLOW_B_REQUEST: AccessRequest = {
  id: "req-flow-b",
  requester: ENG_AGENT.id,
  view: FINANCE_PRIVATE,
  fields: ["employee_salary"],
  reason: "Wants to benchmark team comp against spend.",
  doc: "finops",
  ttl: "7d",
};

// ---- Resource ownership + inbox seed --------------------------------------

/** All resource roots in the demo (extend as more owners mint their own views). */
const RESOURCE_ROOTS: RootKey[] = [CFO_ROOT];

/**
 * Who owns the root of a view — the principal whose inbox an `access_request`
 * for it lands in, and whose token approval attenuates ([03 §6.3]). Falls back
 * to the CFO root owner for any view not explicitly rooted elsewhere.
 */
export const resourceOwnerOf = (v: View): string =>
  RESOURCE_ROOTS.find((root) => root.views.some((x) => viewEq(x, v)))?.owner ?? CFO_ROOT.owner;

/**
 * The reference requests as they arrive in the resource owner's inbox: Flow A
 * (escalate → approvable) and Flow B (forbidden → no approve button).
 */
export const INBOX_SEED: AccessRequest[] = [FLOW_A_REQUEST, FLOW_B_REQUEST];
