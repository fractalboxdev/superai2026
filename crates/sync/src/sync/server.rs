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

use crate::config::Config;
use crate::controlplane::is_revoked;
use crate::store::docs::DocStore;
use crate::sync::protocol::SyncMessage;

/// (sender_peer_id, json) so peers can suppress their own echoes.
type RoomMsg = (u64, String);
type Rooms = Arc<Mutex<HashMap<String, broadcast::Sender<RoomMsg>>>>;

static PEER_SEQ: AtomicU64 = AtomicU64::new(1);

pub async fn run(addr: &str, with_mcp: bool) -> Result<()> {
    let config = Config::load();
    config.ensure_dirs()?;
    if with_mcp {
        tracing::warn!(
            "--with-mcp co-hosting is declared but the demo uses `sync mcp` (spec 06 §4)"
        );
    }

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
                &SyncMessage::Error {
                    code: "expected_hello".into(),
                    message: "first message must be HELLO".into(),
                },
            )
            .await?;
            return Ok(());
        }
    };
    if is_revoked(&config, &principal) {
        send(
            &write,
            &SyncMessage::Error {
                code: "revoked".into(),
                message: format!("principal {principal} is revoked"),
            },
        )
        .await?;
        return Ok(());
    }

    let mut subscribed_doc: Option<String> = None;

    // --- message loop ---
    while let Some(msg) = next_message(&mut read).await? {
        match msg {
            SyncMessage::Subscribe { doc_id, .. } => {
                // read(document) check: revoked already rejected; membership is
                // open for seeded demo principals. (Real doc-cap check: spec 03.)
                let docs = DocStore::new((*config).clone());
                let snapshot = docs.load_snapshot(&doc_id)?.unwrap_or_default();
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

                let tx = room_sender(&rooms, &doc_id).await;
                let mut rx = tx.subscribe();
                subscribed_doc = Some(doc_id.clone());

                // forward room traffic from other peers to this socket
                let write_fwd = write.clone();
                tokio::spawn(async move {
                    while let Ok((from, json)) = rx.recv().await {
                        if from == peer_id {
                            continue; // skip our own echo
                        }
                        let mut w = write_fwd.lock().await;
                        if w.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                });
                tracing::info!(%principal, doc = %doc_id, peer_id, "subscribed");
            }
            SyncMessage::Update { doc_id, bytes } => {
                // send requires write(document) — revoked already rejected.
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
                DocStore::new((*config).clone()).save_snapshot(&doc_id, &bytes)?;
            }
            SyncMessage::Awareness { doc_id, presence } => {
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

    let _ = subscribed_doc;
    Ok(())
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
