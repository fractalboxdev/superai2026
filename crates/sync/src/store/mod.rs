//! On-host store (spec 02 §6): the SQLite brain index ([`index_db`]) +
//! per-doc Loro snapshots + Markdown cards, all under `~/.contextful/`.

pub mod docs;
pub mod index_db;

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

    /// Load the index from SQLite (empty if the db doesn't exist yet). A
    /// legacy `brain.index.json` is migrated into the db on first load.
    pub fn load_index(&self) -> Result<BrainIndex> {
        let db_path = self.config.db_path();
        let json_path = self.config.index_path();
        if !db_path.exists() && json_path.exists() {
            let text = std::fs::read_to_string(&json_path)?;
            let legacy: BrainIndex = serde_json::from_str(&text)
                .with_context(|| format!("parsing legacy index at {}", json_path.display()))?;
            self.save_index(&legacy)?;
            std::fs::rename(&json_path, json_path.with_extension("json.migrated"))?;
            tracing::info!("migrated brain.index.json into brain.db");
            return Ok(legacy);
        }
        let conn = index_db::open(&db_path)?;
        index_db::load(&conn)
    }

    pub fn save_index(&self, index: &BrainIndex) -> Result<()> {
        self.config.ensure_dirs()?;
        let mut conn = index_db::open(&self.config.db_path())?;
        index_db::save(&mut conn, index)
    }

    /// Ranked FTS5 full-text search over card bodies → memory ids.
    pub fn search_cards(&self, query: &str) -> Result<Vec<String>> {
        let conn = index_db::open(&self.config.db_path())?;
        index_db::search_cards(&conn, query)
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
