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
2. The brain keeps working: structured `brain.query` + redaction need no LLM; offline mode swaps **both** cloud defaults — agent compute Vercel Sandbox → the on-host local runtime (under OS-enforced isolation, [04 §2](./04-sandbox-agents.md)), and inference Bedrock → **local LM Studio** — so synthesis/agent answers run entirely on the host, no internet.
3. (Optional) Re-enable cloud to switch synthesis back to **Bedrock/Claude** for higher quality.

## 2. Test layers

| Layer | Tool | Asserts |
|---|---|---|
| Collaborative editing e2e | flare-dispatch + Playwright | two browser peers, real-time convergence, presence |
| Acceptance | flare-dispatch (CDP) | boot host (`serve` + `mcp`) + seeded control plane; Flows A, C, D end-to-end |
| Capability unit/property | Rust `cargo test` | attenuation never widens; field/row drops the right columns/rows; **Flow B salary invariant** as a property test |
| Brain | Rust `cargo test` | synthesis dedupe/supersede; anomaly threshold; learning suppresses re-flag; **no Markdown card body contains a value whose source field the caller is denied** (card-scrub property test); `acl_tag` taint never decreases through synthesis/`remember` |

CI: `flare-dispatch-action`; Rust suites under `cargo test`; JS test scripts exposed as `test` so `turbo run test` discovers them.

## 3. Scaffold / Status

Acceptance harness and flows are spec-only this pass. The capability property test (Flow B) is the first `cargo test` to wire up against `crates/sync/src/access/biscuit.rs` once attenuation is implemented (milestone **M0**, [00 §9](./00-overview.md)).
