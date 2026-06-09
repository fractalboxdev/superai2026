// Contextful · brain query layer (prototype).
//
// Structured retrieval over FinOps fixtures, capability-filtered. Mirrors
// specs/02-brain-memory.md §4: each field/row is authorized BEFORE any data
// reaches the agent or an LLM. Redaction is signalled (listed), never silent;
// a view with no grant is a typed denial that triggers the request flow.

import {
  authorize,
  rowAllowed,
  viewId,
  type Capability,
  type RowScope,
  type View,
} from "./access";

export type Cell = string | number;
export type Row = Record<string, Cell>;

export type Dataset = {
  view: View;
  /** every column the underlying source actually has. */
  columns: string[];
  rows: Row[];
};

export type QuerySpec = {
  view: View;
  fields: string[];
};

export type BrainResult =
  | {
      ok: false;
      reason: "no_grant" | "view_denied" | "wrong_op" | "no_dataset";
      /** human-readable, safe-to-show explanation. */
      answer: string;
    }
  | {
      ok: true;
      fields: string[];
      redacted: string[];
      rowFilter: RowScope[];
      rows: Row[];
      /** synthesized natural-language answer over the permitted projection. */
      answer: string;
    };

const DENIAL_COPY: Record<string, string> = {
  no_grant: "Denied — your token carries no grant for this view. You can raise an access request.",
  view_denied: "Denied — this view is explicitly excluded from your token and cannot be re-granted by attenuation.",
  wrong_op: "Denied — your token does not permit this operation on the view.",
  no_dataset: "No such dataset is loaded in the brain.",
};

/**
 * Run a structured `brain.query` through the capability boundary.
 * Always returns redacted/denied state explicitly so the caller (agent) knows
 * whether to answer, redact, or request access.
 */
export function brainQuery(cap: Capability, datasets: Dataset[], spec: QuerySpec): BrainResult {
  const ds = datasets.find((d) => viewId(d.view) === viewId(spec.view));
  if (!ds) return { ok: false, reason: "no_dataset", answer: DENIAL_COPY.no_dataset };

  const decision = authorize(cap, { op: "query", view: spec.view, fields: spec.fields });
  if (decision.decision === "denied") {
    return { ok: false, reason: decision.reason, answer: DENIAL_COPY[decision.reason] };
  }

  // Always carry the team key so row-scoped aggregates remain labelled.
  const projection = uniq(["team", "period", ...decision.grantedFields]).filter((f) => ds.columns.includes(f));
  const visibleRows = ds.rows
    .filter((row) => rowAllowed(row, decision.rowFilter))
    .map((row) => project(row, projection));

  return {
    ok: true,
    fields: decision.grantedFields,
    redacted: decision.redactedFields,
    rowFilter: decision.rowFilter,
    rows: visibleRows,
    answer: synthesize(ds, decision.grantedFields, visibleRows, decision.redactedFields),
  };
}

function synthesize(ds: Dataset, fields: string[], rows: Row[], redacted: string[]): string {
  if (rows.length === 0) return "No rows are visible within your row scope.";

  const has = (f: string) => fields.includes(f) && ds.columns.includes(f);
  const sum = (f: string) => rows.reduce((acc, r) => acc + numeric(r[f]), 0);

  const parts: string[] = [];
  if (has("gross")) parts.push(`Gross spend ${money(sum("gross"))}`);

  if (has("credits") && has("gross")) {
    const net = sum("gross") - sum("credits");
    parts.push(`net of credits ${money(net)}`);
    if (has("discount_tier")) {
      const tier = String(rows[0]?.discount_tier ?? "—");
      parts.push(`at discount tier ${tier}`);
    }
  } else if (has("net")) {
    parts.push(`net spend ${money(sum("net"))}`);
  }

  let answer = parts.length ? `${parts.join(", ")} across ${rows.length} team(s).` : `${rows.length} row(s) visible.`;
  if (redacted.length) answer += ` Withheld (redacted): ${redacted.join(", ")}.`;
  return answer;
}

function project(row: Row, fields: string[]): Row {
  const out: Row = {};
  for (const f of fields) if (f in row) out[f] = row[f];
  return out;
}

function uniq<T>(xs: T[]): T[] {
  return [...new Set(xs)];
}

function numeric(v: Cell | undefined): number {
  return typeof v === "number" ? v : 0;
}

function money(n: number): string {
  return `$${Math.round(n).toLocaleString("en-US")}`;
}
