# 02 · Brain & Memory

**Anchors:** `crates/sync` subcommands `ingest` / `mcp`; modules `brain/`, `store/`; `packages/protocol/src/brain.ts`.

## 1. Model

The brain turns raw SaaS + document data into **synthesized, capability-tagged memory** that agents retrieve through the MCP server ([06](./06-mcp-interface.md)), always filtered by the caller's Biscuit token ([03](./03-access-control.md)).

Two representations work together:

1. **Human-readable Markdown** is the source of truth for *synthesized* memory — in the spirit of LLMWiki / GBrain / mem0, the LLM organizes knowledge as a tree of Markdown files a human can read, edit, and `git diff`. Files live under `~/.contextful/brain/`.
2. **A file-based index** (SQLite + DuckDB) holds the structured/queryable layer: immutable raw events, embeddings, anomalies, learnings, and provenance pointing back at the Markdown.

> **Divergence from reference:** the reference (original `superai2026/specs/SPEC.md` draft) stored synthesized memory in a relational `memory.body` column. Here the synthesized memory is **Markdown files** (per the LLMWiki/GBrain requirement); the relational tables become the *index over* those files plus the raw/derived data.

**Each card is acl-tagged.** Because a synthesized card is free-form prose, it **cannot be column-redacted** the way a structured query can. So every Markdown card is stamped with the **maximum access requirement** (`acl_tag`) of every fact it contains, and **synthesis never mixes acl-tags within one card** — facts that need different access live in different cards. Access to a card is therefore **all-or-nothing** against its tag (see [§4](#4-retrieval-capability-filtered) and [03 §4](./03-access-control.md)). This is what keeps the salary invariant intact once memory is prose rather than columns.

## 2. Pipeline

```mermaid
flowchart LR
    CON["Connectors<br/>(Stripe mock, Exa, stubs)"] --> ING["Ingest<br/>raw events + provenance + acl_tag"]
    ING --> EXT["Extract<br/>LLM: atomic facts, entities"]
    EXT --> SYN["Synthesize<br/>dedupe, supersede, summarize → Markdown"]
    SYN --> IDX["Index<br/>embeddings + structured views"]
    IDX --> SRV["Serve<br/>capability-filtered retrieval"]
    SYN --> ANO["Anomaly + Learning<br/>baseline vs. period; corrections"]
    ANO --> SRV
```

- **Ingest** writes immutable `raw_event` rows, each carrying provenance and an `acl_tag` mapping to the resource/field model ([03 §2](./03-access-control.md)).
- **Extract** uses the LLM to pull atomic facts and entities from raw events and document text.
- **Synthesize** dedupes, **supersedes** stale facts (never destructive overwrite — old facts are marked superseded with a timestamp), summarizes, and writes/updates **Markdown context cards**.
- **Index** computes embeddings (DuckDB VSS / `sqlite-vec`) and structured views for fast retrieval.
- **Serve** answers retrieval requests, authorizing each candidate before it reaches the agent/LLM.
- **Anomaly + Learning** compares period metrics to a rolling baseline, emits `anomaly` rows + a memory, and absorbs human corrections as `learning` rows that bias future synthesis.

## 3. Index data model (file-based)

| Table | Purpose | Key columns |
|---|---|---|
| `raw_event` | immutable ingested record | `id, source_id, view, payload(json), ingested_at, acl_tag` |
| `memory` | index row for a synthesized Markdown card | `id, kind, topic, path, acl_tag, confidence, period, supersedes, created_at` |
| `provenance` | memory ↔ source link | `memory_id, raw_event_id` (or `doc_id` for doc-derived) |
| `embedding` | vector for semantic search | `memory_id, vector` |
| `anomaly` | detected deviation | `id, view, metric, period, baseline, observed, severity, acl_tag, memory_id` |
| `learning` | correction/feedback for future synthesis | `id, topic, statement, applies_from, acl_tag, provenance_id, source` |

`memory.path` points at the Markdown file; the `body` lives in that file. `acl_tag` on every raw event maps to the resource/field model; **retrieved memories inherit the access requirements of their provenance.** Every *derived* row — `memory`, `anomaly`, `learning` — carries its own `acl_tag` set to the **max** acl of the facts/sources it was computed from (**taint propagation**); it is never lower than its inputs. `learning` rows carry a `provenance_id` so a human correction that quotes a privileged value inherits that value's acl rather than becoming world-readable.

## 4. Retrieval (capability-filtered)

1. Resolve candidate memories by **hybrid** match: semantic (embeddings) + keyword (SQLite **FTS5** over Markdown) + structured (view/predicate). File-tree navigation and `grep` over the Markdown are first-class too.
2. For each candidate, authorize against the caller's Biscuit token ([03 §4](./03-access-control.md)): **structured rows** are field/row-redacted column-by-column; **Markdown cards** are authorized **all-or-nothing** against the card's `acl_tag` (prose cannot be column-redacted), with a value-scrub pass as defense-in-depth.
3. **Drop** anything the caller does not dominate **before** it reaches the agent or any LLM.

The redaction boundary lives **only in the brain query layer / MCP path** — structured `brain.query` + field/row redaction need **no LLM at all**, which is what makes the local-first guarantee hold even with the cloud disconnected. **The capability guarantee holds for callers that reach the brain through MCP.** Direct host-filesystem access to `~/.contextful/` (the Markdown tree, `brain.duckdb` whose `raw_event.payload` holds *un-redacted* source JSON, and `caps/`) is **outside** the trust boundary; the offline local runtime must therefore run under enforced isolation ([04 §2](./04-sandbox-agents.md)) before it can claim the same guarantee as the Vercel Sandbox path.

## 5. Scoping

Memory is scoped along two axes:

- **Principal scope** (mem0-style): `user` / `agent` / `session`.
- **Tier** (icarus-style): `working` (per-task scratch) / `archive` (per-agent history) / `wiki` (shared synthesized source of truth).
- **Brain scope per room**: each document declares which sources/views its sandbox may draw on. This bounds context *in addition to* the agent's own token — sharing a room never widens an agent's reach.

## 6. Storage layout

```
~/.contextful/
  control/                 # principals, keys, envelopes, tailnet config  (see 07)
  docs/
    <doc_id>.loro          # per-doc Loro snapshot + oplog                 (see 01)
  brain/
    <topic>/*.md           # human-readable synthesized memory (source of truth)
  brain.duckdb             # raw_event, memory, provenance, embedding, anomaly, learning
  fixtures/
    stripe/*.csv           # Kaggle-derived mock data                      (see 05)
  caps/                    # issued/attenuated token records (audit/revocation)  (see 03)
```

Prefer **DuckDB** for columnar FinOps aggregates, **SQLite** for transactional KV + FTS5 keyword search, and **`sqlite-vec`** for vector search (exact/brute-force — fine at demo scale and persists reliably; DuckDB's `vss` HNSW persistence is experimental and off by default). Loro per-doc. Everything on-host; nothing requires cloud to read or edit.

## 7. Inference

The brain spawns agents to ingest and synthesize. Inference is **trait-based and swappable by config** (see [04 §3](./04-sandbox-agents.md)):

- **Default:** the **Vercel AI Gateway** — a single OpenAI-compatible endpoint (`https://ai-gateway.vercel.sh/v1`, auth via `AI_GATEWAY_API_KEY`) that fronts Claude with cross-provider failover and unified usage/billing. Models are addressed by provider-prefixed slug — `anthropic/claude-opus-4-8` (high-stakes synthesis), `anthropic/claude-sonnet-4-6` (routine extraction), `anthropic/claude-haiku-4-5` (cheap classification). The Rust brain reaches the Gateway with `async-openai`; TypeScript surfaces use the **Vercel AI SDK** (`@ai-sdk/gateway` provider) — see [04 §3](./04-sandbox-agents.md).
- **On-prem / offline:** **LM Studio** via OpenAI-compatible endpoint (`http://localhost:1234/v1`) on the host (e.g. Mac Studio).

Only already-permitted content is ever sent to any backend; structured query + redaction never call an LLM.

## 8. Scaffold / Status

| Spec element | Code |
|---|---|
| `ingest` one-shot pipeline | `crates/sync/src/main.rs` → `connectors` + `brain` |
| Markdown brain read/write/supersede | `crates/sync/src/brain/markdown.rs` ✅ built |
| Extract → synthesize → anomaly/learning | `crates/sync/src/brain/synthesis.rs` ✅ built |
| Capability-filtered retrieval | `crates/sync/src/brain/retrieval.rs` ✅ built |
| Memory / Scope / MemoryRef types | `crates/sync/src/brain/mod.rs` ✅ built |
| File store (Loro snapshots + JSON index) | `crates/sync/src/store/{mod,docs}.rs` ✅ built (JSON index stand-in) |
| TS types | `packages/protocol/src/brain.ts` — `MemoryRef`, `Scope`, `SearchQuery`, `SearchResult` |

**Future:** the columnar/FTS/vector index (**DuckDB / SQLite FTS5 / sqlite-vec** — today the index is a single `brain.index.json`, not `brain.duckdb`), real LLM extract/synthesize, embeddings, non-destructive supersede (the current pass recomputes derived rows; `Memory.supersedes` is modeled but unused), `brain.remember` per-turn read-set taint, compaction.
