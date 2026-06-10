//! Per-document sandbox (spec 04 §1–2).
//!
//! Pluggable behind the [`Sandbox`] trait. Invariants hold regardless of where
//! the sandbox runs: no ambient authority (the only egress is the brain MCP),
//! ephemeral (no durable private state), and created/torn down with room
//! presence. Vercel Sandbox is the default ("agents from anywhere"); the local
//! constrained process is the offline fallback.

pub mod local;
pub mod vercel;

/// The agent's only data egress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Egress {
    /// All data access flows through the brain MCP, capability-checked.
    BrainMcpOnly,
}

#[derive(Debug, Clone)]
pub struct SandboxHandle {
    pub kind: String,
    pub room: String,
    /// the provider's sandbox id (None for the modeled/offline lifecycle).
    pub sandbox_id: Option<String>,
    /// provider dashboard URL for execution logs (None offline / unknown).
    pub logs_url: Option<String>,
    /// soft cap; the runtime recreates the sandbox on room re-entry.
    pub max_lifetime_secs: u64,
}

pub trait Sandbox {
    fn kind(&self) -> &str;
    fn egress(&self) -> Egress {
        Egress::BrainMcpOnly
    }
    /// Provision (or reuse) the sandbox for a room. Created on room entry,
    /// reused/expired with presence (spec 04 §2).
    fn ensure(&self, room: &str) -> anyhow::Result<SandboxHandle>;
}

/// Select the runtime: Vercel Sandbox by default, local constrained process
/// when offline (Flow D).
pub fn select(offline: bool) -> Box<dyn Sandbox> {
    if offline {
        Box::new(local::LocalSandbox)
    } else {
        Box::new(vercel::VercelSandbox)
    }
}

/// Debug status for one room, as served by `GET /debug/sandbox/:room` on the
/// co-hosted MCP HTTP listener. Read-only: ids, the provider's execution-logs
/// URL, and age — never document content or capability material.
pub fn debug_status(room: &str) -> serde_json::Value {
    match vercel::peek(room) {
        Some((handle, age_secs)) => serde_json::json!({
            "room": room,
            "provisioned": true,
            "kind": handle.kind,
            "sandboxId": handle.sandbox_id,
            "logsUrl": handle.logs_url,
            "ageSecs": age_secs,
            "maxLifetimeSecs": handle.max_lifetime_secs,
        }),
        None => serde_json::json!({ "room": room, "provisioned": false }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_status_unprovisioned_room() {
        let v = debug_status("room-with-no-sandbox");
        assert_eq!(v["provisioned"], false);
        assert_eq!(v["room"], "room-with-no-sandbox");
        assert!(v.get("logsUrl").is_none());
    }
}
