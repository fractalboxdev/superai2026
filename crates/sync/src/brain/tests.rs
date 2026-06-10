//! Brain tests (spec 09 §2 brain layer): synthesis aggregates, anomaly→learning
//! suppression (Flow C), and the card-scrub property (an under-authorized caller
//! never receives a card whose acl_tag it can't fully satisfy).

use serde_json::json;

use crate::access::biscuit::attenuate;
use crate::access::request::{approve_request, AccessRequest};
use crate::access::{AttenuationBlock, View};
use crate::brain::synthesis::{detect_anomalies, synthesize};
use crate::brain::{retrieval, BrainIndex, Learning};
use crate::config::{Config, InferenceBackend};
use crate::connectors::stripe::StripeConnector;
use crate::connectors::{AclTag, Connector, Cursor, RawEvent};
use crate::scenario;
use crate::store::Store;

fn temp_store() -> Store {
    let root = std::env::temp_dir().join(format!("contextful-test-{}", uuid::Uuid::new_v4()));
    Store::new(Config {
        root,
        inference: InferenceBackend::Stub,
    })
}

fn ingest_stripe(index: &mut BrainIndex, store: &Store) {
    let conn = StripeConnector::new(store.config.fixtures_dir());
    index
        .raw_events
        .extend(conn.pull(&Cursor::default()).unwrap());
}

fn spend_event(period: &str, team: &str, gross: i64) -> RawEvent {
    RawEvent {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: "stripe".into(),
        view: View::new("stripe", "spend_by_team"),
        payload: json!({ "team": team, "period": period, "gross": gross, "net": gross }),
        ingested_at: "2026-01-01T00:00:00Z".into(),
        acl_tag: AclTag {
            view: View::new("stripe", "spend_by_team"),
            fields: vec!["team".into(), "period".into(), "gross".into(), "net".into()],
        },
    }
}

#[test]
fn synthesis_writes_cards_and_aggregates() {
    let store = temp_store();
    let mut index = BrainIndex::default();
    ingest_stripe(&mut index, &store);

    let written = synthesize(&store, &mut index).unwrap();
    assert!(written >= 2, "expected spend + finance cards");
    assert!(index.memories.iter().any(|m| m.topic == "spend"));
    assert!(index.memories.iter().any(|m| m.topic == "finance"));
    // provenance links spend memory back to raw events
    assert!(!index.provenance.is_empty());
}

/// The scheduled research → memory pipeline (spec 02 §8/§9): exa world cards
/// and daydream hypotheses must survive the next connector re-synthesis, and
/// the daydream loop must not re-dream pairs it already connected.
#[test]
fn world_and_daydream_memories_survive_resynthesis() {
    use crate::brain::MemoryKind;

    fn count(index: &BrainIndex, kind: MemoryKind) -> usize {
        index.memories.iter().filter(|m| m.kind == kind).count()
    }

    let store = temp_store();
    store.config.ensure_dirs().unwrap();
    let mut index = BrainIndex::default();
    ingest_stripe(&mut index, &store);
    synthesize(&store, &mut index).unwrap();

    // scheduled research (offline: the seed/cache path — still world cards)
    let world = crate::brain::world::world_search(
        &store.config,
        &store,
        &mut index,
        crate::cron::EXA_RESEARCH_QUERY,
    )
    .unwrap();
    assert!(!world.is_empty(), "research must synthesize world cards");

    // nightly daydream connects spend/finance/world cards into hypotheses
    let dreamed = crate::brain::daydream::cycle(&store.config, &store, &mut index, 10).unwrap();
    assert!(dreamed >= 1, "daydream must write at least one hypothesis");

    let world_n = count(&index, MemoryKind::WorldFact);
    let dd_n = count(&index, MemoryKind::Daydream);

    // the next hourly stripe ingest re-synthesizes — memory survives
    synthesize(&store, &mut index).unwrap();
    crate::brain::links::self_wire(&store, &mut index);
    assert_eq!(count(&index, MemoryKind::WorldFact), world_n);
    assert_eq!(count(&index, MemoryKind::Daydream), dd_n);

    // and the loop stays idempotent even though parent ids were regenerated
    let again = crate::brain::daydream::cycle(&store.config, &store, &mut index, 10).unwrap();
    assert_eq!(again, 0, "must not re-dream already-connected pairs");
}

#[test]
fn flow_a_granted_token_sees_credits_but_not_salary() {
    let store = temp_store();
    let mut index = BrainIndex::default();
    ingest_stripe(&mut index, &store);

    let req = AccessRequest {
        id: "r".into(),
        requester: "agent:cto/1".into(),
        view: scenario::finance_private(),
        fields: vec!["gross".into(), "credits".into(), "discount_tier".into()],
        row_scope: Some(retrieval::all_teams_scope()),
        reason: "net-of-credits".into(),
        doc: "finops".into(),
        ttl: "7d".into(),
    };
    let granted = approve_request(&scenario::cfo_capability(), &req).unwrap();

    match retrieval::query(
        &index,
        &granted,
        &scenario::finance_private(),
        &[
            "gross".into(),
            "credits".into(),
            "discount_tier".into(),
            "employee_salary".into(),
        ],
    ) {
        retrieval::QueryResult::Ok {
            fields,
            redacted,
            rows,
            ..
        } => {
            assert!(fields.contains(&"credits".to_string()));
            assert!(redacted.contains(&"employee_salary".to_string()));
            assert!(rows.iter().all(|r| r.get("employee_salary").is_none()));
        }
        other => panic!("expected Ok, got {other:?}"),
    }
}

#[test]
fn anomaly_flagged_then_suppressed_by_learning() {
    let store = temp_store();
    let mut index = BrainIndex::default();
    // baseline month (low) + current month (3x spike)
    for team in ["eng", "ops"] {
        index.raw_events.push(spend_event("2026-04", team, 10_000));
        index.raw_events.push(spend_event("2026-05", team, 35_000));
    }

    detect_anomalies(&mut index, "stripe/spend_by_team", "gross");
    assert_eq!(index.anomalies.len(), 1, "spike should be flagged");

    // human correction: "one-off backfill, not a trend"
    index.anomalies.clear();
    index.learnings.push(Learning {
        id: "l1".into(),
        topic: "spend".into(),
        statement: "May spike is a one-off backfill, not a trend.".into(),
        applies_from: "2026-05".into(),
        acl_tag: AclTag {
            view: View::new("stripe", "spend_by_team"),
            fields: vec!["gross".into()],
        },
        provenance_id: None,
        source: "human".into(),
        suppresses_metric: Some("stripe/spend_by_team:gross".into()),
    });

    detect_anomalies(&mut index, "stripe/spend_by_team", "gross");
    assert_eq!(
        index.anomalies.len(),
        0,
        "learning should suppress the re-flag"
    );
    let _ = store; // store unused here; kept for symmetry
}

#[test]
fn card_scrub_under_authorized_caller_denied() {
    let store = temp_store();
    let mut index = BrainIndex::default();
    ingest_stripe(&mut index, &store);
    synthesize(&store, &mut index).unwrap();

    // CTO base token holds spend_by_team only — must NOT read the finance card.
    let cto = scenario::cto_agent_capability();
    let res = retrieval::get_context(&store, &index, &cto, "finance");
    assert!(res.is_err(), "finance card requires finance_private grant");

    // but the public spend card is readable
    assert!(retrieval::get_context(&store, &index, &cto, "spend").is_ok());
}

fn spike_index() -> BrainIndex {
    let mut index = BrainIndex::default();
    for team in ["eng", "ops"] {
        index.raw_events.push(spend_event("2026-04", team, 10_000));
        index.raw_events.push(spend_event("2026-05", team, 35_000));
    }
    index
}

fn learning(applies_from: &str, metric: &str) -> Learning {
    Learning {
        id: "l".into(),
        topic: "spend".into(),
        statement: "annotation".into(),
        applies_from: applies_from.into(),
        acl_tag: AclTag {
            view: View::new("stripe", "spend_by_team"),
            fields: vec!["gross".into()],
        },
        provenance_id: None,
        source: "human".into(),
        suppresses_metric: Some(metric.into()),
    }
}

#[test]
fn learning_in_future_period_does_not_suppress() {
    let mut index = spike_index();
    // a correction that only applies from a LATER period must not suppress the May spike
    index
        .learnings
        .push(learning("2026-06", "stripe/spend_by_team:gross"));
    detect_anomalies(&mut index, "stripe/spend_by_team", "gross");
    assert_eq!(
        index.anomalies.len(),
        1,
        "future-dated learning must not suppress"
    );
}

#[test]
fn learning_for_other_metric_does_not_suppress() {
    let mut index = spike_index();
    // a learning about :net must not suppress a :gross anomaly
    index
        .learnings
        .push(learning("2026-05", "stripe/spend_by_team:net"));
    detect_anomalies(&mut index, "stripe/spend_by_team", "gross");
    assert_eq!(
        index.anomalies.len(),
        1,
        "metric mismatch must not suppress"
    );
}

#[test]
fn detect_anomalies_denied_without_gross_field() {
    // a token that can query the view but is NOT granted `gross` must not get
    // gross-derived anomaly figures (field redaction enforced on the tool).
    let mut index = spike_index();
    detect_anomalies(&mut index, "stripe/spend_by_team", "gross");
    let no_gross = attenuate(
        &scenario::cto_agent_capability(),
        AttenuationBlock {
            by: "cto".into(),
            deny_fields: vec!["gross".into()],
            ..Default::default()
        },
    );
    assert!(matches!(
        retrieval::detect_anomalies(&index, &no_gross, &scenario::spend_by_team()),
        retrieval::QueryResult::Denied { .. }
    ));
}
