import { effectiveCapability, viewId, type Capability } from "@superai2026/protocol/access";
import { NEVER_DELEGABLE } from "@superai2026/protocol/requests";

const SENSITIVE = new Set<string>(NEVER_DELEGABLE);

/**
 * Renders a token's *effective* capability as quiet chips (specs/03 §6.1):
 * view + ops + each granted field + row-scopes. A NEVER_DELEGABLE field
 * (employee_salary) surfaces as a danger chip, so "who can see salary" is
 * answerable at a glance. Computed via effectiveCapability() — the same
 * caps(child) ⊆ caps(parent) fold the host enforces, never a stored secret.
 */
export function CapChips({ cap }: { cap: Capability | undefined }) {
  const eff = cap ? effectiveCapability(cap) : null;
  if (!eff) {
    return <span className="cf-text-muted ac-caps__empty">no capability</span>;
  }
  return (
    <div className="ac-caps">
      <span className="cf-badge cf-badge--primary">{viewId(eff.view)}</span>
      <span className="cf-badge">{[...eff.ops].join(" · ")}</span>
      {[...eff.fields].sort().map((f) => (
        <span key={f} className={`cf-badge${SENSITIVE.has(f) ? " cf-badge--danger" : ""}`}>
          {f}
        </span>
      ))}
      {eff.rows.map((r) => (
        <span key={r.field} className="cf-badge cf-badge--accent">
          {r.field} ∈ {`{${r.in.join(", ")}}`}
        </span>
      ))}
      {eff.deniedViews.map((v) => (
        <span key={viewId(v)} className="cf-badge cf-badge--danger">
          ⃠ {viewId(v)}
        </span>
      ))}
    </div>
  );
}
