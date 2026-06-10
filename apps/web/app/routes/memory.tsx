import type { MetaFunction } from "react-router";
import { viewId } from "@superai2026/protocol/access";
import { DATASETS } from "@superai2026/protocol/scenario";

export const meta: MetaFunction = () => [
  { title: "Memory · Contextful" },
  {
    name: "description",
    content:
      "How data lands on this machine: connector payloads are scrubbed, synthesized, and indexed into the on-host brain (SQLite + FTS5) — then served only through capability-filtered views.",
  },
];

const PRIVATE_FIELDS = new Set(["discount_tier", "credits", "employee_salary"]);

const PIPELINE = [
  {
    name: "Ingest",
    detail: "Connector payloads arrive on-host via sync ingest — nothing is fetched cloud-side.",
  },
  {
    name: "Scrub",
    detail: "Inbound secrets and card PANs are stripped before anything is persisted.",
  },
  {
    name: "Synthesize",
    detail: "Raw events are folded into per-team, per-period views with provenance.",
  },
  {
    name: "Index",
    detail: "Rows land in the brain index — SQLite + FTS5 under ~/.contextful.",
  },
  {
    name: "Serve",
    detail: "brain.query authorizes every field and row against the caller's token first.",
  },
] as const;

// A representative slice of the host audit trail (demo seed) — what the
// ingest → scrub → synthesize → index pipeline logs as data lands.
const EVENTS: { at: string; kind: "ok" | "info" | "block"; text: string }[] = [
  { at: "2 min ago", kind: "ok", text: "stripe · invoice.paid → spend_by_team · +4 rows (period 2026-05)" },
  { at: "2 min ago", kind: "block", text: "scrub · card PAN detected in payload → redacted before persist" },
  { at: "9 min ago", kind: "ok", text: "stripe · credit_note.created → finance_private · credits updated" },
  { at: "14 min ago", kind: "ok", text: "exa · world memory refresh → 6 documents cached on-host" },
  { at: "31 min ago", kind: "info", text: "daydream · linked “Q3 AI Spend Review” ↔ vendor-consolidation (shared vendors)" },
  { at: "1 h ago", kind: "info", text: "cron · nightly synthesis pass → 2 views re-rolled, 0 anomalies" },
];

/** Memory — what this host has ingested, and the path every byte takes to get in. */
export default function MemoryRoute() {
  return (
    <div className="ac-shell">
      <main className="ac-page">
        <header className="ac-page__head">
          <span className="cf-eyebrow">This machine</span>
          <h1>Memory</h1>
          <p className="cf-text-muted">
            Everything the brain knows lives on this host, under <code>~/.contextful</code>. Data
            gets in one way: <code>ingest → scrub → synthesize → index</code> — and it gets out one
            way, through capability-filtered <code>brain.query</code>.
          </p>
        </header>

        <section>
          <p className="ac-card__label">Ingest pipeline</p>
          <ol className="mem-pipeline">
            {PIPELINE.map((s, i) => (
              <li className="cf-card mem-stage" key={s.name}>
                <p className="mem-stage__step">{i + 1}</p>
                <p className="mem-stage__name">{s.name}</p>
                <p className="mem-stage__detail">{s.detail}</p>
              </li>
            ))}
          </ol>
        </section>

        <section>
          <p className="ac-card__label">Indexed views</p>
          <div className="cf-card" style={{ overflowX: "auto" }}>
            <table className="cf-table mem-views">
              <thead>
                <tr>
                  <th>view</th>
                  <th>rows</th>
                  <th>columns</th>
                </tr>
              </thead>
              <tbody>
                {DATASETS.map((d) => (
                  <tr key={viewId(d.view)}>
                    <td>{viewId(d.view)}</td>
                    <td>{d.rows.length}</td>
                    <td>
                      <span className="cf-chips">
                        {d.columns.map((c) => (
                          <span key={c} className={`cf-chip${PRIVATE_FIELDS.has(c) ? " cf-chip--private" : ""}`}>
                            {c}
                          </span>
                        ))}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="ac-audit">
          <p className="ac-card__label">Recent ingest activity</p>
          <div className="cf-card cf-log">
            <ul>
              {EVENTS.map((e, i) => (
                <li key={i} className={`cf-log__row cf-log__row--${e.kind}`}>
                  <span className="cf-log__tag">{e.kind}</span>
                  <span>
                    {e.text} <span className="mem-when">· {e.at}</span>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <p className="ac-note">
          Red chips are finance-private fields — they are indexed here, but a{" "}
          <code>brain.query</code> only returns them when the caller&rsquo;s capability token grants
          the field, and <code>employee_salary</code> has no approval path at all.
        </p>
      </main>
    </div>
  );
}
