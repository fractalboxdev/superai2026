//! Presence / awareness (spec 01 §5).
//!
//! Rides Loro's `EphemeralStore` in the client; here it is the typed payload the
//! relay broadcasts. **Invariant:** awareness carries presence + cursors only —
//! never brain query results or any brain-derived content.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresenceMode {
    Reading,
    Writing,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    pub principal: String,
    pub display_name: String,
    pub mode: PresenceMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_anchor: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_end: Option<u64>,
    /// heartbeat timestamp (ms since epoch).
    pub heartbeat: u64,
}
