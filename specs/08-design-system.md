# 08 · Design System

**Anchors:** `packages/design-system` (`@superai2026/design-system`) — spec'd here; portable from `../superai2026/packages/design-system`. Spec-only build this pass.

**Direction:** *Trust Indigo* — light-first with a full dark theme. Tokens are plain **CSS custom properties** (framework-agnostic, no build step, no Tailwind). One design system shared by the marketing site (Astro) and the product (Next.js).

## 1. Brand pillars

| Pillar | Expressed as |
|---|---|
| **Trust** | deep indigo primary, generous whitespace, soft elevation, no harsh edges |
| **Clarity** | high text contrast, strict type scale, slate neutrals, one idea per surface |
| **Security** | confident (not loud) color, restrained palette, capability/scope as quiet badges |
| **Collaboration** | warm **amber** accent reserved for presence + people (avatars, live cursors) |
| **Fluid** | fluid `clamp()` type, soft gradients, smooth `ease-out` motion, rounded geometry |

**Voice & tone:** plain-spoken, precise. Lead with the direct claim (*"The CTO's agent can't read the CEO's salary — provably"*), then explain. No fear-mongering.

**Mark:** an open "C" arc (context, collaboration) inside a rounded indigo→sky gradient square (containment, scope), with a single **amber dot** (a person / live presence) in the opening. Ships as `@superai2026/design-system/logo.svg` and as favicon for both apps.

## 2. Tokens (two layers)

- **Primitives** (`--cf-*`): raw palette + scales, theme-agnostic, never used directly in app code.
- **Semantic** (`--color-*`, `--space-*`, `--radius-*`, …): role tokens. Dark mode re-points **semantic only**; primitives stay fixed.

**Palette:** Indigo (primary/trust, `indigo-600` `#4f46e5`) · Sky (secondary/links, `sky-500` `#0ea5e9`) · Amber (**presence/collaboration only**, `amber-500` `#f59e0b`) · Slate (neutrals, text `slate-900`, paper `slate-50`). Status: success `#16a34a`, warning `amber-600`, danger `#dc2626`.

**Semantic roles (excerpt):** `--color-bg` (paper) · `--color-surface` (cards) · `--color-surface-sunken` · `--color-text` / `--color-text-muted` · `--color-border` · `--color-primary` (indigo-600) · `--color-accent` (**amber — presence only**) · `--color-link` · `--color-focus`.

**Typography:** Inter (`--font-sans` → Geist → system-ui); Geist Mono → `ui-monospace`. Scale `--text-xs … --text-6xl`; display sizes **fluid** via `clamp()`. Headings: tight leading/tracking, `text-wrap: balance`. Body: `--leading-relaxed` (1.65), `text-wrap: pretty`, max `--container-prose` (68ch).

**Space / radius / elevation / motion:** 4px base space scale · radius `xs 4 → 2xl 28 → full` · five soft slate-tinted shadows + `--shadow-ring-primary` (indigo CTA glow) · `--ease-out` `cubic-bezier(.16,1,.3,1)`, durations fast/normal/slow `120/200/320`, collapse to `0ms` under `prefers-reduced-motion`. Gradients: `--gradient-brand` (indigo→sky), `--gradient-hero` (radial indigo), `--gradient-collab` (amber, presence flourishes).

## 3. Component primitives (`cf-*`)

| Class | Variants | Use |
|---|---|---|
| `.cf-btn` | `--primary`/`--secondary`/`--ghost`/`--lg`/`--sm` | actions; primary = brand gradient + glow |
| `.cf-card` | `--raised`/`--interactive` | content surfaces |
| `.cf-badge` | `--primary`/`--accent`/`--success`/`--danger` | status, scope, **capability chips** |
| `.cf-input` | — | text fields; tokenized focus ring |
| `.cf-eyebrow` | — | section kicker |
| `.cf-presence` / `.cf-presence__dot` | — | overlapping avatar/presence stack (collaboration) |
| `.cf-container` | `--wide`/`--prose` | centered max-width layout |
| utilities | — | `.cf-text-gradient`, `.cf-text-muted`, `.cf-stack`, `.cf-visually-hidden` |

## 4. Consumption & governance

Both apps depend on `"@superai2026/design-system": "workspace:*"` and import once at the root:

```ts
import "@superai2026/design-system/styles.css";
```

App-specific styles reference **semantic tokens only**. Package exports: `./styles.css`, `./tokens.css`, `./base.css`, `./components.css`, `./tokens.json`, `./logo.svg`.

**Accessibility:** body text WCAG AA (≥4.5:1); large text ≥3:1; never amber for body text on light. A new color pairing must pass AA before merge. **Rebrand blast radius = one file** (`tokens.css` + `tokens.json` mirror). Keep OG image (1200×630), theme color, and wordmark in sync with tokens ([CLAUDE.md SEO rules](../CLAUDE.md)).

## 5. Scaffold / Status

Spec-only this pass. The reference implementation at `../superai2026/packages/design-system` (`tokens.css` 312 lines, `components.css` 169 lines, `tokens.json`, `logo.svg`) is portable as-is into `packages/design-system` when the web UI work begins.
