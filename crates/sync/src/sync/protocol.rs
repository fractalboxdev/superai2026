//! Wire protocol between `serve` and peers (spec 01 §4).
//!
//! CRDT payloads are **opaque Loro bytes** — the relay never parses them. The
//! TS mirror lives in `packages/protocol/src/sync.ts`.

use serde::{Deserialize, Serialize};

use crate::sync::presence::PresenceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SyncMessage {
    /// C→S: peer announces itself and its token.
    Hello {
        proto: String,
        principal: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        biscuit: Option<String>,
    },
    /// S→C: handshake accepted.
    HelloOk {
        doc_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server_vv: Option<Vec<u8>>,
    },
    /// C→S: subscribe to a document (requires read(document)).
    Subscribe {
        doc_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_vv: Option<Vec<u8>>,
    },
    /// S→C: full snapshot for catch-up.
    Snapshot { doc_id: String, bytes: Vec<u8> },
    /// C↔S: a Loro update (send requires write(document)). On rebroadcast the
    /// relay stamps `from` with the authenticated sender principal — clients
    /// send it empty and never trust their own value.
    Update {
        doc_id: String,
        bytes: Vec<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// C↔S: ephemeral presence/awareness (read(document)).
    Awareness {
        doc_id: String,
        presence: PresenceState,
    },
    /// C→S→room: a structured access decision about an in-doc ask, addressed
    /// to one principal (`to`); peers ignore notifications not addressed to
    /// them. Carries decision metadata only — never brain content (the same
    /// invariant as presence). The relay stamps `from` with the sender.
    Notify {
        doc_id: String,
        to: String,
        #[serde(default)]
        from: String,
        /// deny reason wire string, e.g. `no_grant`.
        reason: String,
        message: String,
    },
    /// S→C: an error (e.g. authorization denied).
    Error { code: String, message: String },
}

impl SyncMessage {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("SyncMessage serializes")
    }
}
