import { useEffect, useRef } from "react";
import type { LoroRoom } from "@/lib/loroRoom";

// A plaintext editing surface bound to the room's Loro `body` text container.
// Local input diffs into the CRDT (room.applyText); remote merges flow back via
// room.text, applied here while preserving the caret. A textarea keeps caret
// handling honest under concurrent edits; the rich-text tree is spec 01 §2.
export function DocEditor({ room }: { room: LoroRoom }) {
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el || el.value === room.text) return;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    el.value = room.text;
    const len = room.text.length;
    el.setSelectionRange(Math.min(start, len), Math.min(end, len));
  }, [room.text]);

  return (
    <textarea
      ref={ref}
      defaultValue={room.text}
      onInput={(e) => room.applyText(e.currentTarget.value)}
      spellCheck={false}
      aria-label="Document body — live CRDT"
      style={{
        width: "100%",
        minHeight: "13rem",
        resize: "vertical",
        border: "none",
        outline: "none",
        background: "transparent",
        font: "inherit",
        lineHeight: 1.7,
        color: "inherit",
        padding: 0,
      }}
    />
  );
}
