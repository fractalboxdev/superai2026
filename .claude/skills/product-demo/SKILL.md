---
name: product-demo
description: Record a scripted product-demo video of demo.contextful.work by driving it with Playwright from an editable markdown story (demos/story.md). Use when asked to record a demo, run the product demo, regenerate the demo video, or update/extend the demo story. Outputs a .webm (and .mp4 if ffmpeg is present) under demos/recordings/.
---

# Product demo

Drive `https://demo.contextful.work` with Playwright following the story in `demos/story.md`, overlay each scene's caption on screen, and record a video.

The story file is the single source of truth — the user edits it directly. Scenes are `## Scene:` headings, captions are `>` blockquotes, steps are `- action: argument` list items. The full action table is documented at the top of `demos/story.md` itself.

> **Node via mise:** non-interactive shells may not have `node` on PATH (the user activates node through mise in `.zshrc`). If `node`/`pnpm` fail with `env: node: No such file or directory`, prefix every command below with `mise exec node@24 --`.

## 1. One-time setup

The runner is self-contained in this skill directory (deliberately **outside** the pnpm workspace — do not add it to `pnpm-workspace.yaml`). If `node_modules` is missing here:

```bash
cd .claude/skills/product-demo
pnpm install --ignore-workspace
pnpm exec playwright install chromium
```

(Playwright's postinstall is blocked by pnpm's build-script policy — that's fine; the explicit `playwright install chromium` downloads the browser.)

## 2. Run the demo

From the repo root:

```bash
node .claude/skills/product-demo/scripts/run-demo.mjs                 # default story, headless
node .claude/skills/product-demo/scripts/run-demo.mjs demos/story.md  # explicit story path
node .claude/skills/product-demo/scripts/run-demo.mjs --headed        # watch it live
node .claude/skills/product-demo/scripts/run-demo.mjs --base-url=http://localhost:5173   # against local dev
```

`DEMO_BASE_URL` env var also overrides the target. The script prints each scene/step as it executes and exits non-zero on failure, saving whatever was recorded up to that point.

Output: `demos/recordings/<story>-<timestamp>.webm`, converted to `.mp4` automatically when `ffmpeg` is on PATH. Recordings are gitignored.

## 3. If a step fails

A failing step is almost always a selector that no longer matches the live app. Don't guess:

1. Read the failing step from the script output (it logs `· action: arg` lines up to the failure).
2. Check the real markup in `apps/web/app/routes/` (routes: `/`, `/directory`, `/delegate`, `/inbox` — see `apps/web/app/routes.ts`) and fix the selector **in the story file**, not in the runner.
3. If the live deploy is behind the code, verify with `/healthcheck` or run against local dev (`pnpm dev:web` + `--base-url=http://localhost:5173`).
4. Re-run until it completes, then report the final video path.

## 4. Editing or authoring stories

When the user asks for a new or changed demo narrative:

- Edit `demos/story.md` (or create another `demos/*.md` and pass its path). Keep captions short — one sentence per scene, they render as an overlay at the bottom of the video.
- Prefer `wait for:` before interacting with anything that loads async, and end scenes with a `pause:` so viewers can read the screen.
- Use selectors that are stable and human-meaningful (`text=`, headings, labels) over brittle CSS chains.
- After editing, always do a full run to verify the recording completes end-to-end before reporting done.

## Guardrails

- The demo runs against the **live** site read-mostly; avoid steps that mutate shared state (approving real inbox items, sending grants) unless the user explicitly asks — prefer `--base-url` against local dev for write flows.
- Don't commit recordings; `demos/recordings/` is gitignored.
- Don't modify `run-demo.mjs` to special-case one story — extend the DSL only if an action is genuinely missing, and document it in the table in `demos/story.md`.
