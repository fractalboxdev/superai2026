import type { MetaFunction } from "react-router";
import { type RouteDecision } from "@superai2026/protocol/requests";
import { principal, principalColor, resourceOwnerOf, tag } from "@superai2026/protocol/scenario";
import { AvatarDot } from "@/components/AvatarDot";
import { CapChips } from "@/components/CapChips";
import { useAccess, type InboxItem } from "@/lib/accessStore";

export const meta: MetaFunction = () => [
  { title: "Inbox · Contextful" },
  {
    name: "description",
    content:
      "Incoming agent access requests for resources you own. The routing decision shows as a badge; approve mints exactly the requested scope — a salary request renders no approve button at all.",
  },
];

const ROUTE_BADGE: Record<RouteDecision["decision"], { cls: string; label: string }> = {
  auto: { cls: "cf-badge--success", label: "auto" },
  escalate: { cls: "cf-badge--accent", label: "escalate" },
  forbidden: { cls: "cf-badge--danger", label: "forbidden" },
};

function Requester({ id }: { id: string }) {
  const p = principal(id);
  return (
    <div className="ac-request__who">
      <AvatarDot id={id} fallback={p ? tag(p) : "◆"} color={principalColor(id)} />
      <div className="ac-principal__id">
        <p className="ac-principal__name">{p?.name ?? id}</p>
        <p className="ac-principal__sub ac-mono">{id}</p>
      </div>
    </div>
  );
}

function RequestCard({ item }: { item: InboxItem }) {
  const { caps, approve, deny } = useAccess();
  const { req, route, status } = item;
  const badge = ROUTE_BADGE[route.decision];
  const rows = req.rowScope?.find((r) => r.field === "team")?.in;

  return (
    <article className="cf-card ac-request">
      <div className="ac-request__top">
        <Requester id={req.requester} />
        <span className={`cf-badge ${badge.cls}`}>{badge.label}</span>
      </div>

      <p className="ac-request__reason">&ldquo;{req.reason}&rdquo;</p>

      <div className="ac-request__scope">
        query view({req.view.source}, {req.view.view})<br />
        fields: [{req.fields.join(", ")}]<br />
        rows: team ∈ {rows ? `{${rows.join(", ")}}` : "all"}<br />
        doc: {req.doc} &nbsp;·&nbsp; ttl: {req.ttl}
      </div>

      {route.decision === "forbidden" ? (
        // The salary invariant as a UI affordance: no approve button is rendered.
        <p className="cf-forbidden">⛔ {route.reason}</p>
      ) : route.decision === "auto" ? (
        <p className="ac-status ac-status--auto">
          Inside the auto-approve envelope — already granted by the runtime, shown for the record.
        </p>
      ) : status === "pending" ? (
        <div className="ac-request__actions">
          <button className="cf-btn cf-btn--primary cf-btn--sm" onClick={() => approve(req.id)}>
            Approve (scoped)
          </button>
          <button className="cf-btn cf-btn--ghost cf-btn--sm" onClick={() => deny(req.id)}>
            Deny
          </button>
        </div>
      ) : status === "approved" ? (
        <div className="ac-status ac-status--ok">
          <p>✓ Approved — minted scoped token to {req.requester}, who retries and answers.</p>
          <CapChips cap={caps[req.requester]} />
        </div>
      ) : (
        <p className="ac-status ac-status--deny">✗ Denied — {req.requester} stays blocked.</p>
      )}
    </article>
  );
}

/**
 * 6.3 Inbox — the resource owner accepts or declines incoming agent access
 * requests. Each shows the requester, the requested scope, and the
 * routeRequest() decision as a badge. Approve mints exactly the requested scope
 * (salary always denied); a forbidden (salary) item renders no approve button.
 */
export default function InboxRoute() {
  const { requests, log, reset } = useAccess();
  // Every demo request is rooted at the same owner; name them as the inbox owner.
  const ownerId = requests[0] ? resourceOwnerOf(requests[0].req.view) : "cfo";
  const ownerName = principal(ownerId)?.name ?? ownerId;

  return (
    <div className="ac-shell">
      <main className="ac-page">
        <header className="ac-page__head ac-page__head--row">
          <div>
            <span className="cf-eyebrow">Access control</span>
            <h1>Inbox</h1>
            <p className="cf-text-muted">
              Incoming agent requests for resources owned by <strong>{ownerName}</strong>. Approve
              mints exactly the requested scope; a salary request is forbidden with no approve path.
            </p>
          </div>
          <button className="cf-btn cf-btn--ghost cf-btn--sm" onClick={reset}>
            Reset demo
          </button>
        </header>

        <div className="ac-inbox">
          {requests.length === 0 ? (
            <p className="cf-text-muted">No incoming requests.</p>
          ) : (
            requests.map((item) => <RequestCard key={item.req.id} item={item} />)
          )}
        </div>

        <section className="ac-audit">
          <p className="ac-card__label">Audit trail</p>
          <div className="cf-card cf-log">
            {log.length === 0 ? (
              <p className="cf-text-muted ac-caps__empty">
                Approve or deny a request to populate the audit trail.
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
        </section>
      </main>
    </div>
  );
}
