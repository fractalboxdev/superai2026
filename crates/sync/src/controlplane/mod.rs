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

/// The resource owner whose authority `grant` delegates from (spec 03 §1).
const RESOURCE_OWNER: &str = "cfo";

/// Model a resource owner approving a scoped grant (Flow A, spec 09 §1). Loads
/// the owner's (CFO) capability and attenuates it down to the requested fields
/// (with an all-teams row scope) via the same `approve_request` path the web UI
/// uses — so the salary invariant is enforced here too — then persists the
/// delegated token to `caps/`.
///
/// Guarded: refuses to grant to the resource owner itself (which would clobber
/// the root token), to an unregistered principal, with empty fields, or on a
/// view the owner does not actually hold.
pub fn grant(config: &Config, to: &str, view_id: &str, fields: &[String], ttl: &str) -> Result<()> {
    use crate::access::biscuit::effective_capability;
    use crate::access::request::{approve_request, AccessRequest};
    use crate::access::{RowScope, View};
    use crate::scenario;

    if to == RESOURCE_OWNER {
        anyhow::bail!(
            "cannot grant to the resource owner '{RESOURCE_OWNER}' — it holds the root token"
        );
    }

    // The requester must be a registered principal (mirrors `mint`).
    let reg = registry::Registry::load(config)?;
    if !reg.principals.iter().any(|p| p.id() == to) {
        anyhow::bail!("unknown principal '{to}' — register it (e.g. `ctl seed`) first");
    }

    // Normalize fields: trim, drop empties, dedupe (preserving order).
    let mut seen = std::collections::BTreeSet::new();
    let fields: Vec<String> = fields
        .iter()
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty() && seen.insert(f.clone()))
        .collect();
    if fields.is_empty() {
        anyhow::bail!("--fields must list at least one field");
    }

    let (source, name) = view_id
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("view must be 'source/view', got '{view_id}'"))?;
    let view = View::new(source, name);

    let owner = load_capability(config, RESOURCE_OWNER).unwrap_or_else(scenario::cfo_capability);
    // The approver must actually hold the requested view, else the grant would
    // misreport (approve_request delegates the owner's authority block as-is).
    let owner_view = effective_capability(&owner).map(|e| e.view.id());
    if owner_view.as_deref() != Some(view.id().as_str()) {
        anyhow::bail!(
            "'{RESOURCE_OWNER}' does not hold {} (holds {})",
            view.id(),
            owner_view.unwrap_or_else(|| "<none>".into())
        );
    }

    let req = AccessRequest {
        id: format!("grant-{to}"),
        requester: to.to_string(),
        view,
        fields: fields.clone(),
        row_scope: Some(vec![RowScope {
            field: "team".into(),
            values: scenario::ALL_TEAMS.iter().map(|s| s.to_string()).collect(),
        }]),
        reason: "ctl grant".into(),
        doc: "finops".into(),
        ttl: ttl.to_string(),
    };

    let granted = approve_request(&owner, &req).map_err(anyhow::Error::from)?;
    save_capability(config, &granted)?;
    println!(
        "granted {to}: {} on {view_id} (ttl {ttl}); all-teams row scope; salary always denied",
        fields.join(", ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::biscuit::effective_capability;
    use crate::config::{Config, InferenceBackend};

    fn temp() -> Config {
        let root = std::env::temp_dir().join(format!("cp-test-{}", uuid::Uuid::new_v4()));
        Config {
            root,
            inference: InferenceBackend::Stub,
        }
    }

    #[test]
    fn grant_happy_path_persists_salary_free_token() {
        let c = temp();
        seed(&c).unwrap();
        grant(
            &c,
            "agent:cto/1",
            "stripe/finance_private",
            &["gross".into(), "credits".into()],
            "7d",
        )
        .unwrap();
        let cap = load_capability(&c, "agent:cto/1").unwrap();
        let eff = effective_capability(&cap).unwrap();
        assert!(eff.fields.contains("credits"));
        assert!(!eff.fields.contains("employee_salary"));
    }

    #[test]
    fn grant_refuses_salary() {
        let c = temp();
        seed(&c).unwrap();
        assert!(grant(
            &c,
            "agent:cto/1",
            "stripe/finance_private",
            &["employee_salary".into()],
            "7d"
        )
        .is_err());
    }

    #[test]
    fn grant_refuses_resource_owner_self_grant() {
        let c = temp();
        seed(&c).unwrap();
        // would otherwise clobber caps/cfo.json with an attenuated token
        assert!(grant(&c, "cfo", "stripe/finance_private", &["gross".into()], "7d").is_err());
        // and the CFO's root token still carries salary
        let cfo = load_capability(&c, "cfo").unwrap();
        assert!(effective_capability(&cfo)
            .unwrap()
            .fields
            .contains("employee_salary"));
    }

    #[test]
    fn grant_refuses_unknown_principal() {
        let c = temp();
        seed(&c).unwrap();
        assert!(grant(
            &c,
            "agent:ghost/9",
            "stripe/finance_private",
            &["gross".into()],
            "7d"
        )
        .is_err());
    }

    #[test]
    fn grant_refuses_empty_or_blank_fields() {
        let c = temp();
        seed(&c).unwrap();
        assert!(grant(&c, "agent:cto/1", "stripe/finance_private", &[], "7d").is_err());
        assert!(grant(
            &c,
            "agent:cto/1",
            "stripe/finance_private",
            &["   ".into()],
            "7d"
        )
        .is_err());
    }

    #[test]
    fn grant_refuses_view_owner_does_not_hold() {
        let c = temp();
        seed(&c).unwrap();
        // CFO holds finance_private, not spend_by_team
        assert!(grant(
            &c,
            "agent:cto/1",
            "stripe/spend_by_team",
            &["gross".into()],
            "7d"
        )
        .is_err());
    }
}
