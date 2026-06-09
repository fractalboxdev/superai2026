//! Capability-based access control (spec 03).
//!
//! Resource / Operation / Field / Row model + capability tokens. The engine
//! (mint / attenuate / authorize) lives in [`biscuit`]; the permission-request
//! and auto-mode envelope flow in [`request`].
//!
//! This is a faithful Rust port of the proven TS prototype
//! (`packages/protocol/src/access.ts`). It is a deliberate stand-in for real
//! Biscuit Datalog (`biscuit-auth`); the block algebra here guarantees the same
//! three properties, computed rather than trusted:
//!
//!   1. `caps(child) ⊆ caps(parent)` — attenuation can only narrow.
//!   2. No capability super-root — one resource-root key per token.
//!   3. Field/row enforcement before any data leaves the brain query layer.

pub mod biscuit;
pub mod request;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// principal id currently holding the token.
    pub holder: String,
    pub blocks: Vec<Block>,
}

/// A resource-root key. Holding one is the authority to mint over its views.
/// The control-plane root deliberately holds NO data views — "no super-root".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootKey {
    pub id: String,
    pub owner: String,
    pub views: Vec<View>,
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
