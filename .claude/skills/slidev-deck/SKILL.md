---
name: slidev-deck
description: Generate and keep in sync a Slidev (sli.dev) slide deck from PRESENTATION.md. Use when the user wants to create, build, update, refresh, or regenerate slides / a deck / a presentation from the project's presentation narrative. Writes Markdown slides to slides/slides.md following the slide table in PRESENTATION.md, and can run/build/export the deck.
---

# slidev-deck — generate & update slides with Slidev (sli.dev)

`PRESENTATION.md` is the **source of truth**. This skill turns its **"Slide deck" table**
into a runnable Slidev deck at `slides/slides.md`, and re-syncs it whenever the narrative
or demo changes.

## Hard rules (inherited from PRESENTATION.md)
- **Fewer than 10 slides.** One idea per slide + one money line. Detail goes in speaker
  notes, not on the slide.
- **Mostly jargon-free.** Only the **technical breakdown slides (max 3)** may use
  technical terms; tag them in the heading (e.g. a small "· technical" note). Every other
  slide must read to a non-technical exec.
- **One Slidev slide per row** of the PRESENTATION.md slide table, in order. The demo is
  ONE slide; protect the money shot (the salary denial) as its climax line.

## How to run
1. **Read `PRESENTATION.md`** — the "Slide deck" table (slide list + which are technical)
   and the Act narrative the speaker notes are drawn from. If the table is missing,
   build from the Acts but still cap at <10 slides / ≤3 technical.
2. **Generate or update `slides/slides.md`** (create `slides/` if missing):
   - First block is **headmatter** (deck config). Slides are separated by a line that is
     only `---`. Per-slide options go in a frontmatter block right after a separator.
   - Keep on-slide text minimal; reveal bullets with `<v-clicks>`. Put the spoken detail
     and stage directions in **speaker notes** — a trailing `<!-- ... -->` HTML comment.
   - Use ` ```mermaid ` fenced blocks for diagrams (reuse/simplify the PRESENTATION.md
     Mermaid). Tag the ≤3 technical slides; never exceed 3.
   - If `slides/slides.md` already exists, **update in place**: keep the headmatter and
     any slide marked `<!-- keep -->`; otherwise regenerate slide bodies from the table.
3. **Do not dump the deck into chat.** Write the file, then report only: deck path, slide
   count, which slide numbers are technical, and the dev command.

## Slidev syntax cheat-sheet
- Headmatter keys: `theme` (use `default` — bundled, no extra install), `title`, `class`,
  `transition`, `mdc: true`.
- Layouts (default theme): `cover`, `center`, `section`, `statement`, `fact`, `quote`,
  `two-cols` (split body with a `::right::` line), `image-right`, `end`.
- Builds/animation: wrap step-revealed bullets in `<v-clicks> … </v-clicks>`; reveal a
  single element with `v-click`.
- Styling: UnoCSS utility classes work inline (`class="mt-8 opacity-80 text-red-500"`).
- Speaker notes: trailing `<!-- … -->` on each slide (shown in presenter mode).
- Code highlight: ` ```ts {2,4-6} `; Mermaid: ` ```mermaid `.

## Run / build / export
- Dev (hot-reload; presenter at `/presenter`, overview at `/overview`):
  `pnpm dlx @slidev/cli@latest slides/slides.md --open`
  (or `cd slides && pnpm install && pnpm dev`)
- Static build: `pnpm dlx @slidev/cli@latest build slides/slides.md`
- Export PDF/PNG: `pnpm dlx @slidev/cli@latest export slides/slides.md`
- `slides/package.json` carries `dev` / `build` / `export` scripts; a local install of
  `@slidev/cli` makes the `slidev` binary available without `dlx`.

## After updating
Report deck path + slide count + technical-slide indices + the dev command, and suggest
previewing it. Never call the deck "done" without confirming it matches the current
PRESENTATION.md table.
