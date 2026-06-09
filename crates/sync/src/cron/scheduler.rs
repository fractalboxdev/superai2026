//! Cron scheduler (spec 05 §3) — the context layer that keeps the brain fresh.
//!
//! Triggers ETL pipelines on a schedule. Real cron-expression parsing +
//! per-source schedules from config are future work; this runs an initial
//! ingest then re-ingests on a fixed interval (`CONTEXTFUL_CRON_SECS`, default
//! 3600s) until interrupted.

use anyhow::Result;
use tokio::time::{interval, Duration};

pub async fn run() -> Result<()> {
    let secs: u64 = std::env::var("CONTEXTFUL_CRON_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);
    tracing::info!(
        interval_secs = secs,
        "cron scheduler started (stripe pipeline)"
    );

    let mut tick = interval(Duration::from_secs(secs));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(e) = super::ingest_once("stripe") {
                    tracing::error!(error = %e, "scheduled ingest failed");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("cron scheduler stopping");
                return Ok(());
            }
        }
    }
}
