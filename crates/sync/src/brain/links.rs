//! Self-wiring links (spec 02 §1, §3) — GBrain-style, zero LLM calls.
//!
//! Scans card bodies for `[[topic]]` wikilinks and `relates_to::<id-or-topic>`
//! / `supersedes::<id-or-topic>` markers, resolving each to the newest memory
//! with that id or topic and writing typed [`Link`] rows. Runs after every
//! synthesis pass; idempotent.

use crate::brain::{BrainIndex, Link, LinkRel};
use crate::store::Store;

/// Extract `[[target]]` wikilinks and `rel::target` markers from a card body.
fn extract(body: &str) -> Vec<(LinkRel, String)> {
    let mut out = Vec::new();
    // [[topic]]
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let target = after[..end].trim();
            if !target.is_empty() {
                out.push((LinkRel::RelatesTo, target.to_string()));
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    // rel::target markers, one per line
    for line in body.lines() {
        let line = line.trim();
        for (marker, rel) in [
            ("relates_to::", LinkRel::RelatesTo),
            ("supersedes::", LinkRel::Supersedes),
            ("grounds::", LinkRel::Grounds),
        ] {
            if let Some(target) = line.strip_prefix(marker) {
                let target = target.trim();
                if !target.is_empty() {
                    out.push((rel, target.to_string()));
                }
            }
        }
    }
    out
}

/// Resolve a link target (memory id or topic) to a memory id.
fn resolve(index: &BrainIndex, source_id: &str, target: &str) -> Option<String> {
    if index.memories.iter().any(|m| m.id == target) {
        return Some(target.to_string());
    }
    index
        .memories
        .iter()
        .filter(|m| m.topic == target && m.id != source_id)
        .max_by(|a, b| a.created_at.cmp(&b.created_at))
        .map(|m| m.id.clone())
}

/// Wire every card's outgoing links. Returns how many new edges were added.
pub fn self_wire(store: &Store, index: &mut BrainIndex) -> usize {
    let mut new_links: Vec<Link> = Vec::new();
    for m in &index.memories {
        let Ok(body) = store.read_card(&m.path) else {
            continue;
        };
        for (rel, target) in extract(&body) {
            let Some(to) = resolve(index, &m.id, &target) else {
                continue; // a dangling [[link]] marks future work, not an error
            };
            if to == m.id {
                continue;
            }
            let exists = index
                .links
                .iter()
                .chain(new_links.iter())
                .any(|l| l.from == m.id && l.to == to && l.rel == rel);
            if !exists {
                new_links.push(Link {
                    from: m.id.clone(),
                    to,
                    rel,
                });
            }
        }
    }
    let n = new_links.len();
    index.links.extend(new_links);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::View;
    use crate::brain::{Memory, MemoryKind};
    use crate::config::Config;
    use crate::connectors::AclTag;

    fn mem(id: &str, topic: &str, path: &str) -> Memory {
        Memory {
            id: id.into(),
            kind: MemoryKind::Wiki,
            topic: topic.into(),
            path: path.into(),
            acl_tag: AclTag {
                view: View::new("stripe", "spend_by_team"),
                fields: vec![],
            },
            confidence: 0.5,
            period: None,
            supersedes: None,
            created_at: format!("2026-06-0{}T00:00:00Z", id.len()),
        }
    }

    #[test]
    fn wikilinks_and_markers_self_wire_idempotently() {
        let root = std::env::temp_dir().join(format!("links-test-{}", uuid::Uuid::new_v4()));
        let config = Config {
            root,
            inference: crate::config::InferenceBackend::Stub,
        };
        config.ensure_dirs().unwrap();
        let store = Store::new(config);

        let p1 = store
            .write_card("spend", "may", "May spend relates to [[finance]].\n")
            .unwrap();
        let p2 = store
            .write_card("finance", "may", "Quiet month.\nsupersedes::spend\n")
            .unwrap();

        let mut index = BrainIndex::default();
        index
            .memories
            .push(mem("a", "spend", &p1.display().to_string()));
        index
            .memories
            .push(mem("bb", "finance", &p2.display().to_string()));

        let added = self_wire(&store, &mut index);
        assert_eq!(added, 2);
        assert!(index
            .links
            .iter()
            .any(|l| l.from == "a" && l.to == "bb" && l.rel == LinkRel::RelatesTo));
        assert!(index
            .links
            .iter()
            .any(|l| l.from == "bb" && l.to == "a" && l.rel == LinkRel::Supersedes));
        // idempotent
        assert_eq!(self_wire(&store, &mut index), 0);
    }

    #[test]
    fn dangling_links_are_not_errors() {
        let root = std::env::temp_dir().join(format!("links-test-{}", uuid::Uuid::new_v4()));
        let config = Config {
            root,
            inference: crate::config::InferenceBackend::Stub,
        };
        config.ensure_dirs().unwrap();
        let store = Store::new(config);
        let p = store
            .write_card("spend", "x", "See [[not-written-yet]].")
            .unwrap();
        let mut index = BrainIndex::default();
        index
            .memories
            .push(mem("a", "spend", &p.display().to_string()));
        assert_eq!(self_wire(&store, &mut index), 0);
    }
}
