---
name: generate-arts
description: Generate brand illustrations for the landing page via Vercel AI Gateway and wire them into apps/landing. Use when asked to create, regenerate, restyle, or add landing-page / design-system artwork.
---

# Generate landing-page arts

Pipeline that turns the design system into on-brand illustrations: design tokens → prompt manifest → Vercel AI Gateway image generation → `apps/landing/public/arts/` → landing page.

## Key files

| File | Role |
| --- | --- |
| `apps/landing/arts.manifest.mjs` | Source of truth: one entry per illustration (id, prompt, alt, aspect ratio, dimensions) plus the shared `stylePrompt` derived from the design system |
| `apps/landing/scripts/generate-arts.mjs` | Generation script (Node, AI SDK via Vercel AI Gateway) |
| `apps/landing/public/arts/` | Output directory — generated images, served at `/arts/<id>.png` |
| `packages/design-system/tokens.json` | Brand palette + pillars the `stylePrompt` must stay in sync with |
| `apps/landing/src/pages/index.astro` | Renders each art slot only if its file exists (build-time `existsSync` check via the `art()` helper) |

## Pipeline steps

1. **Check the API key.** `AI_GATEWAY_API_KEY` is stored dotenvx-encrypted in `.env.production` at the repo root (private decryption key in the untracked `.env.keys`). Run generation through dotenvx:
   ```sh
   npx -y @dotenvx/dotenvx run -f .env.production -- pnpm --filter landing arts:generate
   ```
   If decryption fails (no `.env.keys` on this machine), ask the user for the key or the `.env.keys` file — never hardcode it.
2. **Sync style with the design system.** If `packages/design-system/tokens.json` changed (palette, brand pillars, direction), update `stylePrompt` in the manifest to match before generating. The style constraints (flat vector, limited indigo/sky/amber palette, no text/logos) keep outputs consistent across runs — preserve them unless the user asks for a new art direction.
3. **Edit the manifest, not the script.** New illustration = new entry in `arts` with a unique kebab-case `id`, a scene-only `prompt` (style comes from `stylePrompt`), honest `alt` text, `aspectRatio`, and `width`/`height` matching that ratio.
4. **Generate:**
   ```sh
   pnpm --filter landing arts:generate                      # only missing arts (wrap in dotenvx run, see step 1)
   pnpm --filter landing arts:generate -- --only hero       # specific id(s), comma-separated
   pnpm --filter landing arts:generate -- --force           # regenerate all
   ```
   Default model is `google/gemini-2.5-flash-image` through the gateway; override per-entry with `model: "creator/model-id"` in the manifest. The script post-processes every image with `sharp`: resize to manifest dimensions → flood-fill the near-white background to transparency (edge-connected only, so white fills inside shapes survive) → trim transparent margins → palette PNG (keeps files well under the ~300 KB budget). Final files are transparent silhouettes whose dimensions differ from the manifest; `index.astro` reads the real dimensions from the file at build time.
5. **Review the output.** Read each generated image to verify it is on-brand (palette, no embedded text, scene matches the prompt). Regenerate misses with `--only <id> --force`, tightening the prompt.
6. **Wire into the page.** Existing slots (`hero`, `feature-*` above the feature cards, `how-banner` in the How-it-works section) appear automatically on next build — no page edits needed. For a brand-new slot, add a conditional render in `index.astro` using the `art("<id>")` helper, with `width`/`height` set (CLS budget) and `loading="lazy"` unless above the fold.
   **Presentation and motion conventions live in the repo-root `PRESENTATION.md` — follow it.** In short: free-flow Fibery/PostHog-style placement (never boxed; `.cf-art` drop-shadow silhouettes breaking grids, overflowing cards, straddling section boundaries), `.cf-float` ambient drift + `.cf-reveal`/`.cf-reveal--fade` staggered scroll entrances, `.orb` accents across boundaries, and `prefers-reduced-motion` respected throughout.
7. **Verify.** `pnpm --filter landing build` then `pnpm --filter landing preview` — confirm images render and the build passes with no broken references.

## Constraints

- Images are committed to git (no build-time generation — builds must not call the gateway or require the key).
- Never put text/words inside illustrations; copy belongs in HTML.
- Alt text in the manifest must describe the actual image, not the feature it decorates.
- Per repo SEO rules: keep `width`/`height` on every `<img>` and respect Core Web Vitals budgets (LCP < 2.5s — keep the hero image eager + `fetchpriority="high"`, everything else lazy).
