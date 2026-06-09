// Contextful · capability-based access control (prototype engine).
//
// This is a faithful but simplified stand-in for the real Biscuit Datalog model
// described in specs/03-access-control.md. The properties that matter for the
// demo are enforced here in plain TS so they are testable and run client-side:
//
//   1. caps(child) ⊆ caps(parent) — attenuation can only NARROW, never widen.
//   2. No capability super-root — a token's authority is rooted in exactly one
//      resource-root key; the holder of that key is the only minter for it.
//   3. Field- and row-level enforcement happens before any data leaves the
//      brain query layer (see brain.ts).
//
// Real Biscuit (@biscuit-auth/biscuit-wasm) would replace the block algebra
// below with append-only signed blocks + a Datalog authorizer; the shapes here
// mirror those concepts intentionally.

export type Operation = "read" | "write" | "comment" | "query" | "admin";

/** A named, field-typed projection of a source — the unit of finance privacy. */
export type View = { source: string; view: string };

export const view = (source: string, name: string): View => ({ source, view: name });
export const viewId = (v: View): string => `${v.source}/${v.view}`;
export const viewEq = (a: View, b: View): boolean => viewId(a) === viewId(b);

/** A principal is a human or an agent owned by exactly one human. */
export type Principal =
  | { kind: "human"; id: string; name: string; role: string }
  | { kind: "agent"; id: string; name: string; owner: string };

/** Agent id format: agent:<owner>/<n>. */
export const agentId = (owner: string, n: string): string => `agent:${owner}/${n}`;

/** A row-level predicate: `field IN [...]`. Empty list = matches nothing. */
export type RowScope = { field: string; in: string[] };

/**
 * The root authority block. Only the holder of the matching resource-root key
 * can create one (see {@link mint}). It carries the FULL grant; attenuation
 * blocks below it can only subtract.
 */
export type AuthorityBlock = {
  kind: "authority";
  /** resource-root key id this authority descends from, e.g. "cfo". */
  root: string;
  ops: Operation[];
  view: View;
  /** every field this grant can expose. */
  fields: string[];
  /** row predicates; [] means all rows. */
  rows: RowScope[];
};

/** An append-only attenuation block. Every field here can only narrow. */
export type AttenuationBlock = {
  kind: "attenuation";
  /** principal id that performed the attenuation. */
  by: string;
  /** drop these fields from the effective set. */
  denyFields?: string[];
  /** intersect the effective field set with this allow-list. */
  allowFields?: string[];
  /** mark these views as denied even if the authority covers them. */
  denyViews?: View[];
  /** intersect row predicates (AND across fields, ∩ within a field). */
  rows?: RowScope[];
  /** human-readable ttl, recorded for audit. */
  ttl?: string;
};

export type Block = AuthorityBlock | AttenuationBlock;

/** A capability token: an authority block followed by 0+ attenuations. */
export type Capability = {
  /** principal id currently holding the token. */
  holder: string;
  blocks: Block[];
};

// ---- Root keys (no super-root) -------------------------------------------

/**
 * A resource-root key. Holding one is the authority to mint tokens over its
 * views. The control-plane root deliberately holds NO data views — it can
 * register identities and document membership but cannot mint authority over
 * `finance_private`. That is what makes "no super-root" structural.
 */
export type RootKey = { id: string; owner: string; views: View[] };

export class NoRootAuthority extends Error {
  readonly _tag = "NoRootAuthority";
  constructor(rootId: string, v: View) {
    super(`root '${rootId}' has no authority over ${viewId(v)}`);
  }
}

/** Mint a fresh first-party token from a resource root. */
export function mint(
  root: RootKey,
  holder: string,
  grant: { ops: Operation[]; view: View; fields: string[]; rows?: RowScope[] },
): Capability {
  if (!root.views.some((v) => viewEq(v, grant.view))) {
    throw new NoRootAuthority(root.id, grant.view);
  }
  return {
    holder,
    blocks: [
      {
        kind: "authority",
        root: root.id,
        ops: grant.ops,
        view: grant.view,
        fields: [...grant.fields],
        rows: grant.rows ? grant.rows.map((r) => ({ field: r.field, in: [...r.in] })) : [],
      },
    ],
  };
}

// ---- Attenuation (narrow-only) -------------------------------------------

/** Append an attenuation block, keeping the same holder. */
export function attenuate(cap: Capability, block: Omit<AttenuationBlock, "kind">): Capability {
  return { holder: cap.holder, blocks: [...cap.blocks, { kind: "attenuation", ...block }] };
}

/** Append an attenuation block AND hand the token to a new holder (delegation). */
export function delegateTo(
  cap: Capability,
  holder: string,
  block: Omit<AttenuationBlock, "kind">,
): Capability {
  return { holder, blocks: [...cap.blocks, { kind: "attenuation", ...block }] };
}

// ---- Effective capability + authorization --------------------------------

function intersectRows(a: RowScope[], b: RowScope[]): RowScope[] {
  const out: RowScope[] = a.map((r) => ({ field: r.field, in: [...r.in] }));
  for (const r of b) {
    const existing = out.find((x) => x.field === r.field);
    if (existing) existing.in = existing.in.filter((v) => r.in.includes(v));
    else out.push({ field: r.field, in: [...r.in] });
  }
  return out;
}

export type EffectiveCapability = {
  root: string;
  ops: Set<Operation>;
  view: View;
  fields: Set<string>;
  rows: RowScope[];
  deniedViews: View[];
};

/**
 * Fold the block list into the effective grant. Because every attenuation can
 * only subtract (deny / intersect), the effective field & row sets shrink
 * monotonically down the chain — this is the `caps(child) ⊆ caps(parent)`
 * guarantee, computed rather than trusted.
 */
export function effectiveCapability(cap: Capability): EffectiveCapability | null {
  const auth = cap.blocks[0];
  if (!auth || auth.kind !== "authority") return null;

  let fields = new Set(auth.fields);
  let rows: RowScope[] = auth.rows.map((r) => ({ field: r.field, in: [...r.in] }));
  const deniedViews: View[] = [];

  for (const b of cap.blocks.slice(1)) {
    if (b.kind !== "attenuation") continue;
    if (b.allowFields) {
      const allow = b.allowFields;
      fields = new Set([...fields].filter((f) => allow.includes(f)));
    }
    if (b.denyFields) {
      const deny = b.denyFields;
      fields = new Set([...fields].filter((f) => !deny.includes(f)));
    }
    if (b.denyViews) deniedViews.push(...b.denyViews);
    if (b.rows) rows = intersectRows(rows, b.rows);
  }

  return { root: auth.root, ops: new Set(auth.ops), view: auth.view, fields, rows, deniedViews };
}

export type QueryRequest = { op: Operation; view: View; fields: string[] };

export type AuthDecision =
  | { decision: "denied"; reason: "no_grant" | "view_denied" | "wrong_op" }
  | {
      decision: "ok";
      grantedFields: string[];
      /** requested fields the caller is NOT cleared for — signalled, not silent. */
      redactedFields: string[];
      rowFilter: RowScope[];
    };

/**
 * Authorize a structured query against a token. A view with no grant returns a
 * typed denial (the trigger for the request flow); a covered view drops fields
 * the caller can't see and reports them in `redactedFields`.
 *
 * No LLM is involved — this is what keeps the local-first guarantee intact with
 * the cloud disconnected.
 */
export function authorize(cap: Capability, req: QueryRequest): AuthDecision {
  const eff = effectiveCapability(cap);
  if (!eff) return { decision: "denied", reason: "no_grant" };
  if (!viewEq(eff.view, req.view)) return { decision: "denied", reason: "no_grant" };
  if (eff.deniedViews.some((v) => viewEq(v, req.view))) return { decision: "denied", reason: "view_denied" };
  if (!eff.ops.has(req.op)) return { decision: "denied", reason: "wrong_op" };

  const grantedFields = req.fields.filter((f) => eff.fields.has(f));
  const redactedFields = req.fields.filter((f) => !eff.fields.has(f));
  return { decision: "ok", grantedFields, redactedFields, rowFilter: eff.rows };
}

/** Does a row satisfy every row-scope predicate? */
export function rowAllowed(row: Record<string, unknown>, filter: RowScope[]): boolean {
  return filter.every((scope) => scope.in.includes(String(row[scope.field])));
}
