import { Suspense, lazy, useMemo, useState } from "react";
import { Link } from "react-router";
import type { MetaFunction } from "react-router";
import { viewId, type Capability, type View } from "@superai2026/protocol/access";
import { brainQuery, type BrainResult } from "@superai2026/protocol/brain";
import {
  approveRequest,
  routeRequest,
  type AccessRequest,
  type RouteDecision,
} from "@superai2026/protocol/requests";
import { useWeaverRoom } from "@/lib/weaverRoom";
import { DOCS, DEFAULT_DOC_ID } from "@/lib/docs";
import { initialsOf, isScenarioPrincipal, peerColor, peerKey } from "@/lib/presence";
import DocDebugMenu from "@/components/DocDebugMenu";

// The Weaver editing surface pulls in loro-crdt (WASM) via @weaver/react —
// lazy-loaded so it ships as a separate client chunk and never renders during
// SSR (useWeaverRoom only yields an editor after client hydration).
const WeaverSurface = lazy(() => import("@/components/WeaverSurface"));
import {
  CFO,
  CFO_ENVELOPE,
  CTO_AGENT,
  DATASETS,
  ENG_AGENT,
  FINANCE_PRIVATE,
  FLOW_A_REQUEST,
  FLOW_B_REQUEST,
  PRINCIPALS,
  REGISTRY,
  SPEND_BY_TEAM,
  cfoCapability,
  initialCapability,
  tag,
} from "@superai2026/protocol/scenario";
// Type-only — erased at build; the WASM-backed runtime import stays inside
// the lazy WeaverSurface chunk.
import type { Principal as WeaverPrincipal } from "@weaver/core";

export const meta: MetaFunction = () => [
  { title: "Contextful — capability-scoped company brain" },
  {
    name: "description",
    content:
      "Live demo: humans and AI agents co-edit shared documents, and every agent sees only what its capability token permits. Watch a scoped access request get approved — and a salary read stay blocked.",
  },
];

function Mark() {
  return (
    <svg viewBox="0 0 32 32" role="img" aria-label="Contextful">
      <defs>
        <linearGradient id="appmark" x1="0" y1="0" x2="32" y2="32" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#6366f1" />
          <stop offset="1" stopColor="#0ea5e9" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8" fill="url(#appmark)" />
      <path d="M22 11.4a7 7 0 1 0 0 9.2" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" />
      <circle cx="22.6" cy="16" r="2.6" fill="#f59e0b" />
    </svg>
  );
}

const VIEWS: { view: View; label: string }[] = [
  { view: SPEND_BY_TEAM, label: "stripe / spend_by_team" },
  { view: FINANCE_PRIVATE, label: "stripe / finance_private" },
];

const PRIVATE_FIELDS = new Set(["discount_tier", "credits", "employee_salary"]);
const BASE_FIELDS = new Set(["team", "period"]);

const dotColor = (id: string): string =>
  id === CFO.id ? "var(--cf-sky-500)" : id.startsWith("agent:cto") ? "var(--cf-indigo-500)" : "var(--cf-amber-500)";

type LogEntry = { id: number; kind: "ok" | "deny" | "grant" | "block" | "info"; text: string };

let logSeq = 0;

export default function Home() {
  // One capability token per principal; minted grants replace the holder's token.
  const [caps, setCaps] = useState<Record<string, Capability>>(() => ({
    [CTO_AGENT.id]: initialCapability(CTO_AGENT.id),
    [ENG_AGENT.id]: initialCapability(ENG_AGENT.id),
    [CFO.id]: initialCapability(CFO.id),
  }));
  const [actorId, setActorId] = useState<string>(CTO_AGENT.id);
  const [selView, setSelView] = useState<View>(FINANCE_PRIVATE);
  const [selFields, setSelFields] = useState<string[]>(["gross", "credits", "discount_tier"]);
  const [result, setResult] = useState<BrainResult | null>(null);
  const [pending, setPending] = useState<{ req: AccessRequest; route: RouteDecision } | null>(null);
  const [log, setLog] = useState<LogEntry[]>([]);

  const actor = PRINCIPALS.find((p) => p.id === actorId)!;
  const [activeDocId, setActiveDocId] = useState<string>(DEFAULT_DOC_ID);
  const activeDoc = DOCS.find((d) => d.id === activeDocId)!;
  const room = useWeaverRoom(actor.id, actor.name, activeDocId);
  const { status: syncStatus, peers: livePeers } = room;
  // One presence roster (upstream weaver PR #35): scenario agents +
  // collaborators are always in the room; live sessions light their chip up,
  // and sessions outside the scenario cast get their own chip per session.
  const liveByPrincipal = useMemo(
    () => new Map(livePeers.map((p) => [p.principal, p])),
    [livePeers],
  );
  const guestPeers = useMemo(
    () => livePeers.filter((p) => !isScenarioPrincipal(p.principal)),
    [livePeers],
  );
  const columns = useMemo(
    () => DATASETS.find((d) => viewId(d.view) === viewId(selView))?.columns ?? [],
    [selView],
  );
  // The @-mention directory: the full org cast (humans + agents), mapped to
  // Weaver's Principal shape with the same colors the roster/carets use.
  const mentionPrincipals = useMemo<WeaverPrincipal[]>(
    () =>
      REGISTRY.map((p) => ({
        id: p.id,
        kind: p.kind === "human" ? "user" : "agent",
        label: p.name,
        color: peerColor(p.id),
      })),
    [],
  );
  const selfIdentity = useMemo(
    () => ({ id: actor.id, name: actor.name }),
    [actor.id, actor.name],
  );

  const pushLog = (kind: LogEntry["kind"], text: string) =>
    setLog((l) => [{ id: ++logSeq, kind, text }, ...l].slice(0, 12));

  const switchActor = (id: string) => {
    setActorId(id);
    setResult(null);
    setPending(null);
  };

  const switchView = (v: View) => {
    setSelView(v);
    setResult(null);
    setPending(null);
    // sensible default field selection per view
    setSelFields(
      viewId(v) === viewId(FINANCE_PRIVATE) ? ["gross", "credits", "discount_tier"] : ["gross", "net"],
    );
  };

  const toggleField = (f: string) =>
    setSelFields((fs) => (fs.includes(f) ? fs.filter((x) => x !== f) : [...fs, f]));

  const runQuery = () => {
    const res = brainQuery(caps[actorId], DATASETS, { view: selView, fields: selFields });
    setResult(res);
    setPending(null);
    if (!res.ok) pushLog("deny", `${actor.name} · query ${viewId(selView)} → ${res.reason}`);
    else if (res.redacted.length) pushLog("ok", `${actor.name} · partial: redacted ${res.redacted.join(", ")}`);
    else pushLog("ok", `${actor.name} · query ${viewId(selView)} → ${res.rows.length} row(s)`);
  };

  // Fields the actor still needs (denied entirely, or redacted from a partial result).
  const neededFields = useMemo(() => {
    if (!result) return [];
    if (!result.ok) return selFields.filter((f) => !BASE_FIELDS.has(f));
    return result.redacted;
  }, [result, selFields]);

  const canRequest = result != null && !(result.ok && result.redacted.length === 0) && neededFields.length > 0;

  const requestAccess = () => {
    const req: AccessRequest = {
      id: `req-${++logSeq}`,
      requester: actorId,
      view: selView,
      fields: neededFields,
      rowScope: [{ field: "team", in: ["eng", "ops", "sales", "finance"] }],
      reason: `${actor.name} needs ${neededFields.join(", ")} to answer net-of-credits.`,
      doc: "finops",
      ttl: "7d",
    };
    const route = routeRequest(req, CFO_ENVELOPE);
    setPending({ req, route });
    if (route.decision === "forbidden") pushLog("block", `${actor.name} → ${req.fields.join(", ")}: ${route.reason}`);
    else if (route.decision === "auto") {
      applyGrant(req);
      pushLog("grant", `auto-approved ${req.fields.join(", ")} (${route.reason})`);
    } else pushLog("info", `access request raised → Monica (CFO) decides (${req.fields.join(", ")})`);
  };

  const applyGrant = (req: AccessRequest) => {
    try {
      const granted = approveRequest(cfoCapability(), req);
      setCaps((c) => ({ ...c, [req.requester]: granted }));
      setPending(null);
      pushLog("grant", `Monica (CFO) minted scoped token → ${req.requester} (${req.fields.join(", ")}, ttl ${req.ttl})`);
      // agent retries automatically with the new token
      const res = brainQuery(granted, DATASETS, { view: req.view, fields: selFields });
      setResult(res);
      if (res.ok) pushLog("ok", `${actor.name} retried → answered`);
    } catch (e) {
      pushLog("block", `mint refused: ${(e as Error).message}`);
      setPending(null);
    }
  };

  const denyRequest = () => {
    if (pending) pushLog("deny", `Monica (CFO) denied ${pending.req.fields.join(", ")} — stays blocked`);
    setPending(null);
  };

  // Scenario shortcuts.
  const runFlowA = () => {
    setActorId(CTO_AGENT.id);
    setSelView(FINANCE_PRIVATE);
    setSelFields([...FLOW_A_REQUEST.fields]);
    const res = brainQuery(caps[CTO_AGENT.id] ?? initialCapability(CTO_AGENT.id), DATASETS, {
      view: FINANCE_PRIVATE,
      fields: FLOW_A_REQUEST.fields,
    });
    setResult(res);
    const route = routeRequest(FLOW_A_REQUEST, CFO_ENVELOPE);
    setPending({ req: FLOW_A_REQUEST, route });
    pushLog("deny", `Flow A · Richard (CEO)'s agent denied finance_private → request raised`);
  };

  const runFlowB = () => {
    setActorId(ENG_AGENT.id);
    setSelView(FINANCE_PRIVATE);
    setSelFields([...FLOW_B_REQUEST.fields]);
    const res = brainQuery(caps[ENG_AGENT.id] ?? initialCapability(ENG_AGENT.id), DATASETS, {
      view: FINANCE_PRIVATE,
      fields: FLOW_B_REQUEST.fields,
    });
    setResult(res);
    const route = routeRequest(FLOW_B_REQUEST, CFO_ENVELOPE);
    setPending({ req: FLOW_B_REQUEST, route });
    pushLog("block", `Flow B · salary invariant — no approval path`);
  };

  const resetAll = () => {
    setCaps({
      [CTO_AGENT.id]: initialCapability(CTO_AGENT.id),
      [ENG_AGENT.id]: initialCapability(ENG_AGENT.id),
      [CFO.id]: initialCapability(CFO.id),
    });
    setResult(null);
    setPending(null);
    setLog([]);
  };

  return (
    <div className="app-shell">
      <header className="app-topbar">
        <div className="app-brand">
          <Mark />
          <span>Contextful</span>
        </div>

        <div className="app-doc-meta">
          <span className="app-doc-meta__title">{activeDoc.title}</span>
          <span className="cf-badge cf-badge--accent">on-prem · tailnet</span>
          <span className="cf-badge cf-badge--danger">salary · redacted</span>
        </div>

        <div className="app-topbar__right">
          <span
            className={`cf-badge ${syncStatus === "live" || livePeers.length > 0 ? "cf-badge--success" : ""}`}
            title="Live CRDT sync — cross-tab by default; set VITE_SYNC_URL for the relay"
          >
            {syncStatus === "live"
              ? `● sync live · ${livePeers.length} peer${livePeers.length === 1 ? "" : "s"}`
              : syncStatus === "connecting"
                ? "◌ connecting…"
                : syncStatus === "offline"
                  ? "○ relay offline"
                  : livePeers.length > 0
                    ? `● cross-tab · ${livePeers.length} peer${livePeers.length === 1 ? "" : "s"}`
                    : "◐ local"}
          </span>
          <span className="cf-presence" aria-label="collaborators present">
            {PRINCIPALS.map((p) => {
              const live = liveByPrincipal.get(p.id);
              return (
                <span
                  key={p.id}
                  className={`cf-presence__dot${live ? " cf-presence__dot--live" : ""}`}
                  style={{ background: dotColor(p.id) }}
                  title={`${p.name} · in room${live ? ` · ${live.mode}` : ""}`}
                >
                  {tag(p)}
                </span>
              );
            })}
            {guestPeers.map((p) => (
              <span
                key={peerKey(p)}
                className="cf-presence__dot cf-presence__dot--live"
                style={{ background: peerColor(p.principal) }}
                title={`${p.display_name} · ${p.mode}`}
              >
                {initialsOf(p.display_name) || "◆"}
              </span>
            ))}
          </span>
          <Link className="cf-btn cf-btn--ghost cf-btn--sm" to="/directory">
            Access control →
          </Link>
          <button className="cf-btn cf-btn--ghost cf-btn--sm" onClick={resetAll}>
            Reset demo
          </button>
        </div>
      </header>

      <div className="app-body">
        <aside className="app-sidebar" aria-label="Workspace">
          <p className="app-sidebar__label">Acting as</p>
          <div className="cf-actor-switch">
            {PRINCIPALS.map((p) => (
              <button
                key={p.id}
                className={`cf-actor${p.id === actorId ? " cf-actor--on" : ""}`}
                onClick={() => switchActor(p.id)}
              >
                <span className="cf-presence__dot" style={{ background: dotColor(p.id), marginLeft: 0 }}>
                  {tag(p)}
                </span>
                <span>
                  <span className="cf-actor__name">{p.name}</span>
                  <span className="cf-actor__sub">
                    {p.kind === "agent" ? `owner: ${p.owner}` : p.role}
                  </span>
                </span>
              </button>
            ))}
          </div>

          <p className="app-sidebar__label" style={{ marginTop: "var(--space-4)" }}>
            Run a flow
          </p>
          <div className="cf-stack">
            <button className="cf-btn cf-btn--secondary cf-btn--sm cf-block" onClick={runFlowA}>
              Flow A · request → approve
            </button>
            <button className="cf-btn cf-btn--secondary cf-btn--sm cf-block" onClick={runFlowB}>
              Flow B · salary invariant
            </button>
          </div>

          <p className="app-sidebar__label" style={{ marginTop: "var(--space-4)" }}>
            Documents
          </p>
          <ul className="app-doclist">
            {DOCS.map((d) => (
              <li key={d.id} className="app-doclist__row">
                <button
                  type="button"
                  className="app-doclist__item"
                  aria-current={d.id === activeDocId}
                  onClick={() => setActiveDocId(d.id)}
                  style={{
                    flex: 1,
                    textAlign: "left",
                    background: "none",
                    border: "none",
                    cursor: "pointer",
                    font: "inherit",
                    color: "inherit",
                  }}
                >
                  {d.title}
                </button>
                <DocDebugMenu docId={d.id} docTitle={d.title} />
              </li>
            ))}
          </ul>
        </aside>

        <main className="app-main">
          <section className="app-editor" aria-label="Document">
            <div className="app-editor__inner">
              <span className="cf-eyebrow">Pied Piper · Finance · Engineering</span>
              <h1>{activeDoc.title}</h1>
              <div className="app-editor__metarow">
                <span className="cf-badge">Shared with 5 + 3 agents</span>
                <span className="cf-badge cf-badge--success">
                  {syncStatus === "live"
                    ? "Live"
                    : livePeers.length > 0
                      ? "Live · cross-tab"
                      : "Local"}
                </span>
                <span className="cf-badge cf-badge--primary">acting: {actor.name}</span>
              </div>

              <div className="app-editor__body">
                {room.editor ? (
                  <Suspense fallback={<p className="weaver-loading">Waking the editor…</p>}>
                    <WeaverSurface
                      editor={room.editor}
                      peers={livePeers}
                      onCursorChange={room.setCursor}
                      self={selfIdentity}
                      mentionPrincipals={mentionPrincipals}
                    />
                  </Suspense>
                ) : (
                  <p className="weaver-loading">Waking the editor…</p>
                )}
              </div>

              <QueryResult result={result} actorName={actor.name} />
            </div>
          </section>

          <aside className="app-agentpanel" aria-label="Query console">
            <div>
              <p className="app-agentpanel__label">brain.query — capability-filtered</p>
              <div className="cf-card cf-stack">
                <label className="cf-field-label">View</label>
                <div className="cf-seg">
                  {VIEWS.map((v) => (
                    <button
                      key={viewId(v.view)}
                      className={`cf-seg__btn${viewId(v.view) === viewId(selView) ? " cf-seg__btn--on" : ""}`}
                      onClick={() => switchView(v.view)}
                    >
                      {v.label}
                    </button>
                  ))}
                </div>

                <label className="cf-field-label">Fields</label>
                <div className="cf-chips">
                  {columns.map((c) => (
                    <label key={c} className={`cf-chip${PRIVATE_FIELDS.has(c) ? " cf-chip--private" : ""}`}>
                      <input
                        type="checkbox"
                        checked={selFields.includes(c)}
                        onChange={() => toggleField(c)}
                      />
                      {c}
                    </label>
                  ))}
                </div>

                <button className="cf-btn cf-btn--primary cf-btn--sm cf-block" onClick={runQuery}>
                  Run query as {actor.name}
                </button>
                {canRequest && (
                  <button className="cf-btn cf-btn--secondary cf-btn--sm cf-block" onClick={requestAccess}>
                    Request access · {neededFields.join(", ")}
                  </button>
                )}
              </div>
            </div>

            {pending && (
              <div>
                <p className="app-agentpanel__label">Permission request</p>
                <div className="cf-card">
                  <p className="app-request__title">
                    <span
                      className="cf-presence__dot"
                      style={{ background: dotColor(pending.req.requester), marginLeft: 0 }}
                    >
                      {tag(PRINCIPALS.find((p) => p.id === pending.req.requester)!)}
                    </span>
                    {PRINCIPALS.find((p) => p.id === pending.req.requester)!.name} wants access
                  </p>
                  <p className="app-request__reason">&ldquo;{pending.req.reason}&rdquo;</p>
                  <div className="app-request__scope">
                    read view({pending.req.view.source}, {pending.req.view.view})
                    <br />
                    fields: [{pending.req.fields.join(", ")}]<br />
                    rows: team in {"{eng, ops, sales, finance}"}
                    <br />
                    deny: employee_salary &nbsp;·&nbsp; ttl: {pending.req.ttl}
                  </div>

                  {pending.route.decision === "forbidden" ? (
                    <p className="cf-forbidden">⛔ {pending.route.reason}</p>
                  ) : pending.route.decision === "auto" ? (
                    <p className="cf-text-muted" style={{ marginTop: "var(--space-3)", fontSize: "var(--text-sm)" }}>
                      Auto-approved by envelope.
                    </p>
                  ) : (
                    <div className="app-request__actions">
                      <button className="cf-btn cf-btn--primary cf-btn--sm" onClick={() => applyGrant(pending.req)}>
                        Approve (scoped)
                      </button>
                      <button className="cf-btn cf-btn--ghost cf-btn--sm" onClick={denyRequest}>
                        Deny
                      </button>
                    </div>
                  )}
                </div>
              </div>
            )}

            <div>
              <p className="app-agentpanel__label">Audit trail</p>
              <div className="cf-card cf-log">
                {log.length === 0 ? (
                  <p className="cf-text-muted" style={{ fontSize: "var(--text-sm)" }}>
                    Run a query to populate the audit trail.
                  </p>
                ) : (
                  <ul>
                    {log.map((e) => (
                      <li key={e.id} className={`cf-log__row cf-log__row--${e.kind}`}>
                        <span className="cf-log__tag">{e.kind}</span>
                        {e.text}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          </aside>
        </main>
      </div>
    </div>
  );
}

function QueryResult({ result, actorName }: { result: BrainResult | null; actorName: string }) {
  if (!result) {
    return (
      <div className="cf-result cf-result--empty">
        <p className="cf-text-muted">No query run yet. Use the console on the right.</p>
      </div>
    );
  }

  if (!result.ok) {
    return (
      <div className="cf-result cf-result--deny">
        <p className="cf-result__head">⛔ Denied · {result.reason}</p>
        <p>{result.answer}</p>
      </div>
    );
  }

  const cols = result.rows.length ? Object.keys(result.rows[0]) : result.fields;
  return (
    <div className="cf-result cf-result--ok">
      <p className="cf-result__head">
        ✓ {actorName} · {result.rows.length} row(s)
        {result.redacted.length > 0 && (
          <span className="cf-badge cf-badge--danger" style={{ marginLeft: "var(--space-2)" }}>
            redacted: {result.redacted.join(", ")}
          </span>
        )}
      </p>
      <p className="cf-result__answer">{result.answer}</p>
      {result.rows.length > 0 && (
        <table className="cf-table">
          <thead>
            <tr>
              {cols.map((c) => (
                <th key={c}>{c}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {result.rows.map((r, i) => (
              <tr key={i}>
                {cols.map((c) => (
                  <td key={c}>{String(r[c] ?? "—")}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
