//! Slack workspace-context connector (spec 05 §2) — real, token-based.
//!
//! Poll-based reader over the Slack Web API — `conversations.list` +
//! `conversations.history` — mapped to one view:
//!   - `slack/channel_messages` → channel, user, text, ts
//!
//! No Event Subscriptions, no inbound webhook, no public URL: Contextful polls
//! on `ingest`/`cron`, so the connector stays local-first. The app manifest
//! lives at `infra/slack/manifest.yaml` (install steps in `infra/slack/README.md`).
//!
//! Live vs. offline is selected at RUNTIME: with `SLACK_BOT_TOKEN` set
//! (xoxb-…) `pull` hits the Web API; without it the connector reads fixtures
//! from `<fixtures>/slack/messages.json` when present and otherwise falls
//! back to embedded rows, so `ingest --source slack` works with zero egress.

use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::access::View;
use crate::connectors::{AclTag, Connector, Cursor, RawEvent, ViewField, ViewSchema};

/// One channel message (the view's columns).
#[derive(Debug, Clone, Deserialize)]
pub struct MessageRow {
    pub channel: String,
    pub user: String,
    pub text: String,
    pub ts: String,
}

fn embedded() -> Vec<MessageRow> {
    let m = |channel: &str, user: &str, text: &str, ts: &str| MessageRow {
        channel: channel.into(),
        user: user.into(),
        text: text.into(),
        ts: ts.into(),
    };
    vec![
        m(
            "finops",
            "U_CFO",
            "Reminder: May close-out today — eng gross spend is tracking ~100k, watch the credits burn-down.",
            "1748505600.000100",
        ),
        m(
            "finops",
            "U_CTO",
            "We renegotiated the enterprise tier; expect discount_tier changes to land in the June Stripe export.",
            "1748509200.000200",
        ),
        m(
            "eng",
            "U_ENG1",
            "Inference costs spiked after the batch job change — opened a ticket to move it off-peak.",
            "1748512800.000300",
        ),
    ]
}

pub struct SlackConnector {
    fixtures_dir: std::path::PathBuf,
}

impl SlackConnector {
    pub fn new(fixtures_dir: std::path::PathBuf) -> Self {
        Self { fixtures_dir }
    }

    fn view() -> View {
        View::new("slack", "channel_messages")
    }

    /// Load rows from `<fixtures>/slack/messages.json` if present, else embedded.
    fn rows(&self) -> Vec<MessageRow> {
        let path = self.fixtures_dir.join("slack").join("messages.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(rows) = serde_json::from_str::<Vec<MessageRow>>(&text) {
                return rows;
            }
        }
        embedded()
    }

    fn event_from(row: &MessageRow, now: &str) -> RawEvent {
        RawEvent {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: "slack".into(),
            view: Self::view(),
            payload: json!({
                "channel": row.channel, "user": row.user,
                "text": row.text, "ts": row.ts,
            }),
            ingested_at: now.to_string(),
            acl_tag: AclTag {
                view: Self::view(),
                fields: vec!["channel".into(), "user".into(), "text".into(), "ts".into()],
            },
        }
    }

    /// Bot token from the environment, if usable.
    fn token() -> Option<String> {
        std::env::var("SLACK_BOT_TOKEN")
            .ok()
            .filter(|t| !t.is_empty())
    }

    /// Live pull over the Slack Web API.
    fn pull_live(&self, token: &str, since: &Cursor) -> Result<Vec<RawEvent>> {
        use anyhow::{bail, Context};
        use serde_json::Value;

        fn call(token: &str, method: &str, params: &[(&str, &str)]) -> Result<Value> {
            let mut req = ureq::get(&format!("https://slack.com/api/{method}"))
                .set("Authorization", &format!("Bearer {token}"));
            for (k, v) in params {
                req = req.query(k, v);
            }
            let body: Value = req
                .call()
                .with_context(|| format!("slack api {method}: request failed"))?
                .into_json()
                .with_context(|| format!("slack api {method}: invalid JSON"))?;
            if !body["ok"].as_bool().unwrap_or(false) {
                bail!(
                    "slack api {method} returned error '{}'",
                    body["error"].as_str().unwrap_or("unknown")
                );
            }
            Ok(body)
        }

        let now = Utc::now().to_rfc3339();
        let list = call(
            token,
            "conversations.list",
            &[
                ("types", "public_channel"),
                ("exclude_archived", "true"),
                ("limit", "200"),
            ],
        )?;
        let mut events = Vec::new();
        for ch in list["channels"].as_array().into_iter().flatten() {
            // Only channels the bot was invited into are readable.
            if !ch["is_member"].as_bool().unwrap_or(false) {
                continue;
            }
            let (id, name) = (
                ch["id"].as_str().unwrap_or_default(),
                ch["name"].as_str().unwrap_or_default(),
            );
            let mut params = vec![("channel", id), ("limit", "100")];
            if let Some(oldest) = since.since.as_deref() {
                params.push(("oldest", oldest));
            }
            let history = call(token, "conversations.history", &params)?;
            for msg in history["messages"].as_array().into_iter().flatten() {
                // Plain user messages only — skip joins/topic changes/bot noise.
                if msg["type"].as_str() != Some("message") || msg.get("subtype").is_some() {
                    continue;
                }
                let text = msg["text"].as_str().unwrap_or_default();
                if text.is_empty() {
                    continue;
                }
                let row = MessageRow {
                    channel: name.to_string(),
                    user: msg["user"].as_str().unwrap_or_default().to_string(),
                    text: text.to_string(),
                    ts: msg["ts"].as_str().unwrap_or_default().to_string(),
                };
                events.push(Self::event_from(&row, &now));
            }
        }
        Ok(events)
    }
}

impl Connector for SlackConnector {
    fn source_id(&self) -> &str {
        "slack"
    }

    fn views(&self) -> Vec<ViewSchema> {
        let f = |name: &str| ViewField {
            name: name.into(),
            ty: "string".into(),
            private: false,
        };
        vec![ViewSchema {
            view: Self::view(),
            fields: vec![f("channel"), f("user"), f("text"), f("ts")],
        }]
    }

    fn pull(&self, since: &Cursor) -> Result<Vec<RawEvent>> {
        if let Some(token) = Self::token() {
            return self.pull_live(&token, since);
        }
        let now = Utc::now().to_rfc3339();
        Ok(self
            .rows()
            .iter()
            .map(|r| Self::event_from(r, &now))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_pull_yields_tagged_events() {
        let conn = SlackConnector::new(std::path::PathBuf::from("/nonexistent"));
        let events = conn.pull(&Cursor::default()).unwrap();
        assert!(!events.is_empty());
        for e in &events {
            assert_eq!(e.source_id, "slack");
            assert_eq!(e.view.id(), "slack/channel_messages");
            assert!(e.acl_tag.fields.contains(&"text".to_string()));
            assert!(e.payload["text"].as_str().is_some_and(|t| !t.is_empty()));
        }
    }

    #[test]
    fn fixture_file_overrides_embedded() {
        let dir = std::env::temp_dir().join(format!("contextful-slack-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("slack")).unwrap();
        std::fs::write(
            dir.join("slack").join("messages.json"),
            r#"[{"channel":"general","user":"U1","text":"hello brain","ts":"1.0"}]"#,
        )
        .unwrap();

        let events = SlackConnector::new(dir.clone())
            .pull(&Cursor::default())
            .unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["text"], "hello brain");
        assert_eq!(events[0].payload["channel"], "general");
    }
}
