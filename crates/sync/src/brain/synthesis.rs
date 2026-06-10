//! Extract → synthesize → anomaly/learning (spec 02 §2).
//!
//! Deterministic, no-LLM synthesis: aggregates raw events per view/period into
//! Markdown context cards (acl-tagged, never mixing tags), detects spend spikes
//! against a rolling baseline, and lets human `learning` corrections suppress
//! re-flags (Flow C). Taint propagation: every derived row's `acl_tag` is ≥ its
//! inputs (spec 02 §3).

use std::collections::BTreeMap;

use chrono::Utc;

use crate::brain::markdown::{render_card, slug, CardMeta};
use crate::brain::{Anomaly, BrainIndex, Memory, MemoryKind, Provenance};
use crate::connectors::AclTag;
use crate::store::Store;

/// Threshold for flagging a metric as anomalous vs. the rolling baseline.
const SPIKE_RATIO: f64 = 1.30;

/// Run the synthesis pipeline over the current index, writing Markdown cards and
/// (re)computing memories + anomalies. Returns the number of cards written.
pub fn synthesize(store: &Store, index: &mut BrainIndex) -> anyhow::Result<usize> {
    // recompute derived rows from scratch each pass (idempotent synthesis) —
    // but only the rows THIS pass derives: the spend/finance wiki cards below.
    // World facts, daydream hypotheses, and agent notes (`brain.remember`)
    // are owned by other passes and must survive every ingest.
    index
        .memories
        .retain(|m| !(m.kind == MemoryKind::Wiki && (m.topic == "spend" || m.topic == "finance")));
    index.anomalies.clear();

    let now = Utc::now().to_rfc3339();
    let mut cards_written = 0;

    // group spend_by_team events by period → gross/net sums
    let periods = aggregate_metric(index, "stripe/spend_by_team");

    for (period, agg) in &periods {
        let title = format!("Spend summary · {period}");
        let body = format!(
            "Across {} team(s): gross **${}**, net **${}**.\n",
            agg.count,
            fmt(agg.gross),
            fmt(agg.net),
        );
        let acl = AclTag {
            view: crate::access::View::new("stripe", "spend_by_team"),
            fields: vec!["team".into(), "period".into(), "gross".into(), "net".into()],
        };
        let meta = CardMeta {
            topic: "spend",
            kind: "wiki",
            period: Some(period),
            confidence: 0.9,
            acl_tag: &acl,
        };
        let slug_name = slug(&format!("summary-{period}"));
        let path = store.write_card("spend", &slug_name, &render_card(&meta, &title, &body))?;
        cards_written += 1;

        let memory_id = uuid::Uuid::new_v4().to_string();
        index.memories.push(Memory {
            id: memory_id.clone(),
            kind: MemoryKind::Wiki,
            topic: "spend".into(),
            path: path.display().to_string(),
            acl_tag: acl.clone(),
            confidence: 0.9,
            period: Some(period.clone()),
            supersedes: None,
            created_at: now.clone(),
        });
        let event_ids: Vec<String> = index
            .events_for_view("stripe/spend_by_team")
            .into_iter()
            .filter(|e| e.payload.get("period").and_then(|v| v.as_str()) == Some(period.as_str()))
            .map(|e| e.id.clone())
            .collect();
        for raw_event_id in event_ids {
            index.provenance.push(Provenance {
                memory_id: memory_id.clone(),
                raw_event_id,
            });
        }
    }

    // net-of-credits card from finance_private (CFO-rooted, privileged acl)
    let fin = aggregate_metric(index, "stripe/finance_private");
    for (period, agg) in &fin {
        let net_of_credits = agg.gross - agg.credits;
        let title = format!("Net-of-credits · {period}");
        let body = format!(
            "Gross **${}**, credits **${}** → net-of-credits **${}** at tier *{}*.\n",
            fmt(agg.gross),
            fmt(agg.credits),
            fmt(net_of_credits),
            agg.discount_tier,
        );
        let acl = AclTag {
            view: crate::access::View::new("stripe", "finance_private"),
            fields: vec!["gross".into(), "credits".into(), "discount_tier".into()],
        };
        let meta = CardMeta {
            topic: "finance",
            kind: "wiki",
            period: Some(period),
            confidence: 0.92,
            acl_tag: &acl,
        };
        let slug_name = slug(&format!("net-of-credits-{period}"));
        let path = store.write_card("finance", &slug_name, &render_card(&meta, &title, &body))?;
        cards_written += 1;
        index.memories.push(Memory {
            id: uuid::Uuid::new_v4().to_string(),
            kind: MemoryKind::Wiki,
            topic: "finance".into(),
            path: path.display().to_string(),
            acl_tag: acl,
            confidence: 0.92,
            period: Some(period.clone()),
            supersedes: None,
            created_at: now.clone(),
        });
    }

    // unit-economics card per product from product_economics (CFO-rooted). The
    // card's acl floor is finance_private{gross, credits} — the most privileged
    // tag over its inputs that an issued token can actually grant (tokens are
    // single-view; product_economics is surfaced only through these cards).
    let product_events: Vec<(String, serde_json::Value)> = index
        .events_for_view("stripe/product_economics")
        .into_iter()
        .map(|e| (e.id.clone(), e.payload.clone()))
        .collect();
    for (raw_event_id, p) in &product_events {
        let (Some(product), Some(period)) = (
            p.get("product").and_then(|v| v.as_str()),
            p.get("period").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        let units = p.get("units").and_then(|v| v.as_i64()).unwrap_or(0).max(1);
        let gross = p.get("gross").and_then(|v| v.as_i64()).unwrap_or(0);
        let credits = p.get("credits").and_then(|v| v.as_i64()).unwrap_or(0);
        let infra = p.get("infra_cost").and_then(|v| v.as_i64()).unwrap_or(0);
        let net = gross - credits;
        let contribution = net - infra;
        let margin_pct = if net > 0 { contribution * 100 / net } else { 0 };

        let title = format!("Unit economics · {product} · {period}");
        let body = format!(
            "{} active unit(s): gross **${}** (${}/unit), credits **${}** → net **${}** \
             (${}/unit). Infra cost **${}** (${}/unit) leaves contribution **${}** \
             (${}/unit) — a **{}%** contribution margin.\n",
            fmt(units),
            fmt(gross),
            fmt(gross / units),
            fmt(credits),
            fmt(net),
            fmt(net / units),
            fmt(infra),
            fmt(infra / units),
            fmt(contribution),
            fmt(contribution / units),
            margin_pct,
        );
        let acl = AclTag {
            view: crate::access::View::new("stripe", "finance_private"),
            fields: vec!["gross".into(), "credits".into()],
        };
        let meta = CardMeta {
            topic: "products",
            kind: "wiki",
            period: Some(period),
            confidence: 0.88,
            acl_tag: &acl,
        };
        let slug_name = slug(&format!("unit-economics-{product}-{period}"));
        let path = store.write_card("products", &slug_name, &render_card(&meta, &title, &body))?;
        cards_written += 1;
        let memory_id = uuid::Uuid::new_v4().to_string();
        index.memories.push(Memory {
            id: memory_id.clone(),
            kind: MemoryKind::Wiki,
            topic: "products".into(),
            path: path.display().to_string(),
            acl_tag: acl,
            confidence: 0.88,
            period: Some(period.to_string()),
            supersedes: None,
            created_at: now.clone(),
        });
        index.provenance.push(Provenance {
            memory_id,
            raw_event_id: raw_event_id.clone(),
        });
    }

    detect_anomalies(index, "stripe/spend_by_team", "gross");

    // the rebuilt wiki rows carry fresh ids — drop provenance/links that now
    // point at removed memories (self-wiring re-resolves topic markers after
    // every synthesis pass, so surviving cards re-attach to the new rows)
    let live: std::collections::HashSet<&str> =
        index.memories.iter().map(|m| m.id.as_str()).collect();
    index
        .provenance
        .retain(|p| live.contains(p.memory_id.as_str()));
    index
        .links
        .retain(|l| live.contains(l.from.as_str()) && live.contains(l.to.as_str()));

    Ok(cards_written)
}

/// Detect spend spikes for a metric against the rolling baseline (mean of prior
/// periods). Suppressed when a `learning` row covers the metric (Flow C).
pub fn detect_anomalies(index: &mut BrainIndex, view_id: &str, metric: &str) {
    let agg = aggregate_metric(index, view_id);
    if agg.len() < 2 {
        return; // need a baseline
    }
    let mut periods: Vec<(&String, f64)> = agg
        .iter()
        .map(|(p, a)| {
            (
                p,
                if metric == "net" {
                    a.net as f64
                } else {
                    a.gross as f64
                },
            )
        })
        .collect();
    periods.sort_by(|a, b| a.0.cmp(b.0));

    let (latest_period, observed) = *periods.last().unwrap();
    let prior: Vec<f64> = periods[..periods.len() - 1]
        .iter()
        .map(|(_, v)| *v)
        .collect();
    let baseline = prior.iter().sum::<f64>() / prior.len() as f64;
    if baseline <= 0.0 || observed <= baseline * SPIKE_RATIO {
        return;
    }

    let metric_key = format!("{view_id}:{metric}");
    let suppressed = index.learnings.iter().any(|l| {
        l.suppresses_metric.as_deref() == Some(&metric_key)
            && l.applies_from.as_str() <= latest_period.as_str()
    });
    if suppressed {
        return;
    }

    index.anomalies.push(Anomaly {
        id: uuid::Uuid::new_v4().to_string(),
        view: view_id.to_string(),
        metric: metric.to_string(),
        period: latest_period.clone(),
        baseline,
        observed,
        severity: observed / baseline,
        acl_tag: AclTag {
            view: crate::access::View::new("stripe", "spend_by_team"),
            fields: vec!["team".into(), "period".into(), "gross".into(), "net".into()],
        },
        memory_id: None,
    });
}

#[derive(Default)]
struct Agg {
    count: u32,
    gross: i64,
    net: i64,
    credits: i64,
    discount_tier: String,
}

fn aggregate_metric(index: &BrainIndex, view_id: &str) -> BTreeMap<String, Agg> {
    let mut out: BTreeMap<String, Agg> = BTreeMap::new();
    for e in index.events_for_view(view_id) {
        let p = e
            .payload
            .get("period")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let a = out.entry(p).or_default();
        a.count += 1;
        a.gross += e.payload.get("gross").and_then(|v| v.as_i64()).unwrap_or(0);
        a.net += e.payload.get("net").and_then(|v| v.as_i64()).unwrap_or(0);
        a.credits += e
            .payload
            .get("credits")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if a.discount_tier.is_empty() {
            if let Some(t) = e.payload.get("discount_tier").and_then(|v| v.as_str()) {
                a.discount_tier = t.to_string();
            }
        }
    }
    out
}

fn fmt(n: i64) -> String {
    // thousands separators
    let s = n.abs().to_string();
    let mut chunks: Vec<String> = Vec::new();
    let bytes = s.as_bytes();
    let mut i = bytes.len() as isize;
    while i > 0 {
        let start = (i - 3).max(0) as usize;
        chunks.push(s[start..i as usize].to_string());
        i -= 3;
    }
    chunks.reverse();
    let body = chunks.join(",");
    if n < 0 {
        format!("-{body}")
    } else {
        body
    }
}
