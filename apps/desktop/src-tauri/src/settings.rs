//! App-shell settings (role, principal, addresses). The shell owns *only*
//! this file — brain data stays under `~/.contextful`, owned by the binary
//! (spec 10 §2 "no new trust surface").

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Host,
    Member,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub configured: bool,
    pub role: Role,
    pub principal: String,
    /// Host: bind address for `serve`. Member: the host relay's address.
    pub relay_addr: String,
    /// Member: room/document id to sync.
    pub doc: String,
    /// Host: brain home override (`CONTEXTFUL_HOME`); `None` = `~/.contextful`.
    pub brain_home: Option<String>,
    /// `stub` (offline) · `bedrock` (cloud) · `lmstudio` (on-prem).
    pub inference: String,
    pub autostart: bool,
    pub web_app_url: String,
    pub update_channel: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            configured: false,
            role: Role::Host,
            principal: "cfo".into(),
            relay_addr: "0.0.0.0:7878".into(),
            doc: "finops".into(),
            brain_home: None,
            inference: "stub".into(),
            autostart: false,
            web_app_url: "https://demo.contextful.work".into(),
            update_channel: "stable".into(),
        }
    }
}

impl AppSettings {
    /// `~/Library/Application Support/work.contextful.app/config.json`
    /// (`CONTEXTFUL_DESKTOP_HOME` overrides the directory — used by tests).
    pub fn path() -> PathBuf {
        let dir = std::env::var_os("CONTEXTFUL_DESKTOP_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                home.join("Library/Application Support/work.contextful.app")
            });
        dir.join("config.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Merge a partial JSON patch (camelCase keys, as sent by the UI).
    /// `null` clears an optional field.
    pub fn merged(&self, patch: &serde_json::Value) -> anyhow::Result<Self> {
        let mut value = serde_json::to_value(self)?;
        if let (Some(base), Some(overlay)) = (value.as_object_mut(), patch.as_object()) {
            for (k, v) in overlay {
                base.insert(k.clone(), v.clone());
            }
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Expanded brain home (leading `~` resolved against `$HOME`).
    pub fn brain_home_expanded(&self) -> Option<String> {
        self.brain_home.as_ref().map(|raw| {
            if let Some(rest) = raw.strip_prefix("~/") {
                if let Some(home) = std::env::var_os("HOME") {
                    return PathBuf::from(home)
                        .join(rest)
                        .to_string_lossy()
                        .into_owned();
                }
            }
            raw.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patch_updates_and_clears_fields() {
        let s = AppSettings::default();
        let patched = s
            .merged(&serde_json::json!({
                "role": "member",
                "relayAddr": "studio.ts.net:7878",
                "brainHome": null,
            }))
            .unwrap();
        assert_eq!(patched.role, Role::Member);
        assert_eq!(patched.relay_addr, "studio.ts.net:7878");
        assert_eq!(patched.brain_home, None);
        // untouched fields survive
        assert_eq!(patched.doc, "finops");
    }

    #[test]
    fn roundtrips_camel_case() {
        let s = AppSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("relayAddr"));
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
