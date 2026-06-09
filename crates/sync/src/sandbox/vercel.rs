//! Vercel Sandbox driver — default runtime, "agents from anywhere" (spec 04 §2).
//!
//! Members run agents from anywhere with any harness by providing an identity (a
//! Biscuit token); the agent runs in a Vercel Sandbox microVM and connects back
//! to the host's brain MCP over Tailscale. ~5h cap, recreated on room re-entry.
//!
//! Real orchestration uses the `@vercel/sandbox` SDK (TS) provisioned by the
//! agent runtime; this driver models the lifecycle. Live provisioning is gated
//! behind the `vercel-sandbox` feature + credentials.

use crate::sandbox::{Sandbox, SandboxHandle};

/// Vercel Sandbox lifetime cap (Pro/Enterprise ~5h).
const MAX_LIFETIME_SECS: u64 = 5 * 60 * 60;

#[derive(Default)]
pub struct VercelSandbox;

impl Sandbox for VercelSandbox {
    fn kind(&self) -> &str {
        "vercel"
    }
    fn ensure(&self, room: &str) -> anyhow::Result<SandboxHandle> {
        if !cfg!(feature = "vercel-sandbox") {
            tracing::info!(room, "Vercel Sandbox lifecycle modeled (live provisioning needs `vercel-sandbox` feature + creds)");
        }
        Ok(SandboxHandle {
            kind: "vercel".into(),
            room: room.to_string(),
            max_lifetime_secs: MAX_LIFETIME_SECS,
        })
    }
}
