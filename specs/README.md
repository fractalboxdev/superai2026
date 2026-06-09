# Contextful — Technical Specs

> **Source of truth.** This `specs/` directory is the authoritative technical specification for Contextful in the `2super` repo. It supersedes the earlier draft in `../superai2026/specs/SPEC.md` (kept as a reference). Where the two differ, **this spec wins** — notably: agents run in **Vercel Sandbox**, inference defaults to **AWS Bedrock + Claude** (LM Studio local as offline mode), the brain is a **human-readable Markdown** store, ingestion is **cron-scheduled**, the web is enriched via **Exa**, and infra is provisioned with **Pulumi**.

Contextful is a **local-first company brain**: teams collaborate in shared documents ("rooms"); every member's AI agent gets **attenuated, capability-scoped** access to company context — never one all-powerful "SuperAgent". The motivating scenario is multi-SaaS **FinOps** (Stripe, AWS, Vercel, Claude, …) where Eng / Ops / Finance / CTO / CIO / CFO each need a different, provably-bounded slice of the same brain.

## Index

| # | Spec | Covers |
|---|---|---|
| 00 | [Overview](./00-overview.md) | Problem, personas, what we showcase, architecture, stack, repo map, milestones, glossary |
| 01 | [Rooms & Sync](./01-room-sync.md) | Rooms, Weaver editor, Loro CRDT sync over WebSocket/Tailscale, presence, wire protocol |
| 02 | [Brain & Memory](./02-brain-memory.md) | Markdown brain + SQLite/DuckDB index, ingest→synthesize lifecycle, scoping, storage layout |
| 03 | [Access Control](./03-access-control.md) | Biscuit capabilities, attenuation, field/row auth, permission requests, the salary invariant |
| 04 | [Sandbox & Agents](./04-sandbox-agents.md) | Vercel Sandbox (+ local fallback), agent runtime, inference backends, identity |
| 05 | [Connectors & ETL](./05-connectors-etl.md) | Connector trait, Stripe (mock), Exa, cron scheduling, context layer |
| 06 | [MCP Interface](./06-mcp-interface.md) | Embedded `rmcp` server, brain tools, stdio + HTTP/SSE transports, auth |
| 07 | [Deployment & IaC](./07-deployment-iac.md) | Local-first deploy, Tailscale, Vercel, control plane, Pulumi recipes |
| 08 | [Design System](./08-design-system.md) | Brand pillars, tokens, components (`@superai2026/design-system`) |
| 09 | [Testing & Acceptance](./09-testing-acceptance.md) | flare-dispatch, reference flows A–D, capability property tests |

*New here? Start with [00 · Overview](./00-overview.md). Implementers can jump straight to the spec for their component — every file cross-references the others and ends with a Scaffold / Status map.*

## Scaffold

Each spec ends with a **Scaffold / Status** section pointing at the code that anchors it:

- `crates/sync` — one Rust binary, subcommands `serve | client | mcp | ingest | agent | cron | ctl`.
- `packages/protocol` — shared TypeScript types + Biscuit helper signatures (`@superai2026/protocol`).

Product name is **Contextful**; the workspace/package namespace stays `@superai2026/*`. Domains: `www.contextful.work` (landing), `demo.contextful.work` (demo).
