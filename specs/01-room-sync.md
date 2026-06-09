# 01 · Rooms & Sync

**Anchors:** `crates/sync` subcommands `serve` / `client`; modules `sync/`; `packages/protocol/src/sync.ts`.

## 1. Rooms

A **room** is the collaboration unit. It is a 1:1 binding of:

- one **document** (a Loro CRDT doc),
- a set of **members** — principals (humans *and* their agents) each with a per-member capability (`read` / `write` / `comment`),
- a paired **sandbox** (see [04](./04-sandbox-agents.md)) and a **brain scope** (the sources/views the room's agents may use as context, see [02](./02-brain-memory.md)).

A user creates a room and shares it with team members and their agents; everyone collaborates in real time. Sharing a document does **not** widen what a member's agent can read from the brain — that is always bounded by the agent's own attenuated token ∩ the room's brain scope. The sandbox is **logically** paired 1:1 with the document but **instantiated on room entry** and reused/expired with presence (see [04 §2](./04-sandbox-agents.md)) — not created at document-creation time.

## 2. Editor (Weaver + Loro)

The editor is [Weaver](https://github.com/OpenHackersClub/weaver): headless TypeScript, **LoroDoc as the single source of truth**, Effect-TS plugin composition, OPFS local-first persistence, and **agents as first-class document peers**.

**Integration decision:** keep Weaver's editor + CRDT client; **replace its sync backend** with `crates/sync serve` over Tailscale (WSS). A Weaver transport plugin speaks the Contextful WS protocol (§4) instead of the default Cloudflare backend.

**Document model:** one `LoroDoc` per document.

- a rich-text **tree** container (paragraphs, headings, formatting),
- a `meta` **map** (`title`, `owner`, `members[]`, `created`, `updated`).

**Agents-as-peers:** each agent maps to an agent principal and edits the doc through the same Loro update path as a human, scoped by its `write`/`comment` capability on `document(<doc_id>)`. Agent edits carry an `origin` tag for per-peer undo and provenance.

## 3. Network model

- **Centralized & authoritative — not P2P.** The host's `serve` instance is the single source of truth; all peers (browsers, headless `client`s, sandbox agents) sync through it.
- **Transport:** WSS over **Tailscale** (WireGuard mesh + TLS). Tailscale is set up externally on the host (see [07](./07-deployment-iac.md)); this system assumes the tailnet exists.
- **Local client peer:** `sync client` is a headless peer that syncs documents to **local files** for editing outside the browser (e.g. a CLI or, in future, a native Mac app). It uses the same protocol and reflects local file edits as Loro updates.

## 4. Wire protocol (`serve` ↔ peer)

```mermaid
sequenceDiagram
    participant C as Peer (browser / client / agent)
    participant S as serve (authoritative)
    C->>S: HELLO { proto, principal, biscuit }
    S->>S: verify token; check read(document) cap
    S->>C: HELLO_OK { doc_id, server_vv }
    C->>S: SUBSCRIBE { doc_id, client_vv }
    S->>C: SNAPSHOT { loro_snapshot @ server_vv }
    par live
        C->>S: UPDATE { loro_update_bytes }   %% requires write cap
        S->>C: UPDATE { loro_update_bytes }   %% relayed from peers
        C-->>S: AWARENESS { cursor, selection, presence }  %% ephemeral
        S-->>C: AWARENESS { ... }
    end
```

Message types (mirrored in `packages/protocol/src/sync.ts`):

| Message | Direction | Payload | Authorization |
|---|---|---|---|
| `HELLO` | C→S | `proto`, `principal`, `biscuit` | token verified |
| `HELLO_OK` | S→C | `doc_id`, `server_vv` | — |
| `SUBSCRIBE` | C→S | `doc_id`, `client_vv` | `read(document)` |
| `SNAPSHOT` | S→C | Loro snapshot bytes @ `server_vv` | — |
| `UPDATE` | C↔S | Loro update bytes | send requires `write(document)`; comment-only → `comment(document)` |
| `AWARENESS` | C↔S | cursor, selection, presence (ephemeral) | `read(document)` |

**Technical choices:**

- **CRDT payloads are native Loro bytes.** Sync handshake uses Loro version vectors: a peer exports `doc.export(updates(peer_vv))`, the other `import`s, replies with its own delta. Snapshots use `export(snapshot)` for catch-up.
- **Authorization is per-message.** The Biscuit token arrives in `HELLO` and is re-checked on each `SUBSCRIBE`/`UPDATE`. Capabilities are on `document(<doc_id>)` (see [03](./03-access-control.md)).
- **Persistence:** per-doc Loro **snapshot + oplog** in the file store (`~/.contextful/docs/<doc_id>.loro`), periodically compacted (see [02 §6](./02-brain-memory.md)).

## 5. Presence / awareness ("who is here")

The room shows **who is actively reading vs. writing**. Presence rides Loro's **`EphemeralStore`** (the ephemeral/awareness primitive — timestamp-LWW key-value that sends only changed entries; supersedes the legacy `Awareness` API) — separate from document ops, never persisted, broadcast over the same WS.

`PresenceState` (per peer): principal id, display name, **mode** (`reading` | `writing` | `idle`), cursor anchor + selection range, and a heartbeat timestamp. Agents publish presence too (e.g. `mode = writing` while streaming tokens), so humans can see an agent working in the room. The amber "collaboration / presence" accent in the design system is reserved for these signals.

**Invariant:** awareness is document-ephemeral only and must **never** embed brain query results or any brain-derived content — it carries presence and cursors, nothing else.

## 6. Scaffold / Status

| Spec element | Code |
|---|---|
| `serve` / `client` subcommands | `crates/sync/src/main.rs` (dispatch) |
| WS relay (authoritative) | `crates/sync/src/sync/server.rs` — `run(addr)` ✅ built |
| Headless file-sync peer | `crates/sync/src/sync/client.rs` — `run(addr)` ✅ built |
| Wire messages + version-vector framing | `crates/sync/src/sync/protocol.rs` — `SyncMessage` enum ✅ built |
| Presence / awareness | `crates/sync/src/sync/presence.rs` — `PresenceState` ✅ built |
| TS protocol mirror | `packages/protocol/src/sync.ts` — `SyncMessage`, `PresenceState`, `RoomId`, `PeerId` |

**Future:** real Loro export/import handshake, OPFS↔serve reconciliation, compaction, Weaver transport plugin in `apps/web`, native client file watcher.
