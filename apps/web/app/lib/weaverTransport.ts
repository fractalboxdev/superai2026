// Contextful transport plugin for the Weaver editor (spec 01 §2 "Integration
// decision": keep Weaver's editor + CRDT client, replace its sync backend).
//
// Binds a Weaver `LoroDoc` — the single source of truth for the room — to the
// Contextful sync fabric instead of Weaver's default Cloudflare backend:
//   • BroadcastChannel — always on, syncs Loro updates across browser tabs
//     with no backend (the public Vercel demo shows live collaboration).
//   • WebSocket relay  — when VITE_SYNC_URL is set, speaks the Contextful
//     wire protocol (spec 01 §4: HELLO / SUBSCRIBE / SNAPSHOT / UPDATE /
//     AWARENESS) to `sync serve`; inbound frames are validated with Effect
//     Schema (see ./wire). The relay treats Loro bytes as opaque, so the
//     Weaver block tree replaces the old plaintext `body` container with no
//     wire change.
//   • localStorage      — snapshot persistence per doc, so a reload (and the
//     next tab) rehydrates instantly. Key is versioned: v2 = Weaver block
//     tree ("content" LoroTree); v1 plaintext `body` snapshots are ignored.
//
// Presence (spec 01 §5) rides the same channels as ephemeral AWARENESS
// messages — `PresenceState` (principal, display_name, reading/writing mode,
// heartbeat), never persisted, never carrying document or brain content.
//
// This module is client-only (loro-crdt is WASM-backed): it is reached
// exclusively via dynamic import from `useWeaverRoom`, so it never executes
// in the SSR bundle.

import { LoroDoc, LoroMap, LoroText } from "loro-crdt";
import {
  awareness as awarenessMsg,
  hello,
  subscribe as subscribeMsg,
  update as updateMsg,
  type PresenceState,
} from "@superai2026/protocol/sync";
import { decodePresence, decodeWire } from "./wire";
import { DOCS } from "./docs";

export type RoomStatus = "local" | "connecting" | "live" | "offline";

export interface RoomTransportOptions {
  docId: string;
  principal: string;
  displayName: string;
  /** WS relay URL (VITE_SYNC_URL). Omit for the cross-tab-only demo path. */
  relayUrl?: string | undefined;
  onStatus: (status: RoomStatus) => void;
  /** Live peers (within the staleness window), excluding self. */
  onPeers: (peers: PresenceState[]) => void;
}

export interface RoomTransport {
  dispose: () => void;
}

const STALE_MS = 15_000;
const HEARTBEAT_MS = 5_000;
const WRITING_MS = 2_000;
const SAVE_DEBOUNCE_MS = 400;
const RECONNECT_BASE_MS = 1_000;
const RECONNECT_MAX_MS = 30_000;

// relay ERROR codes that end the session for good — reconnecting would just
// be rejected again (transient drops have no ERROR frame, only a close)
const FATAL_ERROR_CODES = new Set(["revoked", "no_capability", "expected_hello"]);

// v2 = Weaver block tree ("content" LoroTree). v1 (unversioned key) held the
// plaintext `body` container and must not hydrate the Weaver editor.
const storageKey = (docId: string) => `contextful:doc:v2:${docId}`;
const channelName = (docId: string) => `contextful:room:${docId}`;

const toNums = (b: Uint8Array): number[] => Array.from(b);
const toBytes = (n: readonly number[]): Uint8Array => Uint8Array.from(n);

function loadSnapshot(docId: string): Uint8Array | null {
  try {
    const raw = localStorage.getItem(storageKey(docId));
    return raw ? Uint8Array.from(atob(raw), (c) => c.charCodeAt(0)) : null;
  } catch {
    return null;
  }
}

function saveSnapshot(docId: string, bytes: Uint8Array) {
  try {
    let bin = "";
    for (const byte of bytes) bin += String.fromCharCode(byte);
    localStorage.setItem(storageKey(docId), btoa(bin));
  } catch {
    /* quota / unavailable — non-fatal for the demo */
  }
}

// Deterministic seed: build the doc's initial block tree on a throwaway
// LoroDoc with a FIXED peer id, so every tab / peer generates byte-identical
// ops. Importing the seed is therefore idempotent — two fresh tabs (or a
// fresh tab joining a relay doc started from the same seed) converge to ONE
// copy of the content instead of duplicating paragraphs. Container names and
// block shape mirror @weaver/core's editor (`content` tree, `kind` / `attrs`
// / `text` keys).
const SEED_PEER_ID = 7_777_777n;

function buildSeed(docId: string): Uint8Array {
  const seedText = DOCS.find((d) => d.id === docId)?.seed ?? "";
  const paragraphs = seedText.split("\n\n").filter((p) => p.length > 0);
  const doc = new LoroDoc();
  doc.setPeerId(SEED_PEER_ID);
  const tree = doc.getTree("content");
  const blocks = paragraphs.length > 0 ? paragraphs : [""];
  for (const para of blocks) {
    const node = tree.createNode();
    node.data.set("kind", "paragraph");
    node.data.setContainer("attrs", new LoroMap());
    const text = node.data.setContainer("text", new LoroText());
    if (para.length > 0) text.insert(0, para);
  }
  doc.commit({ origin: "seed" });
  return doc.export({ mode: "snapshot" });
}

/**
 * Attach the Contextful transports to a Weaver editor's `LoroDoc`.
 *
 * Hydration order: deterministic seed first (guarantees the editing surface
 * is never block-less), then the persisted local snapshot (a superset of the
 * seed history plus prior edits), then — on the relay path — the server's
 * SNAPSHOT as it arrives. All three are CRDT imports, so order only affects
 * latency, not convergence.
 */
export function attachRoomTransport(
  doc: LoroDoc,
  opts: RoomTransportOptions,
): RoomTransport {
  const { docId, principal, displayName, relayUrl, onStatus, onPeers } = opts;

  let disposed = false;
  let ws: WebSocket | undefined;
  let bc: BroadcastChannel | undefined;
  let heartbeat: ReturnType<typeof setInterval> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  let reconnectAttempt = 0;
  let fatal = false;
  let lastLocalEdit = 0;
  const peers = new Map<string, PresenceState>();

  const publishPeers = () => {
    if (disposed) return;
    const now = Date.now();
    for (const [id, p] of peers) {
      if (now - p.heartbeat >= STALE_MS) peers.delete(id);
    }
    onPeers([...peers.values()].filter((p) => p.principal !== principal));
  };

  const upsertPeer = (p: PresenceState) => {
    if (p.principal === principal) return;
    peers.set(p.principal, p);
    publishPeers();
  };

  // ---- hydrate ------------------------------------------------------------
  doc.import(buildSeed(docId));
  const persisted = loadSnapshot(docId);
  if (persisted) {
    try {
      doc.import(persisted);
    } catch {
      /* corrupt local snapshot — seed already guarantees a usable doc */
    }
  }
  saveSnapshot(docId, doc.export({ mode: "snapshot" }));

  // ---- outbound -----------------------------------------------------------
  const broadcast = (bytes: Uint8Array) => {
    const nums = toNums(bytes);
    bc?.postMessage({ kind: "update", bytes: nums });
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(updateMsg(docId, nums)));
    }
  };

  // Debounce persistence: every CRDT event (local AND each inbound frame)
  // lands here, and a full-snapshot localStorage write per frame would block
  // the main thread on busy multi-peer sessions.
  const scheduleSave = () => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      saveSnapshot(docId, doc.export({ mode: "snapshot" }));
    }, SAVE_DEBOUNCE_MS);
  };

  // Ship the full update log on local commits (idempotent imports on peers;
  // keeps the relay's overwrite-persistence complete — real version-vector
  // deltas are spec 01 §4 Future). Weaver's own doc.subscribe DOM rerender
  // and this subscription coexist on the same LoroDoc.
  const unsub = doc.subscribe((e) => {
    if (disposed) return;
    if (e.by === "local") {
      lastLocalEdit = Date.now();
      broadcast(doc.export({ mode: "update" }));
    }
    scheduleSave();
  });

  // ---- cross-tab transport (no backend required) ---------------------------
  bc = new BroadcastChannel(channelName(docId));
  bc.onmessage = (ev) => {
    if (disposed || !ev.data) return;
    const data = ev.data as {
      kind?: string;
      bytes?: number[];
      presence?: PresenceState;
    };
    if (data.kind === "update" && Array.isArray(data.bytes)) {
      doc.import(toBytes(data.bytes));
    } else if (data.kind === "awareness" && data.presence) {
      // validate like WS frames — another tab's payload is still untrusted input
      const presence = decodePresence(data.presence);
      if (presence) upsertPeer(presence);
    }
  };

  // ---- relay transport (opt-in, Contextful wire protocol §4) ---------------
  // Transient drops (relay restart, network blip) reconnect with exponential
  // backoff; a fatal relay ERROR (revoked/no-capability) stays offline.
  const scheduleReconnect = () => {
    const delay = Math.min(
      RECONNECT_BASE_MS * 2 ** reconnectAttempt,
      RECONNECT_MAX_MS,
    );
    reconnectAttempt += 1;
    reconnectTimer = setTimeout(connect, delay);
  };

  const connect = () => {
    if (disposed || fatal || !relayUrl) return;
    onStatus("connecting");
    try {
      ws = new WebSocket(relayUrl);
    } catch {
      onStatus("offline");
      return;
    }
    ws.onopen = () => {
      if (disposed) return;
      reconnectAttempt = 0;
      onStatus("live");
      ws!.send(JSON.stringify(hello(principal)));
      ws!.send(JSON.stringify(subscribeMsg(docId)));
      broadcast(doc.export({ mode: "update" }));
    };
    ws.onmessage = (ev) => {
      if (disposed) return;
      const msg = decodeWire(typeof ev.data === "string" ? ev.data : "");
      if (!msg) return;
      if (msg.type === "SNAPSHOT" || msg.type === "UPDATE") {
        if (msg.bytes.length) doc.import(toBytes(msg.bytes));
      } else if (msg.type === "AWARENESS") {
        upsertPeer(msg.presence);
      } else if (msg.type === "ERROR") {
        console.warn(`[contextful] relay error ${msg.code}: ${msg.message}`);
        if (FATAL_ERROR_CODES.has(msg.code)) {
          fatal = true;
          onStatus("offline");
          ws?.close();
        }
      }
    };
    ws.onclose = () => {
      if (disposed) return;
      onStatus("offline");
      if (!fatal) scheduleReconnect();
    };
    // a failed connection also fires onclose — reconnect is handled there
    ws.onerror = () => {};
  };

  if (relayUrl) {
    connect();
  } else {
    onStatus("local");
  }

  // ---- presence heartbeat over both transports -----------------------------
  const beat = () => {
    if (disposed) return;
    const presence: PresenceState = {
      principal,
      display_name: displayName,
      mode: Date.now() - lastLocalEdit < WRITING_MS ? "writing" : "reading",
      heartbeat: Date.now(),
    };
    bc?.postMessage({ kind: "awareness", presence });
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(awarenessMsg(docId, presence)));
    }
    publishPeers(); // prune stale peers even when nobody is talking
  };
  beat();
  heartbeat = setInterval(beat, HEARTBEAT_MS);

  return {
    dispose: () => {
      disposed = true;
      if (heartbeat) clearInterval(heartbeat);
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (saveTimer) {
        // flush the pending debounced write so the last edits survive reload
        clearTimeout(saveTimer);
        saveSnapshot(docId, doc.export({ mode: "snapshot" }));
      }
      unsub();
      bc?.close();
      ws?.close();
    },
  };
}
