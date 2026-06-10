//! Editor agent: watch a relay document, answer questions from the brain
//! (spec 04 §1 over the spec 01 wire).
//!
//! A real Loro peer (the relay still never parses CRDT bytes): it subscribes to
//! one doc, merges snapshots/updates into a local `LoroDoc`, and when the text
//! settles it scans the `body` container for `Q:` lines that no `A (…)` line
//! answers yet. Each question is matched against the brain's Markdown memory
//! (`~/.contextful/brain/**`), authorized against the agent's capability token
//! — all-or-nothing per card, same rule as `brain.get_context` — and the answer
//! is inserted into the document below the question, shipped back as a normal
//! Loro update. Answers ride the CRDT, never awareness (presence invariant).

use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::time::interval;
use tokio_tungstenite::tungstenite::Message;

use crate::access::Capability;
use crate::brain::retrieval::card_authorized;
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

/// A `Q:` line with no `A (…)` reply yet. `insert_at` is the unicode-char
/// position right after the question's line break (Loro text indices are
/// unicode code points on both the Rust and JS peers).
#[derive(Debug, PartialEq, Eq)]
pub struct PendingQuestion {
    pub question: String,
    pub insert_at: usize,
    /// true when the question is the last line and has no trailing newline.
    pub needs_newline: bool,
}

/// Scan the body text for unanswered questions. A question is a line whose
/// trimmed content starts with `Q:`; it counts as answered when the next
/// non-empty line starts with `A:` or `A (`.
pub fn find_unanswered(text: &str) -> Vec<PendingQuestion> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut offsets = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    for l in &lines {
        offsets.push(offset);
        offset += l.chars().count() + 1; // +1 for the '\n' split away
    }

    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(q) = line.trim_start().strip_prefix("Q:") else {
            continue;
        };
        let question = q.trim();
        if question.is_empty() {
            continue;
        }
        let answered = lines
            .iter()
            .skip(i + 1)
            .map(|l| l.trim_start())
            .find(|l| !l.is_empty())
            .is_some_and(|l| l.starts_with("A:") || l.starts_with("A ("));
        if answered {
            continue;
        }
        let has_newline = i + 1 < lines.len();
        out.push(PendingQuestion {
            question: question.to_string(),
            insert_at: offsets[i] + line.chars().count() + usize::from(has_newline),
            needs_newline: !has_newline,
        });
    }
    out
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
            if !card_authorized(cap, &m.acl_tag) {
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

    let ldoc = loro::LoroDoc::new();
    let body = ldoc.get_text("body");
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

                let text = body.to_string();
                let mut pending: Vec<PendingQuestion> = find_unanswered(&text)
                    .into_iter()
                    .filter(|q| !answered.contains(&q.question))
                    .collect();
                if pending.is_empty() {
                    continue;
                }
                write.send(Message::Text(presence(PresenceMode::Writing).to_json())).await?;

                // bottom-up so earlier insert positions stay valid
                pending.sort_by_key(|q| std::cmp::Reverse(q.insert_at));
                for q in &pending {
                    let answer = answer_for(&store, &index, &cap, &q.question);
                    tracing::info!(question = %q.question, "answering from brain");
                    let block = format!(
                        "{}A ({principal} · from brain memory): {answer}\n",
                        if q.needs_newline { "\n" } else { "" },
                    );
                    body.insert(q.insert_at, &block)
                        .map_err(|e| anyhow::anyhow!("loro insert: {e}"))?;
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

    #[test]
    fn finds_unanswered_q_lines_only() {
        let text =
            "intro prose, even with a question mark?\n\nQ: Unit economics of compression product\n";
        let qs = find_unanswered(text);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].question, "Unit economics of compression product");
        assert!(!qs[0].needs_newline);
        // insert position is right after the Q line's newline (end of text here)
        assert_eq!(qs[0].insert_at, text.chars().count());
    }

    #[test]
    fn answered_questions_are_skipped() {
        let text = "Q: one\nA (cfo · from brain memory): done\n\nQ: two";
        let qs = find_unanswered(text);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].question, "two");
        assert!(qs[0].needs_newline); // last line, no trailing newline
        assert_eq!(qs[0].insert_at, text.chars().count());
    }

    #[test]
    fn unicode_offsets_are_char_based() {
        let text = "héllo wörld — naïve\nQ: economics of compression\n";
        let qs = find_unanswered(text);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].insert_at, text.chars().count());
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
        assert!(card_authorized(
            &crate::scenario::cfo_capability(),
            &compression.acl_tag
        ));
        // CTO agent's token is spend_by_team only → all-or-nothing denial
        assert!(!card_authorized(
            &crate::scenario::cto_agent_capability(),
            &compression.acl_tag
        ));
    }
}
