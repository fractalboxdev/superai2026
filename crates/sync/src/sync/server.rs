//! The authoritative sync relay (spec 01 §3–4).
//!
//! Centralized & authoritative — not P2P. All peers (browsers, headless
//! clients, sandbox agents) sync through this one instance. CRDT payloads are
//! opaque Loro bytes the relay broadcasts to room peers; the latest
//! client-pushed snapshot is persisted per doc. Authorization is per-message:
//! the token in HELLO is re-checked on SUBSCRIBE/UPDATE; revoked principals are
//! rejected (try `sync ctl revoke --principal …`).

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::tungstenite::Message;

use crate::access::token::{scope_allows_doc, VerifiedScope};
use crate::access::Operation;
use crate::config::Config;
use crate::controlplane::{is_revoked, is_token_revoked, load_verified_scope};
use crate::store::docs::{is_safe_doc_id, DocStore};
use crate::sync::protocol::SyncMessage;

/// (sender_peer_id, json) so peers can suppress their own echoes.
type RoomMsg = (u64, String);
type Rooms = Arc<Mutex<HashMap<String, broadcast::Sender<RoomMsg>>>>;

static PEER_SEQ: AtomicU64 = AtomicU64::new(1);

pub async fn run(addr: &str) -> Result<()> {
    let config = Config::load();
    config.ensure_dirs()?;

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "sync relay listening (authoritative)");
    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));
    let config = Arc::new(config);

    loop {
        let (stream, peer) = listener.accept().await?;
        let rooms = rooms.clone();
        let config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, rooms, config).await {
                tracing::debug!(%peer, error = %e, "connection closed");
            }
        });
    }
}

async fn handle_conn(stream: TcpStream, rooms: Rooms, config: Arc<Config>) -> Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (write, mut read) = ws.split();
    let write = Arc::new(Mutex::new(write));
    let peer_id = PEER_SEQ.fetch_add(1, Ordering::Relaxed);

    // --- HELLO ---
    let principal = match next_message(&mut read).await? {
        Some(SyncMessage::Hello { principal, .. }) => principal,
        _ => {
            send(
                &write,
                &err("expected_hello", "first message must be HELLO"),
            )
            .await?;
            return Ok(());
        }
    };
    if is_revoked(&config, &principal) {
        send(
            &write,
            &err("revoked", &format!("principal {principal} is revoked")),
        )
        .await?;
        return Ok(());
    }

    // Real Biscuit verification at session start: the principal's persisted
    // token is signature-checked against the root's registered public key and
    // its doc rights derived from the Datalog facts (spec 01 §4). The browser
    // does not yet send its token in HELLO, so the relay uses the control
    // plane's copy — the proof is the same signed Biscuit either way.
    let scope = match load_verified_scope(&config, &principal) {
        Some(s) => s,
        None => {
            send(
                &write,
                &err(
                    "no_capability",
                    &format!(
                        "no verified capability token for {principal} — run `ctl seed`/`ctl grant`"
                    ),
                ),
            )
            .await?;
            return Ok(());
        }
    };
    if is_token_revoked(&config, &scope.revocation_ids) {
        send(
            &write,
            &err("revoked", &format!("token for {principal} is revoked")),
        )
        .await?;
        return Ok(());
    }

    // --- message loop ---
    while let Some(msg) = next_message(&mut read).await? {
        // Per-message authorization: re-check revocation (both the principal
        // list and the Biscuit revocation ids) on every data message — a
        // principal revoked mid-session must stop being served — enforce the
        // token's doc-level read/write rights, and reject unsafe doc ids before
        // they touch the filesystem. CRDT payloads stay opaque Loro bytes;
        // structured data is gated by the brain MCP path.
        if is_revoked(&config, &principal) || is_token_revoked(&config, &scope.revocation_ids) {
            send(
                &write,
                &err("revoked", &format!("principal {principal} is revoked")),
            )
            .await?;
            break;
        }
        match msg {
            SyncMessage::Subscribe { doc_id, .. } => {
                // subscribe = read(document), proven by the verified token
                if !doc_authorized(&write, &scope, &doc_id, Operation::Read).await? {
                    continue;
                }
                // Subscribe to the room FIRST so updates that arrive while we load
                // and send the snapshot are buffered for delivery (no lost update).
                let tx = room_sender(&rooms, &doc_id).await;
                let mut rx = tx.subscribe();

                let snapshot = DocStore::new((*config).clone())
                    .load_snapshot(&doc_id)?
                    .unwrap_or_default();
                send(
                    &write,
                    &SyncMessage::HelloOk {
                        doc_id: doc_id.clone(),
                        server_vv: None,
                    },
                )
                .await?;
                send(
                    &write,
                    &SyncMessage::Snapshot {
                        doc_id: doc_id.clone(),
                        bytes: snapshot,
                    },
                )
                .await?;

                // forward room traffic from other peers to this socket
                let write_fwd = write.clone();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok((from, json)) => {
                                if from == peer_id {
                                    continue; // skip our own echo
                                }
                                let mut w = write_fwd.lock().await;
                                if w.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                            // a lagged slow peer skips dropped messages rather than dying
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                });
                tracing::info!(%principal, doc = %doc_id, peer_id, "subscribed");
            }
            SyncMessage::Update { doc_id, bytes } => {
                // update = write(document), proven by the verified token;
                // revocation re-checked above.
                if !doc_authorized(&write, &scope, &doc_id, Operation::Write).await? {
                    continue;
                }
                if let Some(tx) = rooms.lock().await.get(&doc_id) {
                    let _ = tx.send((
                        peer_id,
                        SyncMessage::Update {
                            doc_id: doc_id.clone(),
                            bytes: bytes.clone(),
                        }
                        .to_json(),
                    ));
                }
                // persist client-pushed bytes as the latest snapshot for catch-up
                DocStore::new((*config).clone()).save_snapshot(&doc_id, &bytes)?;
            }
            SyncMessage::Snapshot { doc_id, bytes } => {
                // persisting a snapshot = write(document)
                if !doc_authorized(&write, &scope, &doc_id, Operation::Write).await? {
                    continue;
                }
                DocStore::new((*config).clone()).save_snapshot(&doc_id, &bytes)?;
            }
            SyncMessage::Awareness { doc_id, presence } => {
                // presence injection into a room is gated like reading it —
                // a token without read(doc) must not reach the room's peers
                if !doc_authorized(&write, &scope, &doc_id, Operation::Read).await? {
                    continue;
                }
                if let Some(tx) = rooms.lock().await.get(&doc_id) {
                    let _ = tx.send((
                        peer_id,
                        SyncMessage::Awareness { doc_id, presence }.to_json(),
                    ));
                }
            }
            SyncMessage::Hello { .. } | SyncMessage::HelloOk { .. } | SyncMessage::Error { .. } => {
            }
        }
    }

    Ok(())
}

/// Per-message doc guard: rejects unsafe doc ids before they touch the
/// filesystem, then enforces the token's doc-level right for `op`. Sends the
/// error frame itself; `Ok(false)` means "denied, keep the session".
async fn doc_authorized(
    write: &Arc<Mutex<WsSink>>,
    scope: &VerifiedScope,
    doc_id: &str,
    op: Operation,
) -> Result<bool> {
    if !is_safe_doc_id(doc_id) {
        send(write, &err("bad_doc_id", "unsafe doc_id")).await?;
        return Ok(false);
    }
    if !scope_allows_doc(scope, doc_id, op) {
        let op_name = match op {
            Operation::Write => "write",
            _ => "read",
        };
        send(
            write,
            &err(
                "forbidden",
                &format!("token does not grant {op_name} on {doc_id}"),
            ),
        )
        .await?;
        return Ok(false);
    }
    Ok(true)
}

fn err(code: &str, message: &str) -> SyncMessage {
    SyncMessage::Error {
        code: code.to_string(),
        message: message.to_string(),
    }
}

async fn room_sender(rooms: &Rooms, doc_id: &str) -> broadcast::Sender<RoomMsg> {
    let mut map = rooms.lock().await;
    map.entry(doc_id.to_string())
        .or_insert_with(|| broadcast::channel(256).0)
        .clone()
}

async fn next_message(
    read: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Result<Option<SyncMessage>> {
    while let Some(frame) = read.next().await {
        match frame? {
            Message::Text(text) => match serde_json::from_str::<SyncMessage>(&text) {
                Ok(m) => return Ok(Some(m)),
                Err(e) => tracing::warn!(error = %e, "ignoring malformed message"),
            },
            Message::Close(_) => return Ok(None),
            Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {}
        }
    }
    Ok(None)
}

type WsSink =
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, Message>;

async fn send(write: &Arc<Mutex<WsSink>>, msg: &SyncMessage) -> Result<()> {
    let mut w = write.lock().await;
    w.send(Message::Text(msg.to_json())).await?;
    Ok(())
}
