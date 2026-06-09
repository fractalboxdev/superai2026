//! Local constrained-process sandbox — offline fallback (spec 04 §2).
//!
//! On-host, an agent runs as a constrained child process whose only socket is
//! the brain MCP, with resource limits. This is the path used for the
//! fully-offline proof (Flow D) and the one architected for `wasmtime`
//! isolation of untrusted code. Until OS-enforced isolation lands, the local
//! fallback must run under it before claiming the Vercel path's guarantee
//! (spec 02 §4, spec 04 §2).

use crate::sandbox::{Sandbox, SandboxHandle};

#[derive(Default)]
pub struct LocalSandbox;

impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str {
        "local"
    }
    fn ensure(&self, room: &str) -> anyhow::Result<SandboxHandle> {
        tracing::info!(
            room,
            "local constrained-process sandbox (offline; OS-isolation = future)"
        );
        Ok(SandboxHandle {
            kind: "local".into(),
            room: room.to_string(),
            max_lifetime_secs: u64::MAX, // bounded by room presence, not a hard cap
        })
    }
}
