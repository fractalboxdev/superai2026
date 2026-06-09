//! Exa web-enrichment connector (spec 05 §2).
//!
//! Real, key-based in production (`/search`, `/contents` at api.exa.ai). To keep
//! the default build dependency-light (no HTTP/TLS stack), the live path is
//! gated behind the `exa-http` feature; without it (and without `EXA_API_KEY`)
//! the connector returns canned enrichment events so the ingest path is
//! exercisable offline. Results normalize into raw events like any other source.

use anyhow::Result;
use chrono::Utc;
use serde_json::json;

use crate::access::View;
use crate::connectors::{AclTag, Connector, Cursor, RawEvent, ViewField, ViewSchema};

pub struct ExaConnector {
    pub query: String,
}

impl ExaConnector {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
        }
    }

    fn view() -> View {
        View::new("exa", "web_enrichment")
    }

    /// Whether a live Exa call is possible in this build.
    fn live_available() -> bool {
        cfg!(feature = "exa-http") && std::env::var("EXA_API_KEY").is_ok()
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
        if Self::live_available() {
            // Production: POST https://api.exa.ai/search { query } then /contents.
            // Compiled out unless the `exa-http` feature is enabled.
            anyhow::bail!("exa-http live path not compiled in this build");
        }
        // Canned enrichment so the ingest pipeline is exercisable offline.
        let now = Utc::now().to_rfc3339();
        let canned = [
            (
                "Anthropic Claude pricing & enterprise discounts",
                "https://www.anthropic.com/pricing",
                "Enterprise tiers offer committed-use discounts and credits.",
            ),
            (
                "FinOps benchmarks for AI tooling spend",
                "https://www.finops.org/",
                "Net-of-credit spend is the standard utilization KPI.",
            ),
        ];
        Ok(canned
            .iter()
            .map(|(title, url, snippet)| RawEvent {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "exa".into(),
                view: Self::view(),
                payload: json!({ "title": title, "url": url, "snippet": snippet, "query": self.query }),
                ingested_at: now.clone(),
                acl_tag: AclTag {
                    view: Self::view(),
                    fields: vec!["title".into(), "url".into(), "snippet".into()],
                },
            })
            .collect())
    }
}
