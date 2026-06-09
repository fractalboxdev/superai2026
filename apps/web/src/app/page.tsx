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

const docs = [
  { id: "finops", title: "Q3 FinOps Review", active: true },
  { id: "evals", title: "Agent workflow evals" },
  { id: "vendors", title: "Vendor consolidation" },
  { id: "budget", title: "2026 budget draft" },
];

export default function Home() {
  return (
    <div className="app-shell">
      <header className="app-topbar">
        <div className="app-brand">
          <Mark />
          <span>Contextful</span>
        </div>

        <div className="app-doc-meta">
          <span className="app-doc-meta__title">Q3 FinOps Review</span>
          <span className="cf-badge cf-badge--primary">capability: spend_by_team</span>
          <span className="cf-badge cf-badge--danger">salary · redacted</span>
        </div>

        <div className="app-topbar__right">
          <span className="cf-presence" aria-label="3 collaborators present">
            <span className="cf-presence__dot" style={{ background: "var(--cf-indigo-500)" }}>CT</span>
            <span className="cf-presence__dot" style={{ background: "var(--cf-sky-500)" }}>CF</span>
            <span className="cf-presence__dot" style={{ background: "var(--cf-amber-500)" }}>◆</span>
          </span>
          <button className="cf-btn cf-btn--primary cf-btn--sm">Share</button>
        </div>
      </header>

      <div className="app-body">
        <aside className="app-sidebar" aria-label="Documents">
          <p className="app-sidebar__label">Workspace</p>
          <ul className="app-doclist">
            {docs.map((d) => (
              <li key={d.id} className="app-doclist__item" aria-current={d.active ? "true" : undefined}>
                {d.title}
              </li>
            ))}
          </ul>
        </aside>

        <main className="app-main">
          <section className="app-editor" aria-label="Document">
            <div className="app-editor__inner">
              <span className="cf-eyebrow">Finance · Engineering · Operations</span>
              <h1>Q3 FinOps Review</h1>
              <div className="app-editor__metarow">
                <span className="cf-badge">Shared with 5 + 3 agents</span>
                <span className="cf-badge cf-badge--success">Live</span>
                <span className="cf-badge cf-badge--accent">on-prem · tailnet</span>
              </div>

              <div className="app-editor__body">
                <p>
                  Engineering reports Claude Code utilization is up across the platform team. The open
                  question for this review: is the spend justified once credits and our discount tier
                  are applied?
                </p>
                <p className="muted">
                  The CTO&rsquo;s agent can read team-level spend, but the salary column stays redacted —
                  no token in this document grants it. Below, the CFO has approved a scoped pull so the
                  brain can answer net-of-credits.
                </p>
                <p>
                  <strong>Finding.</strong> Net spend after credits is down 18% month-over-month while
                  agent-completed workflows rose 31%. Recommendation: keep the current tier; revisit at
                  Q4 once the Ops evals close.
                </p>
              </div>
            </div>
          </section>

          <aside className="app-agentpanel" aria-label="Agents and requests">
            <p className="app-agentpanel__label">Permission request</p>
            <div className="cf-card">
              <p className="app-request__title">
                <span className="cf-presence__dot" style={{ background: "var(--cf-indigo-500)", marginLeft: 0 }}>CT</span>
                CTO&rsquo;s agent wants access
              </p>
              <p className="app-request__reason">
                &ldquo;To judge whether this month&rsquo;s Claude spend is net-justified after credits.&rdquo;
              </p>
              <div className="app-request__scope">
                read view(stripe, finance_private)<br />
                fields: [credits, discount_tier]<br />
                rows: team in &#123;eng, ops, sales, finance&#125;<br />
                deny: employee_salary &nbsp;·&nbsp; ttl: 7d
              </div>
              <div className="app-request__actions">
                <button className="cf-btn cf-btn--primary cf-btn--sm">Approve (scoped)</button>
                <button className="cf-btn cf-btn--ghost cf-btn--sm">Deny</button>
              </div>
            </div>

            <p className="app-agentpanel__label">Brain answer</p>
            <div className="cf-card">
              <p className="app-answer__by">CFO agent · via brain.query</p>
              <p>
                Net spend after credits: <strong>−18% MoM</strong>. Utilization <strong>+31%</strong>.
                Discount tier applied. <span className="cf-text-muted">Salary withheld (redacted).</span>
              </p>
            </div>
          </aside>
        </main>
      </div>
    </div>
  );
}
