// The room registry. Every entry is a live LoroDoc room (spec 01 §1–2): one
// document, switchable in the sidebar, synced via CRDT across tabs (and the
// relay when configured). `id` is the `doc_id` on the wire; `seed` is the body
// prose a fresh room starts with (the title renders separately from `title`).
export type DocMeta = { id: string; title: string; seed: string };

export const DOCS: DocMeta[] = [
  {
    id: "finops",
    title: "Q3 FinOps Review",
    seed:
      "Gilfoyle reports Claude Code utilization is up across the platform team. " +
      "The open question for this review: is the spend justified once credits and our " +
      "discount tier are applied?\n\n" +
      "Each principal queries the same brain but sees only what their capability token " +
      "permits. Switch “Acting as” and run a query — the brain redacts fields " +
      "you aren’t cleared for, and a denied view is what triggers a scoped access request.",
  },
  {
    id: "agent-evals",
    title: "Agent workflow evals",
    seed:
      "Outcome and eval views for the operations team. Track which agent workflows " +
      "actually moved a metric this quarter, and which just burned tokens.",
  },
  {
    id: "vendor-consolidation",
    title: "Vendor consolidation",
    seed:
      "Which SaaS + AI tools overlap? List renewal dates and consolidation candidates " +
      "before the Q3 close so Finance can model the savings.",
  },
  {
    id: "budget-2026",
    title: "2026 budget draft",
    seed:
      "Draft allocations by team. Finance-private figures (discount tier, credits, " +
      "salaries) stay redacted unless your capability token grants them.",
  },
];

export const DEFAULT_DOC_ID = "finops";
