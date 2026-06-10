// useWeaverRoom — binds one document to a live Weaver editor + Loro CRDT room
// (spec 01 §2). The editor's `LoroDoc` is the single source of truth; the
// Contextful transport plugin (./weaverTransport) syncs it across tabs via
// BroadcastChannel and — when VITE_SYNC_URL is set — to `sync serve` over the
// wire protocol (§4), with PresenceState awareness (§5) on both channels.
//
// @weaver/core is WASM-backed (loro-crdt), so both it and the transport are
// imported lazily inside the effect — they never touch the SSR/server bundle.
// `editor` is `null` during SSR and first client render; the route mounts the
// editing surface (React.lazy) only once it resolves.

import { useCallback, useEffect, useRef, useState } from "react";
import type { Editor } from "@weaver/core";
import type { PresenceState } from "@superai2026/protocol/sync";
import type { RoomNotice, RoomStatus, RoomTransport } from "./weaverTransport";

export type { RoomNotice, RoomStatus };

const SYNC_URL_KEY = "contextful:syncUrl";
const CHUNK_RELOAD_KEY = "contextful:chunkReload";

// A failed dynamic import almost always means a stale deploy: this tab's HTML
// references hashed chunks that no longer exist on the CDN, so the lazy editor
// import rejects and the page hangs at "Waking the editor…". Reload once to
// pick up the new asset manifest; the session flag prevents a reload loop.
export function recoverStaleChunk(err: unknown): never {
  try {
    if (!sessionStorage.getItem(CHUNK_RELOAD_KEY)) {
      sessionStorage.setItem(CHUNK_RELOAD_KEY, "1");
      window.location.reload();
    }
  } catch {
    /* storage unavailable — fall through to the rethrow */
  }
  throw err;
}

function clearChunkReloadFlag() {
  try {
    sessionStorage.removeItem(CHUNK_RELOAD_KEY);
  } catch {
    /* ignore */
  }
}

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

export type WeaverRoom = {
  /** The live Weaver editor (client-only; `null` until ready). */
  editor: Editor | null;
  status: RoomStatus;
  /** Live peers (presence within the staleness window), excluding self. */
  peers: PresenceState[];
  /**
   * Publish the local caret onto the presence record so remote peers render
   * it in their caret overlay (upstream weaver PR #35). No-op until the
   * transport attaches.
   */
  setCursor: (cursor: { blockId: string; offset: number } | null) => void;
};

export function useWeaverRoom(
  principal: string,
  displayName: string,
  docId: string,
  onNotice?: (notice: RoomNotice) => void,
): WeaverRoom {
  const [editor, setEditor] = useState<Editor | null>(null);
  const [status, setStatus] = useState<RoomStatus>("local");
  const [peers, setPeers] = useState<PresenceState[]>([]);
  const transportRef = useRef<RoomTransport | null>(null);
  // ref-stable so a re-rendered callback never tears down the transport
  const noticeRef = useRef(onNotice);
  useEffect(() => {
    noticeRef.current = onNotice;
  }, [onNotice]);
  const setCursor = useCallback(
    (cursor: { blockId: string; offset: number } | null) => {
      transportRef.current?.setCursor(cursor);
    },
    [],
  );

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    const relayUrl = resolveSyncUrl();
    setEditor(null);
    setPeers([]);
    setStatus(relayUrl ? "connecting" : "local");

    void (async () => {
      const [{ createEditor }, { attachRoomTransport }] = await Promise.all([
        import("@weaver/core"),
        import("./weaverTransport"),
      ]).catch(recoverStaleChunk);
      clearChunkReloadFlag();
      if (disposed) return;

      // `seed: false` — initial content comes from the transport's
      // deterministic seed import, not Weaver's blank-paragraph template
      // (which would duplicate across peers). `origin` tags every local
      // commit with the acting principal for provenance / per-peer undo.
      const ed = createEditor({ origin: principal, seed: false });
      const transport = attachRoomTransport(ed.doc, {
        docId,
        principal,
        displayName,
        relayUrl,
        onStatus: (s) => {
          if (!disposed) setStatus(s);
        },
        onPeers: (p) => {
          if (!disposed) setPeers(p);
        },
        onNotice: (n) => {
          if (!disposed) noticeRef.current?.(n);
        },
      });
      transportRef.current = transport;
      cleanup = () => {
        transportRef.current = null;
        transport.dispose();
        ed.dispose();
      };
      setEditor(ed);
    })();

    return () => {
      disposed = true;
      cleanup?.();
      setEditor(null);
    };
  }, [principal, displayName, docId]);

  return { editor, status, peers, setCursor };
}
