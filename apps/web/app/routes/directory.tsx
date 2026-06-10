import type { MetaFunction } from "react-router";
import { Link } from "react-router";
import { type Principal } from "@superai2026/protocol/access";
import { humans, ownedAgents, principalColor, tag } from "@superai2026/protocol/scenario";
import { CapChips } from "@/components/CapChips";
import { useAccess } from "@/lib/accessStore";

export const meta: MetaFunction = () => [
  { title: "Company directory · Contextful" },
  {
    name: "description",
    content:
      "Who exists in the control plane and what each principal's capability token grants — every human and the agents they own, with salary surfaced as a danger chip.",
  },
];

function PrincipalDot({ p }: { p: Principal }) {
  return (
    <span className="cf-presence__dot" style={{ background: principalColor(p.id), marginLeft: 0 }}>
      {tag(p)}
    </span>
  );
}

/**
 * 6.1 Company directory — the control-plane principal registry: every human and,
 * nested under each, the agents they own. Each principal's effective capability
 * is shown as quiet chips; salary appears as a danger chip. Read-only and
 * membership-rooted — no data authority is mintable here.
 */
export default function DirectoryRoute() {
  const { caps } = useAccess();

  return (
    <div className="ac-shell">
      <main className="ac-page">
        <header className="ac-page__head">
          <span className="cf-eyebrow">Access control</span>
          <h1>Company directory</h1>
          <p className="cf-text-muted">
            Everyone in the control plane and what their capability token actually grants — computed
            with <code>effectiveCapability()</code>, the same <code>caps(child) ⊆ caps(parent)</code>{" "}
            fold the host enforces. Token secrets never leave the host; only the computed scope shows.
          </p>
        </header>

        <div className="ac-grid">
          {humans().map((h) => {
            const agents = ownedAgents(h.id);
            return (
              <article className="cf-card ac-principal" key={h.id}>
                <div className="ac-principal__head">
                  <PrincipalDot p={h} />
                  <div className="ac-principal__id">
                    <p className="ac-principal__name">{h.name}</p>
                    <p className="ac-principal__sub">human · {h.role}</p>
                  </div>
                </div>
                <CapChips cap={caps[h.id]} />

                <div className="ac-agents">
                  <p className="ac-agents__label">
                    Owns {agents.length} agent{agents.length === 1 ? "" : "s"}
                  </p>
                  {agents.length === 0 ? (
                    <p className="cf-text-muted ac-caps__empty">No agents.</p>
                  ) : (
                    agents.map((a) => (
                      <div className="ac-agent" key={a.id}>
                        <div className="ac-principal__head">
                          <PrincipalDot p={a} />
                          <div className="ac-principal__id">
                            <p className="ac-principal__name">{a.name}</p>
                            <p className="ac-principal__sub ac-mono">{a.id}</p>
                          </div>
                          <Link
                            className="cf-btn cf-btn--ghost cf-btn--sm ac-agent__action"
                            to={`/delegate?owner=${encodeURIComponent(h.id)}&agent=${encodeURIComponent(a.id)}`}
                          >
                            Delegate →
                          </Link>
                        </div>
                        <CapChips cap={caps[a.id]} />
                      </div>
                    ))
                  )}
                </div>
              </article>
            );
          })}
        </div>

        <p className="ac-note">
          Read-only · membership-rooted. No data authority is mintable here — delegation narrows a
          token you already hold, and the CFO root is the only minter of <code>finance_private</code>.
        </p>
      </main>
    </div>
  );
}
