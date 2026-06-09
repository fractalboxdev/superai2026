# superai2026 — Contextful

A **local-first company brain** where humans and AI agents collaborate in shared
documents, and every agent sees **exactly what it is permitted to** — capability-scoped,
attenuable, field/row-enforced. The one-line claim: *"the CTO's agent can't read the
CEO's salary — provably."* See [`specs/`](./specs) for the full design.

| Path | What | Stack |
| --- | --- | --- |
| `apps/landing` | Marketing / landing page | Astro (static) → Vercel |
| `apps/web` | Interactive capability console (Flows A & B) + live presence | Next.js 15, React 19 → Vercel |
| `crates/sync` | Backend: capabilities, brain, MCP, Loro relay, control plane | Rust (7 subcommands) — self-hosted |
| `packages/protocol` | Capability engine + brain query + wire/MCP mirrors | TypeScript |
| `tests/acceptance` | End-to-end Flow A/B tests against the binary | vitest |
| `infra/` | Pulumi cloud recipes (standalone) | Pulumi TS |

The on-host backend is implemented and tested. Cloud edges (Bedrock, Vercel Sandbox,
Exa HTTP, real Biscuit-WASM, Pulumi `apply`) are interface-complete and feature-gated
off, so the default build runs fully offline.

## Prerequisites

- Node ≥ 22.13 and [pnpm](https://pnpm.io) 11
- Rust (stable) via [rustup](https://rustup.rs)

## Quick start

```bash
pnpm install            # JS deps for the whole workspace
pnpm dev:web            # the Next.js capability console
pnpm test               # protocol unit + acceptance e2e

# Backend (state under ~/.contextful; override with CONTEXTFUL_HOME):
cargo run -p sync -- ctl seed                 # seed principals, roots, envelopes, tokens
cargo run -p sync -- ingest --source stripe   # ingest mock FinOps data → synthesize cards
cargo run -p sync -- serve                     # Loro WS relay (authoritative peer)
cargo run -p sync -- mcp --principal cfo       # brain over MCP (JSON-RPC stdio)
cargo test -p sync                             # capability + brain tests (incl. salary invariant)
```

To see live presence in the web app, run the relay and start it with
`NEXT_PUBLIC_SYNC_URL=ws://localhost:7878/ pnpm dev:web`.

See [CLAUDE.md](./CLAUDE.md) for the full command reference and architecture.
