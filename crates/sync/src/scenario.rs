//! The Q3 FinOps demo scenario (spec 00 §2–4) — Rust mirror of
//! `packages/protocol/src/scenario.ts`. Personas, root keys, views, envelopes,
//! and initial tokens, shared by tests, `ctl seed`, and the brain.

use crate::access::{
    biscuit::mint_with_docs, request::AutoApprove, request::Envelope, Capability, Operation,
    RootKey, RowScope, View,
};
use crate::identity::Principal;

pub fn spend_by_team() -> View {
    View::new("stripe", "spend_by_team")
}
pub fn finance_private() -> View {
    View::new("stripe", "finance_private")
}

pub const PERIOD: &str = "2026-05";
pub const ALL_TEAMS: &[&str] = &["eng", "ops", "sales", "finance"];

// The demo cast is always the Pied Piper team (HBO Silicon Valley), displayed
// as "Name (Role)". Principal ids and owners stay stable (`cfo`, `agent:cto/1`,
// …) — they are wire/CLI identifiers shared with packages/protocol and the
// acceptance suite.

pub fn cfo() -> Principal {
    Principal::Human {
        id: "cfo".into(),
        name: "Monica (CFO)".into(),
        role: "finance".into(),
    }
}
pub fn cto_agent() -> Principal {
    Principal::Agent {
        id: "agent:cto/1".into(),
        name: "Richard's agent".into(),
        owner: "cto".into(),
    }
}
pub fn eng_agent() -> Principal {
    Principal::Agent {
        id: "agent:eng/1".into(),
        name: "Dinesh's agent".into(),
        owner: "eng".into(),
    }
}

pub fn principals() -> Vec<Principal> {
    vec![cto_agent(), eng_agent(), cfo()]
}

/// CFO owns the finance resource root — sole minter of finance_private authority.
pub fn cfo_root() -> RootKey {
    RootKey {
        id: "cfo".into(),
        owner: "cfo".into(),
        views: vec![finance_private(), spend_by_team()],
        public_key: None,
    }
}

/// Control-plane root: identity/membership only — holds NO data views by design.
pub fn control_plane_root() -> RootKey {
    RootKey {
        id: "control-plane".into(),
        owner: "control-plane".into(),
        views: vec![],
        public_key: None,
    }
}

fn s(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

/// Every seeded principal may read/write any room through the relay; the
/// relay re-verifies this from the signed token per message (spec 01 §4).
fn all_docs() -> Vec<String> {
    vec!["*".into()]
}

/// The CFO holds the full finance root token (sole salary authority).
pub fn cfo_capability() -> Capability {
    mint_with_docs(
        &cfo_root(),
        "cfo",
        vec![Operation::Query, Operation::Read],
        finance_private(),
        s(&[
            "team",
            "period",
            "gross",
            "net",
            "discount_tier",
            "credits",
            "employee_salary",
        ]),
        vec![],
        all_docs(),
    )
    .expect("cfo root covers finance_private")
}

/// Richard's agent: team-level spend only (no finance_private) until Flow A grants it.
pub fn cto_agent_capability() -> Capability {
    mint_with_docs(
        &cfo_root(),
        "agent:cto/1",
        vec![Operation::Query, Operation::Read],
        spend_by_team(),
        s(&["team", "period", "gross", "net"]),
        vec![],
        all_docs(),
    )
    .expect("cfo root covers spend_by_team")
}

/// Dinesh's agent: usage view, own team rows only. Never any salary path.
pub fn eng_agent_capability() -> Capability {
    mint_with_docs(
        &cfo_root(),
        "agent:eng/1",
        vec![Operation::Query, Operation::Read],
        spend_by_team(),
        s(&["team", "period", "gross", "net"]),
        vec![RowScope {
            field: "team".into(),
            values: s(&["eng"]),
        }],
        all_docs(),
    )
    .expect("cfo root covers spend_by_team")
}

pub fn initial_capability(principal_id: &str) -> Option<Capability> {
    match principal_id {
        "cfo" => Some(cfo_capability()),
        "agent:cto/1" => Some(cto_agent_capability()),
        "agent:eng/1" => Some(eng_agent_capability()),
        _ => None,
    }
}

/// The CFO's auto-mode envelope.
pub fn cfo_envelope() -> Envelope {
    Envelope {
        owner: "cfo".into(),
        auto_approve: vec![AutoApprove {
            view: spend_by_team(),
            max_ttl: "7d".into(),
        }],
        never_delegate: s(&["employee_salary"]),
    }
}
