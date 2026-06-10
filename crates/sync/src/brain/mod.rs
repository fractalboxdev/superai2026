//! Brain & memory data model (spec 02 §3).
//!
//! Human-readable Markdown cards are the source of truth for synthesized memory
//! (`brain/<topic>/*.md`); the structures here are the *index over* those files
//! plus raw/derived rows, persisted in the SQLite + FTS5 index
//! ([`crate::store::index_db`]).

pub mod daydream;
pub mod links;
pub mod markdown;
pub mod mcp;
pub mod retrieval;
pub mod synthesis;
pub mod world;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::connectors::{AclTag, RawEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Wiki,
    Anomaly,
    Learning,
    /// Public, cited world knowledge (Exa) — default-readable, never authority
    /// (spec 02 §8).
    WorldFact,
    /// A daydream-loop hypothesis card (spec 02 §9).
    Daydream,
}

impl MemoryKind {
    /// The frontmatter `kind:` string (matches the serde snake_case name).
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryKind::Wiki => "wiki",
            MemoryKind::Anomaly => "anomaly",
            MemoryKind::Learning => "learning",
            MemoryKind::WorldFact => "world_fact",
            MemoryKind::Daydream => "daydream",
        }
    }
}

/// Memory tier (icarus-style; spec 02 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Working,
    Archive,
    Wiki,
}

/// Index row for a synthesized Markdown card. `path` points at the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub kind: MemoryKind,
    pub topic: String,
    pub path: String,
    pub acl_tag: AclTag,
    pub confidence: f32,
    pub period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub memory_id: String,
    pub raw_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub id: String,
    pub view: String,
    pub metric: String,
    pub period: String,
    pub baseline: f64,
    pub observed: f64,
    pub severity: f64,
    pub acl_tag: AclTag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
}

/// A human correction that biases future synthesis and suppresses re-flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Learning {
    pub id: String,
    pub topic: String,
    pub statement: String,
    pub applies_from: String,
    pub acl_tag: AclTag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_id: Option<String>,
    pub source: String,
    /// the anomaly metric this learning suppresses (e.g. "spend_by_team:gross").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppresses_metric: Option<String>,
}

/// Typed graph edge between memories (spec 02 §3 `link`): self-wired
/// wikilinks, supersede chains, and world-memory grounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkRel {
    RelatesTo,
    Supersedes,
    /// world card → private card it grounds (spec 02 §8).
    Grounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub rel: LinkRel,
}

/// The in-memory brain index (spec 02 §3), loaded from / saved to the SQLite
/// store ([`crate::store::index_db`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrainIndex {
    #[serde(default)]
    pub raw_events: Vec<RawEvent>,
    #[serde(default)]
    pub memories: Vec<Memory>,
    #[serde(default)]
    pub provenance: Vec<Provenance>,
    #[serde(default)]
    pub anomalies: Vec<Anomaly>,
    #[serde(default)]
    pub learnings: Vec<Learning>,
    #[serde(default)]
    pub links: Vec<Link>,
}

impl BrainIndex {
    pub fn events_for_view(&self, view_id: &str) -> Vec<&RawEvent> {
        self.raw_events
            .iter()
            .filter(|e| e.view.id() == view_id)
            .collect()
    }
}

/// One synthesized card to persist + index — see [`write_memory`].
pub(crate) struct CardWrite<'a> {
    pub kind: MemoryKind,
    /// frontmatter topic; also the directory the card file lands in.
    pub topic: &'a str,
    /// index topic when it differs (world cards use the citation url).
    pub index_topic: Option<String>,
    pub slug: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub confidence: f32,
    pub acl_tag: AclTag,
}

/// Render + persist a Markdown card and register its index row — the one
/// write path shared by `brain.remember`, world facts, and the daydream loop.
pub(crate) fn write_memory(
    store: &crate::store::Store,
    index: &mut BrainIndex,
    card: CardWrite,
) -> anyhow::Result<Memory> {
    let meta = markdown::CardMeta {
        topic: card.topic,
        kind: card.kind.as_str(),
        period: None,
        confidence: card.confidence,
        acl_tag: &card.acl_tag,
    };
    let rendered = markdown::render_card(&meta, card.title, card.body);
    let path = store.write_card(card.topic, card.slug, &rendered)?;
    let memory = Memory {
        id: uuid::Uuid::new_v4().to_string(),
        kind: card.kind,
        topic: card.index_topic.unwrap_or_else(|| card.topic.to_string()),
        path: path.display().to_string(),
        acl_tag: card.acl_tag,
        confidence: card.confidence,
        period: None,
        supersedes: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    index.memories.push(memory.clone());
    Ok(memory)
}
