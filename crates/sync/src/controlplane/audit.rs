//! Host-persisted audit trail (spec 03 §7 "auditable grants", §8).
//!
//! An append-only JSONL file at `<CONTEXTFUL_HOME>/audit.jsonl`. Every
//! authority-changing or boundary event is recorded: grants, mints,
//! revocations, request routing, per-call tool denials, egress blocks, and
//! inbound redactions. Detail payloads carry **labels and counts, never the
//! value that was blocked** — the audit trail must be safe to read at a lower
//! clearance than the data it talks about.
//!
//! Writes are best-effort: an audit failure is logged, it never turns into a
//! denial of the underlying operation (the Flow D offline floor keeps working
//! on a read-only disk).

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;

pub const GRANT: &str = "grant";
pub const MINT: &str = "mint";
pub const REVOKE: &str = "revoke";
/// A capability-checked call was refused (revoked principal, unverifiable
/// token, or a typed query/context denial).
pub const DENIED: &str = "denied";
/// An `access_request` was routed: auto / escalate / forbidden.
pub const REQUEST_ROUTED: &str = "request_routed";
/// The egress firewall blocked an outbound query (tags only, never terms).
pub const EGRESS_BLOCKED: &str = "egress_blocked";
/// Inbound web content carried something secret-shaped and was redacted.
pub const INBOUND_SCRUBBED: &str = "inbound_scrubbed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// RFC 3339 UTC timestamp.
    pub ts: String,
    /// The principal the event is about (grantee, caller, requester).
    pub actor: String,
    pub action: String,
    #[serde(default)]
    pub detail: Value,
}

/// Append one event to the audit trail (best-effort, see module docs).
pub fn record(config: &Config, actor: &str, action: &str, detail: Value) {
    let event = AuditEvent {
        ts: Utc::now().to_rfc3339(),
        actor: actor.to_string(),
        action: action.to_string(),
        detail,
    };
    if let Err(e) = append(config, &event) {
        tracing::warn!(error = %e, action, "audit append failed");
    }
}

fn append(config: &Config, event: &AuditEvent) -> anyhow::Result<()> {
    use std::io::Write;
    config.ensure_dirs()?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(config.audit_path())?;
    writeln!(file, "{}", serde_json::to_string(event)?)?;
    Ok(())
}

/// The last `n` audit events, oldest first. Absent file → empty (a fresh
/// host has an empty history, not an error).
pub fn tail(config: &Config, n: usize) -> Vec<AuditEvent> {
    let Ok(text) = std::fs::read_to_string(config.audit_path()) else {
        return Vec::new();
    };
    let events: Vec<AuditEvent> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let skip = events.len().saturating_sub(n);
    events.into_iter().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InferenceBackend;
    use serde_json::json;

    fn temp() -> Config {
        let root = std::env::temp_dir().join(format!("audit-test-{}", uuid::Uuid::new_v4()));
        Config {
            root,
            inference: InferenceBackend::Stub,
        }
    }

    #[test]
    fn record_then_tail_roundtrips_in_order() {
        let c = temp();
        record(&c, "agent:cto/1", DENIED, json!({ "tool": "brain.query" }));
        record(
            &c,
            "cfo",
            GRANT,
            json!({ "view": "stripe/finance_private" }),
        );
        let events = tail(&c, 10);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, DENIED);
        assert_eq!(events[1].action, GRANT);
        assert_eq!(events[1].actor, "cfo");
    }

    #[test]
    fn tail_returns_only_the_most_recent_n() {
        let c = temp();
        for i in 0..5 {
            record(&c, "p", MINT, json!({ "i": i }));
        }
        let events = tail(&c, 2);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].detail["i"], json!(3));
        assert_eq!(events[1].detail["i"], json!(4));
    }

    #[test]
    fn absent_trail_is_empty_not_an_error() {
        assert!(tail(&temp(), 10).is_empty());
    }
}
