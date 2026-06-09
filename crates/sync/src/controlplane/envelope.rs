//! Auto-mode policy envelopes per owner (spec 03 §5, spec 07 §3).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::access::request::Envelope;
use crate::config::Config;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvelopeStore {
    #[serde(default)]
    pub envelopes: Vec<Envelope>,
}

impl EnvelopeStore {
    fn path(config: &Config) -> std::path::PathBuf {
        config.control_dir().join("envelopes.json")
    }

    pub fn load(config: &Config) -> Result<EnvelopeStore> {
        match std::fs::read_to_string(Self::path(config)) {
            Ok(text) => serde_json::from_str(&text).context("parsing envelopes"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EnvelopeStore::default()),
            Err(e) => Err(e).context("reading envelopes"),
        }
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        config.ensure_dirs()?;
        std::fs::write(Self::path(config), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn upsert(&mut self, env: Envelope) {
        if let Some(existing) = self.envelopes.iter_mut().find(|e| e.owner == env.owner) {
            *existing = env;
        } else {
            self.envelopes.push(env);
        }
    }

    pub fn for_owner(&self, owner: &str) -> Option<&Envelope> {
        self.envelopes.iter().find(|e| e.owner == owner)
    }
}
