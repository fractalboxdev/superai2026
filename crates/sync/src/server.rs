//! Sync server — placeholder implementation.

use anyhow::Result;

/// Start the sync server bound to `addr`.
///
/// TODO: implement the local-first sync protocol — transport, oplog/CRDT
/// reconciliation, and persistence. Internals to be specified later.
pub async fn run(addr: &str) -> Result<()> {
    tracing::info!(%addr, "sync server starting (placeholder)");
    // Placeholder: the real server loop goes here.
    Ok(())
}
