# ART-DIRECTION.md — landing page visual conventions

How `apps/landing` presents content visually. The landing page must follow these rules; `.claude/skills/generate-arts/SKILL.md` (the illustration pipeline) defers to this document. For story, messaging, and narrative flow, the landing page follows the repo-root [PRESENTATION.md](../../PRESENTATION.md).

## Art direction

- Flat vector illustrations in the brand palette only: indigo (`#4f46e5`/`#6366f1`) dominant, sky (`#0ea5e9`) secondary, amber (`#f59e0b`) as a small accent — see `packages/design-system/tokens.json`. The shared style prompt lives in `apps/landing/arts.manifest.mjs`; keep it in sync with the design system.
- No text or letterforms inside illustrations — copy belongs in HTML. (Logos are the exception and carry their own style prompt in the manifest.)
- Subjects fill the generated frame; whitespace and composition come from CSS, never from margins baked into the image.
- All generated images are post-processed to transparent silhouettes (background knockout + trim) by `apps/landing/scripts/generate-arts.mjs`.

## Free-flow layout (Fibery / PostHog reference)

- Illustrations are **never boxed** in cards, frames, or bordered containers.
- Every art `<img>` gets the design-system `.cf-art` class: a layered `drop-shadow` that follows the artwork's silhouette, not a bounding box.
- Art breaks the grid: bleed past container columns (hero), overflow card edges (features), and straddle section boundaries (the how-it-works banner sits across the background change).
- Alternate small offsets and rotations (±1.5–2.5°) between sibling illustrations so placement feels organic, not templated.
- Blurred `.orb` accents in brand colors float behind content and across section boundaries for ambient depth.

## Motion

- **Scroll reveals**: sections, cards, and copy enter with `.cf-reveal` (rise + fade); illustrations use `.cf-reveal--fade` (opacity only — their transform belongs to the float animation). Stagger siblings with `--cf-reveal-delay` (~90–120ms steps). Driven by the IntersectionObserver script in `index.astro`, gated on `html.js` so no-JS users and crawlers see everything.
- **Ambient float**: every illustration and orb drifts continuously via `.cf-float`. Vary `--cf-float-dur` / `--cf-float-delay` per instance so nothing moves in lockstep; static offsets go in `--cf-float-tx` / `--cf-float-rot` so they compose with the animation.
- Motion primitives and the `--duration-entrance` token live in the design system (`packages/design-system/`), not in page CSS.
- **`prefers-reduced-motion` is non-negotiable**: floats stop, reveals show instantly.

## Performance & accessibility budget

- Every `<img>` has `width`/`height` (CLS < 0.1). Hero art is `loading="eager" fetchpriority="high"`; everything else `loading="lazy"`.
- Generated images stay under ~300 KB (the pipeline's palette-PNG step enforces this in practice).
- Decorative art inside `aria-hidden` containers uses empty `alt`; standalone art carries honest alt text describing the image, not the feature it decorates.
- Per the workspace SEO rules: content must be readable without JavaScript and Core Web Vitals budgets apply (LCP < 2.5s, INP < 200ms).
