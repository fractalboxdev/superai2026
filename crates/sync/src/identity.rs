//! Principals & identity (spec 03 §1).
//!
//! A principal is a human or an agent owned by exactly one human. Agents hold
//! no root authority — only attenuated tokens minted by their owner.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Principal {
    Human {
        id: String,
        name: String,
        role: String,
    },
    Agent {
        id: String,
        name: String,
        /// owning human's principal id.
        owner: String,
    },
}

impl Principal {
    pub fn id(&self) -> &str {
        match self {
            Principal::Human { id, .. } => id,
            Principal::Agent { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Principal::Human { name, .. } => name,
            Principal::Agent { name, .. } => name,
        }
    }

    pub fn is_agent(&self) -> bool {
        matches!(self, Principal::Agent { .. })
    }
}

/// Agent id format: `agent:<owner>/<n>`.
pub fn agent_id(owner: &str, n: &str) -> String {
    format!("agent:{owner}/{n}")
}
