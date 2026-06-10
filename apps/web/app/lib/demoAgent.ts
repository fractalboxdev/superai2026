// useDemoAgent — an in-app simulated agent peer so the live site demos
// real-time sync on its own, without a relay or a second visitor.
//
// It attaches a SECOND editor + room transport in the same tab as Gilfoyle's
// agent (`agent:gilfoyle/1` — a guest outside the scenario cast, so it never
// collides with an identity the visitor can act as): two BroadcastChannel
// instances in one page deliver to each other, so the bot's CRDT commits and
// presence (labeled caret, roster dot, "writing" mode) reach the visitor's
// editor exactly like a real cross-tab peer. The bot owns one clearly-marked
// paragraph at the end of the doc and rewrites it in a slow loop — bounded
// content, char-by-char typing, caret riding along.
//
// Scope guards:
//   • Off when VITE_SYNC_URL is set — on the relay path a real headless peer
//     (`sync client`) should be the one editing, not every visitor's browser.
//   • One bot per browser even with many tabs open: tabs queue on a Web Lock
//     and the next tab takes over if the leader closes.
//   • All imports are lazy (loro-crdt WASM) — nothing reaches the SSR bundle.

import { useEffect } from "react";
import { resolveSyncUrl } from "./weaverRoom";

const PRINCIPAL = "agent:gilfoyle/1";
const DISPLAY_NAME = "Gilfoyle's agent";
/** The bot finds (or creates) the paragraph that starts with this marker. */
const MARKER = "Gilfoyle's agent · ";

const LINES = [
  "rechecking this week's burn — net spend lands 12% under budget once credits apply.",
  "credits reconciled; flagging the discount tier for Monica to confirm.",
  "utilization is up across the platform team, tokens-per-PR trending down.",
  "renewal scan queued — two overlapping vendors are consolidation candidates.",
];

const sleep = (ms: number, signal: AbortSignal) =>
  new Promise<void>((res) => {
    if (signal.aborted) return res();
    const t = setTimeout(res, ms);
    signal.addEventListener("abort", () => {
      clearTimeout(t);
      res();
    }, { once: true });
  });

const jitter = (base: number, spread: number) => base + Math.random() * spread;

export function useDemoAgent(docId: string): void {
  useEffect(() => {
    // Off on any relay path (env or `?sync=` override) — there a real peer
    // (`sync client` / `sync agent --watch-doc`) does the editing.
    if (resolveSyncUrl()) return;
    const ctrl = new AbortController();
    let dispose: (() => void) | undefined;

    const run = async () => {
      const [core, { attachRoomTransport }] = await Promise.all([
        import("@weaver/core"),
        import("./weaverTransport"),
      ]);
      if (ctrl.signal.aborted) return;
      const { createEditor, rootId, getChildren, getBlock } = core;

      const ed = createEditor({ origin: PRINCIPAL, seed: false });
      const transport = attachRoomTransport(ed.doc, {
        docId,
        principal: PRINCIPAL,
        displayName: DISPLAY_NAME,
        onStatus: () => {},
        onPeers: () => {},
      });
      dispose = () => {
        transport.dispose();
        ed.dispose();
      };

      // Re-resolved every cycle — the visitor may edit or delete the block.
      const ensureBlock = () => {
        const root = rootId(ed);
        for (const id of getChildren(ed, root)) {
          if (getBlock(ed, id)?.kind !== "paragraph") continue;
          if (ed.commands.text.read(id).startsWith(MARKER)) return id;
        }
        const id = ed.commands.block.insert({
          parentId: root,
          index: getChildren(ed, root).length,
          kind: "paragraph",
        });
        ed.commands.text.insert({ blockId: id, offset: 0, value: MARKER });
        return id;
      };

      await sleep(jitter(3000, 2000), ctrl.signal);
      let i = Math.floor(Math.random() * LINES.length);
      while (!ctrl.signal.aborted) {
        const blockId = ensureBlock();
        const stale = ed.commands.text.length(blockId) - MARKER.length;
        if (stale > 0) {
          ed.commands.text.delete({ blockId, offset: MARKER.length, length: stale });
        }
        transport.setCursor({ blockId, offset: MARKER.length });
        for (const ch of LINES[i++ % LINES.length]) {
          if (ctrl.signal.aborted) return;
          const offset = ed.commands.text.length(blockId);
          ed.commands.text.insert({ blockId, offset, value: ch });
          transport.setCursor({ blockId, offset: offset + ch.length });
          await sleep(jitter(55, 70), ctrl.signal);
        }
        await sleep(jitter(8000, 6000), ctrl.signal);
      }
    };

    // Leader election across tabs: the lock callback's promise is held until
    // unmount, so exactly one tab animates the bot and the next queued tab
    // takes over when it closes. (No Web Locks → just run; worst case is one
    // bot session per tab, which presence renders as separate sessions.)
    if (typeof navigator !== "undefined" && navigator.locks) {
      navigator.locks
        .request(`contextful-demo-agent/${docId}`, { signal: ctrl.signal }, run)
        .catch(() => {});
    } else {
      void run();
    }

    return () => {
      ctrl.abort();
      dispose?.();
    };
  }, [docId]);
}
