//! Editor agent: watch a relay document, answer questions from the brain
//! (spec 04 §1 over the spec 01 wire).
//!
//! A real Loro peer (the relay still never parses CRDT bytes): it subscribes to
//! one doc, merges snapshots/updates into a local `LoroDoc`, and when the doc
//! settles it scans the Weaver block tree (`content` LoroTree, one paragraph
//! per node with a `text` container — the same shape `@weaver/core` edits) for
//! `Q:` paragraphs that no `A (…)` paragraph answers yet. Each question is
//! matched against the brain's Markdown memory (`~/.contextful/brain/**`),
//! authorized against the agent's capability token — all-or-nothing per card,
//! same rule as `brain.get_context` — and the answer is inserted as a new
//! paragraph below the question, shipped back as a normal Loro update.
//! Answers ride the CRDT, never awareness (presence invariant).

use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use loro::{LoroDoc, LoroMap, LoroText};
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message;

use crate::access::Capability;
use crate::brain::retrieval::card_readable;
use crate::brain::{BrainIndex, Memory};
use crate::config::Config;
use crate::controlplane::load_capability;
use crate::scenario;
use crate::store::Store;
use crate::sync::presence::{PresenceMode, PresenceState};
use crate::sync::protocol::SyncMessage;

/// Quiet period after the last remote edit before we scan for questions, so we
/// answer a finished line, not one mid-keystroke.
const SETTLE_MS: u64 = 900;

/// The Weaver editor's block tree container (`@weaver/core` TREE_NAME).
const TREE_NAME: &str = "content";

/// A `Q:` paragraph with no `A (…)` reply yet. `insert_index` is the root-level
/// block index directly below the question.
#[derive(Debug, PartialEq, Eq)]
pub struct PendingQuestion {
    pub question: String,
    pub insert_index: usize,
}

/// Ordered top-level block texts of a Weaver doc (one entry per block node).
pub fn read_blocks(doc: &LoroDoc) -> Vec<String> {
    let tree = doc.get_tree(TREE_NAME);
    tree.children(None)
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            tree.get_meta(id)
                .ok()
                .and_then(|meta| meta.get("text"))
                .and_then(|v| v.as_container().cloned())
                .and_then(|c| c.into_text().ok())
                .map(|t| t.to_string())
                .unwrap_or_default()
        })
        .collect()
}

/// Scan block texts for unanswered questions. A question is a block whose
/// trimmed text starts with `Q:`; it counts as answered when the next
/// non-empty block starts with `A:` or `A (`.
pub fn find_unanswered(blocks: &[String]) -> Vec<PendingQuestion> {
    blocks
        .iter()
        .enumerate()
        .filter_map(|(i, block)| {
            let question = block.trim_start().strip_prefix("Q:")?.trim();
            if question.is_empty() {
                return None;
            }
            let answered = blocks
                .iter()
                .skip(i + 1)
                .map(|b| b.trim_start())
                .find(|b| !b.is_empty())
                .is_some_and(|b| b.starts_with("A:") || b.starts_with("A ("));
            (!answered).then(|| PendingQuestion {
                question: question.to_string(),
                insert_index: i + 1,
            })
        })
        .collect()
}

/// Insert a paragraph block at a root-level index — the same node shape
/// `@weaver/core` creates (`kind` / `attrs` / `text` keys), so the web editor
/// renders it like any human paragraph.
pub fn insert_paragraph(doc: &LoroDoc, index: usize, content: &str) -> Result<()> {
    let tree = doc.get_tree(TREE_NAME);
    let err = |e: loro::LoroError| anyhow::anyhow!("loro tree write: {e}");
    let index = index.min(tree.children_num(None).unwrap_or(0));
    let node = tree.create_at(None, index).map_err(err)?;
    let meta = tree.get_meta(node).map_err(err)?;
    meta.insert("kind", "paragraph").map_err(err)?;
    meta.insert_container("attrs", LoroMap::new())
        .map_err(err)?;
    let text = meta
        .insert_container("text", LoroText::new())
        .map_err(err)?;
    text.insert(0, content).map_err(err)?;
    Ok(())
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "what", "whats", "how", "our", "are", "this", "that", "into",
    "from", "about", "tell", "please",
];

fn tokens(question: &str) -> Vec<String> {
    question
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOPWORDS.contains(t))
        .map(String::from)
        .collect()
}

/// Keyword score of a memory card against the question: filename stems carry
/// the synthesized subject (e.g. `unit-economics-compression-2026-05`), topics
/// the coarse grouping.
fn match_score(toks: &[String], m: &Memory) -> usize {
    let stem = std::path::Path::new(&m.path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let topic = m.topic.to_lowercase();
    toks.iter()
        .map(|t| {
            if stem.contains(t.as_str()) {
                2
            } else if topic.contains(t.as_str()) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// Answer one question from memory, capability-filtered. The best-matching card
/// is authorized all-or-nothing against its `acl_tag` (the `brain.get_context`
/// rule) — a match the token can't read returns a denial, not the content.
pub fn answer_for(store: &Store, index: &BrainIndex, cap: &Capability, question: &str) -> String {
    let toks = tokens(question);
    let best = index
        .memories
        .iter()
        .map(|m| (match_score(&toks, m), m))
        .max_by_key(|(s, _)| *s);
    match best {
        Some((score, m)) if score >= 2 => {
            if !card_readable(cap, &m.acl_tag) {
                return format!(
                    "Denied — the matching memory requires {} on {}, which this token does not \
                     grant. Raise an access request.",
                    m.acl_tag.fields.join(", "),
                    m.acl_tag.view.id()
                );
            }
            match store.read_card(&m.path) {
                Ok(card) => compose_answer(&card, m),
                Err(e) => format!("matched memory at {} is unreadable: {e}", m.path),
            }
        }
        _ => "No memory in the brain matches this question yet — ingest more sources or rephrase."
            .to_string(),
    }
}

/// Flatten a Markdown card (frontmatter + `# title` + body) into one answer
/// line with source + acl attribution.
fn compose_answer(card: &str, m: &Memory) -> String {
    let mut lines = card.lines().peekable();
    if lines.peek() == Some(&"---") {
        lines.next();
        for l in lines.by_ref() {
            if l == "---" {
                break;
            }
        }
    }
    let mut title = String::new();
    let mut body = Vec::new();
    for l in lines {
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        if title.is_empty() {
            if let Some(h) = t.strip_prefix("# ") {
                title = h.to_string();
                continue;
            }
        }
        body.push(t);
    }
    let source = m
        .path
        .split_once("/brain/")
        .map(|(_, rel)| format!("brain/{rel}"))
        .unwrap_or_else(|| m.path.clone());
    let head = if title.is_empty() {
        String::new()
    } else {
        format!("{title} — ")
    };
    format!(
        "{head}{} [source: {source} · acl {}]",
        body.join(" "),
        m.acl_tag.view.id()
    )
}

/// Loopback dial address for a relay bind address. A co-hosted agent dials its
/// own relay, so wildcard binds (`0.0.0.0`, `[::]`) become `127.0.0.1`.
pub fn dial_addr(bind_addr: &str) -> String {
    match bind_addr.rsplit_once(':') {
        Some(("0.0.0.0" | "[::]" | "::" | "", port)) => format!("127.0.0.1:{port}"),
        _ => bind_addr.to_string(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Run the editor agent against a relay doc until Ctrl-C.
pub async fn watch(addr: &str, doc_id: &str, principal: &str) -> Result<()> {
    let config = Config::load();
    let store = Store::new(config.clone());
    let index = store.load_index()?;
    let cap = load_capability(&config, principal)
        .or_else(|| scenario::initial_capability(principal))
        .ok_or_else(|| anyhow::anyhow!("no capability for principal '{principal}'"))?;

    let url = format!("ws://{addr}/");
    let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws.split();
    tracing::info!(%url, %principal, doc = %doc_id, "editor agent connected");

    write
        .send(Message::Text(
            SyncMessage::Hello {
                proto: "contextful/1".into(),
                principal: principal.into(),
                biscuit: None,
            }
            .to_json(),
        ))
        .await?;
    write
        .send(Message::Text(
            SyncMessage::Subscribe {
                doc_id: doc_id.into(),
                client_vv: None,
            }
            .to_json(),
        ))
        .await?;

    let ldoc = LoroDoc::new();
    let mut answered: HashSet<String> = HashSet::new();
    let mut dirty_since: Option<Instant> = None;
    let mut scan = interval(Duration::from_millis(250));
    let mut heartbeat = interval(Duration::from_secs(5));

    let presence = |mode: PresenceMode| SyncMessage::Awareness {
        doc_id: doc_id.to_string(),
        presence: PresenceState {
            principal: principal.into(),
            display_name: format!("{principal} · editor agent"),
            mode,
            session: None,
            cursor_block: None,
            cursor_anchor: None,
            selection_end: None,
            heartbeat: now_ms(),
        },
    };

    loop {
        tokio::select! {
            frame = read.next() => {
                let Some(frame) = frame else { break };
                match frame? {
                    Message::Text(text) => match serde_json::from_str::<SyncMessage>(&text) {
                        Ok(SyncMessage::Snapshot { doc_id: d, bytes })
                        | Ok(SyncMessage::Update { doc_id: d, bytes })
                            if d == doc_id && !bytes.is_empty() =>
                        {
                            match ldoc.import(&bytes) {
                                Ok(_) => dirty_since = Some(Instant::now()),
                                Err(e) => tracing::warn!(error = %e, "import failed"),
                            }
                        }
                        Ok(SyncMessage::Error { code, message }) => {
                            tracing::error!(%code, %message, "relay rejected us");
                            break;
                        }
                        _ => {}
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = scan.tick() => {
                let settled = dirty_since
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(SETTLE_MS));
                if !settled {
                    continue;
                }
                dirty_since = None;

                let blocks = read_blocks(&ldoc);
                let mut pending: Vec<PendingQuestion> = find_unanswered(&blocks)
                    .into_iter()
                    .filter(|q| !answered.contains(&q.question))
                    .collect();
                if pending.is_empty() {
                    continue;
                }
                write.send(Message::Text(presence(PresenceMode::Writing).to_json())).await?;

                // bottom-up so earlier insert indices stay valid
                pending.sort_by_key(|q| std::cmp::Reverse(q.insert_index));
                for q in &pending {
                    let answer = answer_for(&store, &index, &cap, &q.question);
                    tracing::info!(question = %q.question, "answering from brain");
                    let block = format!("A ({principal} · from brain memory): {answer}");
                    insert_paragraph(&ldoc, q.insert_index, &block)?;
                    answered.insert(q.question.clone());
                }
                ldoc.commit();
                // ship the full update log (same as the web peer) so the relay's
                // overwrite-persistence stays a complete snapshot
                let bytes = ldoc
                    .export(loro::ExportMode::all_updates())
                    .map_err(|e| anyhow::anyhow!("loro export: {e}"))?;
                write
                    .send(Message::Text(
                        SyncMessage::Update { doc_id: doc_id.into(), bytes }.to_json(),
                    ))
                    .await?;
            }
            _ = heartbeat.tick() => {
                if write.send(Message::Text(presence(PresenceMode::Reading).to_json())).await.is_err() {
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("editor agent shutting down");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::MemoryKind;
    use crate::connectors::AclTag;

    fn mem(topic: &str, path: &str, view: (&str, &str), fields: &[&str]) -> Memory {
        Memory {
            id: "m".into(),
            kind: MemoryKind::Wiki,
            topic: topic.into(),
            path: path.into(),
            acl_tag: AclTag {
                view: crate::access::View::new(view.0, view.1),
                fields: fields.iter().map(|f| f.to_string()).collect(),
            },
            confidence: 0.9,
            period: Some("2026-05".into()),
            supersedes: None,
            created_at: "2026-06-01T00:00:00Z".into(),
        }
    }

    fn blocks(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn dial_addr_rewrites_wildcard_binds_to_loopback() {
        assert_eq!(dial_addr("0.0.0.0:7878"), "127.0.0.1:7878");
        assert_eq!(dial_addr("[::]:7878"), "127.0.0.1:7878");
        assert_eq!(dial_addr("127.0.0.1:7878"), "127.0.0.1:7878");
        assert_eq!(dial_addr("192.168.1.5:9000"), "192.168.1.5:9000");
    }

    #[test]
    fn finds_unanswered_q_blocks_only() {
        let b = blocks(&[
            "intro prose, even with a question mark?",
            "Q: Unit economics of compression product",
        ]);
        let qs = find_unanswered(&b);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].question, "Unit economics of compression product");
        assert_eq!(qs[0].insert_index, 2);
    }

    #[test]
    fn answered_questions_are_skipped() {
        let b = blocks(&["Q: one", "A (cfo · from brain memory): done", "", "Q: two"]);
        let qs = find_unanswered(&b);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].question, "two");
        assert_eq!(qs[0].insert_index, 4);
    }

    #[test]
    fn weaver_tree_roundtrip_reads_and_inserts_blocks() {
        // build a doc the way the web's deterministic seed does
        let doc = LoroDoc::new();
        for (i, para) in ["notes.", "Q: economics of compression"].iter().enumerate() {
            insert_paragraph(&doc, i, para).unwrap();
        }
        doc.commit();
        assert_eq!(
            read_blocks(&doc),
            blocks(&["notes.", "Q: economics of compression"])
        );

        let qs = find_unanswered(&read_blocks(&doc));
        assert_eq!(qs.len(), 1);
        insert_paragraph(&doc, qs[0].insert_index, "A (cfo · from brain memory): …").unwrap();
        doc.commit();

        let after = read_blocks(&doc);
        assert_eq!(after.len(), 3);
        assert!(after[2].starts_with("A (cfo"));
        assert!(find_unanswered(&after).is_empty());
    }

    #[test]
    fn best_card_wins_and_authorization_gates_it() {
        let compression = mem(
            "products",
            "/x/brain/products/unit-economics-compression-2026-05.md",
            ("stripe", "finance_private"),
            &["gross", "credits"],
        );
        let inference = mem(
            "products",
            "/x/brain/products/unit-economics-inference-2026-05.md",
            ("stripe", "finance_private"),
            &["gross", "credits"],
        );
        let toks = tokens("Unit economics of compression product");
        assert!(match_score(&toks, &compression) > match_score(&toks, &inference));

        // CFO's token grants finance_private{gross,credits,…} → authorized
        assert!(card_readable(
            &crate::scenario::cfo_capability(),
            &compression.acl_tag
        ));
        // Richard's agent's token is spend_by_team only → all-or-nothing denial
        assert!(!card_readable(
            &crate::scenario::cto_agent_capability(),
            &compression.acl_tag
        ));
    }
}
