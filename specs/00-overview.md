# 00 · Overview

**Status:** Draft v1 · **Product:** Contextful · **Repo:** `2super` (`superai2026` workspace)

## 1. What Contextful is

A **local-first company brain** where humans and AI agents collaborate in shared documents, and every agent sees **exactly what it is permitted to** — nothing more. Three pillars:

1. **Attenuated, capability-based access.** Access is delegated by [Biscuit](https://www.biscuitsec.org/) tokens that can only be *narrowed*, never widened. A person grants their agent a strict subset of their own authority. There is no company-wide "SuperAgent" that can read everything.
2. **A brain that synthesizes context.** It ingests SaaS + document data, extracts atomic facts, synthesizes human-readable memory, detects anomalies, learns from corrections, **grounds itself in public world knowledge (Exa), and daydreams new connections in the background** — in the spirit of mem0 / GBrain / LLMWiki, but the memory is **human-readable Markdown** ([02 §8](./02-brain-memory.md) world memory, [02 §9](./02-brain-memory.md) daydreaming).
3. **Local-first, cloud-optional.** Everything runs on-host (on-prem) over a Tailscale tailnet. Cloud is optional: managed inference (Vercel AI Gateway), agent compute (Vercel Sandbox), and web hosting (Vercel). **Raw source data and un-redacted brain content stay on-host**; only already-permitted, capability-redacted content is ever sent to cloud — and that path can be turned off entirely (Flow D).

> **The one-line claim:** *"The CTO's agent can't read the CEO's salary — provably."*

## 2. Problem & scenario

A 100-person software company runs a **FinOps** initiative to improve utilization and justify spend across many AI + SaaS tools (Claude, Notion, Slack, Linear, AWS, Vercel, Stripe). The pain:

- **Engineering** knows whether using Claude Code makes sense, but has no visibility into pricing, discount tiers, or credits.
- **Operations** owns workflow outcomes and evaluations, but has no spend visibility.
- **Finance** sees a $100k/month token bill but can't tell if it's reasonable.
- **CTO** has more visibility but won't commit invoices to a shared GitHub repo.
- **CIO** sees the burn but not what it's for.
- Per-cloud tools (AWS Budgets, vantage.sh) are siloed and miss connectors; only the **CFO** knows which spend is offset by credits, the discount tier, and team budgets.

Obviously you **cannot** solve this with a single shared context store the whole company can query — an engineer must not be able to read everyone's salary. Contextful's answer is one brain with **per-principal attenuated access**.

## 3. Personas & the access problem

Six personas, encoded directly as Biscuit token scopes (see [03 · Access Control](./03-access-control.md)):

| Persona | Can read | Notes |
|---|---|---|
| **Engineering** | usage views | no finance-private fields |
| **Operations** | outcome / eval views | can comment on spend |
| **Finance** | spend aggregates | not employee-level |
| **CTO** | broad read | mints scoped agent tokens for the team |
| **CIO** | total burn, drill-down by grant | |
| **CFO** | finance-private (sole authority root) | approves attenuated grants; owns `employee_salary`, `discount_tier`, `credits` |

(The **CEO** is the persona who naively wanted a single all-context "SuperAgent" — the storyline's antipattern. The salary invariant below is exactly what such a SuperAgent would violate.)

**The salary invariant (acceptance):** an engineer's agent can never obtain `employee_salary` — there is no token and no approval path outside the CFO's own root that yields it. Proven, not promised (see [09](./09-testing-acceptance.md) Flow B).

## 4. What we showcase

The demo proves two things: **you can answer real questions with the company brain**, and **the brain actually grows**. Four reference flows (full detail in [09](./09-testing-acceptance.md)):

- **Flow A — request → approve → scoped pull.** A CTO question hits a denied finance-private query; the agent raises a structured `request_access`; the CFO approves a narrowed grant (credits + discount tier, salary redacted, 7-day TTL); the agent retries and answers net-of-credits.
- **Flow B — the salary invariant (negative test).** An engineering agent's attempt to read `employee_salary` is denied at the field level and has no approval path. It stays blocked.
- **Flow C — the brain grows.** End-of-month ingest + synthesis flags a token-spend spike; a human annotates "one-off backfill, not a trend"; the correction is stored as a *learning* and suppresses the re-flag next month.
- **Flow D — local-first proof.** Disconnect the cloud uplink; editing and the brain keep working. Offline mode swaps **both** cloud defaults — Vercel Sandbox → the on-host local runtime, and the Vercel AI Gateway → **local LM Studio** — and structured query + redaction need no LLM at all. Re-enable cloud to switch synthesis back to the Vercel AI Gateway (Claude) for higher quality. (The offline runtime relies on OS-enforced isolation; see [04 §2](./04-sandbox-agents.md).)

Three further flows ([09](./09-testing-acceptance.md)) show the brain *grounding* and *thinking* on its own: **Flow E** — synthesis grounds a spend anomaly against public pricing it fetched from Exa (egress-firewalled, no private value leaves); **Flow F** — an agent researches a question typed in a doc and writes a cited answer back inline; **Flow G** — overnight, the **daydream loop** connects two unrelated cards into a cited insight ("Claude usage relates to the expiring discount tier") and surfaces it only to the people cleared to see it.

Collaboration is shown live: members + their agents co-edit a room (a "meeting room"); presence shows who is reading vs. writing; the CFO's agent pulls from the right context after approval.

The web app also makes the access model **directly operable** by people (see [03 §6](./03-access-control.md)): a **company directory** of every principal and what their token can see; a **delegation** form where a member grants their own agent a narrowed subset of their authority; and an **inbox** where a resource owner accepts or declines incoming agent `access_request`s (Flow A approvals land here; Flow B salary requests appear as forbidden, with no approve button).

## 5. Design principles

The brand stands for **Trust, Clarity, Security, Collaboration, Fluid**. These map to concrete tokens and components in [08 · Design System](./08-design-system.md). Voice: plain-spoken and precise — lead with the direct claim, then explain; no fear-mongering.

## 6. Tech stack

| Layer | Choice |
|---|---|
| Editor | [Weaver](https://github.com/OpenHackersClub/weaver) (headless TS, Loro source of truth, Effect-TS plugins, agents-as-peers) |
| CRDT | [Loro](https://loro.dev) (`loro` crate / `loro-crdt` npm) |
| Web | **React Router 7** (framework mode, Vite · SSR/SSG) + React 19 — embeds Weaver client-side; landing stays Astro (static) |
| Workflow / app code | TypeScript, [Effect-TS](https://effect.website) (reactive FP) — `effect` `Schema` validates the wire protocol today; `@effect/platform` Socket + `@effect-rx/rx-react` are the planned deepening |
| Binary / connectors / sandbox control | Rust (tokio, clap) — owns lifecycle/identity; Vercel call via `packages/sandbox-bridge` (Node) |
| Capabilities | Biscuit (`biscuit-auth` crate / `@biscuit-auth/biscuit-wasm`) |
| MCP | `rmcp` (official Rust MCP SDK) |
| Inference | Vercel AI Gateway → Claude (default; Vercel AI SDK `@ai-sdk/gateway` in TS, `async-openai` in Rust) · LM Studio OpenAI-compat (on-prem/offline) |
| Storage | Markdown brain + SQLite / DuckDB (file-based); Loro per-doc snapshots |
| Agent compute | Vercel Sandbox (default, from anywhere) · local constrained process (offline) |
| Web enrichment / world memory | [Exa](https://exa.ai) — public grounding, egress-firewalled |
| Background synthesis | Daydream loop (GBrain dream-cycle / [Gwern DDL](https://gwern.net/ai-daydreaming)) on cron |
| Networking | Tailscale (external to this system, on the host) |
| IaC | Pulumi |
| Tests | [flare-dispatch](https://github.com/OpenHackersClub/flare-dispatch) + `cargo test` |

**Web app framework (`apps/web`).** Migrating off **Next.js** to **React Router 7** (framework mode, Vite-native SSR/SSG) to make embedding **Weaver** a first-class concern rather than an exception. Weaver is a client-side, WASM-backed (Loro), Effect-TS editor; under Next's App Router it had to be quarantined behind `dynamic(ssr:false)` with bespoke webpack/Turbopack WASM config. On Vite the editor mounts as an ordinary client component, Rolldown bundles `loro-crdt`'s WASM as a lazy async asset, and **Effect-TS** is the app's validation layer — `effect` `Schema` decodes the wire protocol today; the `@effect/platform` `Socket` transport + `@effect-rx/rx-react` store are the planned deepening (the relay transport currently uses the browser `WebSocket` + a `BroadcastChannel` cross-tab fallback). SSR/SSG is retained for the SEO shell (per-route metadata, canonical, OG, JSON-LD, `llms.txt`/sitemap) so the app stays crawlable; the WASM editor is lazy-loaded behind interaction to protect Core Web Vitals. The **landing stays Astro (static)**. *(Router choice: **React Router 7** for the most battle-tested Vercel integration in this pnpm/Turborepo monorepo — explicit `vercelPreset()`, multi-Function bundle splitting, Node runtime for WASM. **TanStack Start** is the sanctioned alternative if its type-safe router + server functions are preferred; both are Vite-based, so the Weaver/Effect-TS embedding is identical.)*

## 7. Repo map

| Component | Location | Notes |
|---|---|---|
| Sync / brain / MCP / agent binary | `crates/sync` | one Rust binary, 7 subcommands |
| Web app (editor host + UI) | `apps/web` | React Router 7 (Vite) + React 19, Effect-TS — **migrated off Next.js**. Live Loro CRDT editor built (`loro-crdt`, cross-tab + relay); full Weaver rich-text/plugin editor still future |
| Landing | `apps/landing` | Astro static (`www.contextful.work`) |
| Shared protocol types | `packages/protocol` | `@superai2026/protocol` — built this pass |
| Design system | `packages/design-system` | `@superai2026/design-system` — spec'd here, port from reference |
| Vercel Sandbox bridge | `packages/sandbox-bridge` | `@superai2026/sandbox-bridge` — thin `@vercel/sandbox` wrapper the Rust driver spawns; spec-only this pass |
| Tests | `flare-dispatch` (external) | acceptance + e2e |

## 8. Architecture

```mermaid
flowchart TD
    subgraph Browser["Browser (member machine)"]
        W["apps/web — React Router 7 (Vite) + Weaver editor"]
        LD["LoroDoc (WASM) + OPFS"]
        W --- LD
    end
    subgraph Cloud["Cloud (optional)"]
        VS["Vercel Sandbox — per-doc agent runtime (≤5h)"]
        BR2["Vercel AI Gateway — Claude inference"]
        EX["Exa — world memory"]
        VC["Vercel — landing + web hosting"]
    end
    subgraph Host["On-prem host (Mac Studio) — Tailscale tailnet"]
        BIN["crates/sync — one Rust binary"]
        subgraph BIN
            SE["serve — Loro CRDT relay (authoritative)"]
            MC["mcp — brain over MCP"]
            BRN["brain — ingest/synthesize/retrieve"]
            CAP["access — Biscuit verify/mint/attenuate"]
            CON["connectors — Stripe/Exa + cron"]
            CTL["ctl — control plane"]
            SBX["sandbox — local fallback runtime"]
        end
        STORE[("~/.contextful — Markdown brain + .db + Loro")]
        LM["LM Studio (local inference)"]
    end
    W -- "Loro sync (WSS)" --> SE
    VS -- "brain MCP over Tailscale (identity)" --> MC
    MC --> BRN --> STORE
    CAP -. "attenuated tokens" .- MC
    CON --> BRN
    CON -- "world queries (egress-firewalled, public terms only)" --> EX
    BRN -- default --> BR2
    BRN -- offline --> LM
    VS -- "Claude" --> BR2
```

**Trust boundary:** the host is authoritative and holds the data; the browser holds only what it was granted; sandboxes have **zero ambient authority** — their only data egress is the brain MCP, every call capability-checked.

## 9. Build milestones

```mermaid
flowchart LR
    M0["M0 · Capabilities core<br/>biscuit verify/attenuate/mint + Flow B test"]
    M1["M1 · Sync<br/>Loro relay serve/client + Weaver transport"]
    M2["M2 · Brain + MCP<br/>ingest Stripe, synthesize, capability-filtered query"]
    M3["M3 · Sandbox + requests<br/>Vercel Sandbox, request_access, auto-mode"]
    M4["M4 · Reference flows<br/>A/B/C/D in apps/web + flare-dispatch"]
    M0 --> M1 --> M2 --> M3 --> M4
```

**World memory** (Exa grounding + egress firewall) and the **daydream loop** land in **M2** as brain enrichments; both are cloud-optional and degrade to cache / local LM Studio offline (Flow D).

## 10. Glossary

- **Room** — a document + its members (humans + agents) + a paired sandbox and brain scope.
- **Principal** — a human or an agent identity. Agents are owned by exactly one human; id `agent:<owner>/<n>`.
- **Attenuation** — appending a Biscuit block that strictly narrows authority. `caps(agent) ⊆ caps(owner)`.
- **View** — a named, field-typed projection of a source (e.g. `stripe/spend_by_team`). The unit of finance privacy.
- **Brain scope** — the set of sources/views a document's sandbox may draw on.
- **Envelope** — an owner's auto-mode policy describing which requests auto-approve vs. escalate.
- **Directory** — the web roster of every principal (humans + their agents) and the effective scope each token grants ([03 §6.1](./03-access-control.md#61-company-directory)).
- **Delegation** — a member narrowing their own token and handing it to an agent they own (`delegateTo()`); intra-owner, needs no approval. The inverse-direction action to a cross-owner `access_request`.
- **Inbox** — the web surface where a resource owner accepts/declines agent `access_request`s ([03 §6.3](./03-access-control.md#63-inbox--accept-or-decline-agent-access-requests)).
- **World memory** — public, cited knowledge fetched from the web (Exa); tagged `world`, default-readable, never authority ([02 §8](./02-brain-memory.md)).
- **Egress firewall** — the outbound check that lets only public-tainted query terms leave the host, so enrichment can't exfiltrate a private value ([03 §4](./03-access-control.md)).
- **Daydreaming** — a background, cron-scheduled loop that samples acl-admissible card pairs, proposes non-obvious connections (grounded via world memory), and keeps the valuable ones as hypothesis insight cards. GBrain dream-cycle / Gwern DDL ([02 §9](./02-brain-memory.md)).
- **The salary invariant** — the provable property that no non-CFO path yields `employee_salary` — and that daydreaming, which connects cards, never breaches it.

## 11. Scaffold / Status

This overview maps onto the whole repo. The Rust binary subcommands and modules that anchor each subsystem are listed per spec file.

**Built and verified this pass** (`cargo test`: 11 passing; `turbo run test`: 9 protocol unit + 4 acceptance e2e; clippy `-D warnings` clean):

- `crates/sync` — a working library + 7-subcommand binary (`serve`, `client`, `ingest`, `cron`, `mcp`, `agent`, `ctl`). Real capability engine (M0), brain ingest/synthesis/retrieval over a file index + Markdown cards (M2), JSON-RPC MCP server (M6), Loro WS relay with presence + revocation (M1), control plane (M7), and the sandbox/agent/inference traits with `StubInference` (M3).
- `packages/protocol` — capability engine + brain query (TS), and wire/MCP mirrors (`sync.ts`, `brain.mcp.ts`).
- `apps/web` — **React Router 7 (Vite)** capability console (Flows A & B); **every document is a live Loro CRDT room** (`loro-crdt`) editable in-browser with cross-tab (BroadcastChannel) + opt-in relay sync; inbound wire frames validated via Effect `Schema`. Migrated off Next.js.
- `tests/acceptance` — end-to-end flows against the built binary.
- `infra/` — Pulumi recipes (standalone; `pulumi preview` to apply).

**Live this pass** (selected at runtime by creds; offline degrades to cache/deterministic floor, never fakes): real Biscuit tokens (`biscuit-auth` ed25519 + Datalog authorizer, per-message relay auth, egress firewall), SQLite+FTS5 index (`brain.db`), Exa HTTP world memory, Stripe test-mode ingest, Vercel AI Gateway / AWS Bedrock / LM Studio inference, Vercel Sandbox via `packages/sandbox-bridge`, MCP streamable-HTTP transport with per-call session auth, cron-expression schedules, self-wiring links, daydream loop, and the real Weaver editor (vendored `@weaver/*`) in `apps/web`.

**Still future:** TS-side biscuit-wasm in the browser, sqlite-vec embeddings + DuckDB columnar tier, `wasmtime` local-sandbox isolation, OAuth connectors (Notion/Slack/Linear/AWS/Vercel), Pulumi `apply` automation, flare-dispatch browser e2e, the access-control web UI (03 §6), and the macOS app (10).
