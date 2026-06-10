---
name: export-pptx
description: Export the Slidev deck (slides/slides.md) to PowerPoint (.pptx) using Slidev's export feature. Use when asked to export the presentation/deck/slides to PowerPoint, .ppt, or .pptx, or to produce a shareable PowerPoint file of the talk.
---

# export-pptx — export the Slidev deck to PowerPoint

Exports `slides/slides.md` to a `.pptx` via `slidev export --format pptx`. Slidev renders
each slide headlessly with Playwright Chromium and embeds it as a full-slide image; speaker
notes (`<!-- ... -->` blocks) are carried into the PowerPoint notes pane. The result is
**not editable text** in PowerPoint — it's a pixel-faithful deck for sharing/presenting.

## How to run

1. From `slides/` (Node must be on PATH — see Troubleshooting):

   ```sh
   cd slides && pnpm run export:pptx
   ```

   This runs `mkdir -p dist && slidev export --format pptx --output dist/contextful-slides.pptx`
   (Slidev does not create the output directory itself — keep the `mkdir -p`).

2. Useful flag variations (append after `pnpm run export:pptx --`):
   - `--with-clicks` — one PPTX slide **per click step** (animations become separate slides).
     Default exports one slide per Slidev slide with all clicks revealed.
   - `--dark` — export the dark variant.
   - `--timeout 60000` — raise per-page render timeout if Mermaid-heavy slides time out.
   - `--output dist/<name>.pptx` — different filename.

3. Verify the file exists and is non-trivial (`ls -lh slides/dist/*.pptx` — expect ≥ a few
   hundred KB), then report the absolute path. Do not commit `slides/dist/`.

## Prerequisites (already set up — re-check only on failure)

- `playwright-chromium` is a devDependency of `slides/package.json`, and is listed under
  `allowBuilds` in `slides/pnpm-workspace.yaml` (its postinstall downloads the browser).
  If the browser cache was wiped: `cd slides && pnpm rebuild playwright-chromium`.
- A dev server is NOT required — export spins up its own instance.

## Troubleshooting

- **`env: node: No such file or directory`** — non-interactive shells here don't load nvm.
  Prefix the command: `export PATH="$HOME/.nvm/versions/node/v24.14.1/bin:$PATH" && ...`
  (check `ls ~/.nvm/versions/node` for the current version).
- **Missing/blank Mermaid diagrams** — raise `--timeout`.
- **Wants editable text in PowerPoint** — Slidev cannot do that; offer PDF export
  (`pnpm run export`) or the hosted deck instead, and say why.
