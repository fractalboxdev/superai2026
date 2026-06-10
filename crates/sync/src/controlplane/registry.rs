//! Principal registry & root-key catalog (spec 07 §3).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::access::RootKey;
use crate::config::Config;
use crate::identity::Principal;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub principals: Vec<Principal>,
    /// resource-root key catalog (public metadata; private key material would
    /// live in the owner's keystore, not here).
    #[serde(default)]
    pub roots: Vec<RootKey>,
}

impl Registry {
    fn path(config: &Config) -> std::path::PathBuf {
        config.control_dir().join("registry.json")
    }

    pub fn load(config: &Config) -> Result<Registry> {
        match std::fs::read_to_string(Self::path(config)) {
            Ok(text) => serde_json::from_str(&text).context("parsing registry"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(e) => Err(e).context("reading registry"),
        }
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        config.ensure_dirs()?;
        std::fs::write(Self::path(config), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn register_principal(&mut self, p: Principal) {
        if let Some(existing) = self.principals.iter_mut().find(|x| x.id() == p.id()) {
            *existing = p;
        } else {
            self.principals.push(p);
        }
    }

    pub fn register_root(&mut self, r: RootKey) {
        if let Some(existing) = self.roots.iter_mut().find(|x| x.id == r.id) {
            *existing = r;
        } else {
            self.roots.push(r);
        }
    }
}
