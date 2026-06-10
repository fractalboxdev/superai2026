// The Weaver rich-text editing surface (spec 01 §2): a contenteditable host
// driven imperatively by @weaver/dom from the editor's LoroDoc — blocks,
// marks, history, selection. Remote CRDT imports rerender via the bridge's
// own doc subscription; sync is the transport plugin's job (lib/weaverTransport).
//
// Presence cursors (spec 01 §5, upstream weaver PR #35): the local caret is
// mirrored out through `onCursorChange` (→ the transport's presence record),
// and every remote peer that carries a cursor renders as a colored caret +
// name flag via @weaver/dom's presence overlay. Roster and carets draw from
// the same identity set (lib/presence).
//
// Full Weaver chrome (upstream playground parity):
//   • @-mentions — `useMentions` wires the bridge's trigger detection into
//     the floating `MentionMenu` typeahead; picking a row writes a `mention`
//     mark via the editor command (CRDT-persisted, synced like any edit).
//   • Facepile — Google-Docs-style avatar stack. Weaver's `PresenceFacepile`
//     reads a `PresenceHub`, while this app's presence rides the Contextful
//     wire as `PresenceState[]`; a local render-only hub bridges the two
//     (records mirrored in, nothing published back out).
//
// Client-only: statically imports @weaver/react (→ loro-crdt WASM), so the
// route loads it via React.lazy once `useWeaverRoom` hands back an editor.

import { useEffect, useMemo, useRef, useState } from "react";
import type { Editor, PresenceRecord, Principal } from "@weaver/core";
import { createPresenceHub, getBlock, getChildren, rootId } from "@weaver/core";
import {
  EditorRoot,
  MentionMenu,
  PresenceFacepile,
  useMentions,
  useSelection,
} from "@weaver/react";
import {
  attachPresenceOverlay,
  type PresenceCursor,
  type PresenceOverlay,
} from "@weaver/dom";
import type { PresenceState } from "@superai2026/protocol/sync";
import { avatarOf, peerColor, peerKey } from "@/lib/presence";

export interface WeaverSurfaceProps {
  editor: Editor;
  /** Live remote peers (excluding self) — carets render for any with a cursor. */
  peers?: PresenceState[];
  /** Fires when the local caret moves; feed it to the room transport. */
  onCursorChange?: (cursor: { blockId: string; offset: number } | null) => void;
  /** The acting principal — rendered as a face in the roster alongside peers. */
  self?: { id: string; name: string };
  /**
   * Render a labeled caret for the acting principal at this position. Used by
   * scripted flows (the editor-agent demo) whose programmatic inserts never
   * move the native DOM caret — without it the typing has no visible author.
   */
  selfCursor?: { blockId: string; offset: number } | null;
  /** Directory of mentionable people/agents for the @-typeahead. */
  mentionPrincipals?: ReadonlyArray<Principal>;
}

// Wire presence carries no `kind`, so the `agent:` id-scheme prefix (e.g.
// `agent:cto/1`) is the contract for guests outside the scenario registry.
const kindOf = (principalId: string): "user" | "agent" =>
  principalId.startsWith("agent:") ? "agent" : "user";

const toCursors = (peers: PresenceState[]): PresenceCursor[] =>
  peers
    .filter((p) => p.cursor_block !== undefined && p.cursor_anchor !== undefined)
    .map((p) => ({
      peerId: peerKey(p),
      label: p.display_name,
      color: peerColor(p.principal),
      blockId: p.cursor_block!,
      offset: p.cursor_anchor!,
    }));

const toRecord = (p: PresenceState): PresenceRecord => ({
  peerId: peerKey(p),
  principalId: p.principal,
  label: p.display_name,
  color: peerColor(p.principal),
  avatarUrl: avatarOf(p.principal),
  kind: kindOf(p.principal),
  mode: p.mode === "writing" ? "generating" : "idle",
  cursor:
    p.cursor_block !== undefined && p.cursor_anchor !== undefined
      ? { blockId: p.cursor_block, offset: p.cursor_anchor }
      : null,
});

const NO_PRINCIPALS: ReadonlyArray<Principal> = [];

// An unanswered mention-ask means its tagged agent is working on a reply —
// surface that as a "thinking" caret at the end of the ask block (and a
// `generating` face in the facepile). Mirrors the Rust watcher's ask rules
// (crates/sync agent/editor.rs): `@<agent name> <question>` is answered once
// the next non-empty block starts with `A:` / `A (`.
const thinkingCursorsOf = (
  editor: Editor,
  agents: ReadonlyArray<Principal>,
): PresenceCursor[] => {
  if (agents.length === 0) return [];
  const byLongest = [...agents].sort((a, b) => b.label.length - a.label.length);
  const blocks = getChildren(editor, rootId(editor)).filter(
    (id) => getBlock(editor, id)?.hasInline,
  );
  const texts = blocks.map((id) => editor.commands.text.read(id));
  const out: PresenceCursor[] = [];
  texts.forEach((text, i) => {
    const rest = text.trimStart();
    if (!rest.startsWith("@")) return;
    const body = rest.slice(1);
    const target = byLongest.find((a) =>
      body.toLowerCase().startsWith(a.label.toLowerCase()),
    );
    if (!target) return;
    const question = body
      .slice(target.label.length)
      .replace(/^[\s,:—–-]+/, "")
      .trim();
    if (!question) return;
    const next = texts
      .slice(i + 1)
      .map((t) => t.trimStart())
      .find((t) => t.length > 0);
    if (next && (next.startsWith("A:") || next.startsWith("A ("))) return;
    out.push({
      peerId: `thinking:${target.id}`,
      label: `${target.label} is thinking`,
      color: target.color ?? "var(--color-accent)",
      blockId: blocks[i]!,
      offset: text.length,
    });
  });
  return out;
};

const thinkingKey = (cursors: PresenceCursor[]): string =>
  cursors.map((c) => `${c.peerId}@${c.blockId}:${c.offset}`).join("|");

export default function WeaverSurface({
  editor,
  peers = [],
  onCursorChange,
  self,
  selfCursor = null,
  mentionPrincipals = NO_PRINCIPALS,
}: WeaverSurfaceProps) {
  // @-mention wiring: the bridge reports `@query` triggers through
  // `bridgeOptions`; `mentions.hostRef` doubles as the host element ref for
  // the presence overlay below (one contenteditable, one ref).
  const mentions = useMentions(editor, { principals: mentionPrincipals });
  const hostRef = mentions.hostRef;

  // Local caret → presence record. `useSelection` is live because the DOM
  // bridge mirrors selectionchange into core selection (upstream PR #35).
  const selection = useSelection(editor);
  const cursor = useMemo(
    () =>
      selection === null
        ? null
        : { blockId: selection.focus.blockId, offset: selection.focus.offset },
    [selection],
  );
  useEffect(() => {
    onCursorChange?.(cursor);
  }, [onCursorChange, cursor]);

  // Agents currently "thinking": one synthetic caret per unanswered mention
  // ask, recomputed on every doc change (below) so the indicator appears the
  // moment an agent is tagged and drops when its `A (…)` reply lands.
  const agentPrincipals = useMemo(
    () => mentionPrincipals.filter((p) => p.kind === "agent"),
    [mentionPrincipals],
  );
  const thinkingRef = useRef<PresenceCursor[]>([]);
  const [thinking, setThinking] = useState<PresenceCursor[]>([]);

  // Render-only hub for the facepile: wire-borne peers (+ self) are mirrored
  // into a local PresenceHub each change; stale sessions are evicted so a
  // departed peer's face drops with its wire record.
  const hub = useMemo(() => createPresenceHub(), []);
  useEffect(() => () => hub.dispose(), [hub]);
  useEffect(() => {
    const want = new Map<string, PresenceRecord>();
    if (self) {
      want.set(`${self.id}#local`, {
        peerId: `${self.id}#local`,
        principalId: self.id,
        label: self.name,
        color: peerColor(self.id),
        avatarUrl: avatarOf(self.id),
        kind: kindOf(self.id),
        mode: "idle",
        cursor: null,
      });
    }
    // Skip wire peers sharing the acting principal (e.g. a second tab): the
    // self record above is authoritative for the local principal, so the
    // facepile's last-wins principalId dedup can't let a peer record win our
    // own face.
    for (const p of peers) {
      if (p.principal === self?.id) continue;
      want.set(peerKey(p), toRecord(p));
    }
    // A thinking agent shows in the facepile as `generating`, even when no
    // wire peer carries its presence (the watcher answers over the relay).
    for (const t of thinking) {
      const id = t.peerId.slice("thinking:".length);
      if (id === self?.id) continue;
      const agent = agentPrincipals.find((a) => a.id === id);
      want.set(t.peerId, {
        peerId: t.peerId,
        principalId: id,
        label: agent?.label ?? id,
        color: t.color,
        avatarUrl: avatarOf(id),
        kind: "agent",
        mode: "generating",
        cursor: null,
      });
    }
    for (const rec of hub.all()) {
      if (!want.has(rec.peerId)) hub.remove(rec.peerId);
    }
    for (const rec of want.values()) hub.set(rec);
  }, [hub, peers, self, thinking, agentPrincipals]);

  // Remote cursors (+ the scripted self caret) → caret overlay. The overlay
  // attaches once per editor mount; redraws ride peer/selfCursor updates
  // (props) and doc changes (carets must follow block layout as content
  // moves).
  const overlayRef = useRef<PresenceOverlay | null>(null);
  const cursorsRef = useRef<PresenceCursor[]>([]);
  cursorsRef.current = [
    ...toCursors(peers),
    ...(self && selfCursor
      ? [
          {
            peerId: `${self.id}#local`,
            label: self.name,
            color: peerColor(self.id),
            blockId: selfCursor.blockId,
            offset: selfCursor.offset,
          },
        ]
      : []),
  ];

  useEffect(() => {
    // EditorRoot (child) attaches the bridge before this parent effect runs,
    // so the contenteditable host exists by now.
    const host = hostRef.current;
    if (!host) return;
    const overlay = attachPresenceOverlay(host);
    overlayRef.current = overlay;
    // Tag blocks carrying a "⛔ Denied" agent reply so CSS can paint them
    // pink. Deferred a frame: the DOM bridge subscribes to the same doc, and
    // this must read the block text AFTER it re-renders.
    const markDenied = () =>
      requestAnimationFrame(() => {
        for (const el of host.querySelectorAll<HTMLElement>("[data-block-id]")) {
          el.classList.toggle(
            "weaver-block--denied",
            (el.textContent ?? "").includes("⛔ Denied"),
          );
        }
      });
    const refreshThinking = () => {
      const next = thinkingCursorsOf(editor, agentPrincipals);
      if (thinkingKey(next) !== thinkingKey(thinkingRef.current)) {
        thinkingRef.current = next;
        setThinking(next);
      }
    };
    const draw = () =>
      overlay.render([...cursorsRef.current, ...thinkingRef.current]);
    refreshThinking();
    draw();
    markDenied();
    const unsubDoc = editor.doc.subscribe(() => {
      refreshThinking();
      draw();
      markDenied();
    });
    return () => {
      unsubDoc();
      overlayRef.current = null;
      overlay.dispose();
    };
  }, [editor, hostRef, agentPrincipals]);

  useEffect(() => {
    overlayRef.current?.render([...cursorsRef.current, ...thinkingRef.current]);
  }, [peers, self, selfCursor, thinking]);

  return (
    <div className="weaver-host">
      <div className="weaver-collab-bar" contentEditable={false}>
        <PresenceFacepile hub={hub} />
      </div>
      <EditorRoot
        editor={editor}
        className="weaver-surface"
        hostRef={hostRef}
        bridgeOptions={mentions.bridgeOptions}
      />
      <MentionMenu mentions={mentions} />
    </div>
  );
}
