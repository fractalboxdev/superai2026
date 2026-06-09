//! Per-doc Loro snapshot + oplog persistence (spec 01 §4, spec 02 §6).
//!
//! The authoritative relay treats CRDT payloads as **opaque Loro bytes** — it
//! broadcasts updates and persists the latest snapshot without parsing them, so
//! the server needs no Loro dependency. Real export/import + compaction live in
//! the Weaver client and the (future) native file-sync peer.

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::Config;

pub struct DocStore {
    config: Config,
}

impl DocStore {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    fn snapshot_path(&self, doc_id: &str) -> PathBuf {
        self.config.docs_dir().join(format!("{doc_id}.loro"))
    }

    /// Load the persisted snapshot bytes for a document, if any.
    pub fn load_snapshot(&self, doc_id: &str) -> Result<Option<Vec<u8>>> {
        let path = self.snapshot_path(doc_id);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading snapshot {}", path.display())),
        }
    }

    /// Persist the latest snapshot bytes for a document.
    pub fn save_snapshot(&self, doc_id: &str, bytes: &[u8]) -> Result<()> {
        self.config.ensure_dirs()?;
        let path = self.snapshot_path(doc_id);
        std::fs::write(&path, bytes).with_context(|| format!("writing snapshot {}", path.display()))
    }
}
