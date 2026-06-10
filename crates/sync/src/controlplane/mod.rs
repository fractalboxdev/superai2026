//! Control plane (spec 07 §3) — identity-root only.
//!
//! Registers principals and document membership and seeds auto-mode envelopes;
//! it **cannot** mint authority over data resources (those are rooted at their
//! owners, spec 03 §1). Backed by config files under `~/.contextful/control/`
//! and issued tokens under `~/.contextful/caps/`.

pub mod envelope;
pub mod keys;
pub mod registry;

use std::path::PathBuf;

use anyhow::Result;

use crate::access::token::{verify_capability, VerifiedScope};
use crate::access::{AuthorityBlock, Block, Capability};
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

/// Load a principal's issued capability token from `caps/` and VERIFY it:
/// the embedded Biscuit's signature is checked against the root's registered
/// public key and the effective scope is re-derived from the token alone
/// (per-field Datalog runs). The returned capability is clamped to that
/// verified scope — editing the JSON mirror can never widen access.
pub fn load_capability(config: &Config, principal: &str) -> Option<Capability> {
    let stored = read_stored(config, principal)?;
    let scope = verified_scope_logged(config, &stored, principal)?;
    // fail closed on token-level (Biscuit revocation-id) revocation too — the
    // name list and the id list are written together by `revoke`, but loads
    // must not depend on that staying true
    if is_token_revoked(config, &scope.revocation_ids) {
        tracing::warn!(%principal, "capability token is revoked");
        return None;
    }
    Some(clamp_to_scope(&stored, scope))
}

/// Read a principal's stored capability mirror (unverified).
fn read_stored(config: &Config, principal: &str) -> Option<Capability> {
    let text = std::fs::read_to_string(cap_path(config, principal)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Verify a stored capability, deriving its scope from the token alone.
fn verify_stored(config: &Config, stored: &Capability, principal: &str) -> Result<VerifiedScope> {
    let root_id = match stored.blocks.first() {
        Some(Block::Authority(a)) => a.root.clone(),
        _ => anyhow::bail!("no authority block"),
    };
    let reg = registry::Registry::load(config)?;
    let root = reg
        .roots
        .iter()
        .find(|r| r.id == root_id)
        .ok_or_else(|| anyhow::anyhow!("unknown root '{root_id}'"))?;
    let pub_hex = root
        .public_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("root '{root_id}' has no registered public key"))?;
    let root_pub = keys::public_key_from_hex(pub_hex)?;
    verify_capability(stored, &root_pub, principal).map_err(anyhow::Error::from)
}

/// [`verify_stored`], logging (never propagating) verification failures.
fn verified_scope_logged(
    config: &Config,
    stored: &Capability,
    principal: &str,
) -> Option<VerifiedScope> {
    match verify_stored(config, stored, principal) {
        Ok(scope) => Some(scope),
        Err(e) => {
            tracing::warn!(%principal, error = %e, "capability failed verification");
            #[cfg(test)]
            eprintln!("capability verification failed for {principal}: {e:#}");
            None
        }
    }
}

/// Rebuild a capability whose JSON mirror is exactly the verified scope.
/// The signed token (full chain) is preserved for further attenuation.
fn clamp_to_scope(stored: &Capability, scope: VerifiedScope) -> Capability {
    Capability {
        holder: scope.holder.clone(),
        blocks: vec![Block::Authority(AuthorityBlock {
            root: scope.root,
            ops: scope.ops,
            view: scope.view,
            fields: scope.fields.into_iter().collect(),
            rows: scope.rows,
            docs: scope
                .docs
                .into_iter()
                .map(|(pat, _)| pat)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
        })],
        token: stored.token.clone(),
    }
}

/// Load + verify a capability, returning the token-derived scope (for callers
/// that need doc rights or revocation ids, e.g. the sync relay).
pub fn load_verified_scope(config: &Config, principal: &str) -> Option<VerifiedScope> {
    let stored = read_stored(config, principal)?;
    verified_scope_logged(config, &stored, principal)
}

/// Persist a principal's capability token under `caps/` (audit/revocation).
pub fn save_capability(config: &Config, cap: &Capability) -> Result<()> {
    config.ensure_dirs()?;
    let path = cap_path(config, &cap.holder);
    std::fs::write(path, serde_json::to_string_pretty(cap)?)?;
    Ok(())
}

/// Record a revoked token holder in the revocation list (checked on every
/// session and tool call by the relay/MCP auth path, spec 07 §3). If the
/// principal has an issued Biscuit, its cryptographic revocation identifiers
/// are recorded too (`caps/revoked_ids.json`).
pub fn revoke(config: &Config, principal: &str) -> Result<()> {
    config.ensure_dirs()?;
    let path = config.caps_dir().join("revoked.json");
    let mut list = read_string_list(&path);
    if !list.iter().any(|p| p == principal) {
        list.push(principal.to_string());
    }
    std::fs::write(path, serde_json::to_string_pretty(&list)?)?;

    // biscuit revocation ids (one per block) — token-level revocation
    if let Some(token) = read_stored(config, principal).and_then(|c| c.token) {
        if let Ok(ub) = biscuit_auth::UnverifiedBiscuit::from_base64(&token) {
            let ids_path = config.caps_dir().join("revoked_ids.json");
            let mut ids = read_string_list(&ids_path);
            for id in ub.revocation_identifiers() {
                let id = hex::encode(id);
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            std::fs::write(ids_path, serde_json::to_string_pretty(&ids)?)?;
        }
    }
    Ok(())
}

/// Read a JSON string-list file; absent or unparseable means empty.
fn read_string_list(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Is any of a verified token's revocation ids on the revocation list?
pub fn is_token_revoked(config: &Config, revocation_ids: &[String]) -> bool {
    let ids = read_string_list(&config.caps_dir().join("revoked_ids.json"));
    revocation_ids.iter().any(|id| ids.contains(id))
}

pub fn is_revoked(config: &Config, principal: &str) -> bool {
    read_string_list(&config.caps_dir().join("revoked.json"))
        .iter()
        .any(|p| p == principal)
}

/// Seed the demo control plane: principals, root catalog (with real ed25519
/// keypairs), envelopes, and each principal's initial capability — signed as
/// a real Biscuit by its resource root's private key (spec 07 §3).
pub fn seed(config: &Config) -> Result<()> {
    use crate::access::token::sign;
    use crate::scenario;
    config.ensure_dirs()?;

    let cfo_keys = keys::ensure_root_key(config, "cfo")?;
    let cp_keys = keys::ensure_root_key(config, "control-plane")?;

    let mut registry = registry::Registry::default();
    for p in scenario::principals() {
        registry.register_principal(p);
    }
    let mut cfo_root = scenario::cfo_root();
    cfo_root.public_key = Some(cfo_keys.public().to_bytes_hex());
    let mut cp_root = scenario::control_plane_root();
    cp_root.public_key = Some(cp_keys.public().to_bytes_hex());
    registry.register_root(cfo_root);
    registry.register_root(cp_root);
    registry.save(config)?;

    let mut envelopes = envelope::EnvelopeStore::default();
    envelopes.upsert(scenario::cfo_envelope());
    envelopes.save(config)?;

    for p in scenario::principals() {
        if let Some(cap) = scenario::initial_capability(p.id()) {
            let signed = sign(&cap, &cfo_keys).map_err(anyhow::Error::from)?;
            save_capability(config, &signed)?;
        }
    }

    println!(
        "seeded {} → principals, roots (ed25519), envelopes, signed biscuit tokens",
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

    let owner = load_capability(config, RESOURCE_OWNER).ok_or_else(|| {
        anyhow::anyhow!("no verified token for '{RESOURCE_OWNER}' — run `ctl seed` first")
    })?;
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
