# 04 · Sandbox & Agents

**Anchors:** `crates/sync` subcommand `agent`; modules `sandbox/`, `agent/`.

## 1. Per-document sandbox

Each document is paired 1:1 with a **secure sandbox** in which that room's agents run. Inside the sandbox an agent can query the brain to fetch relevant data; where it has insufficient access it raises a permission request to the resource/agent owner ([03 §5](./03-access-control.md)).

Invariants (independent of where the sandbox runs):

- **No ambient authority.** The agent's *only* data egress is the **brain MCP** ([06](./06-mcp-interface.md)); there is no arbitrary network or filesystem access. Every call is capability-checked.
- **Insufficient-access path.** A denied query → `request_access` → on approval the owner mints an attenuated token → the sandbox receives it → the agent retries.
- **Ephemeral, no durable private state.** Sandboxes are created and torn down with the room session; anything durable must be written back to the brain via `brain.remember`.

## 2. Runtime: Vercel Sandbox (default) + local fallback

The sandbox runtime is **pluggable** behind a `Sandbox` trait.

- **Vercel Sandbox — default ("agents from anywhere").** Members run agents from anywhere, local or cloud, with **any harness** (Claude Code, OpenClaw, …) — just provide an **identity** (a Biscuit token). The agent runs in a Vercel Sandbox microVM and connects back to the host's brain MCP over Tailscale.
  - **5-hour cap.** A sandbox lives at most ~5 hours (Pro/Enterprise). We **recreate the sandbox whenever someone re-enters the room** (and may `extendTimeout` an active one), so the limit is invisible in normal use.
  - **Created/torn down with room presence.** Entering a room provisions (or reuses) its sandbox; leaving lets it expire.
- **Local constrained process — offline fallback.** On-host, an agent runs as a constrained child process whose only socket is the brain MCP, with resource limits. This is the path used for the **fully-offline / on-prem proof** (Flow D), and the path *architected for* `wasmtime` isolation of untrusted agent/connector code with a capability-shaped host API.

```mermaid
flowchart TD
    R["Room (re-)entered"] --> P{"Sandbox alive?"}
    P -- no/expired --> CR["create Vercel Sandbox (≤5h)"]
    P -- yes --> RE["reuse"]
    CR --> AG["agent runtime (identity = biscuit)"]
    RE --> AG
    AG -- "brain MCP over Tailscale" --> MCP["host: sync mcp"]
    AG -. "denied → request_access" .-> OWN["owner approves → attenuated token"]
    OWN --> AG
```

**Ownership & lifecycle.** Sandbox provisioning is owned by the **host**, never the member — entering a room only *signals* presence and carries no authority to spin up compute. `serve` is authoritative for room presence; on a room's first entrant it calls `Sandbox::ensure(room_id)` (**single-flight** per room), which **reuses** a live sandbox (optionally `extendTimeout`) or **provisions** a fresh one via the configured driver. The host mints the agent's attenuated Biscuit identity at launch ([03](./03-access-control.md)). When the last member leaves, the host stops refreshing the sandbox and lets it expire; re-entry recreates it.

**Rust owns control; the Vercel call is the only TypeScript.** The lifecycle decision, the `room → sandbox` registry, and identity minting all live in Rust (`crates/sync`) — that is where the trust root and brain data are. The **local/offline driver** (`sandbox/local.rs`) is pure Rust. Only the **Vercel driver** (`sandbox/vercel.rs`) needs TypeScript: `@vercel/sandbox` is a TS-only SDK and Vercel ships the SDK as the supported contract (no stable public REST surface), so `sandbox/vercel.rs` shells out to a thin Node bridge — `packages/sandbox-bridge` (`@superai2026/sandbox-bridge`) — that wraps `Sandbox.create` / `extendTimeout` / teardown. The bridge is **hands-and-feet only**: it makes no policy decisions and never touches Biscuit minting. (Start as a per-call `node` subprocess; graduate to a warm host-side sidecar over a local socket for log streaming.)

> **Divergence from reference:** the reference (original `superai2026/specs/SPEC.md` draft) ran agents only in an on-host process. Here **Vercel Sandbox is the default** runtime (to support "run agents from anywhere with any harness"); the on-host constrained process is retained as the offline fallback.
>
> The trust boundary is **data-egress, not compute-locality**: a cloud sandbox holds **no ambient authority** and only ever sees the redacted, capability-filtered slice the brain returns — so cloud compute does not widen data exposure. Both runtimes share MCP-only-egress and ephemerality, but they are **not** equally isolated *yet*: the Vercel microVM is enforced today, whereas the local process's filesystem isolation is the future `wasmtime`/OS-sandbox work. Until that lands, the local fallback must run under OS-enforced isolation before it can claim the Vercel path's guarantee (see [02 §4](./02-brain-memory.md) — direct host-FS access bypasses the capability layer).

## 3. Agent runtime & inference

An agent is an **LLM loop** whose only tool surface is the brain MCP. The runtime carries the agent's identity, queries the brain, raises permission requests, and writes durable findings back via `brain.remember`. It also drives ingestion and synthesis jobs ([02](./02-brain-memory.md)).

Write-back is **taint-tracked**: a memory written via `brain.remember` is stamped with at least the max `acl_tag` of every source the agent read in that turn, so privileged context can't be laundered into a low-acl card ([06 §1](./06-mcp-interface.md)).

Inference is **trait-based**, swapped by config:

| Backend | Crate | Use |
|---|---|---|
| **Vercel AI Gateway** (default) | `async-openai` pointed at `https://ai-gateway.vercel.sh/v1` (auth via `AI_GATEWAY_API_KEY`) | high-quality synthesis & agent turns — one key fronts Claude, with cross-provider failover |
| **OpenAI-compatible** (LM Studio) | `async-openai` pointed at `http://localhost:1234/v1` | on-prem / offline mode |
| `StubInference` | — | default when no backend feature is enabled (keeps the scaffold compiling) |

Because the Gateway speaks the OpenAI API, the cloud and offline backends share **one** `async-openai` client implementation — only the base URL, key, and model id differ.

**Client SDKs — one Gateway, two callers**, both sharing `AI_GATEWAY_API_KEY` and the `anthropic/claude-*` model slugs:

- **Rust (`crates/sync` inference trait)** — `async-openai` against the Gateway's OpenAI-compatible endpoint; drives brain synthesis and the Rust agent loop.
- **TypeScript (Vercel Sandbox agent harnesses, `apps/web`)** — the **Vercel AI SDK** (`ai` + the `@ai-sdk/gateway` provider, e.g. `gateway('anthropic/claude-opus-4-8')`), which gives streaming, tool-calling, and Gateway routing/observability out of the box. On Vercel it can authenticate via an OIDC token instead of a static key.

> **Divergence from reference:** the reference defaulted to LM Studio + Gemma. Here the **default is the Vercel AI Gateway** (a single OpenAI-compatible endpoint that routes to Claude with unified usage/billing and provider failover), with LM Studio as the explicit on-prem/offline backend. The local-first guarantee still holds because the data boundary (field/row redaction, structured query) needs **no** LLM — and the offline demo runs on LM Studio.

## 4. Learning from past mistakes & anomalies

Agents improve month-over-month: the synthesis pipeline detects anomalies against a rolling baseline, and human corrections in the document are captured as `learning` rows that suppress false re-flags and bias future synthesis ([02 §2](./02-brain-memory.md), [09](./09-testing-acceptance.md) Flow C).

## 5. Scaffold / Status

| Spec element | Code |
|---|---|
| `agent` subcommand | `crates/sync/src/main.rs` → `agent::runtime::run` |
| Agent loop (MCP-only tool surface, request_access) | `crates/sync/src/agent/runtime.rs` ✅ built (trait + stub driver) |
| Inference trait + StubInference (+ OpenAI-compatible backend feature) | `crates/sync/src/agent/inference.rs` ✅ built (trait + stub driver) |
| Sandbox trait + lifecycle | `crates/sync/src/sandbox/mod.rs` ✅ built (trait + stub driver) |
| Vercel Sandbox driver (Rust control) | `crates/sync/src/sandbox/vercel.rs` ✅ built (trait + stub driver) |
| Vercel Sandbox Node bridge (`@vercel/sandbox` wrapper) | `packages/sandbox-bridge` (Node, stub) |
| Local constrained-process driver | `crates/sync/src/sandbox/local.rs` ✅ built (trait + stub driver) |

**Future:** real Vercel Sandbox orchestration via the `packages/sandbox-bridge` Node wrapper, recreate-on-re-entry wiring, `wasmtime` isolation, Vercel AI Gateway / LM Studio calls, agent harness adapters (Claude Code / OpenClaw).
