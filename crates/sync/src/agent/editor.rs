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
//!
//! It also answers **mention asks**: a block like
//! `@Monica (CFO)'s analyst agent What's CEO's Salary` addressed to the
//! watcher (or an agent it owns). The asker is the principal the relay
//! attributed the block to (`UPDATE.from`), and the answer is authorized
//! against the **asker's** token — it lands in a doc the asker reads. A card
//! the asker can't read becomes a `⛔ Denied · <reason>` paragraph plus a
//! NOTIFY frame addressed to the asker, so their client surfaces the denial
//! actively. The notification carries decision metadata only, never card
//! content.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use loro::{LoroDoc, LoroMap, LoroText};
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message;

use crate::access::biscuit::authorize;
use crate::access::{AuthDecision, Capability, DenyReason, Operation, QueryRequest};
use crate::brain::retrieval::card_readable;
use crate::brain::{BrainIndex, Memory};
use crate::config::Config;
use crate::connectors::AclTag;
use crate::controlplane::load_capability;
use crate::identity::Principal;
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

/// Ordered top-level blocks of a Weaver doc as `(tree node id, text)` — the id
/// is stable across edits, so it keys block→author attribution.
pub fn read_blocks_with_ids(doc: &LoroDoc) -> Vec<(String, String)> {
    let tree = doc.get_tree(TREE_NAME);
    tree.children(None)
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let text = tree
                .get_meta(id)
                .ok()
                .and_then(|meta| meta.get("text"))
                .and_then(|v| v.as_container().cloned())
                .and_then(|c| c.into_text().ok())
                .map(|t| t.to_string())
                .unwrap_or_default();
            (id.to_string(), text)
        })
        .collect()
}

/// Ordered top-level block texts of a Weaver doc (one entry per block node).
pub fn read_blocks(doc: &LoroDoc) -> Vec<String> {
    read_blocks_with_ids(doc)
        .into_iter()
        .map(|(_, t)| t)
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

/// A mention ask: `@<directory name> <question>` with no `A (…)` reply yet.
#[derive(Debug, PartialEq, Eq)]
pub struct MentionAsk {
    /// tree node id of the asking block — keys author attribution.
    pub block_id: String,
    /// directory principal id the ask is addressed to (e.g. `agent:cfo/1`).
    pub target_id: String,
    pub question: String,
    pub insert_index: usize,
    /// the full block text — the dedup key, like `question` for `Q:` lines.
    pub raw: String,
}

/// Parse one block as a mention ask against the directory (`(id, name)`
/// pairs): `@` + a known display name (case-insensitive, longest first) +
/// the question. Plain prose with an `@` that matches nobody is not an ask.
pub fn parse_mention_ask(text: &str, directory: &[(String, String)]) -> Option<(String, String)> {
    let rest = text.trim_start().strip_prefix('@')?;
    let mut targets: Vec<&(String, String)> = directory.iter().collect();
    targets.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));
    for (id, name) in targets {
        let Some(prefix) = rest.get(..name.len()) else {
            continue;
        };
        if !prefix.eq_ignore_ascii_case(name) {
            continue;
        }
        let question = rest[name.len()..]
            .trim_start_matches(|c: char| {
                c.is_whitespace() || matches!(c, ',' | ':' | '—' | '–' | '-')
            })
            .trim_end();
        if question.is_empty() {
            return None;
        }
        return Some((id.clone(), question.to_string()));
    }
    None
}

/// Scan blocks for unanswered mention asks (answered = next non-empty block
/// starts with `A:` / `A (`, same rule as `Q:` lines).
pub fn find_mention_asks(
    blocks: &[(String, String)],
    directory: &[(String, String)],
) -> Vec<MentionAsk> {
    blocks
        .iter()
        .enumerate()
        .filter_map(|(i, (block_id, text))| {
            let (target_id, question) = parse_mention_ask(text, directory)?;
            let answered = blocks
                .iter()
                .skip(i + 1)
                .map(|(_, b)| b.trim_start())
                .find(|b| !b.is_empty())
                .is_some_and(|b| b.starts_with("A:") || b.starts_with("A ("));
            (!answered).then(|| MentionAsk {
                block_id: block_id.clone(),
                target_id,
                question,
                insert_index: i + 1,
                raw: text.clone(),
            })
        })
        .collect()
}

/// Is an ask addressed to `target_id` for this watcher to answer? Yes when it
/// is the watcher itself, or an agent the watcher owns (the CFO-side watcher
/// running as `cfo` answers for "Monica (CFO)'s analyst agent").
pub fn addressed_to(directory: &[Principal], target_id: &str, watcher: &str) -> bool {
    if target_id == watcher {
        return true;
    }
    directory.iter().any(|p| {
        p.id() == target_id && matches!(p, Principal::Agent { owner, .. } if owner == watcher)
    })
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

/// Structured outcome of one question against the brain, capability-filtered.
#[derive(Debug, PartialEq, Eq)]
pub enum AskOutcome {
    Answered(String),
    /// the best-matching card exists but the token can't read it (all-or-nothing).
    Denied {
        reason: DenyReason,
        view: String,
        fields: Vec<String>,
    },
    NoMatch,
}

/// Why a card read was denied — the structured sibling of `card_readable`.
/// An `Ok` with redactions still denies the all-or-nothing card read, and
/// reads as `no_grant` (fields the token does not grant).
fn deny_reason(cap: &Capability, tag: &AclTag) -> DenyReason {
    match authorize(
        cap,
        &QueryRequest {
            op: Operation::Query,
            view: tag.view.clone(),
            fields: tag.fields.clone(),
        },
    ) {
        AuthDecision::Denied(reason) => reason,
        AuthDecision::Ok { .. } => DenyReason::NoGrant,
    }
}

/// Resolve one question from memory. The best-matching card is authorized
/// all-or-nothing against its `acl_tag` (the `brain.get_context` rule) — a
/// match the token can't read is a denial, not the content.
pub fn resolve_ask(
    store: &Store,
    index: &BrainIndex,
    cap: &Capability,
    question: &str,
) -> AskOutcome {
    let toks = tokens(question);
    let best = index
        .memories
        .iter()
        .map(|m| (match_score(&toks, m), m))
        .max_by_key(|(s, _)| *s);
    match best {
        Some((score, m)) if score >= 2 => {
            if !card_readable(cap, &m.acl_tag) {
                return AskOutcome::Denied {
                    reason: deny_reason(cap, &m.acl_tag),
                    view: m.acl_tag.view.id(),
                    fields: m.acl_tag.fields.clone(),
                };
            }
            AskOutcome::Answered(match store.read_card(&m.path) {
                Ok(card) => compose_answer(&card, m),
                Err(e) => format!("matched memory at {} is unreadable: {e}", m.path),
            })
        }
        _ => AskOutcome::NoMatch,
    }
}

/// [`resolve_ask`] flattened to the `Q:`-line answer string.
pub fn answer_for(store: &Store, index: &BrainIndex, cap: &Capability, question: &str) -> String {
    match resolve_ask(store, index, cap, question) {
        AskOutcome::Answered(answer) => answer,
        AskOutcome::Denied { view, fields, .. } => format!(
            "Denied — the matching memory requires {} on {}, which this token does not \
             grant. Raise an access request.",
            fields.join(", "),
            view
        ),
        AskOutcome::NoMatch => {
            "No memory in the brain matches this question yet — ingest more sources or rephrase."
                .to_string()
        }
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
    // block id → authoring principal, from the relay's UPDATE.from stamp.
    // Blocks that arrive in the catch-up snapshot have no author and are
    // never answered as mention asks (no asker to authorize against).
    let mut authors: HashMap<String, String> = HashMap::new();
    let directory = scenario::directory();
    let dir_names: Vec<(String, String)> = directory
        .iter()
        .map(|p| (p.id().to_string(), p.name().to_string()))
        .collect();
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
                            if d == doc_id && !bytes.is_empty() =>
                        {
                            match ldoc.import(&bytes) {
                                Ok(_) => dirty_since = Some(Instant::now()),
                                Err(e) => tracing::warn!(error = %e, "import failed"),
                            }
                        }
                        Ok(SyncMessage::Update { doc_id: d, bytes, from })
                            if d == doc_id && !bytes.is_empty() =>
                        {
                            // attribute blocks this update introduced to its
                            // (relay-stamped) sender — first author wins
                            let before: HashSet<String> = read_blocks_with_ids(&ldoc)
                                .into_iter()
                                .map(|(id, _)| id)
                                .collect();
                            match ldoc.import(&bytes) {
                                Ok(_) => {
                                    dirty_since = Some(Instant::now());
                                    if let Some(sender) = from {
                                        for (id, _) in read_blocks_with_ids(&ldoc) {
                                            if !before.contains(&id) {
                                                authors.entry(id).or_insert_with(|| sender.clone());
                                            }
                                        }
                                    }
                                }
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

                let blocks = read_blocks_with_ids(&ldoc);
                let texts: Vec<String> = blocks.iter().map(|(_, t)| t.clone()).collect();

                // (insert index, reply paragraph) for both ask kinds, plus the
                // NOTIFY frames to ship after the CRDT update lands
                let mut replies: Vec<(usize, String)> = Vec::new();
                let mut notifies: Vec<SyncMessage> = Vec::new();

                let pending: Vec<PendingQuestion> = find_unanswered(&texts)
                    .into_iter()
                    .filter(|q| !answered.contains(&q.question))
                    .collect();
                for q in pending {
                    let answer = answer_for(&store, &index, &cap, &q.question);
                    tracing::info!(question = %q.question, "answering from brain");
                    replies.push((
                        q.insert_index,
                        format!("A ({principal} · from brain memory): {answer}"),
                    ));
                    answered.insert(q.question.clone());
                }

                let asks: Vec<MentionAsk> = find_mention_asks(&blocks, &dir_names)
                    .into_iter()
                    .filter(|a| !answered.contains(&a.raw))
                    .filter(|a| addressed_to(&directory, &a.target_id, principal))
                    .collect();
                for ask in asks {
                    // mention asks are answered with the ASKER's token — the
                    // reply lands in a doc the asker reads. No attributed
                    // author (e.g. a pre-snapshot block) means no one to
                    // authorize against: leave it unanswered.
                    let Some(asker) = authors.get(&ask.block_id).cloned() else {
                        tracing::debug!(ask = %ask.raw, "mention ask has no attributed author; skipping");
                        continue;
                    };
                    let target_name = dir_names
                        .iter()
                        .find(|(id, _)| *id == ask.target_id)
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| ask.target_id.clone());
                    let asker_cap = load_capability(&config, &asker)
                        .or_else(|| scenario::initial_capability(&asker));
                    let outcome = match &asker_cap {
                        Some(acap) => resolve_ask(&store, &index, acap, &ask.question),
                        None => AskOutcome::Denied {
                            reason: DenyReason::NoGrant,
                            view: String::new(),
                            fields: vec![],
                        },
                    };
                    let reply = match outcome {
                        AskOutcome::Answered(answer) => {
                            tracing::info!(question = %ask.question, %asker, "answering mention ask");
                            format!("A ({principal} · for {asker} · from brain memory): {answer}")
                        }
                        AskOutcome::Denied { reason, view, fields } => {
                            let scope = if fields.is_empty() {
                                format!("{asker} holds no capability token")
                            } else {
                                format!(
                                    "the matching memory requires {} on {view}, which {asker}'s \
                                     token does not grant",
                                    fields.join(", ")
                                )
                            };
                            tracing::info!(question = %ask.question, %asker, reason = reason.wire_str(), "denying mention ask");
                            notifies.push(SyncMessage::Notify {
                                doc_id: doc_id.to_string(),
                                to: asker.clone(),
                                from: principal.to_string(),
                                reason: reason.wire_str().to_string(),
                                message: format!(
                                    "{asker} asked {target_name} “{}” — {scope}. Raise an access \
                                     request.",
                                    ask.question
                                ),
                            });
                            format!(
                                "A ({principal} · for {asker}): ⛔ Denied · {} — {scope}. Raise an \
                                 access request.",
                                reason.wire_str()
                            )
                        }
                        AskOutcome::NoMatch => format!(
                            "A ({principal} · for {asker}): No memory in the brain matches this \
                             question yet — ingest more sources or rephrase."
                        ),
                    };
                    replies.push((ask.insert_index, reply));
                    answered.insert(ask.raw.clone());
                }

                if replies.is_empty() {
                    continue;
                }
                write.send(Message::Text(presence(PresenceMode::Writing).to_json())).await?;

                // bottom-up so earlier insert indices stay valid
                replies.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
                for (at, block) in &replies {
                    insert_paragraph(&ldoc, *at, block)?;
                }
                ldoc.commit();
                // ship the full update log (same as the web peer) so the relay's
                // overwrite-persistence stays a complete snapshot
                let bytes = ldoc
                    .export(loro::ExportMode::all_updates())
                    .map_err(|e| anyhow::anyhow!("loro export: {e}"))?;
                write
                    .send(Message::Text(
                        SyncMessage::Update { doc_id: doc_id.into(), bytes, from: None }.to_json(),
                    ))
                    .await?;
                // the denial notification rides AFTER the reply paragraph so
                // the asker's client has the context block by the time the
                // toast fires; it carries decision metadata, never card content
                for n in notifies {
                    write.send(Message::Text(n.to_json())).await?;
                }
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

    fn dir_pairs() -> Vec<(String, String)> {
        crate::scenario::directory()
            .iter()
            .map(|p| (p.id().to_string(), p.name().to_string()))
            .collect()
    }

    #[test]
    fn parses_mention_asks_addressed_to_directory_names() {
        let dir = dir_pairs();
        let (target, q) =
            parse_mention_ask("@Monica (CFO)'s analyst agent What's CEO's Salary", &dir).unwrap();
        assert_eq!(target, "agent:cfo/1");
        assert_eq!(q, "What's CEO's Salary");
        // a name outside the directory is prose, not an ask
        assert!(parse_mention_ask("@Gavin Belson what's our runway", &dir).is_none());
        // a bare mention with no question is not an ask
        assert!(parse_mention_ask("@Monica (CFO)'s analyst agent", &dir).is_none());
        // plain prose never parses
        assert!(parse_mention_ask("email me at v@fractalbox.dev", &dir).is_none());
    }

    #[test]
    fn mention_asks_skip_answered_and_carry_block_ids() {
        let dir = dir_pairs();
        let blocks = vec![
            (
                "b1".to_string(),
                "@Monica (CFO)'s analyst agent What's CEO's Salary".to_string(),
            ),
            (
                "b2".to_string(),
                "A (cfo · for agent:eng/1): ⛔ Denied · no_grant — …".to_string(),
            ),
            (
                "b3".to_string(),
                "@Dinesh (CTO)'s agent, eng spend this period?".to_string(),
            ),
        ];
        let asks = find_mention_asks(&blocks, &dir);
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].block_id, "b3");
        assert_eq!(asks[0].target_id, "agent:eng/1");
        assert_eq!(asks[0].question, "eng spend this period?");
        assert_eq!(asks[0].insert_index, 3);
    }

    #[test]
    fn watcher_answers_for_itself_and_its_owned_agents_only() {
        let dir = crate::scenario::directory();
        assert!(addressed_to(&dir, "cfo", "cfo"));
        // "Monica (CFO)'s analyst agent" is owned by cfo → the cfo watcher answers
        assert!(addressed_to(&dir, "agent:cfo/1", "cfo"));
        assert!(!addressed_to(&dir, "agent:cfo/1", "eng"));
        assert!(!addressed_to(&dir, "agent:eng/1", "cfo"));
    }

    #[test]
    fn salary_ask_resolves_to_no_grant_denial_for_dinesh_agent() {
        let salary = mem(
            "finance",
            "/x/brain/finance/employee-salary-2026-05.md",
            ("stripe", "finance_private"),
            &["team", "period", "employee_salary"],
        );
        let mut index = BrainIndex::default();
        index.memories.push(salary);
        let store = Store::new(crate::config::Config {
            root: std::env::temp_dir().join(format!("contextful-test-{}", uuid::Uuid::new_v4())),
            inference: crate::config::InferenceBackend::Stub,
        });

        match resolve_ask(
            &store,
            &index,
            &crate::scenario::eng_agent_capability(),
            "What's CEO's Salary",
        ) {
            AskOutcome::Denied {
                reason,
                view,
                fields,
            } => {
                assert_eq!(reason.wire_str(), "no_grant");
                assert_eq!(view, "stripe/finance_private");
                assert!(fields.contains(&"employee_salary".to_string()));
            }
            other => panic!("expected no_grant denial, got {other:?}"),
        }
        // the CFO's own root token reads the same card
        assert!(matches!(
            resolve_ask(
                &store,
                &index,
                &crate::scenario::cfo_capability(),
                "What's CEO's Salary"
            ),
            AskOutcome::Answered(_)
        ));
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
