# @superai2026/design-system

The **Contextful** design system — design tokens, base element styles, and framework-agnostic
component primitives. Direction: **Trust Indigo** (light-first, with a full dark theme).

Brand pillars: **Trust · Clarity · Security · Collaboration · Fluid**.

## Usage

Import the bundle once at your app root (works in Astro and Next.js — it's plain CSS):

```ts
import "@superai2026/design-system/styles.css";
```

Or import layers individually: `tokens.css`, `base.css`, `components.css`.

Then consume **semantic tokens** (never primitives) in your own CSS:

```css
.thing {
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-md);
}
```

And use component primitives by class:

```html
<a class="cf-btn cf-btn--primary cf-btn--lg">Watch the demo</a>
<span class="cf-badge cf-badge--accent">presence</span>
```

## Dark mode

Dark theme activates automatically with `prefers-color-scheme: dark`. To force a theme, set
`data-theme="dark"` or `data-theme="light"` on `<html>`.

## Files

| File | What |
| --- | --- |
| `tokens.css` | Primitive palette + semantic role tokens (+ dark theme) |
| `base.css` | Modern reset + element defaults wired to tokens |
| `components.css` | `cf-*` component primitives (button, card, badge, input, presence…) |
| `styles.css` | Entry point that imports all three in order |
| `tokens.json` | Reference mirror of tokens for tooling/docs |

Full documentation: [`specs/DESIGN-SYSTEM.md`](../../specs/DESIGN-SYSTEM.md).
