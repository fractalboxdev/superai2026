//! Connectors & ETL orchestration (spec 05 §3): one-shot `ingest` and the
//! scheduled `cron` pipeline that keeps the brain fresh.

pub mod scheduler;

use anyhow::{bail, Result};

use crate::brain::synthesis::synthesize;
use crate::config::Config;
use crate::connectors::exa::ExaConnector;
use crate::connectors::stripe::StripeConnector;
use crate::connectors::{Connector, Cursor};
use crate::store::Store;

/// Run a connector's `pull`, write `raw_event` rows, and trigger synthesis.
/// Idempotent: prior events for the same source are replaced.
pub fn ingest_once(source: &str) -> Result<()> {
    let config = Config::load();
    let store = Store::new(config.clone());
    let mut index = store.load_index()?;

    let events = match source {
        "stripe" => StripeConnector::new(config.fixtures_dir()).pull(&Cursor::default())?,
        "exa" => ExaConnector::new("AI tooling spend benchmarks").pull(&Cursor::default())?,
        other => bail!("unknown source '{other}' (known: stripe, exa)"),
    };

    index.raw_events.retain(|e| e.source_id != source);
    let n = events.len();
    index.raw_events.extend(events);

    let cards = synthesize(&store, &mut index)?;
    store.save_index(&index)?;

    println!("ingested {n} events from '{source}'; synthesized {cards} card(s)");
    if !index.anomalies.is_empty() {
        println!("⚠ {} anomaly(ies) flagged", index.anomalies.len());
    }
    Ok(())
}
