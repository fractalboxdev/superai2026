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
import type { RoomStatus, RoomTransport } from "./weaverTransport";

export type { RoomStatus };

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
): WeaverRoom {
  const [editor, setEditor] = useState<Editor | null>(null);
  const [status, setStatus] = useState<RoomStatus>("local");
  const [peers, setPeers] = useState<PresenceState[]>([]);
  const transportRef = useRef<RoomTransport | null>(null);
  const setCursor = useCallback(
    (cursor: { blockId: string; offset: number } | null) => {
      transportRef.current?.setCursor(cursor);
    },
    [],
  );

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    const relayUrl = import.meta.env.VITE_SYNC_URL as string | undefined;
    setEditor(null);
    setPeers([]);
    setStatus(relayUrl ? "connecting" : "local");

    void (async () => {
      const [{ createEditor }, { attachRoomTransport }] = await Promise.all([
        import("@weaver/core"),
        import("./weaverTransport"),
      ]);
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
