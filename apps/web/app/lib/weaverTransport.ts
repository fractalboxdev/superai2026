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

/** A NOTIFY frame addressed to this peer — decision metadata, never content. */
export type RoomNotice = { from: string; reason: string; message: string };

export interface RoomTransportOptions {
  docId: string;
  principal: string;
  displayName: string;
  /** WS relay URL (VITE_SYNC_URL). Omit for the cross-tab-only demo path. */
  relayUrl?: string | undefined;
  onStatus: (status: RoomStatus) => void;
  /** Live peers (within the staleness window), excluding self. */
  onPeers: (peers: PresenceState[]) => void;
  /** Access-decision notifications addressed to this peer's principal. */
  onNotice?: ((notice: RoomNotice) => void) | undefined;
}

export interface RoomTransport {
  /**
   * Carry the local caret on the presence record (upstream weaver PR #35:
   * cursors and the roster draw from one identity set). Re-broadcasts
   * immediately when the caret actually moved; the heartbeat keeps it alive.
   */
  setCursor: (cursor: { blockId: string; offset: number } | null) => void;
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

// v3 = Weaver block tree, keyed by the doc's seed-text hash: a snapshot saved
// under an older seed revision (or one poisoned by the fixed-peer-id collision
// the hash now prevents — see buildSeed) must not hydrate the editor. v2
// (unhashed) and v1 (plaintext `body`) entries are simply never read again.
const storageKey = (docId: string) =>
  `contextful:doc:v3:${docId}:${seedPeerId(docId).toString(36)}`;
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
// LoroDoc with a peer id DERIVED FROM THE SEED TEXT, so every tab / peer
// generates byte-identical ops. Importing the seed is therefore idempotent —
// two fresh tabs (or a fresh tab joining a relay doc started from the same
// seed) converge to ONE copy of the content instead of duplicating paragraphs.
//
// The peer id must change whenever the seed copy changes: two DIFFERENT op
// histories under the SAME (peer, counter) pairs are CRDT corruption — Loro's
// wasm panics (`RuntimeError: unreachable`) on the first read after merging
// them, taking the whole app down. A fixed peer id (the old `7_777_777n`) did
// exactly that the first time a seed was reworded while relay/localStorage
// docs still carried the old seed's history. Hashing the seed text instead
// makes a reworded seed a distinct peer: merging old + new yields duplicated
// paragraphs (cosmetic), never a poisoned doc.
//
// Container names and block shape mirror @weaver/core's editor (`content`
// tree, `kind` / `attrs` / `text` keys).
const seedTextOf = (docId: string): string =>
  DOCS.find((d) => d.id === docId)?.seed ?? "";

/** FNV-1a 64-bit — stable, dependency-free hash of the seed copy. */
const fnv1a64 = (s: string): bigint => {
  let h = 0xcbf29ce484222325n;
  for (let i = 0; i < s.length; i++) {
    h ^= BigInt(s.charCodeAt(i));
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return h;
};

const seedPeerId = (docId: string): bigint => fnv1a64(seedTextOf(docId)) | 1n;

// `@[Label](principalId)` in seed copy → the visible text `@Label` carrying
// the same `mention` mark the @-picker writes, so seeded names are real
// tagged chips, not prose.
const SEED_MENTION = /@\[([^\]]+)\]\(([^)]+)\)/g;

export type SeedMention = {
  start: number;
  end: number;
  value: { userId: string; label: string; kind: "user" | "agent" };
};

/** Exported for the console's "Reset demo" — it rebuilds the seed via editor commands. */
export function parseSeedParagraph(para: string): { text: string; mentions: SeedMention[] } {
  let text = "";
  let last = 0;
  const mentions: SeedMention[] = [];
  for (const m of para.matchAll(SEED_MENTION)) {
    text += para.slice(last, m.index);
    const label = `@${m[1]!}`;
    const start = text.length;
    text += label;
    mentions.push({
      start,
      end: start + label.length,
      value: {
        userId: m[2]!,
        label,
        kind: m[2]!.startsWith("agent:") ? "agent" : "user",
      },
    });
    last = m.index + m[0].length;
  }
  text += para.slice(last);
  return { text, mentions };
}

function buildSeed(docId: string): Uint8Array {
  const seedText = seedTextOf(docId);
  const paragraphs = seedText.split("\n\n").filter((p) => p.length > 0);
  const doc = new LoroDoc();
  doc.setPeerId(seedPeerId(docId));
  // Mirrors @weaver/core's mention style (expand: "none") — required before
  // a custom mark can be applied on this throwaway doc.
  doc.configTextStyle({ mention: { expand: "none" } });
  const tree = doc.getTree("content");
  const blocks = paragraphs.length > 0 ? paragraphs : [""];
  for (const para of blocks) {
    const node = tree.createNode();
    node.data.set("kind", "paragraph");
    node.data.setContainer("attrs", new LoroMap());
    const text = node.data.setContainer("text", new LoroText());
    const { text: plain, mentions } = parseSeedParagraph(para);
    if (plain.length > 0) text.insert(0, plain);
    for (const m of mentions) text.mark({ start: m.start, end: m.end }, "mention", m.value);
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
  const { docId, principal, displayName, relayUrl, onStatus, onPeers, onNotice } = opts;

  let disposed = false;
  let ws: WebSocket | undefined;
  let bc: BroadcastChannel | undefined;
  let heartbeat: ReturnType<typeof setInterval> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  let reconnectAttempt = 0;
  let fatal = false;
  let lastLocalEdit = 0;
  let cursor: { blockId: string; offset: number } | null = null;
  // One session per transport attach: two tabs of the same principal publish
  // two presence records instead of clobbering each other (weaver PR #35).
  const session = Math.random().toString(36).slice(2, 8);
  const peers = new Map<string, PresenceState>();

  const peerKey = (p: PresenceState) => `${p.principal}#${p.session ?? ""}`;

  const isSelf = (p: PresenceState) =>
    p.principal === principal && (p.session === undefined || p.session === session);

  const publishPeers = () => {
    if (disposed) return;
    const now = Date.now();
    for (const [id, p] of peers) {
      if (now - p.heartbeat >= STALE_MS) peers.delete(id);
    }
    onPeers([...peers.values()].filter((p) => !isSelf(p)));
  };

  const upsertPeer = (p: PresenceState) => {
    if (isSelf(p)) return;
    peers.set(peerKey(p), p);
    publishPeers();
  };

  // Poisoned bytes (e.g. divergent histories under one peer id) often import
  // without throwing and only panic Loro's wasm on the first READ — which in
  // React render means a full "Application Error" page. So before letting any
  // snapshot near the live editor doc, merge it on a throwaway doc and read it
  // back; a panic there is a caught exception on a doc we discard.
  const mergesCleanly = (...histories: Uint8Array[]): boolean => {
    try {
      const scratch = new LoroDoc();
      for (const bytes of histories) scratch.import(bytes);
      scratch.toJSON();
      return true;
    } catch {
      return false;
    }
  };

  // ---- hydrate ------------------------------------------------------------
  const seedBytes = buildSeed(docId);
  doc.import(seedBytes);
  const persisted = loadSnapshot(docId);
  if (persisted && mergesCleanly(seedBytes, persisted)) {
    doc.import(persisted);
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
      const bytes = toBytes(data.bytes);
      // another tab may still run an older bundle whose history collides —
      // drop its frames rather than corrupt this tab's doc (see mergesCleanly)
      if (mergesCleanly(doc.export({ mode: "snapshot" }), bytes)) {
        doc.import(bytes);
      }
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
    // The relay overwrite-persists the latest client blob as the doc snapshot,
    // so the first thing we push must already CONTAIN the relay's history.
    // Announcing our local state in onopen — before the SNAPSHOT arrives —
    // would regress the persisted doc to this tab's (often seed-only) state.
    let announced = false;
    ws.onopen = () => {
      if (disposed) return;
      reconnectAttempt = 0;
      onStatus("live");
      ws!.send(JSON.stringify(hello(principal)));
      ws!.send(JSON.stringify(subscribeMsg(docId)));
    };
    ws.onmessage = (ev) => {
      if (disposed) return;
      const msg = decodeWire(typeof ev.data === "string" ? ev.data : "");
      if (!msg) return;
      if (msg.type === "SNAPSHOT" || msg.type === "UPDATE") {
        if (msg.bytes.length) {
          const bytes = toBytes(msg.bytes);
          if (!mergesCleanly(doc.export({ mode: "snapshot" }), bytes)) {
            // Relay history is incompatible with this doc (poisoned blob or a
            // peer-id collision). Importing it would corrupt the live editor
            // doc and crash the app on the next render — stay offline instead.
            console.error(
              `[contextful] relay ${msg.type} for "${docId}" does not merge with the local doc — going offline (reset the relay's persisted doc to recover)`,
            );
            fatal = true;
            onStatus("offline");
            ws?.close();
            return;
          }
          doc.import(bytes);
        }
        if (msg.type === "SNAPSHOT" && !announced) {
          // merged seed + relay history → safe to announce as the new snapshot
          announced = true;
          broadcast(doc.export({ mode: "update" }));
        }
      } else if (msg.type === "AWARENESS") {
        upsertPeer(msg.presence);
      } else if (msg.type === "NOTIFY") {
        // act only on notifications addressed to this peer's principal
        if (msg.to === principal) {
          onNotice?.({ from: msg.from ?? "", reason: msg.reason, message: msg.message });
        }
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
      session,
      ...(cursor
        ? { cursor_block: cursor.blockId, cursor_anchor: cursor.offset }
        : {}),
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
    setCursor: (next) => {
      const moved =
        (cursor === null) !== (next === null) ||
        (next !== null &&
          (cursor!.blockId !== next.blockId || cursor!.offset !== next.offset));
      cursor = next;
      // Re-beat only on a real move — selectionchange fires liberally and an
      // unchanged caret would just spam the room with identical records.
      if (moved) beat();
    },
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
