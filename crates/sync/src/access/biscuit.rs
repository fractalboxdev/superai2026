//! The capability engine: mint / attenuate / authorize + field/row authorizer
//! (spec 03 §3–4).
//!
//! This block algebra computes scopes (and powers the property tests); the
//! cryptographic proof is the real `biscuit-auth` token managed by
//! [`super::token`]: [`attenuate`]/[`delegate_to`] append a real signed
//! Biscuit block whenever the capability carries a token, and production
//! loads re-derive the effective scope from the verified token alone.

use std::collections::BTreeSet;

use crate::access::{
    AttenuationBlock, AuthDecision, AuthorityBlock, Block, Capability, DenyReason, Operation,
    QueryRequest, RootKey, RowScope, View,
};

#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    #[error("root '{root}' has no authority over {view}")]
    NoRootAuthority { root: String, view: String },
}

/// Mint a fresh first-party token from a resource root (unsigned — see
/// [`super::token::sign`] for the signing step, done where the root's private
/// key lives).
pub fn mint(
    root: &RootKey,
    holder: &str,
    ops: Vec<Operation>,
    view: View,
    fields: Vec<String>,
    rows: Vec<RowScope>,
) -> Result<Capability, AccessError> {
    mint_with_docs(root, holder, ops, view, fields, rows, Vec::new())
}

/// [`mint`], plus document grants for the sync relay ("*" = any doc).
#[allow(clippy::too_many_arguments)]
pub fn mint_with_docs(
    root: &RootKey,
    holder: &str,
    ops: Vec<Operation>,
    view: View,
    fields: Vec<String>,
    rows: Vec<RowScope>,
    docs: Vec<String>,
) -> Result<Capability, AccessError> {
    if !root.views.iter().any(|v| v.id() == view.id()) {
        return Err(AccessError::NoRootAuthority {
            root: root.id.clone(),
            view: view.id(),
        });
    }
    Ok(Capability {
        holder: holder.to_string(),
        blocks: vec![Block::Authority(AuthorityBlock {
            root: root.id.clone(),
            ops,
            view,
            fields,
            rows,
            docs,
        })],
        token: None,
    })
}

/// Append an attenuation block, keeping the same holder. If the capability
/// carries a signed token, a real Biscuit block is appended too (offline,
/// keyless — Biscuit's defining property), so the proof narrows with the
/// mirror.
pub fn attenuate(cap: &Capability, block: AttenuationBlock) -> Capability {
    attenuate_as(cap, &cap.holder.clone(), block)
}

/// Append an attenuation block AND hand the token to a new holder (delegation).
pub fn delegate_to(cap: &Capability, holder: &str, block: AttenuationBlock) -> Capability {
    attenuate_as(cap, holder, block)
}

fn attenuate_as(cap: &Capability, holder: &str, block: AttenuationBlock) -> Capability {
    let token = cap.token.as_deref().map(|t| {
        super::token::append_attenuation(t, &block, Some(holder))
            .expect("appending an attenuation block to a well-formed token cannot fail")
    });
    let mut blocks = cap.blocks.clone();
    blocks.push(Block::Attenuation(block));
    Capability {
        holder: holder.to_string(),
        blocks,
        token,
    }
}

#[derive(Debug, Clone)]
pub struct EffectiveCapability {
    pub root: String,
    pub ops: BTreeSet<Operation>,
    pub view: View,
    pub fields: BTreeSet<String>,
    pub rows: Vec<RowScope>,
    pub denied_views: Vec<View>,
}

fn intersect_rows(a: &[RowScope], b: &[RowScope]) -> Vec<RowScope> {
    let mut out: Vec<RowScope> = a.to_vec();
    for r in b {
        if let Some(existing) = out.iter_mut().find(|x| x.field == r.field) {
            existing.values.retain(|v| r.values.contains(v));
        } else {
            out.push(r.clone());
        }
    }
    out
}

/// Fold the block list into the effective grant. Because every attenuation can
/// only subtract, the effective field & row sets shrink monotonically down the
/// chain — the `caps(child) ⊆ caps(parent)` guarantee, computed not trusted.
pub fn effective_capability(cap: &Capability) -> Option<EffectiveCapability> {
    let auth = match cap.blocks.first() {
        Some(Block::Authority(a)) => a,
        _ => return None,
    };

    let mut fields: BTreeSet<String> = auth.fields.iter().cloned().collect();
    let mut rows: Vec<RowScope> = auth.rows.clone();
    let mut denied_views: Vec<View> = Vec::new();

    for b in cap.blocks.iter().skip(1) {
        if let Block::Attenuation(a) = b {
            if let Some(allow) = &a.allow_fields {
                fields.retain(|f| allow.contains(f));
            }
            if !a.deny_fields.is_empty() {
                fields.retain(|f| !a.deny_fields.contains(f));
            }
            denied_views.extend(a.deny_views.iter().cloned());
            if let Some(r) = &a.rows {
                rows = intersect_rows(&rows, r);
            }
        }
    }

    Some(EffectiveCapability {
        root: auth.root.clone(),
        ops: auth.ops.iter().copied().collect(),
        view: auth.view.clone(),
        fields,
        rows,
        denied_views,
    })
}

/// Authorize a structured query. A view with no grant returns a typed denial
/// (the trigger for the request flow); a covered view drops fields the caller
/// can't see and lists them in `redacted_fields`. No LLM involved.
pub fn authorize(cap: &Capability, req: &QueryRequest) -> AuthDecision {
    let eff = match effective_capability(cap) {
        Some(e) => e,
        None => return AuthDecision::Denied(DenyReason::NoGrant),
    };
    if eff.view.id() != req.view.id() {
        return AuthDecision::Denied(DenyReason::NoGrant);
    }
    if eff.denied_views.iter().any(|v| v.id() == req.view.id()) {
        return AuthDecision::Denied(DenyReason::ViewDenied);
    }
    if !eff.ops.contains(&req.op) {
        return AuthDecision::Denied(DenyReason::WrongOp);
    }

    let granted_fields: Vec<String> = req
        .fields
        .iter()
        .filter(|f| eff.fields.contains(*f))
        .cloned()
        .collect();
    let redacted_fields: Vec<String> = req
        .fields
        .iter()
        .filter(|f| !eff.fields.contains(*f))
        .cloned()
        .collect();

    AuthDecision::Ok {
        granted_fields,
        redacted_fields,
        row_filter: eff.rows,
    }
}

/// Does a row satisfy every row-scope predicate?
pub fn row_allowed(value_of: impl Fn(&str) -> Option<String>, filter: &[RowScope]) -> bool {
    filter.iter().all(|scope| {
        value_of(&scope.field)
            .map(|v| scope.values.contains(&v))
            .unwrap_or(false)
    })
}
