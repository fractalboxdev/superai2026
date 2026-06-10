// useLoroRoom — binds one document to a live Loro CRDT room (spec 01 §2).
//
// Every doc is a `LoroDoc` with a `body` text container. Local edits diff into
// the CRDT and ship to peers; remote updates merge back and re-render. Two
// transports, both opt-out-free for the demo:
//   • BroadcastChannel — always on, syncs across browser tabs with no backend
//     (so the public Vercel demo shows live collaboration in two tabs).
//   • WebSocket relay  — when VITE_SYNC_URL is set, speaks the Contextful wire
//     protocol (§4) to `sync serve`; inbound frames are validated via Effect
//     Schema (see ./wire).
// loro-crdt is WASM-backed and imported lazily inside the effect, so it never
// touches the SSR/server bundle.

import { useCallback, useEffect, useRef, useState } from "react";
import type { LoroDoc, LoroText } from "loro-crdt";
import {
  awareness as awarenessMsg,
  hello,
  subscribe,
  update as updateMsg,
  type PresenceState,
} from "@superai2026/protocol/sync";
import { DOCS } from "./docs";

export type RoomStatus = "local" | "connecting" | "live" | "offline";

export type LoroRoom = {
  /** Current document body text. */
  text: string;
  status: RoomStatus;
  /** Live peers (presence within the staleness window), excluding self. */
  peers: PresenceState[];
  /** Apply a local edit: diff `value` into the CRDT body. */
  applyText: (value: string) => void;
};

const STALE_MS = 15_000;
const HEARTBEAT_MS = 5_000;
const WRITING_MS = 2_000;

const seedFor = (docId: string) => DOCS.find((d) => d.id === docId)?.seed ?? "";
const storageKey = (docId: string) => `contextful:doc:${docId}`;
const channelName = (docId: string) => `contextful:room:${docId}`;
const SYNC_URL_KEY = "contextful:syncUrl";

// Relay URL resolution: `?sync=ws://…` overrides and persists (so the deployed
// demo can point at a local `sync serve`), `?sync=off` clears the override;
// otherwise the persisted override, then the build-time VITE_SYNC_URL.
// Browsers treat loopback as a secure context, so https://demo.contextful.work
// may open ws://127.0.0.1:7878 directly.
export function resolveSyncUrl(): string | undefined {
  try {
    const param = new URLSearchParams(window.location.search).get("sync");
    if (param === "off") localStorage.removeItem(SYNC_URL_KEY);
    else if (param) {
      localStorage.setItem(SYNC_URL_KEY, param);
      return param;
    } else {
      const stored = localStorage.getItem(SYNC_URL_KEY);
      if (stored) return stored;
    }
  } catch {
    /* SSR / storage unavailable — fall through to the env default */
  }
  return import.meta.env.VITE_SYNC_URL as string | undefined;
}

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

export function useLoroRoom(
  principal: string,
  displayName: string,
  docId: string,
): LoroRoom {
  const [text, setText] = useState<string>("");
  const [status, setStatus] = useState<RoomStatus>("local");
  const [peers, setPeers] = useState<Record<string, PresenceState>>({});

  const docRef = useRef<LoroDoc | null>(null);
  const textRef = useRef<LoroText | null>(null);
  const lastTypedRef = useRef<number>(0);

  const applyText = useCallback((value: string) => {
    const doc = docRef.current;
    const body = textRef.current;
    if (!doc || !body) return;
    lastTypedRef.current = Date.now();
    body.update(value);
    doc.commit();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unsub: (() => void) | undefined;
    let ws: WebSocket | undefined;
    let bc: BroadcastChannel | undefined;
    let heartbeat: ReturnType<typeof setInterval> | undefined;

    const relayUrl = resolveSyncUrl();
    setText("");
    setPeers({});
    setStatus(relayUrl ? "connecting" : "local");

    void (async () => {
      const { LoroDoc } = await import("loro-crdt");
      if (disposed) return;

      const doc = new LoroDoc();
      const body = doc.getText("body");
      docRef.current = doc;
      textRef.current = body;

      // Hydrate: a local snapshot wins. Otherwise seed only when there's no
      // relay — a relay peer takes the server SNAPSHOT as source of truth, so
      // it never double-seeds. localStorage is shared across tabs, so the first
      // tab seeds and the rest read it back.
      const persisted = loadSnapshot(docId);
      if (persisted) {
        doc.import(persisted);
      } else if (!relayUrl) {
        body.update(seedFor(docId));
        doc.commit();
        saveSnapshot(docId, doc.export({ mode: "snapshot" }));
      }
      setText(body.toString());

      const broadcast = (bytes: Uint8Array) => {
        const nums = toNums(bytes);
        bc?.postMessage({ kind: "update", bytes: nums });
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify(updateMsg(docId, nums)));
        }
      };

      // Re-render on every commit; ship the full update log on local edits
      // (idempotent imports on peers; keeps the relay's overwrite-persistence
      // complete — real version-vector deltas are spec 01 §4 Future).
      unsub = doc.subscribe((e) => {
        setText(body.toString());
        if (e.by === "local") broadcast(doc.export({ mode: "update" }));
        saveSnapshot(docId, doc.export({ mode: "snapshot" }));
      });

      // Cross-tab transport — no backend required.
      bc = new BroadcastChannel(channelName(docId));
      bc.onmessage = (ev) => {
        if (disposed || !ev.data) return;
        const data = ev.data as { kind?: string; bytes?: number[]; presence?: PresenceState };
        if (data.kind === "update" && Array.isArray(data.bytes)) {
          doc.import(toBytes(data.bytes));
        } else if (data.kind === "awareness" && data.presence) {
          const p = data.presence;
          if (p.principal !== principal) setPeers((prev) => ({ ...prev, [p.principal]: p }));
        }
      };

      // Relay transport — opt-in via VITE_SYNC_URL. Effect Schema (./wire) is
      // imported here, so it stays out of the bundle on the cross-tab-only path.
      if (relayUrl) {
        const { decodeWire } = await import("./wire");
        try {
          ws = new WebSocket(relayUrl);
          ws.onopen = () => {
            if (disposed) return;
            setStatus("live");
            ws!.send(JSON.stringify(hello(principal)));
            ws!.send(JSON.stringify(subscribe(docId)));
            broadcast(doc.export({ mode: "update" }));
          };
          ws.onmessage = (ev) => {
            const msg = decodeWire(typeof ev.data === "string" ? ev.data : "");
            if (!msg) return;
            if (msg.type === "SNAPSHOT" || msg.type === "UPDATE") {
              if (msg.bytes.length) doc.import(toBytes(msg.bytes));
            } else if (msg.type === "AWARENESS" && msg.presence.principal !== principal) {
              setPeers((prev) => ({ ...prev, [msg.presence.principal]: msg.presence }));
            }
          };
          ws.onclose = () => {
            if (!disposed) setStatus("offline");
          };
          ws.onerror = () => {
            if (!disposed) setStatus("offline");
          };
        } catch {
          setStatus("offline");
        }
      }

      // Presence heartbeat over both transports.
      const beat = () => {
        const presence: PresenceState = {
          principal,
          display_name: displayName,
          mode: Date.now() - lastTypedRef.current < WRITING_MS ? "writing" : "reading",
          heartbeat: Date.now(),
        };
        bc?.postMessage({ kind: "awareness", presence });
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify(awarenessMsg(docId, presence)));
        }
      };
      beat();
      heartbeat = setInterval(beat, HEARTBEAT_MS);
    })();

    return () => {
      disposed = true;
      if (heartbeat) clearInterval(heartbeat);
      unsub?.();
      bc?.close();
      ws?.close();
      docRef.current = null;
      textRef.current = null;
    };
  }, [principal, displayName, docId]);

  const now = Date.now();
  const livePeers = Object.values(peers).filter((p) => now - p.heartbeat < STALE_MS);
  return { text, status, peers: livePeers, applyText };
}
