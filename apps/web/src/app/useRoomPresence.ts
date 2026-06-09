"use client";

// Live room presence against the Rust relay (`sync serve`, spec 01 §5).
// Opt-in: only connects when NEXT_PUBLIC_SYNC_URL is set (e.g.
// ws://localhost:7878/), so the page never breaks on mixed-content when served
// over HTTPS without a relay. When connected, shows real cross-peer presence.

import { useEffect, useRef, useState } from "react";
import {
  awareness,
  hello,
  parseMessage,
  subscribe,
  type PresenceState,
} from "@superai2026/protocol/sync";

export type PresenceStatus = "disabled" | "connecting" | "live" | "offline";

const STALE_MS = 15_000;

export function useRoomPresence(principal: string, displayName: string, doc = "finops") {
  const [status, setStatus] = useState<PresenceStatus>("disabled");
  const [peers, setPeers] = useState<Record<string, PresenceState>>({});
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const url = process.env.NEXT_PUBLIC_SYNC_URL;
    if (!url) {
      setStatus("disabled");
      return;
    }

    setStatus("connecting");
    setPeers({});
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch {
      setStatus("offline");
      return;
    }
    wsRef.current = ws;
    let heartbeat: ReturnType<typeof setInterval> | undefined;

    ws.onopen = () => {
      setStatus("live");
      ws.send(JSON.stringify(hello(principal)));
      ws.send(JSON.stringify(subscribe(doc)));
      const beat = () => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(
            JSON.stringify(
              awareness(doc, {
                principal,
                display_name: displayName,
                mode: "reading",
                heartbeat: Date.now(),
              }),
            ),
          );
        }
      };
      beat();
      heartbeat = setInterval(beat, 5000);
    };

    ws.onmessage = (ev) => {
      const msg = parseMessage(typeof ev.data === "string" ? ev.data : "");
      if (msg?.type === "AWARENESS" && msg.presence.principal !== principal) {
        setPeers((prev) => ({ ...prev, [msg.presence.principal]: msg.presence }));
      }
    };
    ws.onclose = () => setStatus("offline");
    ws.onerror = () => setStatus("offline");

    return () => {
      if (heartbeat) clearInterval(heartbeat);
      ws.close();
    };
  }, [principal, displayName, doc]);

  const now = Date.now();
  const livePeers = Object.values(peers).filter((p) => now - p.heartbeat < STALE_MS);
  return { status, peers: livePeers };
}
