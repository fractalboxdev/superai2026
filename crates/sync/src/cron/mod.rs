//! Connectors & ETL orchestration (spec 05 §3): one-shot `ingest` and the
//! scheduled `cron` pipeline that keeps the brain fresh.

pub mod scheduler;

use anyhow::{bail, Result};

use crate::brain::synthesis::synthesize;
use crate::config::Config;
use crate::connectors::slack::SlackConnector;
use crate::connectors::stripe::StripeConnector;
use crate::connectors::{Connector, Cursor};
use crate::store::Store;

/// The scheduled enrichment query (spec 02 §8) — what the brain proactively
/// researches when the `exa` cron job fires.
pub const EXA_RESEARCH_QUERY: &str = "AI tooling spend benchmarks";

/// Run a connector's `pull`, write `raw_event` rows, and trigger synthesis.
/// Idempotent: prior events for the same source are replaced.
pub fn ingest_once(source: &str) -> Result<()> {
    let config = Config::load();
    let store = Store::new(config.clone());
    let mut index = store.load_index()?;

    if source == "exa" {
        // research, not raw ETL: world_search runs the egress firewall,
        // scrubs inbound content, and synthesizes world_fact memory cards
        // directly — the cards the daydream cycle later connects (spec 02 §8/§9)
        let memories =
            crate::brain::world::world_search(&config, &store, &mut index, EXA_RESEARCH_QUERY)?;
        let wired = crate::brain::links::self_wire(&store, &mut index);
        store.save_index(&index)?;
        println!(
            "researched '{EXA_RESEARCH_QUERY}' via exa; synthesized {} world card(s); wired {wired} link(s)",
            memories.len()
        );
        return Ok(());
    }

    let events = match source {
        "stripe" => StripeConnector::new(config.fixtures_dir()).pull(&Cursor::default())?,
        "slack" => SlackConnector::new(config.fixtures_dir()).pull(&Cursor::default())?,
        other => bail!("unknown source '{other}' (known: stripe, exa, slack)"),
    };

    index.raw_events.retain(|e| e.source_id != source);
    let n = events.len();
    index.raw_events.extend(events);

    let cards = synthesize(&store, &mut index)?;
    // GBrain-style self-wiring: extract [[wikilinks]] / rel:: markers from the
    // refreshed cards into typed link rows (zero LLM calls, spec 02 §1)
    let wired = crate::brain::links::self_wire(&store, &mut index);
    store.save_index(&index)?;

    println!(
        "ingested {n} events from '{source}'; synthesized {cards} card(s); wired {wired} link(s)"
    );
    if !index.anomalies.is_empty() {
        println!("⚠ {} anomaly(ies) flagged", index.anomalies.len());
    }
    Ok(())
}
