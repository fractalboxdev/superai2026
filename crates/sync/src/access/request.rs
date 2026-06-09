//! Permission requests, auto-mode envelopes, and grant minting (spec 03 §5).
//!
//! The salary invariant lives here and in [`super::biscuit`]: no request that
//! names a `never_delegable` field can become a token for a non-owner — it is
//! rejected by routing (no path offered) AND by the minter (defense in depth).

use serde::{Deserialize, Serialize};

use crate::access::{biscuit::attenuate, AttenuationBlock, Capability, RowScope, View};

/// Fields that may never be delegated to an agent by any approval path.
pub const NEVER_DELEGABLE: &[&str] = &["employee_salary"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessRequest {
    pub id: String,
    /// requesting principal id, e.g. `agent:cto/1`.
    pub requester: String,
    pub view: View,
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_scope: Option<Vec<RowScope>>,
    pub reason: String,
    pub doc: String,
    pub ttl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoApprove {
    pub view: View,
    pub max_ttl: String,
}

/// An owner's auto-mode policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub owner: String,
    #[serde(default)]
    pub auto_approve: Vec<AutoApprove>,
    #[serde(default)]
    pub never_delegate: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteDecision {
    Auto { reason: String },
    Escalate { reason: String },
    Forbidden { reason: String },
}

/// Decide how a request is handled: auto-approve, escalate, or forbid.
pub fn route_request(req: &AccessRequest, envelope: &Envelope) -> RouteDecision {
    let blocked: Vec<String> = req
        .fields
        .iter()
        .filter(|f| envelope.never_delegate.iter().any(|n| n == *f))
        .cloned()
        .collect();
    if !blocked.is_empty() {
        return RouteDecision::Forbidden {
            reason: format!(
                "{} is never delegable — no approval path exists (salary invariant).",
                blocked.join(", ")
            ),
        };
    }
    if let Some(a) = envelope
        .auto_approve
        .iter()
        .find(|a| a.view.id() == req.view.id())
    {
        return RouteDecision::Auto {
            reason: format!("inside envelope for {} (≤ {})", req.view.id(), a.max_ttl),
        };
    }
    RouteDecision::Escalate {
        reason: "outside auto-approve envelope — owner decides".to_string(),
    }
}

#[derive(Debug, thiserror::Error)]
#[error("refusing to mint a token granting {fields:?} — salary invariant")]
pub struct SalaryInvariantViolation {
    pub fields: Vec<String>,
}

/// Approve a request by attenuating the approver's own capability down to the
/// exact requested scope and delegating it to the requester. Never grants a
/// `NEVER_DELEGABLE` field — enforced here regardless of caller.
pub fn approve_request(
    approver_cap: &Capability,
    req: &AccessRequest,
) -> Result<Capability, SalaryInvariantViolation> {
    let forbidden: Vec<String> = req
        .fields
        .iter()
        .filter(|f| NEVER_DELEGABLE.contains(&f.as_str()))
        .cloned()
        .collect();
    if !forbidden.is_empty() {
        return Err(SalaryInvariantViolation { fields: forbidden });
    }

    let mut delegated = approver_cap.clone();
    delegated.holder = req.requester.clone();
    Ok(attenuate(
        &delegated,
        AttenuationBlock {
            by: approver_cap.holder.clone(),
            deny_fields: NEVER_DELEGABLE.iter().map(|s| s.to_string()).collect(),
            allow_fields: Some(req.fields.clone()),
            deny_views: Vec::new(),
            rows: req.row_scope.clone(),
            ttl: Some(req.ttl.clone()),
        },
    ))
}
