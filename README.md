# Contextful

**Your Agents. Your Data. Your Rules.**

🌐 **Website:** [contextful.work](https://contextful.work/) · 🚀 **Live demo:** [demo.contextful.work](https://demo.contextful.work/)

<p align="center">
  <img src="slides/public/arts/slide-close.png" alt="The Pied Piper team lined up confidently with presence dots above them — Contextful's closing slide art" width="520" />
</p>

Every company wants one AI that knows everything. That's exactly the thing you must
never build. **Contextful** is the company brain that gets *smarter* as it gets *more
careful* — a **local-first collaboration workspace for your agents**, scoped per
person, approved at the boundary, run in a trusted environment you choose: on-prem or
your own cloud (BYOC).

Each member's agent holds only *their* context. When an answer needs something across
a boundary, the request is **routed to the owner's agent, approved, and scoped** — the
data crosses the line for *that question only*. Every agent sees **exactly what it is
permitted to** — capability-scoped, attenuable, field/row-enforced. The one-line claim:
*"the CTO's agent can't read the CEO's salary — provably."* See
[PRESENTATION.md](./PRESENTATION.md) for the full story and [`specs/`](./specs) for the
design.

| Path | What | Stack |
| --- | --- | --- |
| `apps/landing` | Marketing / landing page — [www.contextful.work](https://www.contextful.work) | Astro (static) → Vercel |
| `apps/web` | Interactive capability console (Flows A & B) + live presence — [demo.contextful.work](https://demo.contextful.work) | React Router 7 (Vite), React 19 → Vercel |
| `crates/sync` | Backend: capabilities, brain, MCP, Loro relay, control plane | Rust (7 subcommands) — self-hosted |
| `packages/protocol` | Capability engine + brain query + wire/MCP mirrors | TypeScript |
| `tests/acceptance` | End-to-end Flow A/B tests against the binary | vitest |
| `infra/` | Pulumi cloud recipes (standalone) | Pulumi TS |

The on-host backend is implemented and tested. Cloud edges (Bedrock, Vercel Sandbox,
Exa HTTP, real Biscuit-WASM, Pulumi `apply`) are interface-complete and feature-gated
off, so the default build runs fully offline.

## Built on

- [Weaver](https://github.com/OpenHackersClub/weaver) — open-source local-first CRDT
  editor by the repository author [@debuggingfuture](https://github.com/debuggingfuture);
  powers the collaborative document editing in `apps/web`.
- [Loro](https://loro.dev) — the CRDT engine underneath: every document is a live
  `loro-crdt` room, synced through the Rust relay (`sync serve`).
- [Biscuit](https://www.biscuitsec.org) — attenuable capability tokens behind the
  scoped-grant model (real Biscuit-WASM is feature-gated; an offline mirror runs by default).
- [Model Context Protocol](https://modelcontextprotocol.io) — the brain is exposed to
  agents over MCP (JSON-RPC stdio via `sync mcp`).

## Prerequisites

- Node ≥ 22.13 and [pnpm](https://pnpm.io) 11
- Rust (stable) via [rustup](https://rustup.rs)

## Quick start

```bash
pnpm install            # JS deps for the whole workspace
pnpm dev:web            # the capability console (React Router 7 + Vite)
pnpm test               # protocol unit + acceptance e2e

# Backend (state under ~/.contextful; override with CONTEXTFUL_HOME):
cargo run -p sync -- ctl seed                 # seed principals, roots, envelopes, tokens
cargo run -p sync -- ingest --source stripe   # ingest mock FinOps data → synthesize cards
cargo run -p sync -- serve                     # Loro WS relay (authoritative peer)
cargo run -p sync -- mcp --principal cfo       # brain over MCP (JSON-RPC stdio)
cargo test -p sync                             # capability + brain tests (incl. salary invariant)
```

To see live presence in the web app, run the relay and start it with
`VITE_SYNC_URL=ws://localhost:7878/ pnpm dev:web`.

See [CLAUDE.md](./CLAUDE.md) for the full command reference and architecture.
