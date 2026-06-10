// Contextful · sync wire protocol (spec 01 §4) — TS mirror of
// `crates/sync/src/sync/protocol.rs` + `presence.rs`. Field names and the
// SCREAMING_SNAKE_CASE `type` tag match the Rust serde representation exactly,
// so a browser peer (Weaver transport plugin) can speak to `sync serve`.

export type RoomId = string;
export type PeerId = number;

export type PresenceMode = "reading" | "writing" | "idle";

export type PresenceState = {
  principal: string;
  display_name: string;
  mode: PresenceMode;
  /**
   * Session discriminator (one per tab/connection) so two sessions of the
   * same principal don't clobber each other — mirrors upstream Weaver's
   * `peerId = principal#session` convention (weaver PR #35).
   */
  session?: string;
  /**
   * Weaver block id (`data-block-id`) the caret sits in; `cursor_anchor` is
   * the character offset within that block. Enough for peers to draw a live
   * caret in their editor overlay.
   */
  cursor_block?: string;
  cursor_anchor?: number;
  selection_end?: number;
  /** heartbeat ms since epoch */
  heartbeat: number;
};

/** CRDT payloads are opaque Loro bytes (JSON array of u8 over the wire). */
export type LoroBytes = number[];

export type SyncMessage =
  | { type: "HELLO"; proto: string; principal: string; biscuit?: string }
  | { type: "HELLO_OK"; doc_id: RoomId; server_vv?: LoroBytes }
  | { type: "SUBSCRIBE"; doc_id: RoomId; client_vv?: LoroBytes }
  | { type: "SNAPSHOT"; doc_id: RoomId; bytes: LoroBytes }
  | { type: "UPDATE"; doc_id: RoomId; bytes: LoroBytes }
  | { type: "AWARENESS"; doc_id: RoomId; presence: PresenceState }
  | { type: "ERROR"; code: string; message: string };

export const PROTO = "contextful/1";

export const hello = (principal: string, biscuit?: string): SyncMessage => ({
  type: "HELLO",
  proto: PROTO,
  principal,
  biscuit,
});

export const subscribe = (doc_id: RoomId): SyncMessage => ({ type: "SUBSCRIBE", doc_id });

export const awareness = (
  doc_id: RoomId,
  presence: PresenceState,
): SyncMessage => ({ type: "AWARENESS", doc_id, presence });

/** Loro update bytes (a delta) for a doc. Send requires `write(document)`. */
export const update = (doc_id: RoomId, bytes: LoroBytes): SyncMessage => ({
  type: "UPDATE",
  doc_id,
  bytes,
});

/** Full Loro snapshot bytes for a doc (catch-up / first publish). */
export const snapshot = (doc_id: RoomId, bytes: LoroBytes): SyncMessage => ({
  type: "SNAPSHOT",
  doc_id,
  bytes,
});

export const parseMessage = (data: string): SyncMessage | null => {
  try {
    return JSON.parse(data) as SyncMessage;
  } catch {
    return null;
  }
};
