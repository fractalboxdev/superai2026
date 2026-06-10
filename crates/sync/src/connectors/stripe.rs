//! Stripe connector — primary demo source (spec 05 §2).
//!
//! Stripe-shaped FinOps data mapped to three views:
//!   - `stripe/spend_by_team`  → team, period, gross, net
//!   - `stripe/finance_private` → adds discount_tier, credits, employee_salary (CFO-rooted)
//!   - `stripe/product_economics` → per-product revenue/units/infra (CFO-rooted;
//!     surfaced only through synthesized cards tagged `finance_private`)
//!
//! With `STRIPE_SECRET_KEY` set, `pull` lists real (test-mode) charges from
//! `api.stripe.com` and aggregates gross/net per team (`metadata.team`, else
//! `unattributed`) for the charge's month, overlaying the fixture rows.
//! The HR-side private columns (employee_salary, discount_tier, credits) are
//! company data that never lives in Stripe — they always come from the local
//! fixture (`<fixtures>/stripe/finance.csv`, else embedded rows) so `ingest`
//! works out of the box and offline (Flow D).

use anyhow::Result;
use chrono::Utc;
use serde_json::json;

use crate::access::View;
use crate::connectors::{AclTag, Connector, Cursor, RawEvent, ViewField, ViewSchema};

/// One team's monthly figures (the union of both views' columns).
struct TeamRow {
    team: &'static str,
    period: &'static str,
    gross: i64,
    net: i64,
    discount_tier: &'static str,
    credits: i64,
    employee_salary: i64,
}

const EMBEDDED: &[TeamRow] = &[
    TeamRow {
        team: "eng",
        period: "2026-05",
        gross: 100_000,
        net: 78_000,
        discount_tier: "Enterprise (Tier 3)",
        credits: 22_000,
        employee_salary: 240_000,
    },
    TeamRow {
        team: "ops",
        period: "2026-05",
        gross: 30_000,
        net: 26_000,
        discount_tier: "Enterprise (Tier 3)",
        credits: 4_000,
        employee_salary: 210_000,
    },
    TeamRow {
        team: "sales",
        period: "2026-05",
        gross: 20_000,
        net: 17_000,
        discount_tier: "Enterprise (Tier 3)",
        credits: 3_000,
        employee_salary: 200_000,
    },
    TeamRow {
        team: "finance",
        period: "2026-05",
        gross: 15_000,
        net: 13_000,
        discount_tier: "Enterprise (Tier 3)",
        credits: 2_000,
        employee_salary: 230_000,
    },
];

/// One product line's monthly figures (Stripe subscription revenue side).
struct ProductRow {
    product: &'static str,
    period: &'static str,
    /// active paid units (subscriptions) in the period.
    units: i64,
    gross: i64,
    credits: i64,
    /// serving/infra cost attributable to the product.
    infra_cost: i64,
}

const EMBEDDED_PRODUCTS: &[ProductRow] = &[
    ProductRow {
        product: "compression",
        period: "2026-05",
        units: 1_840,
        gross: 92_000,
        credits: 9_200,
        infra_cost: 27_600,
    },
    ProductRow {
        product: "inference",
        period: "2026-05",
        units: 3_100,
        gross: 124_000,
        credits: 18_600,
        infra_cost: 74_400,
    },
];

pub struct StripeConnector {
    fixtures_dir: std::path::PathBuf,
}

impl StripeConnector {
    pub fn new(fixtures_dir: std::path::PathBuf) -> Self {
        Self { fixtures_dir }
    }

    fn spend_view() -> View {
        View::new("stripe", "spend_by_team")
    }
    fn private_view() -> View {
        View::new("stripe", "finance_private")
    }
    pub fn product_view() -> View {
        View::new("stripe", "product_economics")
    }

    /// Load rows from `<fixtures>/stripe/finance.csv` if present, else embedded,
    /// then overlay live test-mode aggregates when `STRIPE_SECRET_KEY` is set.
    fn rows(&self) -> Vec<OwnedRow> {
        let csv = self.fixtures_dir.join("stripe").join("finance.csv");
        let mut rows = std::fs::read_to_string(&csv)
            .ok()
            .and_then(|text| parse_csv(&text))
            .unwrap_or_else(|| {
                EMBEDDED
                    .iter()
                    .map(|r| OwnedRow {
                        team: r.team.into(),
                        period: r.period.into(),
                        gross: r.gross,
                        net: r.net,
                        discount_tier: r.discount_tier.into(),
                        credits: r.credits,
                        employee_salary: r.employee_salary,
                    })
                    .collect()
            });

        if let Some(key) = crate::config::nonempty_env("STRIPE_SECRET_KEY") {
            match live_team_aggregates(&key) {
                Ok(live) if !live.is_empty() => overlay_live(&mut rows, live),
                Ok(_) => tracing::info!("stripe live: no charges yet — fixture rows only"),
                Err(e) => tracing::warn!(error = %e, "stripe live pull failed — fixture rows"),
            }
        }
        rows
    }
}

/// Live aggregate per (team, period): gross/net cents→whole-currency sums.
struct LiveAgg {
    team: String,
    period: String,
    gross: i64,
    net: i64,
}

/// List up to 100 recent charges from the real (test-mode) Stripe API and
/// aggregate amount/amount_captured per `metadata.team` and month.
fn live_team_aggregates(key: &str) -> Result<Vec<LiveAgg>> {
    use std::collections::BTreeMap;
    let resp: serde_json::Value = ureq::get("https://api.stripe.com/v1/charges?limit=100")
        .set("authorization", &format!("Bearer {key}"))
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| anyhow::anyhow!("stripe /v1/charges: {e}"))?
        .into_json()?;
    let mut agg: BTreeMap<(String, String), (i64, i64)> = BTreeMap::new();
    for c in resp
        .get("data")
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
    {
        if c.get("paid").and_then(|p| p.as_bool()) != Some(true) {
            continue;
        }
        let team = c
            .pointer("/metadata/team")
            .and_then(|t| t.as_str())
            .unwrap_or("unattributed")
            .to_string();
        let created = c.get("created").and_then(|t| t.as_i64()).unwrap_or(0);
        let period = chrono::DateTime::from_timestamp(created, 0)
            .map(|d| d.format("%Y-%m").to_string())
            .unwrap_or_default();
        let amount = c.get("amount").and_then(|a| a.as_i64()).unwrap_or(0) / 100;
        let captured = c
            .get("amount_captured")
            .and_then(|a| a.as_i64())
            .unwrap_or(0)
            / 100;
        let e = agg.entry((team, period)).or_insert((0, 0));
        e.0 += amount;
        e.1 += captured;
    }
    Ok(agg
        .into_iter()
        .map(|((team, period), (gross, net))| LiveAgg {
            team,
            period,
            gross,
            net,
        })
        .collect())
}

/// Live gross/net replace the fixture figures for matching (team, period);
/// unseen live teams append with empty private columns (Stripe never carries
/// salary/discount data).
fn overlay_live(rows: &mut Vec<OwnedRow>, live: Vec<LiveAgg>) {
    for l in live {
        match rows
            .iter_mut()
            .find(|r| r.team == l.team && r.period == l.period)
        {
            Some(r) => {
                r.gross = l.gross;
                r.net = l.net;
            }
            None => rows.push(OwnedRow {
                team: l.team,
                period: l.period,
                gross: l.gross,
                net: l.net,
                discount_tier: String::new(),
                credits: 0,
                employee_salary: 0,
            }),
        }
    }
}

struct OwnedRow {
    team: String,
    period: String,
    gross: i64,
    net: i64,
    discount_tier: String,
    credits: i64,
    employee_salary: i64,
}

/// Minimal CSV parse for our controlled fixture (no embedded commas/quotes).
/// Header: team,period,gross,net,discount_tier,credits,employee_salary
fn parse_csv(text: &str) -> Option<Vec<OwnedRow>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let _header = lines.next()?;
    let mut out = Vec::new();
    for line in lines {
        let c: Vec<&str> = line.split(',').collect();
        if c.len() < 7 {
            return None;
        }
        out.push(OwnedRow {
            team: c[0].trim().to_string(),
            period: c[1].trim().to_string(),
            gross: c[2].trim().parse().ok()?,
            net: c[3].trim().parse().ok()?,
            discount_tier: c[4].trim().to_string(),
            credits: c[5].trim().parse().ok()?,
            employee_salary: c[6].trim().parse().ok()?,
        });
    }
    Some(out)
}

impl Connector for StripeConnector {
    fn source_id(&self) -> &str {
        "stripe"
    }

    fn views(&self) -> Vec<ViewSchema> {
        let f = |name: &str, ty: &str, private: bool| ViewField {
            name: name.into(),
            ty: ty.into(),
            private,
        };
        vec![
            ViewSchema {
                view: Self::spend_view(),
                fields: vec![
                    f("team", "string", false),
                    f("period", "string", false),
                    f("gross", "int", false),
                    f("net", "int", false),
                ],
            },
            ViewSchema {
                view: Self::private_view(),
                fields: vec![
                    f("team", "string", false),
                    f("period", "string", false),
                    f("gross", "int", false),
                    f("net", "int", false),
                    f("discount_tier", "string", true),
                    f("credits", "int", true),
                    f("employee_salary", "int", true),
                ],
            },
            ViewSchema {
                view: Self::product_view(),
                fields: vec![
                    f("product", "string", false),
                    f("period", "string", false),
                    f("units", "int", true),
                    // gross stays non-private by schema, same convention as the
                    // team views: aggregated totals may appear in outbound queries
                    f("gross", "int", false),
                    f("credits", "int", true),
                    f("infra_cost", "int", true),
                ],
            },
        ]
    }

    fn pull(&self, _since: &Cursor) -> Result<Vec<RawEvent>> {
        let now = Utc::now().to_rfc3339();
        let mut events = Vec::new();
        for r in self.rows() {
            // spend_by_team event (non-private)
            events.push(RawEvent {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "stripe".into(),
                view: Self::spend_view(),
                payload: json!({ "team": r.team, "period": r.period, "gross": r.gross, "net": r.net }),
                ingested_at: now.clone(),
                acl_tag: AclTag {
                    view: Self::spend_view(),
                    fields: vec!["team".into(), "period".into(), "gross".into(), "net".into()],
                },
            });
            // finance_private event (carries the privileged columns)
            events.push(RawEvent {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "stripe".into(),
                view: Self::private_view(),
                payload: json!({
                    "team": r.team, "period": r.period, "gross": r.gross, "net": r.net,
                    "discount_tier": r.discount_tier, "credits": r.credits,
                    "employee_salary": r.employee_salary,
                }),
                ingested_at: now.clone(),
                acl_tag: AclTag {
                    view: Self::private_view(),
                    fields: vec![
                        "team".into(),
                        "period".into(),
                        "gross".into(),
                        "net".into(),
                        "discount_tier".into(),
                        "credits".into(),
                        "employee_salary".into(),
                    ],
                },
            });
        }
        // product_economics events (per-product revenue side; CFO-rooted)
        for r in EMBEDDED_PRODUCTS {
            events.push(RawEvent {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: "stripe".into(),
                view: Self::product_view(),
                payload: json!({
                    "product": r.product, "period": r.period, "units": r.units,
                    "gross": r.gross, "credits": r.credits, "infra_cost": r.infra_cost,
                }),
                ingested_at: now.clone(),
                acl_tag: AclTag {
                    view: Self::product_view(),
                    fields: vec![
                        "product".into(),
                        "period".into(),
                        "units".into(),
                        "gross".into(),
                        "credits".into(),
                        "infra_cost".into(),
                    ],
                },
            });
        }
        Ok(events)
    }
}
