# superai2026

Monorepo with three deployable components:

| Path | What | Stack | Deploy |
| --- | --- | --- | --- |
| `apps/landing` | Marketing / landing page | Astro (static) | Vercel |
| `apps/web` | Web application | Next.js 15, React 19 | Vercel |
| `crates/sync` | Local-first sync engine — one binary, runs as **server and client** | Rust | self-hosted / local |

> Internals are placeholders to be specified later. This repo is scaffolding.

## Prerequisites

- Node ≥ 20 and [pnpm](https://pnpm.io) 11
- Rust (stable) via [rustup](https://rustup.rs)

## Quick start

```bash
pnpm install            # JS deps for the whole workspace
pnpm dev                # run all apps (Turborepo)
pnpm dev:web            # just the Next.js app
pnpm dev:landing        # just the Astro landing page

cargo run -p sync -- serve    # run the sync server
cargo run -p sync -- client   # run the sync client
```

See [CLAUDE.md](./CLAUDE.md) for the full command reference, architecture, and deploy notes.
