//! The daydream loop (spec 02 §9) — background, cron-scheduled connection
//! finding (GBrain dream-cycle / Gwern DDL).
//!
//! Sample → generate → critic → write:
//!
//! 1. **Sample** acl-admissible card pairs. A pair is admissible only if at
//!    least one seeded principal's token can read BOTH cards — daydreaming
//!    never combines context no single principal is cleared to see (the
//!    salary invariant survives the dream, spec 09 Flow G). Graph-adjacent
//!    pairs (shared link) are preferred.
//! 2. **Generate** a candidate connection with the configured inference
//!    backend (offline this is the deterministic stub — still a real pass).
//! 3. **Critic**: drop empty output and pairs already connected by an
//!    existing daydream card.
//! 4. **Write** a `daydream` hypothesis card, `confidence` low, acl =
//!    max(parents) (taint propagation), wired to both parents.

use anyhow::Result;

use crate::agent::inference;
use crate::brain::markdown::slug;
use crate::brain::{retrieval, BrainIndex, Link, LinkRel, Memory, MemoryKind};
use crate::config::Config;
use crate::scenario;
use crate::store::Store;

/// One nightly cycle over the persisted brain. Returns a short report line.
pub fn run_once() -> Result<String> {
    let config = Config::load();
    let store = Store::new(config.clone());
    let mut index = store.load_index()?;
    let written = cycle(&config, &store, &mut index, 3)?;
    store.save_index(&index)?;
    Ok(format!("daydream: {written} insight card(s) written"))
}

/// The principals whose clearance defines acl-admissibility. Verified tokens
/// from the control plane when present (production), else the scenario's
/// initial capabilities (tests / cold start).
fn principal_caps(config: &Config) -> Vec<crate::access::Capability> {
    let mut caps = Vec::new();
    for p in scenario::principals() {
        if let Some(cap) = crate::controlplane::load_capability(config, p.id())
            .or_else(|| scenario::initial_capability(p.id()))
        {
            caps.push(cap);
        }
    }
    caps
}

/// Can any single principal read both cards?
fn admissible(caps: &[crate::access::Capability], a: &Memory, b: &Memory) -> bool {
    caps.iter().any(|cap| {
        retrieval::card_readable(cap, &a.acl_tag) && retrieval::card_readable(cap, &b.acl_tag)
    })
}

fn already_dreamed(index: &BrainIndex, a: &Memory, b: &Memory) -> bool {
    // by card file: the hypothesis card for a topic pair has a deterministic
    // slug, so this survives re-synthesis regenerating the parents' ids
    let slugs = [
        format!("{}.md", slug(&format!("dd-{}-{}", a.topic, b.topic))),
        format!("{}.md", slug(&format!("dd-{}-{}", b.topic, a.topic))),
    ];
    let by_path = index
        .memories
        .iter()
        .filter(|m| m.kind == MemoryKind::Daydream)
        .any(|d| slugs.iter().any(|s| d.path.ends_with(s.as_str())));

    by_path
        || index
            .memories
            .iter()
            .filter(|m| m.kind == MemoryKind::Daydream)
            .any(|d| {
                let touches = |x: &Memory| {
                    index
                        .links
                        .iter()
                        .any(|l| l.from == d.id && l.to == x.id && l.rel == LinkRel::RelatesTo)
                };
                touches(a) && touches(b)
            })
}

/// Sample admissible pairs: graph-adjacent first, then cross-topic, capped.
/// Returns owned clones so the caller can mutate the index while writing.
fn sample_pairs(
    caps: &[crate::access::Capability],
    index: &BrainIndex,
    max: usize,
) -> Vec<(Memory, Memory)> {
    let candidates: Vec<&Memory> = index
        .memories
        .iter()
        .filter(|m| m.kind != MemoryKind::Daydream)
        .collect();
    let mut pairs: Vec<(Memory, Memory)> = Vec::new();
    let linked = |a: &Memory, b: &Memory| {
        index
            .links
            .iter()
            .any(|l| (l.from == a.id && l.to == b.id) || (l.from == b.id && l.to == a.id))
    };
    let push = |a: &Memory, b: &Memory, pairs: &mut Vec<(Memory, Memory)>| {
        if pairs.len() < max
            && a.id != b.id
            && admissible(caps, a, b)
            && !already_dreamed(index, a, b)
            && !pairs
                .iter()
                .any(|(x, y)| (x.id == a.id && y.id == b.id) || (x.id == b.id && y.id == a.id))
        {
            pairs.push((a.clone(), b.clone()));
        }
    };
    // pass 1: graph-adjacent
    for (i, a) in candidates.iter().enumerate() {
        for b in candidates.iter().skip(i + 1) {
            if linked(a, b) {
                push(a, b, &mut pairs);
            }
        }
    }
    // pass 2: anything admissible (different topics first for non-obviousness)
    for (i, a) in candidates.iter().enumerate() {
        for b in candidates.iter().skip(i + 1) {
            if a.topic != b.topic {
                push(a, b, &mut pairs);
            }
        }
    }
    pairs
}

/// Run one sample→generate→critic→write cycle. Returns cards written.
pub fn cycle(
    config: &Config,
    store: &Store,
    index: &mut BrainIndex,
    max_insights: usize,
) -> Result<usize> {
    let caps = principal_caps(config);
    let llm = inference::from_config(config.inference);
    let pairs = sample_pairs(&caps, index, max_insights);

    let mut written = 0;
    for (a, b) in pairs {
        let body_a = store.read_card(&a.path).unwrap_or_default();
        let body_b = store.read_card(&b.path).unwrap_or_default();
        let prompt = format!(
            "You are the company brain daydreaming overnight. Two memory cards:\n\n\
             --- CARD A ({}) ---\n{}\n--- CARD B ({}) ---\n{}\n---\n\
             Propose ONE non-obvious, useful connection between them in 1-2 \
             sentences. If there is none, answer exactly: none.",
            a.topic, body_a, b.topic, body_b
        );
        let insight = match llm.complete(&prompt) {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(error = %e, "daydream generation failed — skipping pair");
                continue;
            }
        };
        // critic: empty / explicit none / trivial echoes are dropped
        let trimmed = insight.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            continue;
        }

        // taint = max(parents): the insight needs BOTH parents' clearance
        let acl = a.acl_tag.max(&b.acl_tag);
        let title = format!("Hypothesis: {} ↔ {}", a.topic, b.topic);
        // topic markers, not ids: re-synthesis regenerates parent ids, and
        // self-wiring re-resolves topics to the newest matching card
        let body = format!(
            "{trimmed}\n\nrelates_to::{}\nrelates_to::{}\n",
            a.topic, b.topic
        );
        let name = slug(&format!("dd-{}-{}", a.topic, b.topic));
        let memory = crate::brain::write_memory(
            store,
            index,
            crate::brain::CardWrite {
                kind: MemoryKind::Daydream,
                topic: "daydream",
                index_topic: None,
                slug: &name,
                title: &title,
                body: &body,
                confidence: 0.3,
                acl_tag: acl,
            },
        )?;
        for parent in [&a.id, &b.id] {
            index.links.push(Link {
                from: memory.id.clone(),
                to: (*parent).clone(),
                rel: LinkRel::RelatesTo,
            });
        }
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::View;
    use crate::connectors::AclTag;

    fn config() -> Config {
        Config {
            root: std::env::temp_dir().join(format!("dd-test-{}", uuid::Uuid::new_v4())),
            inference: crate::config::InferenceBackend::Stub,
        }
    }

    fn mem(id: &str, topic: &str, path: &str, view: View, fields: &[&str]) -> Memory {
        Memory {
            id: id.into(),
            kind: MemoryKind::Wiki,
            topic: topic.into(),
            path: path.into(),
            acl_tag: AclTag {
                view,
                fields: fields.iter().map(|s| s.to_string()).collect(),
            },
            confidence: 0.5,
            period: None,
            supersedes: None,
            created_at: "2026-06-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn daydream_connects_admissible_pair_with_max_taint() {
        let config = config();
        config.ensure_dirs().unwrap();
        let store = Store::new(config.clone());
        let p1 = store
            .write_card("spend", "a", "Spend spiked in May.")
            .unwrap();
        let p2 = store
            .write_card("world", "b", "Public: discounts expire quarterly.")
            .unwrap();

        let mut index = BrainIndex::default();
        index.memories.push(mem(
            "a",
            "spend",
            &p1.display().to_string(),
            View::new("stripe", "spend_by_team"),
            &["gross"],
        ));
        index.memories.push(mem(
            "b",
            "world",
            &p2.display().to_string(),
            View::new("world", "public"),
            &[],
        ));

        let written = cycle(&config, &store, &mut index, 3).unwrap();
        assert_eq!(written, 1);
        let dd = index
            .memories
            .iter()
            .find(|m| m.kind == MemoryKind::Daydream)
            .unwrap();
        // taint = max(parents): requires the private parent's clearance
        assert_eq!(dd.acl_tag.view.id(), "stripe/spend_by_team");
        assert!(dd.acl_tag.fields.contains(&"gross".to_string()));
        // wired to both parents
        assert_eq!(
            index
                .links
                .iter()
                .filter(|l| l.from == dd.id && l.rel == LinkRel::RelatesTo)
                .count(),
            2
        );
        // a second cycle does not re-dream the same pair
        assert_eq!(cycle(&config, &store, &mut index, 3).unwrap(), 0);
    }

    #[test]
    fn flow_g_property_never_pairs_cards_no_principal_can_read_together() {
        let config = config();
        config.ensure_dirs().unwrap();
        let store = Store::new(config.clone());
        let p1 = store.write_card("finance", "a", "salary table").unwrap();
        let p2 = store.write_card("hr", "b", "review notes").unwrap();

        let mut index = BrainIndex::default();
        // finance_private salary card: only the CFO can read it
        index.memories.push(mem(
            "a",
            "finance",
            &p1.display().to_string(),
            View::new("stripe", "finance_private"),
            &["employee_salary"],
        ));
        // a view NO seeded principal holds at all
        index.memories.push(mem(
            "b",
            "hr",
            &p2.display().to_string(),
            View::new("hr", "reviews"),
            &["review"],
        ));

        let caps = principal_caps(&config);
        assert!(!admissible(&caps, &index.memories[0], &index.memories[1]));
        let written = cycle(&config, &store, &mut index, 3).unwrap();
        assert_eq!(written, 0, "no principal holds both — must never pair");
    }
}
