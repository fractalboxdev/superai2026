//! Control plane (spec 07 §3) — identity-root only.
//!
//! Registers principals and document membership and seeds auto-mode envelopes;
//! it **cannot** mint authority over data resources (those are rooted at their
//! owners, spec 03 §1). Backed by config files under `~/.contextful/control/`
//! and issued tokens under `~/.contextful/caps/`.

pub mod envelope;
pub mod registry;

use std::path::PathBuf;

use anyhow::Result;

use crate::access::Capability;
use crate::config::Config;

/// Filesystem-safe key for a principal id (e.g. `agent:cto/1` → `agent_cto_1`).
pub fn principal_key(principal: &str) -> String {
    principal
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn cap_path(config: &Config, principal: &str) -> PathBuf {
    config
        .caps_dir()
        .join(format!("{}.json", principal_key(principal)))
}

/// Load a principal's issued capability token from `caps/`, if present.
pub fn load_capability(config: &Config, principal: &str) -> Option<Capability> {
    let path = cap_path(config, principal);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist a principal's capability token under `caps/` (audit/revocation).
pub fn save_capability(config: &Config, cap: &Capability) -> Result<()> {
    config.ensure_dirs()?;
    let path = cap_path(config, &cap.holder);
    std::fs::write(path, serde_json::to_string_pretty(cap)?)?;
    Ok(())
}

/// Record a revoked token holder in the revocation list (checked on every
/// session and tool call by the relay/MCP auth path, spec 07 §3).
pub fn revoke(config: &Config, principal: &str) -> Result<()> {
    config.ensure_dirs()?;
    let path = config.caps_dir().join("revoked.json");
    let mut list: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    if !list.iter().any(|p| p == principal) {
        list.push(principal.to_string());
    }
    std::fs::write(path, serde_json::to_string_pretty(&list)?)?;
    Ok(())
}

pub fn is_revoked(config: &Config, principal: &str) -> bool {
    let path = config.caps_dir().join("revoked.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
        .map(|list| list.iter().any(|p| p == principal))
        .unwrap_or(false)
}

/// Seed the demo control plane: principals, root catalog, envelopes, and each
/// principal's initial capability token (spec 07 §3).
pub fn seed(config: &Config) -> Result<()> {
    use crate::scenario;
    config.ensure_dirs()?;

    let mut registry = registry::Registry::default();
    for p in scenario::principals() {
        registry.register_principal(p);
    }
    registry.register_root(scenario::cfo_root());
    registry.register_root(scenario::control_plane_root());
    registry.save(config)?;

    let mut envelopes = envelope::EnvelopeStore::default();
    envelopes.upsert(scenario::cfo_envelope());
    envelopes.save(config)?;

    for p in scenario::principals() {
        if let Some(cap) = scenario::initial_capability(p.id()) {
            save_capability(config, &cap)?;
        }
    }

    println!(
        "seeded {} → principals, roots, envelopes, initial tokens",
        config.root.display()
    );
    Ok(())
}

/// Print the seeded control-plane state.
pub fn show(config: &Config) -> Result<()> {
    let registry = registry::Registry::load(config)?;
    let envelopes = envelope::EnvelopeStore::load(config)?;

    println!("root: {}", config.root.display());
    println!("\nprincipals:");
    for p in &registry.principals {
        println!("  - {} ({})", p.id(), p.name());
    }
    println!("\nresource roots (no super-root):");
    for r in &registry.roots {
        let views: Vec<String> = r.views.iter().map(|v| v.id()).collect();
        println!("  - {} owns [{}]", r.id, views.join(", "));
    }
    println!("\nenvelopes:");
    for e in &envelopes.envelopes {
        println!("  - {} never_delegate={:?}", e.owner, e.never_delegate);
    }
    let revoked_path = config.caps_dir().join("revoked.json");
    if let Ok(text) = std::fs::read_to_string(revoked_path) {
        println!("\nrevoked: {text}");
    }
    Ok(())
}
