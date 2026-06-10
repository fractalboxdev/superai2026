//! Cron scheduler (spec 05 §3) — the context layer that keeps the brain fresh.
//!
//! Real cron expressions (`croner`), per-job schedules from
//! `<CONTEXTFUL_HOME>/control/schedules.json` (written with defaults on first
//! run): connector ingests on their own cadence and the nightly daydream
//! cycle off-peak (spec 02 §9). The loop ticks every 30s and fires each job
//! at most once per matching minute.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Local};
use croner::Cron;
use serde::{Deserialize, Serialize};
use tokio::time::{interval, Duration};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// job name: a connector source (`stripe`, `exa`) or `daydream`.
    pub job: String,
    /// 5-field cron expression, local time.
    pub cron: String,
}

fn default_schedules() -> Vec<Schedule> {
    vec![
        Schedule {
            job: "stripe".into(),
            cron: "0 * * * *".into(), // hourly
        },
        Schedule {
            job: "exa".into(),
            cron: "0 6 * * *".into(), // daily, morning
        },
        Schedule {
            job: "daydream".into(),
            cron: "0 3 * * *".into(), // nightly, off-peak (spec 02 §9)
        },
    ]
}

/// Load schedules, writing the defaults on first run so they're editable.
pub fn load_schedules(config: &Config) -> Result<Vec<Schedule>> {
    let path = config.control_dir().join("schedules.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let defaults = default_schedules();
            config.ensure_dirs()?;
            std::fs::write(&path, serde_json::to_string_pretty(&defaults)?)?;
            Ok(defaults)
        }
        Err(e) => Err(e.into()),
    }
}

fn run_job(job: &str) -> Result<()> {
    match job {
        "daydream" => {
            let report = crate::brain::daydream::run_once()?;
            tracing::info!(%report, "daydream cycle complete");
            Ok(())
        }
        source => super::ingest_once(source),
    }
}

/// Is `at` (truncated to the minute) a firing minute for `pattern`?
///
/// The truncation is load-bearing: croner also matches the SECONDS field, and
/// a 5-field pattern compiles to "second 0 only" — the 30s tick virtually
/// never lands on second 0, so an untruncated `now` never fires any job.
fn matches_minute(pattern: &Cron, at: DateTime<Local>) -> bool {
    use chrono::Timelike;
    let at = at
        .with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(at);
    pattern.is_time_matching(&at).unwrap_or(false)
}

pub async fn run() -> Result<()> {
    let config = Config::load();
    let schedules = load_schedules(&config)?;
    let mut jobs: Vec<(Schedule, Cron)> = Vec::new();
    for s in schedules {
        let pattern = Cron::new(&s.cron)
            .parse()
            .map_err(|e| anyhow::anyhow!("bad cron '{}' for job '{}': {e}", s.cron, s.job))?;
        jobs.push((s, pattern));
    }
    tracing::info!(
        jobs = ?jobs.iter().map(|(s, _)| format!("{} @ {}", s.job, s.cron)).collect::<Vec<_>>(),
        "cron scheduler started"
    );

    // fire each job at most once per matching minute
    let mut last_fired: HashMap<String, String> = HashMap::new();
    let mut tick = interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = Local::now();
                let minute = now.format("%Y-%m-%dT%H:%M").to_string();
                for (s, pattern) in &jobs {
                    if matches_minute(pattern, now)
                        && last_fired.get(&s.job) != Some(&minute)
                    {
                        last_fired.insert(s.job.clone(), minute.clone());
                        tracing::info!(job = %s.job, "cron firing");
                        // jobs do blocking I/O (ureq pulls with 20s timeouts) —
                        // keep them off the async worker threads
                        let job = s.job.clone();
                        let done = tokio::task::spawn_blocking(move || run_job(&job)).await;
                        match done {
                            Ok(Err(e)) => {
                                tracing::error!(job = %s.job, error = %e, "scheduled job failed");
                            }
                            Err(e) => {
                                tracing::error!(job = %s.job, error = %e, "scheduled job panicked");
                            }
                            Ok(Ok(())) => {}
                        }
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("cron scheduler stopping");
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn default_schedules_parse_and_match() {
        for s in default_schedules() {
            let pattern = Cron::new(&s.cron).parse().expect("default cron parses");
            // 03:00 local matches the daydream schedule, 03:01 doesn't
            if s.job == "daydream" {
                let at = Local.with_ymd_and_hms(2026, 6, 10, 3, 0, 0).unwrap();
                assert!(matches_minute(&pattern, at));
                let off = Local.with_ymd_and_hms(2026, 6, 10, 3, 1, 0).unwrap();
                assert!(!matches_minute(&pattern, off));
            }
        }
    }

    /// Regression: the scheduler tick lands on arbitrary seconds — a matching
    /// minute must fire regardless (croner would otherwise demand second 0).
    #[test]
    fn matching_minute_fires_at_any_second() {
        let hourly = Cron::new("0 * * * *").parse().unwrap();
        let at = Local.with_ymd_and_hms(2026, 6, 10, 14, 0, 48).unwrap();
        assert!(matches_minute(&hourly, at));

        let every_minute = Cron::new("* * * * *").parse().unwrap();
        let at = Local.with_ymd_and_hms(2026, 6, 10, 14, 7, 31).unwrap();
        assert!(matches_minute(&every_minute, at));
    }

    #[test]
    fn schedules_file_roundtrip() {
        let root = std::env::temp_dir().join(format!("cron-test-{}", uuid::Uuid::new_v4()));
        let config = Config {
            root,
            inference: crate::config::InferenceBackend::Stub,
        };
        let first = load_schedules(&config).unwrap();
        assert_eq!(first.len(), 3);
        // the brain-freshness contract: connector ingest, world research,
        // and the nightly daydream cycle are all scheduled out of the box
        let jobs: Vec<&str> = first.iter().map(|s| s.job.as_str()).collect();
        for job in ["stripe", "exa", "daydream"] {
            assert!(
                jobs.contains(&job),
                "default schedules must include '{job}'"
            );
        }
        // second load reads the persisted file
        let second = load_schedules(&config).unwrap();
        assert_eq!(second.len(), 3);
    }
}
