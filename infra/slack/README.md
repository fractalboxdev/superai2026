# Slack → Contextful ingestion

Contextful ingests Slack channel context through a **poll-based, read-only**
Slack app (no Event Subscriptions, no public URL — the binary stays
local-first). Messages land as `raw_event` rows under the
`slack/channel_messages` view (`channel`, `user`, `text`, `ts`) and are
capability-gated like every other source.

## Install steps

### 1. Create the app from the manifest

1. Open <https://api.slack.com/apps> → **Create New App** → **From a manifest**.
2. Pick your **testing workspace**.
3. Paste the contents of [`manifest.yaml`](./manifest.yaml) (YAML tab) → **Next** → **Create**.

### 2. Install to the workspace & grab the bot token

1. In the app's sidebar: **Install App** → **Install to Workspace** → **Allow**.
2. Copy the **Bot User OAuth Token** (`xoxb-…`).

### 3. Invite the bot to the channels you want ingested

The connector only reads channels the bot is a **member** of. In each channel:

```
/invite @contextful
```

### 4. Run the ingest (live path)

Live vs. offline is selected at runtime by the token — no feature flags:

```sh
export SLACK_BOT_TOKEN=xoxb-...   # from step 2; keep out of shell history if possible
cargo run -p sync -- ingest --source slack
```

Expected output: `ingested N events from 'slack'; synthesized M card(s); wired K link(s)`.

Without `SLACK_BOT_TOKEN`, the same command degrades to fixtures
(`<CONTEXTFUL_HOME>/fixtures/slack/messages.json`, else embedded rows) with
zero egress — useful for demos with no creds.

### 5. Access control (current state)

Ingested messages are stored as `raw_event` rows tagged
`acl_tag = slack/channel_messages` — like every view, they are invisible to
agents until a capability grants that view. Today the demo control plane
models a single finance root (capabilities are single-view), so
`ctl grant --view slack/channel_messages` is refused; wiring a Slack-owned
root key into the seed (multi-view capabilities) is a follow-up tracked in
spec 05's Future list.

## Notes

- **Token handling** — `SLACK_BOT_TOKEN` is env-only; it is never written to
  `~/.contextful` or the repo. Rotate it from the app's **OAuth & Permissions**
  page if it leaks.
- **Scopes** are read-only (`channels:read/history`, `groups:read/history`,
  `users:read`). The app cannot post.
- **Incremental pulls** — the connector accepts a cursor (`oldest` on
  `conversations.history`); the cron scheduler will use it once cron pipelines
  are config-driven (spec 05 §3).
