import { useState } from "react";
import type { MetaFunction } from "react-router";
import { useSearchParams } from "react-router";
import {
  effectiveCapability,
  viewId,
  type Capability,
  type RowScope,
} from "@superai2026/protocol/access";
import { delegableFields } from "@superai2026/protocol/requests";
import { DATASETS, humans, ownedAgents } from "@superai2026/protocol/scenario";
import { CapChips } from "@/components/CapChips";
import { useAccess } from "@/lib/accessStore";

export const meta: MetaFunction = () => [
  { title: "Delegate to my agent · Contextful" },
  {
    name: "description",
    content:
      "Hand one of your own agents a subset of a token you already hold. Narrow-only by construction — fields outside your capability aren't selectable, and salary is never offered.",
  },
];

const TTL_OPTIONS = ["1d", "7d", "30d"] as const;

/** Teams the owner can scope to: their own row-scope, else every team in the view. */
function teamsFor(cap: Capability | undefined): string[] {
  const eff = cap ? effectiveCapability(cap) : null;
  if (!eff) return [];
  const owned = eff.rows.find((r) => r.field === "team")?.in;
  if (owned) return owned;
  const ds = DATASETS.find((d) => viewId(d.view) === viewId(eff.view));
  return ds ? [...new Set(ds.rows.map((r) => String(r.team)))] : [];
}

/** Owners who can delegate: humans who own at least one agent. */
const owners = () => humans().filter((h) => ownedAgents(h.id).length > 0);

/**
 * 6.2 Delegation — a member hands one of their own agents a subset of a token
 * they already hold. Intra-owner, no approval. It can only NARROW: fields
 * outside effectiveCapability(owner) aren't selectable and employee_salary is
 * never offered (delegableFields drops NEVER_DELEGABLE). The result —
 * caps(agent) ⊆ caps(owner) — is shown, not just asserted.
 */
export default function DelegateRoute() {
  const { caps, delegate } = useAccess();
  const [params] = useSearchParams();

  const ownerOpts = owners();
  const initialOwner = ownerOpts.find((h) => h.id === params.get("owner"))?.id ?? ownerOpts[0].id;
  const initialAgents = ownedAgents(initialOwner);
  const initialAgent =
    initialAgents.find((a) => a.id === params.get("agent"))?.id ?? initialAgents[0].id;

  const [ownerId, setOwnerId] = useState(initialOwner);
  const [agentId, setAgentId] = useState(initialAgent);
  const [selFields, setSelFields] = useState<string[]>(() => delegableFields(caps[initialOwner]));
  const [selTeams, setSelTeams] = useState<string[]>(() => teamsFor(caps[initialOwner]));
  const [ttl, setTtl] = useState<string>("7d");
  const [doneAgent, setDoneAgent] = useState<string | null>(null);

  const ownerCap = caps[ownerId];
  const offerFields = delegableFields(ownerCap);
  const offerTeams = teamsFor(ownerCap);
  const agentsOfOwner = ownedAgents(ownerId);

  const changeOwner = (id: string) => {
    setOwnerId(id);
    setAgentId(ownedAgents(id)[0]?.id ?? "");
    setSelFields(delegableFields(caps[id]));
    setSelTeams(teamsFor(caps[id]));
    setDoneAgent(null);
  };

  const toggle = (list: string[], v: string) =>
    list.includes(v) ? list.filter((x) => x !== v) : [...list, v];

  const submit = () => {
    const rows: RowScope[] =
      selTeams.length && selTeams.length < offerTeams.length
        ? [{ field: "team", in: selTeams }]
        : [];
    delegate({ ownerId, agentId, allowFields: selFields, rows, ttl });
    setDoneAgent(agentId);
  };

  // caps(agent) ⊆ caps(owner) — computed after delegation, shown not asserted.
  const subsetHolds = (() => {
    if (!doneAgent) return null;
    const o = effectiveCapability(caps[ownerId]);
    const a = effectiveCapability(caps[doneAgent]);
    if (!o || !a) return false;
    return [...a.fields].every((f) => o.fields.has(f));
  })();

  return (
    <div className="ac-shell">
      <main className="ac-page">
        <header className="ac-page__head">
          <span className="cf-eyebrow">Access control</span>
          <h1>Delegate to my agent</h1>
          <p className="cf-text-muted">
            Narrow a token you already hold and hand the subset to one of your own agents. No
            approval — you can only subtract. <code>employee_salary</code> is never offered.
          </p>
        </header>

        <div className="ac-form-grid">
          <form
            className="cf-card cf-stack ac-form"
            onSubmit={(e) => {
              e.preventDefault();
              submit();
            }}
          >
            <div>
              <label className="cf-field-label" htmlFor="owner">
                Acting as (owner)
              </label>
              <select
                id="owner"
                className="cf-input"
                value={ownerId}
                onChange={(e) => changeOwner(e.target.value)}
              >
                {ownerOpts.map((h) => (
                  <option key={h.id} value={h.id}>
                    {h.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="cf-field-label" htmlFor="agent">
                Delegate to (own agent)
              </label>
              <select
                id="agent"
                className="cf-input"
                value={agentId}
                onChange={(e) => {
                  setAgentId(e.target.value);
                  setDoneAgent(null);
                }}
              >
                {agentsOfOwner.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name} · {a.id}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <p className="cf-field-label">
                Fields <span className="ac-hint">(only what you hold; salary excluded)</span>
              </p>
              <div className="cf-chips">
                {offerFields.map((f) => (
                  <label key={f} className="cf-chip">
                    <input
                      type="checkbox"
                      checked={selFields.includes(f)}
                      onChange={() => setSelFields((s) => toggle(s, f))}
                    />
                    {f}
                  </label>
                ))}
              </div>
            </div>

            <div>
              <p className="cf-field-label">
                Row scope <span className="ac-hint">(teams · ∩ with yours)</span>
              </p>
              <div className="cf-chips">
                {offerTeams.map((t) => (
                  <label key={t} className="cf-chip">
                    <input
                      type="checkbox"
                      checked={selTeams.includes(t)}
                      onChange={() => setSelTeams((s) => toggle(s, t))}
                    />
                    {t}
                  </label>
                ))}
              </div>
            </div>

            <div>
              <label className="cf-field-label" htmlFor="ttl">
                TTL
              </label>
              <select id="ttl" className="cf-input" value={ttl} onChange={(e) => setTtl(e.target.value)}>
                {TTL_OPTIONS.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>

            <button
              type="submit"
              className="cf-btn cf-btn--primary cf-block"
              disabled={!agentId || selFields.length === 0}
            >
              Delegate {selFields.length} field{selFields.length === 1 ? "" : "s"} → {agentId || "—"}
            </button>
          </form>

          <div className="cf-stack">
            <div className="cf-card">
              <p className="ac-card__label">You hold ({ownerId})</p>
              <CapChips cap={ownerCap} />
            </div>

            <div className="cf-card">
              <p className="ac-card__label">{doneAgent ? "Agent now holds" : "Agent currently holds"}</p>
              <CapChips cap={caps[doneAgent ?? agentId]} />
              {doneAgent && (
                <p className={`ac-subset ${subsetHolds ? "ac-subset--ok" : "ac-subset--bad"}`}>
                  {subsetHolds ? "✓" : "✗"} caps(agent) ⊆ caps(owner)
                  {subsetHolds ? " — narrowed, never widened." : " — invariant broken!"}
                </p>
              )}
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
