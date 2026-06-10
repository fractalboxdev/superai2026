// Effect Schema mirror of the Contextful wire protocol
// (packages/protocol/src/sync.ts). Inbound relay / BroadcastChannel frames are
// decoded through `Schema.parseJson` so a malformed or unexpected message is
// rejected at the boundary instead of crashing the room — the Effect-TS layer
// the app prefers over a hand-rolled `JSON.parse`.
import { Schema } from "effect";

const LoroBytes = Schema.Array(Schema.Number);

const PresenceState = Schema.Struct({
  principal: Schema.String,
  display_name: Schema.String,
  mode: Schema.Literal("reading", "writing", "idle"),
  cursor_anchor: Schema.optional(Schema.Number),
  selection_end: Schema.optional(Schema.Number),
  heartbeat: Schema.Number,
});

export const SyncMessage = Schema.Union(
  Schema.Struct({
    type: Schema.Literal("HELLO"),
    proto: Schema.String,
    principal: Schema.String,
    biscuit: Schema.optional(Schema.String),
  }),
  Schema.Struct({
    type: Schema.Literal("HELLO_OK"),
    doc_id: Schema.String,
    server_vv: Schema.optional(LoroBytes),
  }),
  Schema.Struct({
    type: Schema.Literal("SUBSCRIBE"),
    doc_id: Schema.String,
    client_vv: Schema.optional(LoroBytes),
  }),
  Schema.Struct({
    type: Schema.Literal("SNAPSHOT"),
    doc_id: Schema.String,
    bytes: LoroBytes,
  }),
  Schema.Struct({
    type: Schema.Literal("UPDATE"),
    doc_id: Schema.String,
    bytes: LoroBytes,
  }),
  Schema.Struct({
    type: Schema.Literal("AWARENESS"),
    doc_id: Schema.String,
    presence: PresenceState,
  }),
  Schema.Struct({
    type: Schema.Literal("ERROR"),
    code: Schema.String,
    message: Schema.String,
  }),
);

export type WireMessage = Schema.Schema.Type<typeof SyncMessage>;

const decodeJson = Schema.decodeUnknownEither(Schema.parseJson(SyncMessage));

/** Runtime-validated decode of a wire frame; `null` if malformed or unknown. */
export function decodeWire(data: string): WireMessage | null {
  const result = decodeJson(data);
  return result._tag === "Right" ? result.right : null;
}

const decodePresenceUnknown = Schema.decodeUnknownEither(PresenceState);

/**
 * Runtime-validated decode of a bare presence payload (BroadcastChannel
 * frames carry presence outside a wire envelope); `null` if malformed.
 */
export function decodePresence(
  data: unknown,
): Schema.Schema.Type<typeof PresenceState> | null {
  const result = decodePresenceUnknown(data);
  return result._tag === "Right" ? result.right : null;
}
