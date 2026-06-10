//! Capability-filtered retrieval (spec 02 §4, spec 06 §1).
//!
//! Every candidate is authorized against the caller's token **before** it
//! reaches the agent/LLM: structured rows are field/row-redacted; Markdown cards
//! are authorized all-or-nothing against their `acl_tag`. `brain.query` needs no
//! LLM — the path that keeps working offline.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::access::biscuit::{authorize, effective_capability, row_allowed};
use crate::access::{
    AuthDecision, Capability, DenyReason, Operation, QueryRequest, RowScope, View,
};
use crate::brain::markdown::{render_card, slug, CardMeta};
use crate::brain::{BrainIndex, Memory, MemoryKind};
use crate::connectors::AclTag;
use crate::store::Store;
use chrono::Utc;

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QueryResult {
    Denied {
        reason: DenyReason,
        answer: String,
    },
    Ok {
        fields: Vec<String>,
        redacted: Vec<String>,
        rows: Vec<Value>,
        answer: String,
    },
}

fn deny_copy(reason: &DenyReason) -> String {
    match reason {
        DenyReason::NoGrant => {
            "Denied — your token carries no grant for this view. You can raise an access request."
        }
        DenyReason::ViewDenied => {
            "Denied — this view is explicitly excluded from your token and cannot be re-granted by attenuation."
        }
        DenyReason::WrongOp => "Denied — your token does not permit this operation on the view.",
    }
    .to_string()
}

/// Structured `brain.query(view, select, where)` (spec 06 §1).
pub fn query(index: &BrainIndex, cap: &Capability, view: &View, select: &[String]) -> QueryResult {
    let decision = authorize(
        cap,
        &QueryRequest {
            op: Operation::Query,
            view: view.clone(),
            fields: select.to_vec(),
        },
    );
    let (granted, redacted, row_filter) = match decision {
        AuthDecision::Denied(reason) => {
            let answer = deny_copy(&reason);
            return QueryResult::Denied { reason, answer };
        }
        AuthDecision::Ok {
            granted_fields,
            redacted_fields,
            row_filter,
        } => (granted_fields, redacted_fields, row_filter),
    };

    // Keep team/period for labelling row-scoped aggregates, but only if the
    // caller's TOKEN grants them (effective set) — never project a field outside
    // the token's authority, even when not in this query's select.
    let eff_fields = effective_capability(cap)
        .map(|e| e.fields)
        .unwrap_or_default();
    let mut projection: Vec<String> = ["team", "period"]
        .into_iter()
        .filter(|label| eff_fields.contains(*label))
        .map(String::from)
        .collect();
    for f in &granted {
        if !projection.contains(f) {
            projection.push(f.clone());
        }
    }

    let mut rows: Vec<Value> = Vec::new();
    for e in index.events_for_view(&view.id()) {
        let payload = e.payload.as_object();
        let allowed = row_allowed(
            |field| payload.and_then(|m| m.get(field)).map(value_to_string),
            &row_filter,
        );
        if !allowed {
            continue;
        }
        let mut obj = Map::new();
        if let Some(m) = payload {
            for f in &projection {
                if let Some(v) = m.get(f) {
                    obj.insert(f.clone(), v.clone());
                }
            }
        }
        rows.push(Value::Object(obj));
    }

    let answer = synthesize_answer(&granted, &rows, &redacted);
    QueryResult::Ok {
        fields: granted,
        redacted,
        rows,
        answer,
    }
}

/// Views the caller may read at least one field of (spec 06 `brain.list_sources`).
pub fn list_sources(index: &BrainIndex, cap: &Capability) -> Vec<String> {
    let mut views: Vec<String> = index
        .raw_events
        .iter()
        .map(|e| e.view.id())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    views.retain(|vid| {
        let parts: Vec<&str> = vid.splitn(2, '/').collect();
        if parts.len() != 2 {
            return false;
        }
        let view = View::new(parts[0], parts[1]);
        matches!(
            authorize(
                cap,
                &QueryRequest {
                    op: Operation::Query,
                    view,
                    fields: vec!["team".into()],
                },
            ),
            AuthDecision::Ok { granted_fields, .. } if !granted_fields.is_empty()
        )
    });
    views
}

/// Synthesized Markdown context card by topic (spec 06 `brain.get_context`).
/// Authorized all-or-nothing against the card's `acl_tag`.
pub fn get_context(
    store: &Store,
    index: &BrainIndex,
    cap: &Capability,
    topic: &str,
) -> Result<String, String> {
    let mem = index
        .memories
        .iter()
        .filter(|m| m.topic == topic)
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .ok_or_else(|| format!("no context card for topic '{topic}'"))?;

    if !card_authorized(cap, &mem.acl_tag) {
        return Err(format!(
            "Denied — '{topic}' requires {} on {} which your token does not fully grant.",
            mem.acl_tag.fields.join(", "),
            mem.acl_tag.view.id()
        ));
    }
    store.read_card(&mem.path).map_err(|e| e.to_string())
}

/// Full-text search over card bodies (SQLite FTS5), each hit independently
/// authorized (spec 06 `brain.search`). Topic substring match is the
/// fallback so an empty/no-hit query still lists readable cards.
pub fn search(store: &Store, index: &BrainIndex, cap: &Capability, q: &str) -> Vec<Value> {
    let fts_ids = store.search_cards(q).unwrap_or_default();
    let needle = q.to_lowercase();
    let mut seen = std::collections::BTreeSet::new();
    let mut hits: Vec<&Memory> = Vec::new();
    // FTS hits first (already ranked)
    for id in &fts_ids {
        if let Some(m) = index.memories.iter().find(|m| &m.id == id) {
            if seen.insert(m.id.clone()) {
                hits.push(m);
            }
        }
    }
    for m in index.memories.iter() {
        if (m.topic.to_lowercase().contains(&needle) || needle.is_empty())
            && seen.insert(m.id.clone())
        {
            hits.push(m);
        }
    }
    hits.into_iter()
        .filter(|m| card_authorized(cap, &m.acl_tag))
        .map(|m| {
            json!({
                "topic": m.topic,
                "kind": m.kind,
                "period": m.period,
                "path": m.path,
                "acl_view": m.acl_tag.view.id(),
            })
        })
        .collect()
}

/// Anomalies for a view/period the caller may query (spec 06 `brain.detect_anomalies`).
pub fn detect_anomalies(index: &BrainIndex, cap: &Capability, view: &View) -> QueryResult {
    // Anomaly figures are derived from `gross`, so the caller must actually hold
    // `gross` — a denied view OR `gross` landing in `redacted` is a denial,
    // otherwise gross-derived baseline/observed would leak past field redaction.
    match authorize(
        cap,
        &QueryRequest {
            op: Operation::Query,
            view: view.clone(),
            fields: vec!["gross".into()],
        },
    ) {
        AuthDecision::Denied(reason) => {
            let answer = deny_copy(&reason);
            return QueryResult::Denied { reason, answer };
        }
        AuthDecision::Ok { granted_fields, .. } if !granted_fields.iter().any(|f| f == "gross") => {
            return QueryResult::Denied {
                reason: DenyReason::NoGrant,
                answer:
                    "Denied — anomalies require the gross metric, which your token does not grant."
                        .into(),
            };
        }
        AuthDecision::Ok { .. } => {}
    }
    let rows: Vec<Value> = index
        .anomalies
        .iter()
        .filter(|a| a.view == view.id())
        .map(|a| {
            json!({
                "metric": a.metric, "period": a.period, "baseline": a.baseline,
                "observed": a.observed, "severity": a.severity,
            })
        })
        .collect();
    let answer = if rows.is_empty() {
        "No anomalies for this view/period.".to_string()
    } else {
        format!("{} anomaly(ies) detected.", rows.len())
    };
    QueryResult::Ok {
        fields: vec!["metric".into(), "period".into(), "severity".into()],
        redacted: vec![],
        rows,
        answer,
    }
}

/// Write a memory scoped to a document (spec 06 `brain.remember`). The card is
/// stamped with `read_acl` as its acl floor. Full taint tracking — stamping
/// with the max acl_tag of every source the agent read *this turn* — needs a
/// per-session read-set the MCP server does not yet thread through; today the
/// caller passes a doc-scoped floor. Tracking that read-set is future work.
pub fn remember(
    store: &Store,
    index: &mut BrainIndex,
    fact: &str,
    doc: &str,
    read_acl: AclTag,
) -> anyhow::Result<String> {
    let meta = CardMeta {
        topic: doc,
        kind: "wiki",
        period: None,
        confidence: 0.6,
        acl_tag: &read_acl,
    };
    let slug_name = slug(&format!("note-{}", &uuid::Uuid::new_v4().to_string()[..8]));
    let path = store.write_card(doc, &slug_name, &render_card(&meta, "Agent note", fact))?;
    let id = uuid::Uuid::new_v4().to_string();
    index.memories.push(Memory {
        id: id.clone(),
        kind: MemoryKind::Wiki,
        topic: doc.to_string(),
        path: path.display().to_string(),
        acl_tag: read_acl,
        confidence: 0.6,
        period: None,
        supersedes: None,
        created_at: Utc::now().to_rfc3339(),
    });
    Ok(id)
}

/// Public card-readability check (used by the daydream loop's admissibility
/// sampling, spec 02 §9).
pub fn card_readable(cap: &Capability, tag: &AclTag) -> bool {
    card_authorized(cap, tag)
}

/// A caller can read a card iff its token grants every field in the card's tag.
/// World cards (`world/public`) are default-readable by every principal —
/// public, cited knowledge is never authority (spec 02 §8).
fn card_authorized(cap: &Capability, tag: &AclTag) -> bool {
    if tag.view.id() == "world/public" {
        return true;
    }
    matches!(
        authorize(
            cap,
            &QueryRequest {
                op: Operation::Query,
                view: tag.view.clone(),
                fields: tag.fields.clone(),
            },
        ),
        AuthDecision::Ok { ref redacted_fields, .. } if redacted_fields.is_empty()
    )
}

fn synthesize_answer(fields: &[String], rows: &[Value], redacted: &[String]) -> String {
    if rows.is_empty() {
        return "No rows are visible within your row scope.".to_string();
    }
    let has = |f: &str| fields.iter().any(|x| x == f);
    let sum = |f: &str| -> i64 {
        rows.iter()
            .filter_map(|r| r.get(f).and_then(|v| v.as_i64()))
            .sum()
    };

    let mut parts: Vec<String> = Vec::new();
    if has("gross") {
        parts.push(format!("Gross spend ${}", thousands(sum("gross"))));
    }
    if has("credits") && has("gross") {
        let net = sum("gross") - sum("credits");
        parts.push(format!("net of credits ${}", thousands(net)));
        if has("discount_tier") {
            if let Some(t) = rows[0].get("discount_tier").and_then(|v| v.as_str()) {
                parts.push(format!("at discount tier {t}"));
            }
        }
    } else if has("net") {
        parts.push(format!("net spend ${}", thousands(sum("net"))));
    }

    let mut answer = if parts.is_empty() {
        format!("{} row(s) visible.", rows.len())
    } else {
        format!("{} across {} team(s).", parts.join(", "), rows.len())
    };
    if !redacted.is_empty() {
        answer.push_str(&format!(" Withheld (redacted): {}.", redacted.join(", ")));
    }
    answer
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut chunks: Vec<String> = Vec::new();
    let mut i = s.len() as isize;
    while i > 0 {
        let start = (i - 3).max(0) as usize;
        chunks.push(s[start..i as usize].to_string());
        i -= 3;
    }
    chunks.reverse();
    let body = chunks.join(",");
    if n < 0 {
        format!("-{body}")
    } else {
        body
    }
}

/// Convenience for tests/CLI: a fixed all-teams row scope.
pub fn all_teams_scope() -> Vec<RowScope> {
    vec![RowScope {
        field: "team".into(),
        values: vec!["eng".into(), "ops".into(), "sales".into(), "finance".into()],
    }]
}
