//! Headless file-sync client peer (spec 01 §3).
//!
//! A non-browser peer that speaks the same protocol as the Weaver client: it
//! HELLOs, SUBSCRIBEs, persists the snapshot to a local file for editing
//! outside the browser, and relays presence + updates. Real OPFS↔file
//! reconciliation + Loro apply is future work; this proves the wire protocol
//! and gives the relay a second live peer.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;

use crate::config::Config;
use crate::store::docs::DocStore;
use crate::sync::presence::{PresenceMode, PresenceState};
use crate::sync::protocol::SyncMessage;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn run(addr: &str, doc: &str, principal: &str) -> Result<()> {
    let url = format!("ws://{addr}/");
    let (ws, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws.split();
    tracing::info!(%url, %principal, doc, "client connected");

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
                doc_id: doc.into(),
                client_vv: None,
            }
            .to_json(),
        ))
        .await?;

    // read loop: persist snapshots, log relayed traffic
    let config = Config::load();
    let doc_owned = doc.to_string();
    let self_principal = principal.to_string();
    tokio::spawn(async move {
        let docs = DocStore::new(config);
        while let Some(Ok(frame)) = read.next().await {
            if let Message::Text(text) = frame {
                match serde_json::from_str::<SyncMessage>(&text) {
                    Ok(SyncMessage::Snapshot { doc_id, bytes }) => {
                        let _ = docs.save_snapshot(&doc_id, &bytes);
                        tracing::info!(doc = %doc_id, bytes = bytes.len(), "snapshot persisted to file");
                    }
                    Ok(SyncMessage::Update { bytes, .. }) => {
                        tracing::info!(bytes = bytes.len(), "← update from peer");
                    }
                    Ok(SyncMessage::Notify {
                        to,
                        from,
                        reason,
                        message,
                        ..
                    }) => {
                        // an access decision addressed to this machine's
                        // principal — surface it loudly, others quietly
                        if to == self_principal {
                            tracing::warn!(%from, "⛔ Denied · {reason} — {message}");
                        } else {
                            tracing::debug!(%to, %from, %reason, "← notify for another principal");
                        }
                    }
                    Ok(SyncMessage::Awareness { presence, .. }) => {
                        tracing::info!(peer = %presence.principal, mode = ?presence.mode, "← presence");
                    }
                    Ok(SyncMessage::Error { code, message }) => {
                        tracing::error!(%code, %message, "relay rejected us");
                    }
                    _ => {}
                }
            }
        }
        tracing::info!("read loop ended");
    });

    // publish presence + a demo update, then heartbeat until Ctrl-C
    let mut tick = interval(Duration::from_secs(5));
    let presence = |mode| SyncMessage::Awareness {
        doc_id: doc_owned.clone(),
        presence: PresenceState {
            principal: principal.into(),
            display_name: principal.into(),
            mode,
            session: None,
            cursor_block: None,
            cursor_anchor: Some(0),
            selection_end: None,
            heartbeat: now_ms(),
        },
    };

    write
        .send(Message::Text(presence(PresenceMode::Writing).to_json()))
        .await?;
    write
        .send(Message::Text(
            SyncMessage::Update {
                doc_id: doc_owned.clone(),
                bytes: b"contextful-headless-edit".to_vec(),
                from: None,
            }
            .to_json(),
        ))
        .await?;

    loop {
        tokio::select! {
            _ = tick.tick() => {
                if write.send(Message::Text(presence(PresenceMode::Reading).to_json())).await.is_err() {
                    break;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("client shutting down");
                break;
            }
        }
    }
    Ok(())
}
