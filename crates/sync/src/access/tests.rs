//! Capability unit + property tests (spec 09 §2). Flow B salary invariant is
//! the load-bearing one: no token and no approval path yields employee_salary.

use crate::access::biscuit::{
    attenuate, authorize, delegate_to, effective_capability, mint, AccessError,
};
use crate::access::request::{
    approve_request, route_request, RouteDecision, SalaryInvariantViolation,
};
use crate::access::{
    AttenuationBlock, AuthDecision, DenyReason, Operation, QueryRequest, RowScope, View,
};
use crate::scenario;

fn q(view: View, fields: &[&str]) -> QueryRequest {
    QueryRequest {
        op: Operation::Query,
        view,
        fields: fields.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn attenuation_removes_a_field_and_cannot_re_add_it() {
    let parent = scenario::cfo_capability();
    let child = attenuate(
        &parent,
        AttenuationBlock {
            by: "cfo".into(),
            deny_fields: vec!["employee_salary".into()],
            ..Default::default()
        },
    );
    // a later block trying to allow-list the denied field must not resurrect it
    let grandchild = attenuate(
        &child,
        AttenuationBlock {
            by: "cto".into(),
            allow_fields: Some(vec!["employee_salary".into(), "credits".into()]),
            ..Default::default()
        },
    );

    let pf = effective_capability(&parent).unwrap().fields;
    let cf = effective_capability(&child).unwrap().fields;
    let gf = effective_capability(&grandchild).unwrap().fields;

    assert!(pf.contains("employee_salary"));
    assert!(!cf.contains("employee_salary"));
    assert!(!gf.contains("employee_salary"));
    assert!(
        gf.iter().all(|f| pf.contains(f)),
        "caps(child) ⊆ caps(parent)"
    );
}

#[test]
fn row_scopes_intersect_never_widen() {
    let parent = mint(
        &scenario::cfo_root(),
        "cfo",
        vec![Operation::Query],
        scenario::spend_by_team(),
        vec!["team".into(), "gross".into()],
        vec![RowScope {
            field: "team".into(),
            values: vec!["eng".into(), "ops".into(), "sales".into()],
        }],
    )
    .unwrap();
    let child = attenuate(
        &parent,
        AttenuationBlock {
            by: "cfo".into(),
            rows: Some(vec![RowScope {
                field: "team".into(),
                values: vec!["ops".into(), "sales".into(), "finance".into()],
            }]),
            ..Default::default()
        },
    );
    let mut rows = effective_capability(&child)
        .unwrap()
        .rows
        .into_iter()
        .find(|r| r.field == "team")
        .unwrap()
        .values;
    rows.sort();
    assert_eq!(rows, vec!["ops".to_string(), "sales".to_string()]); // ∩, not ∪
}

#[test]
fn no_capability_super_root() {
    let err = mint(
        &scenario::control_plane_root(),
        "agent:eng/1",
        vec![Operation::Query],
        scenario::finance_private(),
        vec!["employee_salary".into()],
        vec![],
    )
    .unwrap_err();
    assert!(matches!(err, AccessError::NoRootAuthority { .. }));
}

#[test]
fn cto_agent_sees_team_spend_but_finance_private_denied() {
    let cap = scenario::cto_agent_capability();
    assert!(matches!(
        authorize(&cap, &q(scenario::spend_by_team(), &["gross", "net"])),
        AuthDecision::Ok { .. }
    ));
    assert!(matches!(
        authorize(&cap, &q(scenario::finance_private(), &["credits"])),
        AuthDecision::Denied(DenyReason::NoGrant)
    ));
}

#[test]
fn flow_a_approval_delegates_salary_free_token() {
    let req = crate::access::request::AccessRequest {
        id: "req-flow-a".into(),
        requester: "agent:cto/1".into(),
        view: scenario::finance_private(),
        fields: vec!["gross".into(), "credits".into(), "discount_tier".into()],
        row_scope: Some(vec![RowScope {
            field: "team".into(),
            values: scenario::ALL_TEAMS.iter().map(|s| s.to_string()).collect(),
        }]),
        reason: "net-of-credits".into(),
        doc: "finops".into(),
        ttl: "7d".into(),
    };
    assert!(matches!(
        route_request(&req, &scenario::cfo_envelope()),
        RouteDecision::Escalate { .. }
    ));

    let granted = approve_request(&scenario::cfo_capability(), &req).unwrap();
    assert_eq!(granted.holder, "agent:cto/1");

    // credits/discount_tier now visible
    match authorize(
        &granted,
        &q(scenario::finance_private(), &["credits", "discount_tier"]),
    ) {
        AuthDecision::Ok { granted_fields, .. } => {
            assert!(granted_fields.contains(&"credits".to_string()));
            assert!(granted_fields.contains(&"discount_tier".to_string()));
        }
        other => panic!("expected Ok, got {other:?}"),
    }
    // ...but salary stays redacted even though the CFO token carries it
    match authorize(
        &granted,
        &q(scenario::finance_private(), &["employee_salary"]),
    ) {
        AuthDecision::Ok {
            redacted_fields,
            granted_fields,
            ..
        } => {
            assert!(redacted_fields.contains(&"employee_salary".to_string()));
            assert!(granted_fields.is_empty());
        }
        other => panic!("expected redacted Ok, got {other:?}"),
    }
}

#[test]
fn flow_b_salary_invariant_no_approval_path() {
    let req = crate::access::request::AccessRequest {
        id: "req-flow-b".into(),
        requester: "agent:eng/1".into(),
        view: scenario::finance_private(),
        fields: vec!["employee_salary".into()],
        row_scope: None,
        reason: "benchmark comp".into(),
        doc: "finops".into(),
        ttl: "7d".into(),
    };
    // routing offers no path
    assert!(matches!(
        route_request(&req, &scenario::cfo_envelope()),
        RouteDecision::Forbidden { .. }
    ));
    // and even a direct mint refuses (defense in depth)
    assert!(matches!(
        approve_request(&scenario::cfo_capability(), &req),
        Err(SalaryInvariantViolation { .. })
    ));
}

#[test]
fn deny_views_cannot_be_re_granted() {
    let cap = attenuate(
        &scenario::cfo_capability(),
        AttenuationBlock {
            by: "cfo".into(),
            deny_views: vec![scenario::finance_private()],
            ..Default::default()
        },
    );
    assert!(matches!(
        authorize(&cap, &q(scenario::finance_private(), &["gross"])),
        AuthDecision::Denied(DenyReason::ViewDenied)
    ));
}

#[test]
fn wrong_operation_is_denied() {
    // a read-only token cannot be used for a query op
    let cap = mint(
        &scenario::cfo_root(),
        "agent:cto/1",
        vec![Operation::Read],
        scenario::spend_by_team(),
        vec!["team".into(), "gross".into()],
        vec![],
    )
    .unwrap();
    assert!(matches!(
        authorize(&cap, &q(scenario::spend_by_team(), &["gross"])),
        AuthDecision::Denied(DenyReason::WrongOp)
    ));
}

#[test]
fn route_request_auto_approves_inside_envelope() {
    // spend_by_team is in the CFO envelope's auto-approve list
    let req = crate::access::request::AccessRequest {
        id: "r".into(),
        requester: "agent:cto/1".into(),
        view: scenario::spend_by_team(),
        fields: vec!["gross".into()],
        row_scope: None,
        reason: "usage".into(),
        doc: "finops".into(),
        ttl: "7d".into(),
    };
    assert!(matches!(
        route_request(&req, &scenario::cfo_envelope()),
        RouteDecision::Auto { .. }
    ));
}

#[test]
fn engineering_agent_has_no_path_to_salary() {
    let cap = scenario::eng_agent_capability();
    // no grant at all for finance_private
    assert!(matches!(
        authorize(&cap, &q(scenario::finance_private(), &["employee_salary"])),
        AuthDecision::Denied(DenyReason::NoGrant)
    ));
    // delegating can only narrow what it has (spend_by_team) — never gain salary
    let delegated = delegate_to(
        &cap,
        "agent:eng/2",
        AttenuationBlock {
            by: "agent:eng/1".into(),
            ..Default::default()
        },
    );
    let eff = effective_capability(&delegated).unwrap();
    assert!(!eff.fields.contains("employee_salary"));
}
