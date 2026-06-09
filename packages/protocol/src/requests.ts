// Contextful · permission requests, auto-mode envelopes, and grant minting.
// See specs/03-access-control.md §5.
//
// The salary invariant lives here and in access.ts: no request that names
// `employee_salary` can ever be turned into a token for a non-finance agent.
// It is rejected by routing (no approval path is offered) AND by the approval
// minter itself (defense in depth) — so it holds even if a UI bug tried to
// approve it.

import {
  attenuate,
  effectiveCapability,
  type Capability,
  type RowScope,
  type View,
  viewId,
} from "./access";

/** Fields that may never be delegated to an agent by any approval path. */
export const NEVER_DELEGABLE = ["employee_salary"] as const;

/**
 * The fields an owner may offer when delegating or approving: every field their
 * own token effectively grants, minus the {@link NEVER_DELEGABLE} set. The
 * delegation form ([03 §6.2]) and inbox ([03 §6.3]) render checkboxes from this
 * list, so a `NEVER_DELEGABLE` field is *structurally never offered* — the
 * salary invariant becomes a UI affordance, not only a server-side refusal.
 */
export function delegableFields(ownerCap: Capability): string[] {
  const eff = effectiveCapability(ownerCap);
  if (!eff) return [];
  const banned = new Set<string>(NEVER_DELEGABLE);
  return [...eff.fields].filter((f) => !banned.has(f));
}

export type AccessRequest = {
  id: string;
  /** requesting principal id, e.g. agent:cto/1. */
  requester: string;
  view: View;
  fields: string[];
  rowScope?: RowScope[];
  reason: string;
  doc: string;
  ttl: string;
};

/** An owner's auto-mode policy. */
export type Envelope = {
  owner: string;
  /** views whose reads auto-approve up to maxTtl. */
  autoApprove: { view: View; maxTtl: string }[];
  /** fields that always escalate / are never delegable. */
  neverDelegate: string[];
};

export type RouteDecision =
  | { decision: "auto"; reason: string }
  | { decision: "escalate"; reason: string }
  | { decision: "forbidden"; reason: string };

/** Decide how a request is handled: auto-approve, escalate to human, or forbid. */
export function routeRequest(req: AccessRequest, envelope: Envelope): RouteDecision {
  const blocked = req.fields.filter((f) => envelope.neverDelegate.includes(f));
  if (blocked.length > 0) {
    return {
      decision: "forbidden",
      reason: `${blocked.join(", ")} is never delegable — no approval path exists (salary invariant).`,
    };
  }
  const auto = envelope.autoApprove.find((a) => viewId(a.view) === viewId(req.view));
  if (auto) return { decision: "auto", reason: `inside envelope for ${viewId(req.view)} (≤ ${auto.maxTtl})` };
  return { decision: "escalate", reason: "outside auto-approve envelope — owner decides" };
}

export class SalaryInvariantViolation extends Error {
  readonly _tag = "SalaryInvariantViolation";
  constructor(fields: string[]) {
    super(`refusing to mint a token granting ${fields.join(", ")} — salary invariant`);
  }
}

/**
 * Approve a request by attenuating the approver's own capability down to the
 * exact requested scope and delegating it to the requester. The approver can
 * only grant what they themselves hold (caps(child) ⊆ caps(parent)), and can
 * never grant a NEVER_DELEGABLE field — enforced here regardless of caller.
 */
export function approveRequest(
  approverCap: Capability,
  req: AccessRequest,
): Capability {
  const forbidden = req.fields.filter((f) => (NEVER_DELEGABLE as readonly string[]).includes(f));
  if (forbidden.length > 0) throw new SalaryInvariantViolation(forbidden);

  // Grant the requested fields plus the non-private row labels (team/period) so
  // row-scoped results stay labelled, then hand the token to the requester.
  const allowFields = [...req.fields];
  for (const label of ["team", "period"]) if (!allowFields.includes(label)) allowFields.push(label);

  return attenuate(
    { ...approverCap, holder: req.requester },
    {
      by: approverCap.holder,
      allowFields,
      denyFields: [...NEVER_DELEGABLE],
      rows: req.rowScope,
      ttl: req.ttl,
    },
  );
}
