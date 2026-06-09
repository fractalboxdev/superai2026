# 06 · MCP Interface

**Anchors:** `crates/sync` subcommand `mcp`; module `brain/mcp.rs`.

The brain is exposed to agents as an **embedded MCP server** built on [`rmcp`](https://crates.io/crates/rmcp) (the official Rust MCP SDK). Any harness that speaks MCP — Claude Code, OpenClaw, a sandboxed agent loop — can connect with an **identity** (a Biscuit token) and query the brain, subject to capability checks on every call.

> `rmcp` = **Rust MCP SDK**, not "reverse MCP". Agents are MCP **clients** of the host's brain MCP **server**.

## 1. Tools

| Tool | Purpose | Caps required |
|---|---|---|
| `brain.list_sources()` | sources/views the caller may see | any read on ≥1 view |
| `brain.search(query, scope?)` | hybrid semantic + structured memory search | per-result field/row auth |
| `brain.get_context(topic)` | synthesized Markdown context card | per-provenance auth |
| `brain.query(view, select, where)` | structured query (view + projection + predicate) | `query(view)` + field/row |
| `brain.detect_anomalies(view, period)` | anomalies for a period | `query(view)` |
| `brain.remember(fact, doc)` | write a memory scoped to a document | `write(document)` |
| `brain.request_access(resource, fields, rows, reason)` | raise a permission request ([03 §5](./03-access-control.md)) | none |

`brain.query` is deterministic and **needs no LLM** — it is the path that keeps working offline. `brain.search` returns results each **independently authorized**; structured rows are field/row-redacted, while Markdown cards (`brain.search`, `brain.get_context`) are authorized **all-or-nothing** against the card's `acl_tag` ([02 §3](./02-brain-memory.md)) since prose cannot be column-redacted. The optional `scope?` on `brain.search` narrows by **brain scope** (sources/views), **tier** (`working`/`archive`/`wiki`), and **principal scope** ([02 §5](./02-brain-memory.md)) — it can only *narrow*, never widen, the caller's authority. `brain.remember` is **taint-tracked**: the written memory is stamped with at least the max `acl_tag` of every source the agent read in that turn, so privileged context can't be laundered into a low-acl card.

## 2. Transports

- **stdio** — for co-located agents (the on-host sandbox; a local Claude Code session).
- **Streamable HTTP** (`rmcp` `StreamableHttpService`) over **Tailscale** — for remote agents, including those running in **Vercel Sandbox** ([04](./04-sandbox-agents.md)). The tailnet provides confidential transport; Tailscale ACLs restrict which nodes may reach the MCP port ([07](./07-deployment-iac.md)).

## 3. Authentication

The caller presents its Biscuit token on the MCP session (and it is re-checked per tool call). The token is the agent's **identity and authority** in one — there is no separate auth system. Every `brain.*` call resolves the caller's capabilities and applies field/row redaction in the brain query layer ([02 §4](./02-brain-memory.md)) before returning.

## 4. Packaging

The MCP server can run as its own subcommand (`sync mcp`) or be co-hosted with the relay (`serve --with-mcp`); the demo uses `sync mcp` over the tailnet. (Open question carried from reference §20 — resolved to `sync mcp` for clarity.)

## 5. Scaffold / Status

| Spec element | Code |
|---|---|
| `mcp` subcommand | `crates/sync/src/main.rs` → `brain::mcp::run` |
| `rmcp` server + `brain.*` tool definitions | `crates/sync/src/brain/mcp.rs` ✅ built |
| Per-call capability check | `crates/sync/src/access/biscuit.rs` ✅ built |
| Tool names / arg + result types (TS) | `packages/protocol/src/brain.ts` — `BrainToolName`, `SearchQuery`, `SearchResult` |

**Future:** real `rmcp` tool handlers, stdio + streamable-HTTP transports, session auth binding, result redaction wiring.
