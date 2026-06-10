//! Egress firewall (spec 03 §4 inverse boundary, spec 02 §8).
//!
//! The outbound check between the brain and any web enrichment (Exa): only
//! *public-tainted* query terms may leave the host. A value that originated in
//! a private view (a salary figure, a discount tier, a credit balance) is
//! blocked — enrichment can never become exfiltration. Pure function, no I/O;
//! the Exa connector calls [`firewall`] on every outbound query.

use serde::{Deserialize, Serialize};

/// Where a candidate outbound term came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "taint", content = "acl_tag")]
pub enum Taint {
    /// World/public knowledge (already-public vendor names, metric names, …).
    Public,
    /// Derived from a private view — carries the acl tag it came from.
    Private(String),
}

/// A term the brain wants to send to the web, tagged with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EgressTerm {
    pub term: String,
    pub taint: Taint,
}

impl EgressTerm {
    pub fn public(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            taint: Taint::Public,
        }
    }
    pub fn private(term: impl Into<String>, acl_tag: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            taint: Taint::Private(acl_tag.into()),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("egress blocked: {count} private-tainted term(s) {tags:?} may not leave the host")]
pub struct EgressViolation {
    pub count: usize,
    /// acl tags of the blocked terms (the terms themselves are NOT echoed —
    /// an error message must not leak what it blocked).
    pub tags: Vec<String>,
}

/// Let only public-tainted terms through. Any private-tainted term fails the
/// whole egress (no silent dropping — a partially-scrubbed query could still
/// leak through term co-occurrence).
pub fn firewall(terms: &[EgressTerm]) -> Result<Vec<String>, EgressViolation> {
    let mut tags: Vec<String> = Vec::new();
    for t in terms {
        if let Taint::Private(tag) = &t.taint {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
    }
    let count = terms
        .iter()
        .filter(|t| matches!(t.taint, Taint::Private(_)))
        .count();
    if count > 0 {
        return Err(EgressViolation { count, tags });
    }
    Ok(terms.iter().map(|t| t.term.clone()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_terms_pass() {
        let out = firewall(&[
            EgressTerm::public("anthropic claude pricing"),
            EgressTerm::public("ai token spend benchmark"),
        ])
        .unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn private_value_never_leaves() {
        // a discount tier value derived from stripe/finance_private
        let err = firewall(&[
            EgressTerm::public("anthropic pricing"),
            EgressTerm::private("tier-3 negotiated 18%", "finance_private"),
        ])
        .unwrap_err();
        assert_eq!(err.count, 1);
        assert_eq!(err.tags, vec!["finance_private".to_string()]);
        // and the error must not echo the blocked term
        assert!(!err.to_string().contains("tier-3"));
    }

    #[test]
    fn salary_figure_blocked_even_alone() {
        let err = firewall(&[EgressTerm::private("245000", "finance_private")]).unwrap_err();
        assert_eq!(err.count, 1);
    }

    #[test]
    fn empty_egress_is_fine() {
        assert_eq!(firewall(&[]).unwrap().len(), 0);
    }
}
