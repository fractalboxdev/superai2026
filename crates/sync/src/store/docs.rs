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

    fn snapshot_path(&self, doc_id: &str) -> Result<PathBuf> {
        if !is_safe_doc_id(doc_id) {
            anyhow::bail!("unsafe doc_id '{doc_id}' (allowed: [A-Za-z0-9_-])");
        }
        Ok(self.config.docs_dir().join(format!("{doc_id}.loro")))
    }

    /// Load the persisted snapshot bytes for a document, if any.
    pub fn load_snapshot(&self, doc_id: &str) -> Result<Option<Vec<u8>>> {
        let path = self.snapshot_path(doc_id)?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading snapshot {}", path.display())),
        }
    }

    /// Persist the latest snapshot bytes for a document.
    pub fn save_snapshot(&self, doc_id: &str, bytes: &[u8]) -> Result<()> {
        let path = self.snapshot_path(doc_id)?;
        self.config.ensure_dirs()?;
        std::fs::write(&path, bytes).with_context(|| format!("writing snapshot {}", path.display()))
    }
}

/// A doc id is safe to use as a filename: non-empty, only `[A-Za-z0-9_-]`.
/// Rejects path separators, `..`, absolute paths — preventing traversal out of
/// `docs/` from a client-controlled `doc_id`.
pub fn is_safe_doc_id(doc_id: &str) -> bool {
    !doc_id.is_empty()
        && doc_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::is_safe_doc_id;

    #[test]
    fn rejects_path_traversal_and_separators() {
        assert!(is_safe_doc_id("finops"));
        assert!(is_safe_doc_id("doc_1-a"));
        assert!(!is_safe_doc_id(""));
        assert!(!is_safe_doc_id("../../etc/passwd"));
        assert!(!is_safe_doc_id("/abs/path"));
        assert!(!is_safe_doc_id("a/b"));
        assert!(!is_safe_doc_id("a.loro"));
        assert!(!is_safe_doc_id("a\\b"));
    }
}
