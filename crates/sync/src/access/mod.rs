//! Capability-based access control (spec 03).
//!
//! Resource / Operation / Field / Row model + capability tokens. The engine
//! (mint / attenuate / authorize) lives in [`biscuit`]; the permission-request
//! and auto-mode envelope flow in [`request`].
//!
//! The block algebra in [`biscuit`] computes the scope; the *proof* is a real
//! `biscuit-auth` signed token ([`token`]) carried in [`Capability::token`]:
//! signature-verified against the resource root's public key, attenuated by
//! appending real Biscuit blocks, and re-authorized per field through a
//! Datalog authorizer. Three properties hold, cryptographically:
//!
//!   1. `caps(child) ⊆ caps(parent)` — attenuation can only narrow.
//!   2. No capability super-root — one resource-root key per token.
//!   3. Field/row enforcement before any data leaves the brain query layer.

pub mod biscuit;
pub mod egress;
pub mod request;
pub mod token;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Read,
    Write,
    Comment,
    Query,
    Admin,
}

/// A named, field-typed projection of a source — the unit of finance privacy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub source: String,
    pub view: String,
}

impl View {
    pub fn new(source: impl Into<String>, view: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            view: view.into(),
        }
    }

    /// Stable id, e.g. `stripe/finance_private`.
    pub fn id(&self) -> String {
        format!("{}/{}", self.source, self.view)
    }
}

/// A row-level predicate: `field IN [...]`. Empty list matches nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowScope {
    pub field: String,
    #[serde(rename = "in")]
    pub values: Vec<String>,
}

/// Root authority block. Only the holder of the matching resource-root key can
/// create one (see [`biscuit::mint`]); it carries the FULL grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityBlock {
    /// resource-root key id this authority descends from, e.g. "cfo".
    pub root: String,
    pub ops: Vec<Operation>,
    pub view: View,
    pub fields: Vec<String>,
    #[serde(default)]
    pub rows: Vec<RowScope>,
    /// document ids (or "*") the holder may read/write through the sync relay
    /// (spec 01 §4 per-message authorization).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub docs: Vec<String>,
}

/// An append-only attenuation block. Every field here can only narrow.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttenuationBlock {
    pub by: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fields: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny_views: Vec<View>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<Vec<RowScope>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Block {
    Authority(AuthorityBlock),
    Attenuation(AttenuationBlock),
}

/// A capability token: an authority block followed by 0+ attenuations.
///
/// `blocks` is the human-readable mirror; `token` is the signed Biscuit that
/// *proves* it. Production loads verify the token and derive the effective
/// scope from it (see [`token::verify_token`]) — tampering with the JSON
/// mirror can never widen access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// principal id currently holding the token.
    pub holder: String,
    pub blocks: Vec<Block>,
    /// base64 signed Biscuit (None only for unsigned in-memory test values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// A resource-root key. Holding one is the authority to mint over its views.
/// The control-plane root deliberately holds NO data views — "no super-root".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootKey {
    pub id: String,
    pub owner: String,
    pub views: Vec<View>,
    /// hex-encoded ed25519 public key (the private half lives in the owner's
    /// keystore under `control/keys/`, never in the registry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub op: Operation,
    pub view: View,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyReason {
    NoGrant,
    ViewDenied,
    WrongOp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthDecision {
    Denied(DenyReason),
    Ok {
        granted_fields: Vec<String>,
        /// requested fields the caller is NOT cleared for — signalled, not silent.
        redacted_fields: Vec<String>,
        row_filter: Vec<RowScope>,
    },
}
