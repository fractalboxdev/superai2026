# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`superai2026` is a monorepo with four deployable components:

- **`apps/landing`** — Astro **static** landing page. Deploys to Vercel.
- **`apps/web`** — React Router 7 (framework mode, Vite, React 19) web app: the interactive capability console (Flows A & B) with opt-in live presence against the relay; hosts the Weaver/Loro CRDT editor. Deploys to Vercel.
- **`apps/desktop`** — Tauri macOS menu-bar app (spec 10): bundles `crates/sync` as a sidecar, supervises it (host `serve --with-mcp` / member `client`), first-run wizard, Keychain identity, Tailscale detection, LaunchAgent auto-start. The Rust shell crate (`apps/desktop/src-tauri`) is a **standalone Cargo workspace** — kept out of the root workspace so ubuntu CI never needs webkit deps; it builds in its own macOS lane (`.github/workflows/desktop.yml`).
- **`crates/sync`** — Rust library + binary implementing the Contextful backend. Seven subcommands: `serve` (Loro WS relay), `client` (headless peer), `ingest`, `cron`, `mcp` (brain over JSON-RPC), `agent`, `ctl` (control plane).
- **`packages/protocol`** — `@superai2026/protocol`: TS capability engine + brain query + wire/MCP mirrors.
- **`tests/acceptance`** — `@superai2026/acceptance`: end-to-end Flow A/B tests against the built binary.
- **`infra/`** — Pulumi recipes (standalone; outside the pnpm workspace).

Two toolchains share the repo root: a **pnpm + Turborepo** workspace for JS (`apps/*`, `packages/*`, `tests/*`) and a **Cargo** workspace for Rust (`crates/*`).

> The backend is implemented and tested end to end: real Biscuit tokens (`biscuit-auth` signed + Datalog-verified), SQLite+FTS5 brain index, live Exa world memory behind the egress firewall, live Stripe test-mode ingest, real inference (Vercel AI Gateway / AWS Bedrock / LM Studio — runtime-selected by creds), Vercel Sandbox provisioning via `packages/sandbox-bridge`, MCP over stdio + streamable HTTP with per-call auth, cron-expression scheduling, self-wiring links, and the daydream loop. Cloud paths are compiled in and selected at RUNTIME by env/creds (`EXA_API_KEY`, `STRIPE_SECRET_KEY`, `AWS_*`, `AI_GATEWAY_API_KEY`, `VERCEL_TOKEN`); with no creds everything degrades to the on-host cache / deterministic offline floor (Flow D), never to fakes. The web app embeds the real Weaver editor (vendored `@weaver/*`) synced through the relay.

## Commands

JS commands run from the repo root via Turborepo unless noted.

| Task | Command |
| --- | --- |
| Install JS deps | `pnpm install` |
| Dev (all apps) | `pnpm dev` |
| Dev landing only | `pnpm dev:landing` |
| Dev web only | `pnpm dev:web` |
| Build all | `pnpm build` |
| Lint all | `pnpm lint` |
| Typecheck all | `pnpm typecheck` |

Run a script in one workspace: `pnpm --filter landing <script>` or `pnpm --filter web <script>`.

### Desktop (`apps/desktop`)

| Task | Command (from `apps/desktop`) |
| --- | --- |
| Stage sidecar (required before any src-tauri build) | `pnpm sidecar` (or `./scripts/prepare-sidecar.sh [--release] [--universal]`) |
| Dev app (sidecar + tauri dev) | `pnpm app:dev` |
| Package .app/.dmg | `pnpm app:build` |
| Rust lint/test on the shell crate | `cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test` |

### Rust (`crates/sync`)

| Task | Command |
| --- | --- |
| Seed control plane | `cargo run -p sync -- ctl seed` |
| Approve a scoped grant (Flow A) | `cargo run -p sync -- ctl grant --to agent:cto/1 --view stripe/finance_private --fields gross,credits,discount_tier` |
| Ingest + synthesize | `cargo run -p sync -- ingest --source stripe` |
| Run relay (+presence) | `cargo run -p sync -- serve --addr 127.0.0.1:7878` |
| Headless peer | `cargo run -p sync -- client --doc finops --principal agent:cto/1` |
| Brain over MCP (stdio) | `cargo run -p sync -- mcp --principal cfo` |
| Agent loop | `cargo run -p sync -- agent --principal agent:cto/1 --ask "..."` |
| Build / Lint / Format | `cargo build -p sync` · `cargo clippy --all-targets -- -D warnings` · `cargo fmt` |
| Test (all) | `cargo test -p sync` |

State lives under `~/.contextful` (override with `CONTEXTFUL_HOME`). `pnpm sync -- serve` is a convenience alias for `cargo run -p sync --`.

### Tests

- **JS:** `pnpm test` (`turbo run test`) runs `@superai2026/protocol` unit tests (vitest) and `@superai2026/acceptance` end-to-end Flow A/B tests against the built binary (the acceptance suite skips if `target/debug/sync` is absent).
- **Rust:** `cargo test -p sync` — capability/property tests (incl. the Flow B salary invariant) and brain synthesis/anomaly/card-scrub tests.

## Architecture

```mermaid
flowchart TD
    subgraph Vercel
        L["apps/landing — Astro (static)"]
        W["apps/web — React Router 7 (Vite)"]
    end
    subgraph Local["User machine — local-first"]
        S["crates/sync — Rust binary"]
        DB[("local store")]
        S -->|serve mode| DB
    end
    W -->|"sync protocol (TBD)"| S
```

- The **landing page** is fully static (no SSR adapter). Vercel runs `astro build` → `dist/`.
- The **web app** is the UI; it talks to the local-first **sync** binary running on the user's machine (transport TBD). `sync` owns persistence and reconciliation.
- **`sync`** is one crate compiled to one binary with two modes. `serve` is the authoritative peer; `client` connects to it. `main.rs` parses the CLI (clap) and dispatches to `server::run` / `client::run`.
- **`packages/`** holds shared JS libraries (empty placeholder) — e.g. future generated TS bindings for the sync protocol.

## Deployment (Vercel)

Two separate Vercel projects backed by the same repo:

- **Landing** — Root Directory `apps/landing`, framework preset Astro.
- **Web** — Root Directory `apps/web`, React Router 7 (Vite). `apps/web/vercel.json` pins `framework: react-router` + `buildCommand: pnpm run build`; `vercelPreset()` (gated on `process.env.VERCEL`) emits the Vercel Build Output. Don't rely on the dashboard framework setting — git deploys auto-revert it to `nextjs`.

Set the project Root Directory in the Vercel dashboard; Vercel handles the monorepo `pnpm install` from the repo root. Turborepo remote caching is optional. `crates/sync` is **not** deployed to Vercel — it runs locally / self-hosted.

## Conventions

- Package manager is **pnpm** (pinned via `packageManager` in the root `package.json`). Do not use npm or yarn.
- TS apps extend `tsconfig.base.json`. The Astro app instead extends `astro/tsconfigs/strict` (Astro-specific).
- Rust: edition, version, and shared dependency versions live in the root `Cargo.toml` `[workspace]`; member crates inherit with `*.workspace = true`. Add shared deps there, not per-crate.
- The parent `/Users/debuggingfuture/workspaces/CLAUDE.md` SEO + agentic-SEO rules apply to `apps/landing` and `apps/web` (per-page metadata, canonical, OG, JSON-LD, `robots.txt`, `llms.txt`, sitemap, Core Web Vitals). Minimal `robots.txt` / `llms.txt` stubs and metadata are scaffolded — extend them, don't remove. `https://example.com` placeholders need the real production domain.
