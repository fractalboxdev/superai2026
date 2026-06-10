//! Connectors & ETL (spec 05).
//!
//! All ingestion goes through one [`Connector`] trait. A connector declares its
//! source id, the views it exposes (the unit of access control), and a `pull`
//! that yields raw events tagged with provenance and an `acl_tag`.

pub mod exa;
pub mod slack;
pub mod stripe;

use serde::{Deserialize, Serialize};

use crate::access::View;

/// A typed field within a view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewField {
    pub name: String,
    pub ty: String,
    /// true if this field is finance-private (e.g. employee_salary, credits).
    #[serde(default)]
    pub private: bool,
}

/// Names a view and its typed fields (spec 05 §1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSchema {
    pub view: View,
    pub fields: Vec<ViewField>,
}

/// The access requirement stamped on every raw event and derived row. Maps to
/// the resource/field model so memories inherit the right access (spec 02 §3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclTag {
    pub view: View,
    /// the fields present in this record (drives field-level auth + card tags).
    pub fields: Vec<String>,
}

impl AclTag {
    /// Merge two tags to the *max* requirement (taint propagation; never lower).
    pub fn max(&self, other: &AclTag) -> AclTag {
        let mut fields = self.fields.clone();
        for f in &other.fields {
            if !fields.contains(f) {
                fields.push(f.clone());
            }
        }
        AclTag {
            view: self.view.clone(),
            fields,
        }
    }
}

/// Incremental pull cursor (spec 05 §1).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cursor {
    pub since: Option<String>,
}

/// An immutable ingested record (spec 02 §3 `raw_event`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawEvent {
    pub id: String,
    pub source_id: String,
    pub view: View,
    pub payload: serde_json::Value,
    pub ingested_at: String,
    pub acl_tag: AclTag,
}

/// The ingestion contract (spec 05 §1).
pub trait Connector {
    fn source_id(&self) -> &str;
    fn views(&self) -> Vec<ViewSchema>;
    fn pull(&self, since: &Cursor) -> anyhow::Result<Vec<RawEvent>>;
}
