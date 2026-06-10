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

// --- real Biscuit token tests (signed proof, not just block algebra) ---

mod real_biscuit {
    use super::*;
    use crate::access::token::{
        append_attenuation, scope_allows_doc, sign, verify_capability, verify_token, TokenError,
    };
    use biscuit_auth::KeyPair;

    #[test]
    fn sign_then_verify_roundtrip_preserves_scope() {
        let keys = KeyPair::new();
        let signed = sign(&scenario::cfo_capability(), &keys).unwrap();
        let scope = verify_token(signed.token.as_deref().unwrap(), &keys.public()).unwrap();
        assert_eq!(scope.holder, "cfo");
        assert_eq!(scope.view.id(), "stripe/finance_private");
        assert!(scope.fields.contains("employee_salary"));
        assert!(scope.ops.contains(&Operation::Query));
        assert!(scope_allows_doc(&scope, "finops", Operation::Write));
    }

    #[test]
    fn forged_token_fails_signature_verification() {
        let real_keys = KeyPair::new();
        let attacker_keys = KeyPair::new();
        // attacker signs a full-authority token with their own key
        let forged = sign(&scenario::cfo_capability(), &attacker_keys).unwrap();
        let err = verify_token(forged.token.as_deref().unwrap(), &real_keys.public());
        assert!(
            err.is_err(),
            "a token signed by the wrong key must not verify"
        );
    }

    #[test]
    fn delegated_token_denies_salary_via_datalog() {
        // Flow A through the REAL token: CFO delegates a salary-free grant;
        // the per-field Datalog authorizer is what rejects employee_salary.
        let keys = KeyPair::new();
        let cfo = sign(&scenario::cfo_capability(), &keys).unwrap();
        let req = crate::access::request::AccessRequest {
            id: "req".into(),
            requester: "agent:cto/1".into(),
            view: scenario::finance_private(),
            fields: vec!["gross".into(), "credits".into()],
            row_scope: None,
            reason: "net-of-credits".into(),
            doc: "finops".into(),
            ttl: "7d".into(),
        };
        let granted = approve_request(&cfo, &req).unwrap();
        let scope = verify_capability(&granted, &keys.public(), "agent:cto/1").unwrap();
        assert!(scope.fields.contains("credits"));
        assert!(scope.fields.contains("gross"));
        assert!(
            !scope.fields.contains("employee_salary"),
            "salary must not survive the attenuation checks"
        );
    }

    #[test]
    fn tampered_json_mirror_cannot_widen_scope() {
        // simulate an attacker editing caps/<p>.json to claim salary access:
        // the verified scope comes from the signed token, so the widened JSON
        // mirror is ignored.
        let keys = KeyPair::new();
        let cfo = sign(&scenario::cfo_capability(), &keys).unwrap();
        let req = crate::access::request::AccessRequest {
            id: "req".into(),
            requester: "agent:cto/1".into(),
            view: scenario::finance_private(),
            fields: vec!["credits".into()],
            row_scope: None,
            reason: "r".into(),
            doc: "finops".into(),
            ttl: "7d".into(),
        };
        let granted = approve_request(&cfo, &req).unwrap();

        let mut tampered = granted.clone();
        tampered.blocks = scenario::cfo_capability().blocks; // claim full authority
        let scope = verify_capability(&tampered, &keys.public(), "agent:cto/1").unwrap();
        assert!(!scope.fields.contains("employee_salary"));
    }

    #[test]
    fn expired_ttl_invalidates_the_whole_token() {
        let keys = KeyPair::new();
        let cfo = sign(&scenario::cfo_capability(), &keys).unwrap();
        let expired = append_attenuation(
            cfo.token.as_deref().unwrap(),
            &AttenuationBlock {
                by: "cfo".into(),
                ttl: Some("0s".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let err = verify_token(&expired, &keys.public()).unwrap_err();
        assert!(matches!(err, TokenError::Invalid(_)));
    }

    #[test]
    fn delegation_changes_verified_holder() {
        let keys = KeyPair::new();
        let cfo = sign(&scenario::cfo_capability(), &keys).unwrap();
        let delegated = delegate_to(
            &cfo,
            "agent:cto/1",
            AttenuationBlock {
                by: "cfo".into(),
                deny_fields: vec!["employee_salary".into()],
                ..Default::default()
            },
        );
        let scope = verify_token(delegated.token.as_deref().unwrap(), &keys.public()).unwrap();
        assert_eq!(scope.holder, "agent:cto/1");
        // and verifying against the wrong expected holder fails
        assert!(matches!(
            verify_capability(&delegated, &keys.public(), "cfo"),
            Err(TokenError::HolderMismatch { .. })
        ));
    }

    #[test]
    fn row_narrowing_survives_in_verified_scope() {
        let keys = KeyPair::new();
        let eng = sign(&scenario::eng_agent_capability(), &keys).unwrap();
        let scope = verify_token(eng.token.as_deref().unwrap(), &keys.public()).unwrap();
        let team = scope.rows.iter().find(|r| r.field == "team").unwrap();
        assert_eq!(team.values, vec!["eng".to_string()]);

        // attenuating rows further intersects, never widens
        let narrowed = attenuate(
            &eng,
            AttenuationBlock {
                by: "agent:eng/1".into(),
                rows: Some(vec![RowScope {
                    field: "team".into(),
                    values: vec!["eng".into(), "ops".into()],
                }]),
                ..Default::default()
            },
        );
        let scope2 = verify_token(narrowed.token.as_deref().unwrap(), &keys.public()).unwrap();
        let team2 = scope2.rows.iter().find(|r| r.field == "team").unwrap();
        assert_eq!(team2.values, vec!["eng".to_string()]);
    }
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
