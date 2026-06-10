//! World memory (spec 02 §8) — public, cited knowledge fetched from Exa.
//!
//! Every outbound query goes through the egress firewall first: terms are
//! taint-checked against the private values currently in the brain, so
//! enrichment can never exfiltrate a private figure. Results land as
//! `world_fact` cards (`acl_tag = world/public`, default-readable, never
//! authority) and can be wired to the private cards they ground via
//! [`ground`] (`grounds` edges).

use anyhow::Result;
use chrono::Utc;

use crate::access::egress::{firewall, EgressTerm, Taint};
use crate::access::View;
use crate::brain::markdown::{render_card, slug, CardMeta};
use crate::brain::{BrainIndex, Link, LinkRel, Memory, MemoryKind};
use crate::config::Config;
use crate::connectors::exa::ExaConnector;
use crate::connectors::{AclTag, Connector, Cursor};
use crate::store::Store;

/// The acl tag for world cards: a public pseudo-view, readable by default.
pub fn world_acl() -> AclTag {
    AclTag {
        view: View::new("world", "public"),
        fields: vec![],
    }
}

/// Taint-tag the words of an outbound query against the brain's private
/// values: any term that equals a value of a private field currently in the
/// index is private-tainted (spec 03 §4). Comparison is case-insensitive and
/// strips `$,` formatting so `$245,000` still matches a stored `245000`.
pub fn taint_terms(index: &BrainIndex, query: &str) -> Vec<EgressTerm> {
    let private_values = private_field_values(index);
    query
        .split_whitespace()
        .map(|w| {
            let norm = normalize(w);
            match private_values
                .iter()
                .find(|(v, _)| *v == norm && !norm.is_empty())
            {
                Some((_, tag)) => EgressTerm {
                    term: w.to_string(),
                    taint: Taint::Private(tag.clone()),
                },
                None => EgressTerm::public(w),
            }
        })
        .collect()
}

/// Values of fields marked private by the connectors' view schemas
/// (finance-private: employee_salary, credits, discount_tier, …), paired with
/// the view they came from.
fn private_field_values(index: &BrainIndex) -> Vec<(String, String)> {
    const PRIVATE_FIELDS: &[&str] = &["employee_salary", "credits", "discount_tier"];
    let mut out = Vec::new();
    for e in &index.raw_events {
        if let Some(map) = e.payload.as_object() {
            for f in PRIVATE_FIELDS {
                if let Some(v) = map.get(*f) {
                    let s = normalize(&crate::brain::world::value_string(v));
                    if !s.is_empty() {
                        out.push((s, e.view.view.clone()));
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn value_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '$' | ',' | '"'))
        .collect::<String>()
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
}

/// Firewalled web search → world cards. Returns the (possibly empty) list of
/// new world memories. Offline (no `EXA_API_KEY`) this serves the on-host
/// cache — still real data from the last online run.
pub fn world_search(
    config: &Config,
    store: &Store,
    index: &mut BrainIndex,
    query: &str,
) -> Result<Vec<Memory>> {
    let terms = taint_terms(index, query);
    let allowed = firewall(&terms).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let clean_query = allowed.join(" ");

    let cache = config.fixtures_dir().join("exa-cache.json");
    let connector = ExaConnector::with_cache(clean_query, cache);
    let events = connector.pull(&Cursor::default())?;

    let mut new_memories = Vec::new();
    let now = Utc::now().to_rfc3339();
    for event in events {
        let url = event
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        // dedupe by source url
        if index
            .memories
            .iter()
            .any(|m| m.kind == MemoryKind::WorldFact && m.topic == url)
        {
            continue;
        }
        let title = event
            .payload
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("world fact");
        let snippet = event
            .payload
            .get("snippet")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let acl = world_acl();
        let meta = CardMeta {
            topic: "world",
            kind: "world_fact",
            period: None,
            confidence: 0.5,
            acl_tag: &acl,
        };
        let body = format!("{snippet}\n\nSource: <{url}>");
        let name = slug(title);
        let path = store.write_card("world", &name, &render_card(&meta, title, &body))?;

        let memory = Memory {
            id: uuid::Uuid::new_v4().to_string(),
            kind: MemoryKind::WorldFact,
            // topic doubles as the citation key (url) for dedupe + display
            topic: url,
            path: path.display().to_string(),
            acl_tag: acl,
            confidence: 0.5,
            period: None,
            supersedes: None,
            created_at: now.clone(),
        };
        index.raw_events.push(event);
        index.memories.push(memory.clone());
        new_memories.push(memory);
    }
    Ok(new_memories)
}

/// Wire a world card to the private card it grounds (spec 02 §8 `grounds`).
/// The edge carries no data — reading either side stays acl-checked — so a
/// world fact can annotate a private anomaly without widening anything.
pub fn ground(index: &mut BrainIndex, world_memory_id: &str, target_memory_id: &str) -> Result<()> {
    let is_world = index
        .memories
        .iter()
        .any(|m| m.id == world_memory_id && m.kind == MemoryKind::WorldFact);
    if !is_world {
        anyhow::bail!("'{world_memory_id}' is not a world_fact memory");
    }
    if !index.memories.iter().any(|m| m.id == target_memory_id) {
        anyhow::bail!("unknown target memory '{target_memory_id}'");
    }
    if index
        .links
        .iter()
        .any(|l| l.from == world_memory_id && l.to == target_memory_id && l.rel == LinkRel::Grounds)
    {
        return Ok(());
    }
    index.links.push(Link {
        from: world_memory_id.to_string(),
        to: target_memory_id.to_string(),
        rel: LinkRel::Grounds,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::RawEvent;
    use serde_json::json;

    fn index_with_private_value() -> BrainIndex {
        let mut index = BrainIndex::default();
        index.raw_events.push(RawEvent {
            id: "e1".into(),
            source_id: "stripe".into(),
            view: View::new("stripe", "finance_private"),
            payload: json!({
                "team": "eng", "period": "2026-05",
                "gross": 120000, "credits": 15000,
                "discount_tier": "tier-3", "employee_salary": 245000
            }),
            ingested_at: "2026-05-31T00:00:00Z".into(),
            acl_tag: AclTag {
                view: View::new("stripe", "finance_private"),
                fields: vec!["employee_salary".into()],
            },
        });
        index
    }

    #[test]
    fn private_values_are_tainted_and_blocked() {
        let index = index_with_private_value();
        // an agent trying to push the salary figure into a web query
        let terms = taint_terms(&index, "salary benchmark 245000 anthropic");
        assert!(terms
            .iter()
            .any(|t| matches!(t.taint, Taint::Private(_)) && t.term == "245000"));
        assert!(firewall(&terms).is_err(), "egress must be blocked");
        // formatting tricks don't help
        let dressed = taint_terms(&index, "comp at $245,000 today");
        assert!(firewall(&dressed).is_err());
        // the discount tier value is private too
        let tier = taint_terms(&index, "negotiated tier-3 pricing");
        assert!(firewall(&tier).is_err());
    }

    #[test]
    fn public_query_passes_taint_check() {
        let index = index_with_private_value();
        let terms = taint_terms(&index, "anthropic claude enterprise pricing");
        assert!(firewall(&terms).is_ok());
    }

    #[test]
    fn world_search_writes_default_readable_cards_offline() {
        let root = std::env::temp_dir().join(format!("world-test-{}", uuid::Uuid::new_v4()));
        let config = Config {
            root,
            inference: crate::config::InferenceBackend::Stub,
        };
        config.ensure_dirs().unwrap();
        let store = Store::new(config.clone());
        let mut index = index_with_private_value();

        // no EXA_API_KEY in tests → cache/seed path; still writes world cards
        let memories =
            world_search(&config, &store, &mut index, "anthropic claude pricing").unwrap();
        assert!(!memories.is_empty());
        assert!(memories.iter().all(|m| m.kind == MemoryKind::WorldFact));
        assert!(memories
            .iter()
            .all(|m| m.acl_tag.view.id() == "world/public"));

        // grounding wires an edge, idempotently
        let private_target = Memory {
            id: "priv-1".into(),
            kind: MemoryKind::Anomaly,
            topic: "finance".into(),
            path: "brain/finance/x.md".into(),
            acl_tag: AclTag {
                view: View::new("stripe", "finance_private"),
                fields: vec!["gross".into()],
            },
            confidence: 0.8,
            period: Some("2026-05".into()),
            supersedes: None,
            created_at: "2026-05-31T00:00:00Z".into(),
        };
        index.memories.push(private_target);
        ground(&mut index, &memories[0].id, "priv-1").unwrap();
        ground(&mut index, &memories[0].id, "priv-1").unwrap();
        assert_eq!(
            index
                .links
                .iter()
                .filter(|l| l.rel == LinkRel::Grounds)
                .count(),
            1
        );
        // a non-world memory cannot be the grounding source
        assert!(ground(&mut index, "priv-1", &memories[0].id).is_err());
    }

    #[test]
    fn world_search_blocks_private_query_entirely() {
        let root = std::env::temp_dir().join(format!("world-test-{}", uuid::Uuid::new_v4()));
        let config = Config {
            root,
            inference: crate::config::InferenceBackend::Stub,
        };
        config.ensure_dirs().unwrap();
        let store = Store::new(config.clone());
        let mut index = index_with_private_value();
        let err = world_search(&config, &store, &mut index, "why is 245000 above market");
        assert!(err.is_err());
        assert!(!err.unwrap_err().to_string().contains("245000"));
    }
}
