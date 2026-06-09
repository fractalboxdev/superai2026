//! Deployment config & on-host storage layout (spec 02 §6, spec 07).
//!
//! Everything lives under `~/.contextful/` (override with `CONTEXTFUL_HOME`).
//! Inference backend selection is config-driven (spec 02 §7, spec 04 §3); the
//! default `Stub` backend keeps the scaffold working with no cloud egress.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InferenceBackend {
    /// Deterministic, no LLM — keeps everything working offline.
    #[default]
    Stub,
    /// AWS Bedrock + Claude (default cloud) — needs `bedrock` feature + creds.
    Bedrock,
    /// LM Studio OpenAI-compatible endpoint on the host — on-prem/offline.
    LmStudio,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub inference: InferenceBackend,
}

impl Config {
    /// Resolve config from the environment. `CONTEXTFUL_HOME` overrides the
    /// root; `CONTEXTFUL_INFERENCE` selects the backend (`bedrock`/`lmstudio`).
    pub fn load() -> Self {
        let root = std::env::var_os("CONTEXTFUL_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                home.join(".contextful")
            });
        let inference = match std::env::var("CONTEXTFUL_INFERENCE").as_deref() {
            Ok("bedrock") => InferenceBackend::Bedrock,
            Ok("lmstudio") => InferenceBackend::LmStudio,
            _ => InferenceBackend::Stub,
        };
        Self { root, inference }
    }

    pub fn brain_dir(&self) -> PathBuf {
        self.root.join("brain")
    }
    pub fn docs_dir(&self) -> PathBuf {
        self.root.join("docs")
    }
    pub fn caps_dir(&self) -> PathBuf {
        self.root.join("caps")
    }
    pub fn control_dir(&self) -> PathBuf {
        self.root.join("control")
    }
    pub fn fixtures_dir(&self) -> PathBuf {
        self.root.join("fixtures")
    }
    /// File-based index over the Markdown brain (stand-in for DuckDB/SQLite +
    /// sqlite-vec; spec 02 §6 lists those as the production columnar/FTS layer).
    pub fn index_path(&self) -> PathBuf {
        self.root.join("brain.index.json")
    }

    /// Create the on-host directory tree if missing.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for d in [
            self.brain_dir(),
            self.docs_dir(),
            self.caps_dir(),
            self.control_dir(),
            self.fixtures_dir(),
        ] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }
}
