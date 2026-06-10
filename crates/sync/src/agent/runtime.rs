//! Agent runtime loop (spec 04 §1, §3).
//!
//! An LLM loop whose **only tool surface is the brain** (here called in-process;
//! over the wire it is the brain MCP, spec 06). It carries the agent's identity
//! (a capability token), queries the brain, raises `request_access` when denied,
//! and would write durable findings via `brain.remember`. The sandbox that hosts
//! it has no ambient authority ([`crate::sandbox`]).

use anyhow::Result;

use crate::access::request::{route_request, AccessRequest, RouteDecision};
use crate::agent::inference;
use crate::brain::retrieval::{self, QueryResult};
use crate::config::Config;
use crate::controlplane::load_capability;
use crate::scenario;
use crate::store::Store;

pub fn run(principal: &str, ask: Option<&str>) -> Result<()> {
    let config = Config::load();
    let store = Store::new(config.clone());
    let index = store.load_index()?;
    // verified token only — an unsigned in-memory fallback would bypass the
    // signature check (run `ctl seed` first)
    let cap = load_capability(&config, principal).ok_or_else(|| {
        anyhow::anyhow!("no verified capability for principal '{principal}' — run `ctl seed`")
    })?;
    let llm = inference::from_config(config.inference);

    println!(
        "agent {principal} online · inference={} · sandbox egress = brain MCP only",
        llm.name()
    );
    println!("sources: {:?}", retrieval::list_sources(&index, &cap));

    let Some(question) = ask else {
        return Ok(());
    };
    println!("\nQ: {question}");

    // The agent wants credit-adjusted spend → reaches for finance_private.
    let view = scenario::finance_private();
    let select = vec![
        "gross".to_string(),
        "credits".to_string(),
        "discount_tier".to_string(),
    ];
    match retrieval::query(&index, &cap, &view, &select) {
        QueryResult::Ok { answer, .. } => {
            println!("A: {}", llm.complete(&answer)?);
        }
        QueryResult::Denied { answer, .. } => {
            println!("brain: {answer}");
            // raise a structured request and route it through the owner's envelope
            let req = AccessRequest {
                id: format!("req-{}", &uuid::Uuid::new_v4().to_string()[..8]),
                requester: principal.to_string(),
                view,
                fields: select,
                row_scope: Some(retrieval::all_teams_scope()),
                reason: question.to_string(),
                doc: "finops".into(),
                ttl: "7d".into(),
            };
            match route_request(&req, &scenario::cfo_envelope()) {
                RouteDecision::Auto { reason } => println!("→ auto-approved ({reason})"),
                RouteDecision::Escalate { reason } => {
                    println!("→ request_access raised; awaiting CFO ({reason})")
                }
                RouteDecision::Forbidden { reason } => println!("→ blocked: {reason}"),
            }
            // fall back to the view the agent CAN read
            if let QueryResult::Ok { answer, .. } = retrieval::query(
                &index,
                &cap,
                &scenario::spend_by_team(),
                &["gross".into(), "net".into()],
            ) {
                println!("A (from permitted view): {}", llm.complete(&answer)?);
            }
        }
    }
    Ok(())
}
