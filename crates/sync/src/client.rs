//! Sync client — placeholder implementation.

use anyhow::Result;

/// Connect to a sync server at `addr` and begin syncing.
///
/// TODO: implement client-side sync (handshake, change feed, local apply).
/// Internals to be specified later.
pub async fn run(addr: &str) -> Result<()> {
    tracing::info!(%addr, "sync client connecting (placeholder)");
    // Placeholder: the real client loop goes here.
    Ok(())
}
