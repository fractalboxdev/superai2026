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
