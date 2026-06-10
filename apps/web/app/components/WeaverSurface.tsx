// The Weaver rich-text editing surface (spec 01 §2): a contenteditable host
// driven imperatively by @weaver/dom from the editor's LoroDoc — blocks,
// marks, history, selection. Remote CRDT imports rerender via the bridge's
// own doc subscription; sync is the transport plugin's job (lib/weaverTransport).
//
// Client-only: statically imports @weaver/react (→ loro-crdt WASM), so the
// route loads it via React.lazy once `useWeaverRoom` hands back an editor.

import type { Editor } from "@weaver/core";
import { EditorRoot } from "@weaver/react";

export default function WeaverSurface({ editor }: { editor: Editor }) {
  return <EditorRoot editor={editor} className="weaver-surface" />;
}
