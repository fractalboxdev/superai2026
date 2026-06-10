import { useState } from "react";
import type { MetaFunction } from "react-router";

export const meta: MetaFunction = () => [
  { title: "Connectors · Contextful" },
  {
    name: "description",
    content:
      "Data sources feeding the local-first company brain. Connectors run on this machine via sync ingest — raw payloads never leave the host, and secrets are scrubbed before anything is indexed.",
  },
];

type Connector = {
  id: string;
  name: string;
  desc: string;
  connected: boolean;
  /** Brand mark under public/connectors/. */
  logo: string;
  /** Capability views this source emits into the brain. */
  views: string[];
  auth?: string;
  lastSync?: string;
};

// Mirrors the backend's runtime-selected sources: `sync ingest --source stripe`
// is live (test mode) and Exa is the egress-firewalled world memory; the rest
// are placeholders for sources the ingest pipeline doesn't wire up yet.
const CONNECTORS: Connector[] = [
  {
    id: "stripe",
    name: "Stripe",
    desc: "Billing events → spend and finance views, synthesized per team and period.",
    connected: true,
    logo: "/connectors/stripe.svg",
    views: ["stripe/spend_by_team", "stripe/finance_private"],
    auth: "restricted key · test mode",
    lastSync: "2 min ago",
  },
  {
    id: "exa",
    name: "Exa",
    desc: "World memory — web research fetched behind the egress firewall, cached on-host.",
    connected: true,
    logo: "/connectors/exa.svg",
    views: ["exa/world_memory"],
    auth: "API key",
    lastSync: "14 min ago",
  },
  {
    id: "posthog",
    name: "PostHog",
    desc: "Product analytics — events and funnels for usage views.",
    connected: false,
    logo: "/connectors/posthog.svg",
    views: [],
  },
  {
    id: "github",
    name: "GitHub",
    desc: "Repos, PRs and review activity for engineering views.",
    connected: false,
    logo: "/connectors/github.svg",
    views: [],
  },
  {
    id: "notion",
    name: "Notion",
    desc: "Wiki pages as ingestable documents.",
    connected: false,
    logo: "/connectors/notion.svg",
    views: [],
  },
  {
    id: "gdrive",
    name: "Google Drive",
    desc: "Docs and sheets as ingestable documents.",
    connected: false,
    logo: "/connectors/googledrive.svg",
    views: [],
  },
];

function Toggle({ on, label, onChange }: { on: boolean; label: string; onChange: () => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={`${label} — ${on ? "connected" : "off"}`}
      className={`cn-toggle${on ? " cn-toggle--on" : ""}`}
      onClick={onChange}
    >
      <span className="cn-toggle__knob" />
    </button>
  );
}

/** Connectors — the data sources this host ingests, and the views each one emits. */
export default function ConnectorsRoute() {
  // Demo-local state: toggling flips the card, nothing is provisioned.
  const [on, setOn] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(CONNECTORS.map((c) => [c.id, c.connected])),
  );
  const toggle = (id: string) => setOn((s) => ({ ...s, [id]: !s[id] }));

  return (
    <div className="ac-shell">
      <main className="ac-page">
        <header className="ac-page__head">
          <span className="cf-eyebrow">Data sources</span>
          <h1>Connectors</h1>
          <p className="cf-text-muted">
            Sources feeding the brain on <em>this machine</em> — each runs locally via{" "}
            <code>sync ingest --source &lt;id&gt;</code>. Raw payloads never leave the host;
            inbound secrets are scrubbed before anything is indexed, and each source only ever
            surfaces through its capability views.
          </p>
        </header>

        <div className="ac-grid">
          {CONNECTORS.map((c) => {
            const isOn = on[c.id];
            return (
              <article className={`cf-card cn-card${isOn ? "" : " cn-card--off"}`} key={c.id}>
                <div className="cn-card__head">
                  <span className="cn-card__logo">
                    <img src={c.logo} alt="" width="18" height="18" loading="lazy" />
                  </span>
                  <p className="cn-card__name">{c.name}</p>
                  <div className="cn-card__status">
                    {isOn ? (
                      <span className="cf-badge cf-badge--success">connected</span>
                    ) : (
                      <span className="cf-badge">off</span>
                    )}
                    <Toggle on={isOn} label={c.name} onChange={() => toggle(c.id)} />
                  </div>
                </div>
                <p className="cn-card__desc">{c.desc}</p>

                {isOn &&
                  (c.views.length > 0 ? (
                    <>
                      <div className="cn-card__views">
                        {c.views.map((v) => (
                          <span className="cf-badge cf-badge--primary" key={v}>
                            <code>{v}</code>
                          </span>
                        ))}
                      </div>
                      <dl className="cn-card__facts">
                        <dt>Auth</dt>
                        <dd>{c.auth}</dd>
                        <dt>Last ingest</dt>
                        <dd>{c.lastSync}</dd>
                      </dl>
                    </>
                  ) : (
                    <p className="cn-card__pending">Waiting for first ingest…</p>
                  ))}
              </article>
            );
          })}
        </div>

        <p className="ac-note">
          Connecting a source grants the <em>host</em> ingest authority only — what each principal
          can read from it is decided per view in the directory, never by the connector itself.
        </p>
      </main>
    </div>
  );
}
