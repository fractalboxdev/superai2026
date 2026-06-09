//! File store (spec 02 §6): the JSON brain index + per-doc Loro snapshots +
//! Markdown cards, all under `~/.contextful/`.

pub mod docs;

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::brain::BrainIndex;
use crate::config::Config;

pub struct Store {
    pub config: Config,
}

impl Store {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Load the index, returning an empty one if it doesn't exist yet.
    pub fn load_index(&self) -> Result<BrainIndex> {
        let path = self.config.index_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("parsing index at {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BrainIndex::default()),
            Err(e) => Err(e).context("reading brain index"),
        }
    }

    pub fn save_index(&self, index: &BrainIndex) -> Result<()> {
        self.config.ensure_dirs()?;
        let path = self.config.index_path();
        let text = serde_json::to_string_pretty(index)?;
        std::fs::write(&path, text).with_context(|| format!("writing index to {}", path.display()))
    }

    /// Absolute path for a Markdown card under `brain/<topic>/<slug>.md`.
    pub fn card_path(&self, topic: &str, slug: &str) -> PathBuf {
        self.config
            .brain_dir()
            .join(topic)
            .join(format!("{slug}.md"))
    }

    pub fn write_card(&self, topic: &str, slug: &str, body: &str) -> Result<PathBuf> {
        let path = self.card_path(topic, slug);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, body).with_context(|| format!("writing card {}", path.display()))?;
        Ok(path)
    }

    pub fn read_card(&self, path: &str) -> Result<String> {
        std::fs::read_to_string(path).with_context(|| format!("reading card {path}"))
    }
}
