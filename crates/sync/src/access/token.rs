//! Real Biscuit tokens (spec 03 §3) — `biscuit-auth` signed blocks + Datalog.
//!
//! The JSON block mirror in [`super`] stays as the human-readable display, but
//! the *proof* is the signed Biscuit carried in [`Capability::token`]:
//!
//! - [`sign`] turns a minted capability into a Biscuit whose authority block
//!   carries the grant as Datalog facts, signed by the resource root's key.
//! - [`append_attenuation`] appends a real (offline, keyless) Biscuit block
//!   whose `check all` expressions can only narrow — Biscuit's append-only
//!   semantics give `caps(child) ⊆ caps(parent)` cryptographically.
//! - [`verify_token`] checks the signature against the root public key and
//!   re-derives the *effective scope from the token alone*: per-field Datalog
//!   authorizer runs decide what survived attenuation. Tampering with the JSON
//!   mirror cannot widen anything — the verified scope wins.
//!
//! Datalog vocabulary (all facts in the signed authority block):
//!
//! ```text
//! cf_holder(principal)        cf_root(root_id)          cf_view(view_id)
//! cf_right(view_id, op)       cf_field(view_id, field)  cf_row(view_id, key, value)
//! cf_doc(doc_pattern, op)     -- doc_pattern "*" = any document (relay auth)
//! ```
//!
//! Attenuation blocks add provenance facts (`cf_attenuated_by(i, by)`,
//! `cf_held_by(i, holder)`, `cf_row_narrow(i, key, value)`) and checks:
//!
//! ```text
//! check all q_field($f), {allow}.contains($f);    -- allow_fields
//! check all q_field($f), !{deny}.contains($f);    -- deny_fields
//! check all q_view($v),  !{deny}.contains($v);    -- deny_views
//! check if  time($t), $t < {exp};                 -- ttl
//! ```
//!
//! `check all` in biscuit fails when *no* fact matches, so every authorizer
//! run supplies sentinel facts `q_field("__none__")` / `q_view("__none__")`
//! (and every allow-set includes the sentinel); a field check therefore never
//! blocks a document-level authorization and vice versa, while real fields
//! still have to pass every narrowing check individually.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, SystemTime};

use biscuit_auth::builder::{AuthorizerBuilder, BiscuitBuilder, BlockBuilder, Term};
use biscuit_auth::macros::rule;
use biscuit_auth::{Biscuit, KeyPair, PublicKey, UnverifiedBiscuit};

use crate::access::{AttenuationBlock, Block, Capability, Operation, RowScope, View};

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("capability has no signed token — run `ctl seed` (or mint+sign) first")]
    Missing,
    #[error("biscuit error: {0}")]
    Biscuit(String),
    #[error("token expired or violates its own checks: {0}")]
    Invalid(String),
    #[error("token holder '{token}' does not match expected '{expected}'")]
    HolderMismatch { token: String, expected: String },
    #[error("bad ttl '{0}' (expected e.g. 30s / 15m / 24h / 7d)")]
    BadTtl(String),
}

impl From<biscuit_auth::error::Token> for TokenError {
    fn from(e: biscuit_auth::error::Token) -> Self {
        TokenError::Biscuit(e.to_string())
    }
}

fn op_str(op: Operation) -> &'static str {
    match op {
        Operation::Read => "read",
        Operation::Write => "write",
        Operation::Comment => "comment",
        Operation::Query => "query",
        Operation::Admin => "admin",
    }
}

fn op_from_str(s: &str) -> Option<Operation> {
    Some(match s {
        "read" => Operation::Read,
        "write" => Operation::Write,
        "comment" => Operation::Comment,
        "query" => Operation::Query,
        "admin" => Operation::Admin,
        _ => return None,
    })
}

/// Parse a ttl like `30s` / `15m` / `24h` / `7d` into a duration.
pub fn parse_ttl(ttl: &str) -> Result<Duration, TokenError> {
    let ttl = ttl.trim();
    // split on the last char's boundary — byte-offset split_at panics on
    // multi-byte input, and the ttl comes from CLI args / AccessRequests
    let (unit_at, _) = ttl
        .char_indices()
        .next_back()
        .ok_or_else(|| TokenError::BadTtl(ttl.into()))?;
    let (num, unit) = ttl.split_at(unit_at);
    let n: u64 = num.parse().map_err(|_| TokenError::BadTtl(ttl.into()))?;
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        "d" => n * 86_400,
        _ => return Err(TokenError::BadTtl(ttl.into())),
    };
    Ok(Duration::from_secs(secs))
}

/// Present in every authorizer run so `check all` (which fails on zero
/// matching facts in biscuit) always has a passing witness. Never a real
/// field or view id; allow-sets include it, deny-sets never do.
const SENTINEL: &str = "__none__";

fn set_term(values: impl IntoIterator<Item = String>) -> Term {
    Term::Set(values.into_iter().map(Term::Str).collect())
}

/// Render one attenuation block as a real Biscuit block.
fn attenuation_block(
    a: &AttenuationBlock,
    block_index: usize,
    holder: Option<&str>,
    now: SystemTime,
) -> Result<BlockBuilder, TokenError> {
    let mut code = String::new();
    let mut params: HashMap<String, Term> = HashMap::new();

    params.insert("idx".into(), Term::Integer(block_index as i64));
    params.insert("by".into(), Term::Str(a.by.clone()));
    code.push_str("cf_attenuated_by({idx}, {by});\n");

    if let Some(h) = holder {
        params.insert("holder".into(), Term::Str(h.to_string()));
        code.push_str("cf_held_by({idx}, {holder});\n");
    }
    if let Some(allow) = &a.allow_fields {
        let with_sentinel = allow
            .iter()
            .cloned()
            .chain(std::iter::once(SENTINEL.to_string()));
        params.insert("allow".into(), set_term(with_sentinel));
        code.push_str("check all q_field($f), {allow}.contains($f);\n");
    }
    if !a.deny_fields.is_empty() {
        params.insert("deny".into(), set_term(a.deny_fields.iter().cloned()));
        code.push_str("check all q_field($f), !{deny}.contains($f);\n");
    }
    if !a.deny_views.is_empty() {
        params.insert(
            "deny_views".into(),
            set_term(a.deny_views.iter().map(|v| v.id())),
        );
        code.push_str("check all q_view($v), !{deny_views}.contains($v);\n");
    }
    if let Some(rows) = &a.rows {
        let mut i = 0;
        for scope in rows {
            for value in &scope.values {
                let kf = format!("rk{i}");
                let vf = format!("rv{i}");
                params.insert(kf.clone(), Term::Str(scope.field.clone()));
                params.insert(vf.clone(), Term::Str(value.clone()));
                code.push_str(&format!("cf_row_narrow({{idx}}, {{{kf}}}, {{{vf}}});\n"));
                i += 1;
            }
            // an empty narrowing list still narrows (to nothing) — record the key
            if scope.values.is_empty() {
                let kf = format!("rk{i}");
                params.insert(kf.clone(), Term::Str(scope.field.clone()));
                code.push_str(&format!("cf_row_narrow_empty({{idx}}, {{{kf}}});\n"));
                i += 1;
            }
        }
    }
    if let Some(ttl) = &a.ttl {
        let exp = now + parse_ttl(ttl)?;
        params.insert("exp".into(), Term::Date(unix(exp)));
        code.push_str("check if time($t), $t < {exp};\n");
    }

    BlockBuilder::new()
        .code_with_params(&code, params, HashMap::new())
        .map_err(TokenError::from)
}

fn unix(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Build + sign a real Biscuit for a capability's block chain using the
/// resource root's keypair. Only the key holder can do this — that *is* the
/// mint authority (spec 03 §1).
pub fn sign(cap: &Capability, root_keys: &KeyPair) -> Result<Capability, TokenError> {
    let auth = match cap.blocks.first() {
        Some(Block::Authority(a)) => a,
        _ => return Err(TokenError::Invalid("no authority block".into())),
    };

    let mut code = String::from("cf_holder({holder});\ncf_root({root});\ncf_view({view});\n");
    let mut params: HashMap<String, Term> = HashMap::new();
    params.insert("holder".into(), Term::Str(cap.holder.clone()));
    params.insert("root".into(), Term::Str(auth.root.clone()));
    params.insert("view".into(), Term::Str(auth.view.id()));

    for (i, op) in auth.ops.iter().enumerate() {
        let p = format!("op{i}");
        params.insert(p.clone(), Term::Str(op_str(*op).to_string()));
        code.push_str(&format!("cf_right({{view}}, {{{p}}});\n"));
    }
    for (i, f) in auth.fields.iter().enumerate() {
        let p = format!("f{i}");
        params.insert(p.clone(), Term::Str(f.clone()));
        code.push_str(&format!("cf_field({{view}}, {{{p}}});\n"));
    }
    let mut ri = 0;
    for scope in &auth.rows {
        for value in &scope.values {
            let kf = format!("rowk{ri}");
            let vf = format!("rowv{ri}");
            params.insert(kf.clone(), Term::Str(scope.field.clone()));
            params.insert(vf.clone(), Term::Str(value.clone()));
            code.push_str(&format!("cf_row({{view}}, {{{kf}}}, {{{vf}}});\n"));
            ri += 1;
        }
    }
    for (i, doc) in auth.docs.iter().enumerate() {
        let p = format!("doc{i}");
        params.insert(p.clone(), Term::Str(doc.clone()));
        // docs grant read+write unconditionally: the authority model has no
        // per-doc op yet, and attenuation blocks cannot narrow cf_doc facts
        // (verify_token reads them from the authority block only). A
        // read-only relay grant needs a per-doc op in `AuthorityBlock::docs`
        // plus a `check all` over cf_doc in `attenuation_block`.
        code.push_str(&format!("cf_doc({{{p}}}, \"read\");\n"));
        code.push_str(&format!("cf_doc({{{p}}}, \"write\");\n"));
    }

    let builder = BiscuitBuilder::new()
        .code_with_params(&code, params, HashMap::new())
        .map_err(TokenError::from)?;
    let mut biscuit = builder.build(root_keys).map_err(TokenError::from)?;

    // replay any attenuation blocks already present in the JSON mirror
    let now = SystemTime::now();
    for (i, b) in cap.blocks.iter().enumerate().skip(1) {
        if let Block::Attenuation(a) = b {
            biscuit = biscuit
                .append(attenuation_block(a, i, Some(&cap.holder), now)?)
                .map_err(TokenError::from)?;
        }
    }

    let mut out = cap.clone();
    out.token = Some(biscuit.to_base64().map_err(TokenError::from)?);
    Ok(out)
}

/// Append a real attenuation block to a serialized token (offline — no key
/// needed; that is Biscuit's defining property).
pub fn append_attenuation(
    token_b64: &str,
    a: &AttenuationBlock,
    holder: Option<&str>,
) -> Result<String, TokenError> {
    let ub = UnverifiedBiscuit::from_base64(token_b64).map_err(TokenError::from)?;
    let idx = ub.block_count();
    let block = attenuation_block(a, idx, holder, SystemTime::now())?;
    let appended = ub.append(block).map_err(TokenError::from)?;
    appended.to_base64().map_err(TokenError::from)
}

/// The scope a verified token actually grants — derived from the token alone.
#[derive(Debug, Clone)]
pub struct VerifiedScope {
    pub holder: String,
    pub root: String,
    pub view: View,
    pub ops: Vec<Operation>,
    pub fields: BTreeSet<String>,
    pub rows: Vec<RowScope>,
    /// doc patterns ("*" or a doc id) → ops, for relay per-message auth.
    pub docs: Vec<(String, Operation)>,
    /// biscuit revocation identifiers (hex) for the revocation list.
    pub revocation_ids: Vec<String>,
}

fn authorizer_for(
    biscuit: &Biscuit,
    code: &str,
    params: HashMap<String, Term>,
) -> Result<biscuit_auth::Authorizer, TokenError> {
    let sentinels = format!("q_field(\"{SENTINEL}\");\nq_view(\"{SENTINEL}\");\n");
    AuthorizerBuilder::new()
        .code_with_params(format!("{sentinels}{code}"), params, HashMap::new())
        .map_err(TokenError::from)?
        .time()
        .build(biscuit)
        .map_err(TokenError::from)
}

fn run_allows(biscuit: &Biscuit, code: &str, params: HashMap<String, Term>) -> bool {
    authorizer_for(biscuit, code, params)
        .and_then(|mut az| az.authorize().map_err(TokenError::from))
        .is_ok()
}

/// Verify a token's signature against the root public key and derive the
/// effective scope from its facts + checks. Every field is individually
/// re-authorized through the Datalog engine, so attenuation `check all`
/// expressions decide what survives.
pub fn verify_token(token_b64: &str, root_pub: &PublicKey) -> Result<VerifiedScope, TokenError> {
    let biscuit = Biscuit::from_base64(token_b64, *root_pub).map_err(TokenError::from)?;

    // base run: ttl + structural checks must hold with no q_* facts at all
    let mut base = authorizer_for(&biscuit, "allow if true;", HashMap::new())?;
    base.authorize()
        .map_err(|e| TokenError::Invalid(e.to_string()))?;

    // authority facts (trusted, signed by the root key)
    let views: Vec<(String,)> = base
        .query(rule!("data($v) <- cf_view($v)"))
        .map_err(TokenError::from)?;
    let view_id = views
        .first()
        .map(|(v,)| v.clone())
        .ok_or_else(|| TokenError::Invalid("token has no cf_view fact".into()))?;
    let (source, name) = view_id
        .split_once('/')
        .ok_or_else(|| TokenError::Invalid(format!("bad view id '{view_id}'")))?;
    let view = View::new(source, name);

    let holders: Vec<(String,)> = base
        .query(rule!("data($h) <- cf_holder($h)"))
        .map_err(TokenError::from)?;
    let authority_holder = holders
        .first()
        .map(|(h,)| h.clone())
        .ok_or_else(|| TokenError::Invalid("token has no cf_holder fact".into()))?;

    let roots: Vec<(String,)> = base
        .query(rule!("data($r) <- cf_root($r)"))
        .map_err(TokenError::from)?;
    let root = roots
        .first()
        .map(|(r,)| r.clone())
        .ok_or_else(|| TokenError::Invalid("token has no cf_root fact".into()))?;

    let rights: Vec<(String, String)> = base
        .query(rule!("data($v, $o) <- cf_right($v, $o)"))
        .map_err(TokenError::from)?;
    let ops: Vec<Operation> = rights
        .iter()
        .filter(|(v, _)| *v == view_id)
        .filter_map(|(_, o)| op_from_str(o))
        .collect();

    let docs_q: Vec<(String, String)> = base
        .query(rule!("data($d, $o) <- cf_doc($d, $o)"))
        .map_err(TokenError::from)?;
    let docs: Vec<(String, Operation)> = docs_q
        .iter()
        .filter_map(|(d, o)| op_from_str(o).map(|op| (d.clone(), op)))
        .collect();

    // candidate fields = authority facts; survivors = per-field Datalog runs
    let field_facts: Vec<(String, String)> = base
        .query(rule!("data($v, $f) <- cf_field($v, $f)"))
        .map_err(TokenError::from)?;
    let mut fields = BTreeSet::new();
    for (v, f) in field_facts.iter().filter(|(v, _)| *v == view_id) {
        let mut params = HashMap::new();
        params.insert("v".to_string(), Term::Str(v.clone()));
        params.insert("f".to_string(), Term::Str(f.clone()));
        let code = "q_field({f}); q_view({v}); allow if cf_field({v}, {f});";
        if run_allows(&biscuit, code, params) {
            fields.insert(f.clone());
        }
    }

    // effective rows = authority row facts ∩ every narrowing group.
    // narrowing facts live in attenuation blocks → query_all (they can only
    // ever shrink the set, so reading them from untrusted blocks is sound).
    let row_facts: Vec<(String, String, String)> = base
        .query(rule!("data($v, $k, $val) <- cf_row($v, $k, $val)"))
        .map_err(TokenError::from)?;
    let mut rows_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (v, k, val) in row_facts.iter().filter(|(v, _, _)| *v == view_id) {
        let _ = v;
        rows_map.entry(k.clone()).or_default().insert(val.clone());
    }
    let narrow_facts: Vec<(i64, String, String)> = base
        .query_all(rule!("data($i, $k, $v) <- cf_row_narrow($i, $k, $v)"))
        .map_err(TokenError::from)?;
    let mut narrow_groups: BTreeMap<(i64, String), BTreeSet<String>> = BTreeMap::new();
    for (i, k, v) in &narrow_facts {
        narrow_groups
            .entry((*i, k.clone()))
            .or_default()
            .insert(v.clone());
    }
    let narrow_empty: Vec<(i64, String)> = base
        .query_all(rule!("data($i, $k) <- cf_row_narrow_empty($i, $k)"))
        .map_err(TokenError::from)?;
    for ((_, key), allowed) in &narrow_groups {
        match rows_map.get_mut(key) {
            Some(set) => set.retain(|v| allowed.contains(v)),
            None => {
                rows_map.insert(key.clone(), allowed.clone());
            }
        }
    }
    for (_, key) in &narrow_empty {
        rows_map.insert(key.clone(), BTreeSet::new());
    }
    let rows: Vec<RowScope> = rows_map
        .into_iter()
        .map(|(field, values)| RowScope {
            field,
            values: values.into_iter().collect(),
        })
        .collect();

    // holder = last delegation in the chain (cf_held_by), else authority holder
    let held_by: Vec<(i64, String)> = base
        .query_all(rule!("data($i, $h) <- cf_held_by($i, $h)"))
        .map_err(TokenError::from)?;
    let holder = held_by
        .iter()
        .max_by_key(|(i, _)| *i)
        .map(|(_, h)| h.clone())
        .unwrap_or(authority_holder);

    let revocation_ids = biscuit
        .revocation_identifiers()
        .iter()
        .map(hex::encode)
        .collect();

    Ok(VerifiedScope {
        holder,
        root,
        view,
        ops,
        fields,
        rows,
        docs,
        revocation_ids,
    })
}

/// Does a verified scope authorize `op` on a given document id?
pub fn scope_allows_doc(scope: &VerifiedScope, doc_id: &str, op: Operation) -> bool {
    scope
        .docs
        .iter()
        .any(|(pat, o)| *o == op && (pat == "*" || pat == doc_id))
}

/// Convenience: verify a capability's embedded token and check the holder.
pub fn verify_capability(
    cap: &Capability,
    root_pub: &PublicKey,
    expected_holder: &str,
) -> Result<VerifiedScope, TokenError> {
    let token = cap.token.as_deref().ok_or(TokenError::Missing)?;
    let scope = verify_token(token, root_pub)?;
    if scope.holder != expected_holder {
        return Err(TokenError::HolderMismatch {
            token: scope.holder,
            expected: expected_holder.to_string(),
        });
    }
    Ok(scope)
}
