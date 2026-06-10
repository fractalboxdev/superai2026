//! Exa web-enrichment connector (spec 05 §2) — real, key-based.
//!
//! With `EXA_API_KEY` set, `pull` POSTs to `https://api.exa.ai/search`
//! (contents inlined) and normalizes results into raw events like any other
//! source; every successful live pull refreshes the on-host cache
//! (`fixtures/exa-cache.json`). Without a key — Flow D offline mode — the
//! connector serves the cache, falling back to a seeded snapshot so the
//! ingest path stays exercisable with zero egress. Callers are responsible
//! for passing the query through the egress firewall first
//! ([`crate::access::egress`]); [`crate::brain::world`] does exactly that.

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::access::View;
use crate::connectors::{AclTag, Connector, Cursor, RawEvent, ViewField, ViewSchema};

pub const EXA_SEARCH_URL: &str = "https://api.exa.ai/search";

pub struct ExaConnector {
    pub query: String,
    /// cache file for offline mode (usually `<CONTEXTFUL_HOME>/fixtures/exa-cache.json`).
    pub cache_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExaResult {
    title: Option<String>,
    url: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExaResponse {
    #[serde(default)]
    results: Vec<ExaResult>,
}

impl ExaConnector {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            cache_path: None,
        }
    }

    pub fn with_cache(query: impl Into<String>, cache_path: PathBuf) -> Self {
        Self {
            query: query.into(),
            cache_path: Some(cache_path),
        }
    }

    fn view() -> View {
        View::new("exa", "web_enrichment")
    }

    fn api_key() -> Option<String> {
        std::env::var("EXA_API_KEY").ok().filter(|k| !k.is_empty())
    }

    /// Live `/search` call (contents inlined via the `contents` option).
    fn search_live(&self, key: &str) -> Result<Vec<ExaResult>> {
        let body = json!({
            "query": self.query,
            "numResults": 5,
            "contents": { "text": { "maxCharacters": 500 } }
        });
        let resp: ExaResponse = ureq::post(EXA_SEARCH_URL)
            .set("x-api-key", key)
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(20))
            .send_json(body)
            .context("exa /search request failed")?
            .into_json()
            .context("exa /search response was not JSON")?;
        Ok(resp.results)
    }

    fn load_cache(&self) -> Option<Vec<ExaResult>> {
        let path = self.cache_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save_cache(&self, results: &[ExaResult]) {
        if let Some(path) = &self.cache_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let entries: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    json!({ "title": r.title, "url": r.url, "text": r.text, "summary": r.summary })
                })
                .collect();
            let _ = std::fs::write(
                path,
                serde_json::to_string_pretty(&entries).unwrap_or_default(),
            );
        }
    }

    /// Seeded snapshot for a cold offline start (cache-shaped, not a mock of
    /// the API — the same rows a prior online run would have cached).
    fn seed_snapshot() -> Vec<ExaResult> {
        vec![
            ExaResult {
                title: Some("Anthropic Claude pricing & enterprise discounts".into()),
                url: "https://www.anthropic.com/pricing".into(),
                text: Some("Enterprise tiers offer committed-use discounts and credits.".into()),
                summary: None,
            },
            ExaResult {
                title: Some("FinOps benchmarks for AI tooling spend".into()),
                url: "https://www.finops.org/".into(),
                text: Some("Net-of-credit spend is the standard utilization KPI.".into()),
                summary: None,
            },
        ]
    }

    fn to_events(&self, results: &[ExaResult]) -> Vec<RawEvent> {
        let now = Utc::now().to_rfc3339();
        results
            .iter()
            .map(|r| {
                let snippet = r
                    .summary
                    .clone()
                    .or_else(|| r.text.clone())
                    .unwrap_or_default();
                RawEvent {
                    id: uuid::Uuid::new_v4().to_string(),
                    source_id: "exa".into(),
                    view: Self::view(),
                    payload: json!({
                        "title": r.title.clone().unwrap_or_else(|| r.url.clone()),
                        "url": r.url,
                        "snippet": snippet,
                        "query": self.query,
                    }),
                    ingested_at: now.clone(),
                    acl_tag: AclTag {
                        view: Self::view(),
                        fields: vec!["title".into(), "url".into(), "snippet".into()],
                    },
                }
            })
            .collect()
    }
}

impl Connector for ExaConnector {
    fn source_id(&self) -> &str {
        "exa"
    }

    fn views(&self) -> Vec<ViewSchema> {
        vec![ViewSchema {
            view: Self::view(),
            fields: vec![
                ViewField {
                    name: "title".into(),
                    ty: "string".into(),
                    private: false,
                },
                ViewField {
                    name: "url".into(),
                    ty: "string".into(),
                    private: false,
                },
                ViewField {
                    name: "snippet".into(),
                    ty: "string".into(),
                    private: false,
                },
            ],
        }]
    }

    fn pull(&self, _since: &Cursor) -> Result<Vec<RawEvent>> {
        if let Some(key) = Self::api_key() {
            match self.search_live(&key) {
                Ok(results) => {
                    self.save_cache(&results);
                    tracing::info!(n = results.len(), query = %self.query, "exa live search");
                    return Ok(self.to_events(&results));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "exa live search failed — serving cache");
                }
            }
        }
        // Flow D offline: cache, then seeded snapshot
        let cached = self.load_cache().unwrap_or_else(Self::seed_snapshot);
        Ok(self.to_events(&cached))
    }
}
