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
// Client-only: statically imports @weaver/react (→ loro-crdt WASM), so the
// route loads it via React.lazy once `useWeaverRoom` hands back an editor.

import { useEffect, useMemo, useRef } from "react";
import type { Editor } from "@weaver/core";
import { EditorRoot, useSelection } from "@weaver/react";
import {
  attachPresenceOverlay,
  type PresenceCursor,
  type PresenceOverlay,
} from "@weaver/dom";
import type { PresenceState } from "@superai2026/protocol/sync";
import { peerColor, peerKey } from "@/lib/presence";

export interface WeaverSurfaceProps {
  editor: Editor;
  /** Live remote peers (excluding self) — carets render for any with a cursor. */
  peers?: PresenceState[];
  /** Fires when the local caret moves; feed it to the room transport. */
  onCursorChange?: (cursor: { blockId: string; offset: number } | null) => void;
}

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

export default function WeaverSurface({
  editor,
  peers = [],
  onCursorChange,
}: WeaverSurfaceProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);

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

  // Remote cursors → caret overlay. The overlay attaches once per editor
  // mount; redraws ride peer updates (prop) and doc changes (carets must
  // follow block layout as content moves).
  const overlayRef = useRef<PresenceOverlay | null>(null);
  const peersRef = useRef(peers);
  peersRef.current = peers;

  useEffect(() => {
    // EditorRoot (child) attaches the bridge before this parent effect runs,
    // so the contenteditable host exists by now.
    const host = hostRef.current;
    if (!host) return;
    const overlay = attachPresenceOverlay(host);
    overlayRef.current = overlay;
    const draw = () => overlay.render(toCursors(peersRef.current));
    draw();
    const unsubDoc = editor.doc.subscribe(() => draw());
    return () => {
      unsubDoc();
      overlayRef.current = null;
      overlay.dispose();
    };
  }, [editor]);

  useEffect(() => {
    overlayRef.current?.render(toCursors(peers));
  }, [peers]);

  return (
    <div className="weaver-host">
      <EditorRoot editor={editor} className="weaver-surface" hostRef={hostRef} />
    </div>
  );
}
