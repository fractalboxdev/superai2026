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
    /// Deterministic, no LLM — keeps everything working offline (Flow D's
    /// no-credential floor; structured query + redaction need no LLM).
    #[default]
    Stub,
    /// Vercel AI Gateway → Claude (default cloud) — `AI_GATEWAY_API_KEY`.
    Gateway,
    /// AWS Bedrock Converse (standard AWS credential chain).
    Bedrock,
    /// LM Studio OpenAI-compatible endpoint on the host — on-prem/offline.
    LmStudio,
}

impl InferenceBackend {
    /// Explicit `CONTEXTFUL_INFERENCE` wins; otherwise auto-detect from
    /// available credentials: Gateway → Bedrock → LM Studio → Stub.
    fn detect() -> Self {
        match std::env::var("CONTEXTFUL_INFERENCE").as_deref() {
            Ok("gateway") => return InferenceBackend::Gateway,
            Ok("bedrock") => return InferenceBackend::Bedrock,
            Ok("lmstudio") => return InferenceBackend::LmStudio,
            Ok("stub") => return InferenceBackend::Stub,
            _ => {}
        }
        let has = |k: &str| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false);
        if has("AI_GATEWAY_API_KEY") {
            InferenceBackend::Gateway
        } else if has("AWS_ACCESS_KEY_ID") || has("AWS_PROFILE") {
            InferenceBackend::Bedrock
        } else if has("LM_STUDIO_URL") {
            InferenceBackend::LmStudio
        } else {
            InferenceBackend::Stub
        }
    }
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
        let inference = InferenceBackend::detect();
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
    /// The SQLite brain index (tables + FTS5; spec 02 §6).
    pub fn db_path(&self) -> PathBuf {
        self.root.join("brain.db")
    }
    /// Legacy JSON index location — migrated into [`Self::db_path`] on load.
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
