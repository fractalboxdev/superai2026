// Presence rendering helpers (spec 01 §5, upstream weaver PR #35): agents and
// collaborators draw from ONE identity set — the roster chips in the topbar
// and the caret overlay in the editor key and color peers identically.

import type { PresenceState } from "@superai2026/protocol/sync";
import { PRINCIPALS, principalColor } from "@superai2026/protocol/scenario";

/** Caret-overlay / roster key — one entry per session, not per principal. */
export const peerKey = (p: PresenceState): string =>
  `${p.principal}#${p.session ?? ""}`;

export const isScenarioPrincipal = (id: string): boolean =>
  PRINCIPALS.some((p) => p.id === id);

// Distinct hues for peers outside the scenario cast (other humans / tabs).
// Design-system palette tokens only — never hardcoded values.
const GUEST_PALETTE = [
  "var(--cf-green-500)",
  "var(--cf-sky-500)",
  "var(--cf-indigo-500)",
  "var(--cf-amber-500)",
  "var(--cf-red-500)",
];

const hashCode = (s: string): number => {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h);
};

/**
 * Stable color per principal: scenario principals reuse the console's dot
 * palette; anyone else hashes into the guest palette so two collaborators
 * are unlikely to share a hue.
 */
export const peerColor = (principal: string): string =>
  isScenarioPrincipal(principal)
    ? principalColor(principal)
    : GUEST_PALETTE[hashCode(principal) % GUEST_PALETTE.length]!;

// SV-cast comic avatars (PRESENTATION.md "Cast avatars — from `assets/`"):
// exported from the role-named art in `assets/` to `public/cast/` and used
// everywhere a persona appears. Principal ids are wire identifiers, display
// roles are the theme — hence the id → role-file indirection (the `cto` id is
// Richard (CEO), the `eng` id is Dinesh (CTO)).
const CAST_AVATARS: Record<string, string> = {
  cto: "/cast/ceo.png",
  cfo: "/cast/cfo.png",
  eng: "/cast/cto.png",
};

/** Comic avatar for a principal; agents share the robot, guests get none. */
export const avatarOf = (principal: string): string | undefined =>
  principal.startsWith("agent:") ? "/cast/agent.png" : CAST_AVATARS[principal];

/** Up-to-two-letter initials for roster chips, e.g. "Ada Lovelace" → "AL". */
export const initialsOf = (label: string): string =>
  label
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]!.toUpperCase())
    .join("");
