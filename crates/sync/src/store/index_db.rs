//! SQLite brain index (spec 02 §3, §6) — the real columnar/FTS layer.
//!
//! One file (`<CONTEXTFUL_HOME>/brain.db`) with the spec's tables
//! (`raw_event`, `memory`, `provenance`, `anomaly`, `learning`, `link`) plus
//! an FTS5 full-text index over the Markdown card bodies. The in-memory
//! working set stays [`BrainIndex`]; load/save are whole-set (the ingest
//! pipeline's replace semantics), FTS queries hit SQL directly. A legacy
//! `brain.index.json` is migrated on first open.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::brain::{Anomaly, BrainIndex, Learning, Link, Memory, Provenance};
use crate::connectors::RawEvent;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("opening brain db at {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS raw_event (
            id TEXT PRIMARY KEY, source_id TEXT NOT NULL, view TEXT NOT NULL,
            payload TEXT NOT NULL, ingested_at TEXT NOT NULL, acl_tag TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory (
            id TEXT PRIMARY KEY, kind TEXT NOT NULL, topic TEXT NOT NULL,
            path TEXT NOT NULL, acl_tag TEXT NOT NULL, confidence REAL NOT NULL,
            period TEXT, supersedes TEXT, created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS provenance (
            memory_id TEXT NOT NULL, raw_event_id TEXT NOT NULL,
            PRIMARY KEY (memory_id, raw_event_id)
        );
        CREATE TABLE IF NOT EXISTS anomaly (
            id TEXT PRIMARY KEY, view TEXT NOT NULL, metric TEXT NOT NULL,
            period TEXT NOT NULL, baseline REAL NOT NULL, observed REAL NOT NULL,
            severity REAL NOT NULL, acl_tag TEXT NOT NULL, memory_id TEXT
        );
        CREATE TABLE IF NOT EXISTS learning (
            id TEXT PRIMARY KEY, topic TEXT NOT NULL, statement TEXT NOT NULL,
            applies_from TEXT NOT NULL, acl_tag TEXT NOT NULL, provenance_id TEXT,
            source TEXT NOT NULL, suppresses_metric TEXT
        );
        CREATE TABLE IF NOT EXISTS link (
            from_id TEXT NOT NULL, to_id TEXT NOT NULL, rel TEXT NOT NULL,
            PRIMARY KEY (from_id, to_id, rel)
        );
        CREATE INDEX IF NOT EXISTS idx_raw_event_view ON raw_event(view);
        CREATE INDEX IF NOT EXISTS idx_memory_topic ON memory(topic);
        "#,
    )?;
    // FTS5 over card text (bundled sqlite ships FTS5)
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS card_fts USING fts5(memory_id UNINDEXED, topic, body);",
    )?;
    Ok(conn)
}

fn js<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serializable")
}

fn from_js<T: serde::de::DeserializeOwned>(s: String) -> Result<T> {
    serde_json::from_str(&s).context("decoding index row json")
}

/// Replace the whole persisted set with `index` (the ingest pipeline already
/// works replace-wise) and rebuild the FTS rows from the card files.
pub fn save(conn: &mut Connection, index: &BrainIndex) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch(
        "DELETE FROM raw_event; DELETE FROM memory; DELETE FROM provenance;
         DELETE FROM anomaly; DELETE FROM learning; DELETE FROM link;
         DELETE FROM card_fts;",
    )?;
    for e in &index.raw_events {
        tx.execute(
            "INSERT INTO raw_event (id, source_id, view, payload, ingested_at, acl_tag)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                e.id,
                e.source_id,
                e.view.id(),
                e.payload.to_string(),
                e.ingested_at,
                js(&e.acl_tag)
            ],
        )?;
    }
    for m in &index.memories {
        tx.execute(
            "INSERT INTO memory (id, kind, topic, path, acl_tag, confidence, period, supersedes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                m.id,
                js(&m.kind),
                m.topic,
                m.path,
                js(&m.acl_tag),
                m.confidence,
                m.period,
                m.supersedes,
                m.created_at
            ],
        )?;
        // card body → FTS (missing file = no FTS row, not an error)
        if let Ok(body) = std::fs::read_to_string(&m.path) {
            tx.execute(
                "INSERT INTO card_fts (memory_id, topic, body) VALUES (?1, ?2, ?3)",
                params![m.id, m.topic, body],
            )?;
        }
    }
    for p in &index.provenance {
        tx.execute(
            "INSERT OR IGNORE INTO provenance (memory_id, raw_event_id) VALUES (?1, ?2)",
            params![p.memory_id, p.raw_event_id],
        )?;
    }
    for a in &index.anomalies {
        tx.execute(
            "INSERT INTO anomaly (id, view, metric, period, baseline, observed, severity, acl_tag, memory_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                a.id, a.view, a.metric, a.period, a.baseline, a.observed, a.severity,
                js(&a.acl_tag), a.memory_id
            ],
        )?;
    }
    for l in &index.learnings {
        tx.execute(
            "INSERT INTO learning (id, topic, statement, applies_from, acl_tag, provenance_id, source, suppresses_metric)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                l.id, l.topic, l.statement, l.applies_from, js(&l.acl_tag),
                l.provenance_id, l.source, l.suppresses_metric
            ],
        )?;
    }
    for l in &index.links {
        tx.execute(
            "INSERT OR IGNORE INTO link (from_id, to_id, rel) VALUES (?1, ?2, ?3)",
            params![l.from, l.to, js(&l.rel)],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn load(conn: &Connection) -> Result<BrainIndex> {
    let mut index = BrainIndex::default();

    let mut stmt = conn.prepare(
        "SELECT id, source_id, payload, ingested_at, acl_tag FROM raw_event ORDER BY rowid",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (id, source_id, payload, ingested_at, acl_tag) = row?;
        let acl_tag: crate::connectors::AclTag = from_js(acl_tag)?;
        index.raw_events.push(RawEvent {
            id,
            source_id,
            view: acl_tag.view.clone(),
            payload: serde_json::from_str(&payload).context("raw_event payload")?,
            ingested_at,
            acl_tag,
        });
    }

    let mut stmt = conn.prepare(
        "SELECT id, kind, topic, path, acl_tag, confidence, period, supersedes, created_at
         FROM memory ORDER BY rowid",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, f64>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, String>(8)?,
        ))
    })?;
    for row in rows {
        let (id, kind, topic, path, acl_tag, confidence, period, supersedes, created_at) = row?;
        index.memories.push(Memory {
            id,
            kind: from_js(kind)?,
            topic,
            path,
            acl_tag: from_js(acl_tag)?,
            confidence: confidence as f32,
            period,
            supersedes,
            created_at,
        });
    }

    let mut stmt = conn.prepare("SELECT memory_id, raw_event_id FROM provenance")?;
    let rows = stmt.query_map([], |r| {
        Ok(Provenance {
            memory_id: r.get(0)?,
            raw_event_id: r.get(1)?,
        })
    })?;
    for row in rows {
        index.provenance.push(row?);
    }

    let mut stmt = conn.prepare(
        "SELECT id, view, metric, period, baseline, observed, severity, acl_tag, memory_id
         FROM anomaly ORDER BY rowid",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, f64>(4)?,
            r.get::<_, f64>(5)?,
            r.get::<_, f64>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, Option<String>>(8)?,
        ))
    })?;
    for row in rows {
        let (id, view, metric, period, baseline, observed, severity, acl_tag, memory_id) = row?;
        index.anomalies.push(Anomaly {
            id,
            view,
            metric,
            period,
            baseline,
            observed,
            severity,
            acl_tag: from_js(acl_tag)?,
            memory_id,
        });
    }

    let mut stmt = conn.prepare(
        "SELECT id, topic, statement, applies_from, acl_tag, provenance_id, source, suppresses_metric
         FROM learning ORDER BY rowid",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    for row in rows {
        let (id, topic, statement, applies_from, acl_tag, provenance_id, source, suppresses) = row?;
        index.learnings.push(Learning {
            id,
            topic,
            statement,
            applies_from,
            acl_tag: from_js(acl_tag)?,
            provenance_id,
            source,
            suppresses_metric: suppresses,
        });
    }

    let mut stmt = conn.prepare("SELECT from_id, to_id, rel FROM link")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (from, to, rel) = row?;
        index.links.push(Link {
            from,
            to,
            rel: from_js(rel)?,
        });
    }

    Ok(index)
}

/// FTS5 search over card bodies → matching memory ids (ranked).
pub fn search_cards(conn: &Connection, query: &str) -> Result<Vec<String>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    // quote each token so user input can't inject FTS syntax
    let match_expr = q
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");
    let mut stmt =
        conn.prepare("SELECT memory_id FROM card_fts WHERE card_fts MATCH ?1 ORDER BY rank")?;
    let rows = stmt.query_map([match_expr], |r| r.get::<_, String>(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::View;
    use crate::brain::MemoryKind;
    use crate::connectors::AclTag;

    fn temp_db() -> (Connection, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("braindb-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("brain.db");
        (open(&path).unwrap(), dir)
    }

    #[test]
    fn roundtrip_and_fts() {
        let (mut conn, dir) = temp_db();
        // a card on disk so FTS has a body to index
        let card = dir.join("spend.md");
        std::fs::write(&card, "Claude token spend spiked in May; one-off backfill.").unwrap();

        let mut index = BrainIndex::default();
        index.memories.push(Memory {
            id: "m1".into(),
            kind: MemoryKind::Wiki,
            topic: "spend".into(),
            path: card.display().to_string(),
            acl_tag: AclTag {
                view: View::new("stripe", "spend_by_team"),
                fields: vec!["gross".into()],
            },
            confidence: 0.8,
            period: Some("2026-05".into()),
            supersedes: None,
            created_at: "2026-05-31T00:00:00Z".into(),
        });
        index.links.push(Link {
            from: "m1".into(),
            to: "m2".into(),
            rel: crate::brain::LinkRel::RelatesTo,
        });
        save(&mut conn, &index).unwrap();

        let loaded = load(&conn).unwrap();
        assert_eq!(loaded.memories.len(), 1);
        assert_eq!(loaded.memories[0].id, "m1");
        assert_eq!(loaded.links.len(), 1);

        let hits = search_cards(&conn, "backfill").unwrap();
        assert_eq!(hits, vec!["m1".to_string()]);
        assert!(search_cards(&conn, "nonexistent-term-xyz")
            .unwrap()
            .is_empty());
        // FTS syntax can't be injected
        assert!(search_cards(&conn, "\"unterminated").is_ok());
    }
}
