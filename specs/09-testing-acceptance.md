# 09 · Testing & Acceptance

**Anchors:** [flare-dispatch](https://github.com/OpenHackersClub/flare-dispatch) (external) for e2e/acceptance; `cargo test` for capability/brain unit + property tests.

These are the scenarios that must work on stage. They double as the acceptance suite.

## 1. Reference flows

### Flow A — request → approve → scoped pull
1. The CTO asks about Claude spend after credits.
2. The CTO's agent calls `brain.query(view="stripe/finance_private", select=[net, credits, discount_tier])` → **denied**.
3. The agent calls `brain.request_access(resource=view("stripe","finance_private"), fields=[credits, discount_tier], rows=all_teams, reason=…)`.
4. The CFO approves → mints an attenuated token: read `spend_by_team` + `credits` + `discount_tier`, **redact `employee_salary`**, all teams, ttl 7d.
5. The agent retries → answers net-of-credits. **Salary never appears.**

### Flow B — the salary invariant (negative test)
1. An engineering agent attempts `brain.query(view="stripe/finance_private", select=[employee_salary])`.
2. **Denied at the field level**; it raises `request_access` for `employee_salary`.
3. The engineering owner has no envelope path; the request escalates; **no non-CFO key can mint it.** The agent stays blocked. *(Proven, not promised.)*

### Flow C — the brain grows
1. End-of-month `sync ingest --source stripe` loads new data → synthesis runs.
2. `brain.detect_anomalies(view="spend_by_team", period=current)` flags a token-spend spike; a memory + `anomaly` row are created.
3. A human annotates "one-off backfill, not a trend" → stored as a `learning`.
4. Next month the same pattern is **not re-flagged** — the brain applied the correction.

### Flow D — local-first (on-prem proof, performed live)
1. Disconnect the cloud uplink entirely; keep editing — Weaver persists to OPFS, Loro accumulates locally, peers sync through the host.
2. The brain keeps working: structured `brain.query` + redaction need no LLM; offline mode swaps **both** cloud defaults — agent compute Vercel Sandbox → the on-host local runtime (under OS-enforced isolation, [04 §2](./04-sandbox-agents.md)), and inference the Vercel AI Gateway → **local LM Studio** — so synthesis/agent answers run entirely on the host, no internet. Already-fetched **world cards serve from cache** ([02 §8](./02-brain-memory.md)); only fresh Exa lookups and the daydream world-grounding pause.
3. (Optional) Re-enable cloud to switch synthesis back to the **Vercel AI Gateway (Claude)** for higher quality.

### Flow E — the brain grows itself with world knowledge (proactive)
1. End-of-month `sync ingest` runs synthesis (Flow C); a spend spike on `spend_by_team` is flagged — the anomaly memory is **private** (finance `acl_tag`).
2. Synthesis (not the agent) issues `brain.world_search("Anthropic Claude API pricing change <period>")`. The **egress firewall** passes it — every term is public; no finance value is in the query.
3. Exa returns the flat `$5/$25` list price and the **2026-03-13** removal of the 1M-context premium → a `world_fact` card is created, **cited**, and `grounds`-linked to the anomaly ([02 §8](./02-brain-memory.md)).
4. The anomaly's surfaced summary becomes a judgement: *"spend up 40% MoM; list price flat, long-context premium removed 2026-03-13 ⇒ usage growth, not a price shock."* The salary line never entered the query or left the host; the world citation is public — readable even by an Engineering agent that can't see the spend.

### Flow F — agent researches a question typed in a document (reactive)
1. In a shared FinOps room a user types: *"Is our Claude spend reasonable vs. industry, and did pricing change this quarter?"*
2. The room's agent ([01](./01-room-sync.md) agents-as-peers) calls `brain.ground("claude spend")` — the **private** spend card, capability-filtered/redacted to this caller — and `brain.world_search` for public pricing/benchmark knowledge (egress-firewalled).
3. The agent writes the answer **back into the document** as a Loro edit: company context interleaved with **cited** world facts. A viewer without `finance_private` sees the world grounding and permitted aggregates, never the salary line.
4. If answering well needs a capability the agent lacks, it raises `request_access` ([03 §5](./03-access-control.md)) — research and permission requests compose.

### Flow G — the brain daydreams (background insight)
1. Off-peak, the daydream loop ([02 §9](./02-brain-memory.md)) samples two **acl-admissible** cards — `[[Claude usage]]` (eng) and `[[discount-tier expiry]]` (a card the same principal may hold) — and the generator proposes a non-obvious link, grounding it with a `world_search` on the vendor's pricing page.
2. The critic keeps it (novel + useful, non-redundant); a `kind=daydream` **hypothesis** card is written, taint = `max(parents)`, `grounds`/`relates_to` edges wired.
3. It surfaces into the room — *"heads up: Claude usage is climbing into a window where the discount tier lapses"* — **only** to principals cleared for the card's tag. A would-be cross-acl daydream (salary × usage) is **never sampled**, so the salary invariant holds (negative assertion).

## 2. Test layers

| Layer | Tool | Asserts |
|---|---|---|
| Collaborative editing e2e | flare-dispatch + Playwright | two browser peers, real-time convergence, presence |
| Acceptance | flare-dispatch (CDP) | boot host (`serve` + `mcp`) + seeded control plane; Flows A, C, D, **E, G** end-to-end (F via the Playwright editor row) |
| Capability unit/property | Rust `cargo test` | attenuation never widens; field/row drops the right columns/rows; **Flow B salary invariant** as a property test; **egress firewall blocks any privately-tainted term from leaving the host**; **daydream sampling never combines cross-acl cards** and insight taint = `max(parents)` (over all non-CFO principals) |
| Brain | Rust `cargo test` | synthesis dedupe/supersede; anomaly threshold; learning suppresses re-flag; **no Markdown card body contains a value whose source field the caller is denied** (card-scrub property test); `acl_tag` taint never decreases through synthesis/`remember`/daydream; world cards are `acl_tag = world` + carry a citation; daydream critic drops redundant/low-novelty candidates |

CI: `flare-dispatch-action`; Rust suites under `cargo test`; JS test scripts exposed as `test` so `turbo run test` discovers them.

## 3. Scaffold / Status

Acceptance harness and flows are spec-only this pass. The capability property test (Flow B) is the first `cargo test` to wire up against `crates/sync/src/access/biscuit.rs` once attenuation is implemented (milestone **M0**, [00 §9](./00-overview.md)).
