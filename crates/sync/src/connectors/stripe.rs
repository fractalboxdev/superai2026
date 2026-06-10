//! Stripe (mock) connector — primary demo source (spec 05 §2).
//!
//! Stripe-shaped FinOps data mapped to three views:
//!   - `stripe/spend_by_team`  → team, period, gross, net
//!   - `stripe/finance_private` → adds discount_tier, credits, employee_salary (CFO-rooted)
//!   - `stripe/product_economics` → per-product revenue/units/infra (CFO-rooted;
//!     surfaced only through synthesized cards tagged `finance_private`)
//!
//! Reads CSV fixtures from `<fixtures>/stripe/*.csv` when present (Kaggle-derived
//! in production); falls back to embedded rows so `ingest` works out of the box.

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

    /// Load rows from `<fixtures>/stripe/finance.csv` if present, else embedded.
    fn rows(&self) -> Vec<OwnedRow> {
        let csv = self.fixtures_dir.join("stripe").join("finance.csv");
        if let Ok(text) = std::fs::read_to_string(&csv) {
            if let Some(rows) = parse_csv(&text) {
                return rows;
            }
        }
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
                    f("gross", "int", true),
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
