//! Brain over MCP (spec 06).
//!
//! A working JSON-RPC 2.0 / MCP server with both spec transports: **stdio**
//! (co-located agents — Claude Code, the local agent loop) and **streamable
//! HTTP** (remote agents — Vercel Sandbox over Tailscale, spec 06 §2).
//! Wire-compatible with MCP clients; the official `rmcp` SDK remains a
//! drop-in transport swap, the tool semantics live here either way.
//!
//! **Session auth binding:** every call re-resolves the principal's token —
//! signature-verified against the registry root key and revocation-checked —
//! so a grant or a revocation takes effect on the very next tool call
//! (spec 06 §3).

use std::io::{BufRead, Write};

use anyhow::Result;
use serde_json::{json, Value};

use crate::access::request::{route_request, AccessRequest, RouteDecision};
use crate::access::{Capability, View};
use crate::brain::{retrieval, BrainIndex};
use crate::config::Config;
use crate::controlplane::{is_revoked, load_capability};
use crate::scenario;
use crate::store::Store;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Handle one JSON-RPC request as `principal`. Returns None for notifications.
/// Capability + revocation are re-checked per call (spec 06 §3).
pub fn dispatch(config: &Config, principal: &str, req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => Some(ok(id, initialize_result())),
        "tools/list" => Some(ok(id, json!({ "tools": tool_defs() }))),
        "ping" => Some(ok(id, json!({}))),
        "tools/call" => {
            // per-call session auth binding: revocation + verified token
            if is_revoked(config, principal) {
                return Some(tool_error(id, &format!("principal {principal} is revoked")));
            }
            let Some(cap) = load_capability(config, principal) else {
                return Some(tool_error(
                    id,
                    &format!("no verified capability for '{principal}' — run `ctl seed`"),
                ));
            };
            let store = Store::new(config.clone());
            let mut index = match store.load_index() {
                Ok(i) => i,
                Err(e) => return Some(err(id, -32603, &format!("index unavailable: {e}"))),
            };
            Some(handle_tool_call(
                config, &store, &mut index, &cap, principal, id, req,
            ))
        }
        _ if id.is_some() => Some(err(id, -32601, &format!("method not found: {method}"))),
        _ => None, // notification we don't act on (e.g. notifications/initialized)
    }
}

/// Run the brain MCP server over stdio as the given principal.
pub fn run(principal: &str) -> Result<()> {
    let config = Config::load();
    // fail fast on a bad identity, then re-verify per call
    if load_capability(&config, principal).is_none() {
        anyhow::bail!("no verified capability for principal '{principal}' — run `ctl seed`");
    }
    tracing::info!(%principal, "brain MCP server ready on stdio");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "ignoring malformed JSON-RPC line");
                continue;
            }
        };
        if let Some(resp) = dispatch(&config, principal, &req) {
            writeln!(out, "{resp}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Streamable-HTTP transport (spec 06 §2): `POST /mcp` carries one JSON-RPC
/// message; the caller's identity rides the `x-contextful-principal` header
/// and is re-verified (token signature + revocation) on every call. Reachable
/// over the tailnet for remote sandbox agents.
pub async fn serve_http(addr: &str) -> Result<()> {
    use axum::{extract::Request, http::StatusCode, response::IntoResponse, routing::post, Router};

    async fn handle(req: Request) -> impl IntoResponse {
        let principal = req
            .headers()
            .get("x-contextful-principal")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if principal.is_empty() {
            return (
                StatusCode::UNAUTHORIZED,
                "missing x-contextful-principal header".to_string(),
            )
                .into_response();
        }
        let bytes = match axum::body::to_bytes(req.into_body(), 1 << 20).await {
            Ok(b) => b,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("body: {e}")).into_response(),
        };
        let rpc: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("json: {e}")).into_response(),
        };
        let config = Config::load();
        match dispatch(&config, &principal, &rpc) {
            Some(resp) => axum::Json(resp).into_response(),
            None => StatusCode::ACCEPTED.into_response(), // notification
        }
    }

    let app = Router::new().route("/mcp", post(handle));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "brain MCP streamable-HTTP endpoint ready");
    axum::serve(listener, app).await?;
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "contextful-brain", "version": env!("CARGO_PKG_VERSION") }
    })
}

fn tool_defs() -> Vec<Value> {
    let view_arg =
        json!({ "type": "string", "description": "view id, e.g. stripe/finance_private" });
    vec![
        json!({ "name": "brain.list_sources", "description": "sources/views the caller may see", "inputSchema": { "type": "object", "properties": {} } }),
        json!({ "name": "brain.search", "description": "keyword memory search", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } } }),
        json!({ "name": "brain.get_context", "description": "synthesized Markdown context card by topic", "inputSchema": { "type": "object", "properties": { "topic": { "type": "string" } }, "required": ["topic"] } }),
        json!({ "name": "brain.query", "description": "structured query: view + projection", "inputSchema": { "type": "object", "properties": { "view": view_arg, "select": { "type": "array", "items": { "type": "string" } } }, "required": ["view", "select"] } }),
        json!({ "name": "brain.detect_anomalies", "description": "anomalies for a view", "inputSchema": { "type": "object", "properties": { "view": view_arg } , "required": ["view"] } }),
        json!({ "name": "brain.remember", "description": "write a memory scoped to a document (taint-tracked)", "inputSchema": { "type": "object", "properties": { "fact": { "type": "string" }, "doc": { "type": "string" } }, "required": ["fact", "doc"] } }),
        json!({ "name": "brain.request_access", "description": "raise a permission request", "inputSchema": { "type": "object", "properties": { "view": view_arg, "fields": { "type": "array", "items": { "type": "string" } }, "reason": { "type": "string" } }, "required": ["view", "fields"] } }),
        json!({ "name": "brain.world_search", "description": "firewalled public web search (Exa) → cited world cards; private-tainted terms are blocked at egress", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } }),
        json!({ "name": "brain.ground", "description": "wire a world card to the private card it grounds (grounds edge)", "inputSchema": { "type": "object", "properties": { "world_id": { "type": "string" }, "memory_id": { "type": "string" } }, "required": ["world_id", "memory_id"] } }),
        json!({ "name": "brain.daydreams", "description": "overnight daydream insight cards the caller is cleared to read", "inputSchema": { "type": "object", "properties": {} } }),
    ]
}

#[allow(clippy::too_many_arguments)]
fn handle_tool_call(
    config: &Config,
    store: &Store,
    index: &mut BrainIndex,
    cap: &Capability,
    principal: &str,
    id: Option<Value>,
    req: &Value,
) -> Value {
    let params = req.get("params").cloned().unwrap_or(json!({}));
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result: Result<Value, String> = match name {
        "brain.list_sources" => Ok(json!({ "views": retrieval::list_sources(index, cap) })),
        "brain.search" => {
            let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({ "results": retrieval::search(store, index, cap, q) }))
        }
        "brain.get_context" => {
            let topic = args.get("topic").and_then(|v| v.as_str()).unwrap_or("");
            match retrieval::get_context(store, index, cap, topic) {
                Ok(body) => Ok(json!({ "card": body })),
                Err(deny) => Ok(json!({ "denied": deny })),
            }
        }
        "brain.query" => {
            let view = parse_view(&args);
            let select = parse_strings(&args, "select");
            match view {
                Some(v) => {
                    Ok(serde_json::to_value(retrieval::query(index, cap, &v, &select)).unwrap())
                }
                None => Err("missing or malformed 'view'".into()),
            }
        }
        "brain.detect_anomalies" => match parse_view(&args) {
            Some(v) => {
                Ok(serde_json::to_value(retrieval::detect_anomalies(index, cap, &v)).unwrap())
            }
            None => Err("missing or malformed 'view'".into()),
        },
        "brain.remember" => {
            let fact = args.get("fact").and_then(|v| v.as_str()).unwrap_or("");
            let doc = args.get("doc").and_then(|v| v.as_str()).unwrap_or("");
            // taint: stamp with the caller's own granted scope as a floor
            let read_acl = crate::connectors::AclTag {
                view: View::new("doc", doc),
                fields: vec![],
            };
            match retrieval::remember(store, index, fact, doc, read_acl) {
                Ok(mid) => match store.save_index(index) {
                    Ok(()) => Ok(json!({ "memory_id": mid })),
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            }
        }
        "brain.request_access" => {
            let view = match parse_view(&args) {
                Some(v) => v,
                None => return err(id, -32602, "missing or malformed 'view'"),
            };
            let fields = parse_strings(&args, "fields");
            let reason = args
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let request = AccessRequest {
                id: format!("req-{}", &uuid::Uuid::new_v4().to_string()[..8]),
                requester: principal.to_string(),
                view,
                fields,
                row_scope: Some(retrieval::all_teams_scope()),
                reason,
                doc: "finops".into(),
                ttl: "7d".into(),
            };
            let route = route_request(&request, &scenario::cfo_envelope());
            let routed = match route {
                RouteDecision::Auto { reason } => json!({ "decision": "auto", "reason": reason }),
                RouteDecision::Escalate { reason } => {
                    json!({ "decision": "escalate", "reason": reason })
                }
                RouteDecision::Forbidden { reason } => {
                    json!({ "decision": "forbidden", "reason": reason })
                }
            };
            Ok(json!({ "request": request, "routing": routed }))
        }
        "brain.world_search" => {
            let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            match crate::brain::world::world_search(config, store, index, q) {
                Ok(memories) => match store.save_index(index) {
                    Ok(()) => Ok(json!({
                        "world_cards": memories.iter().map(|m| json!({
                            "id": m.id, "url": m.topic, "path": m.path,
                        })).collect::<Vec<_>>()
                    })),
                    Err(e) => Err(e.to_string()),
                },
                // egress violations surface as a tool error (terms not echoed)
                Err(e) => Err(e.to_string()),
            }
        }
        "brain.ground" => {
            let world_id = args.get("world_id").and_then(|v| v.as_str()).unwrap_or("");
            let memory_id = args.get("memory_id").and_then(|v| v.as_str()).unwrap_or("");
            match crate::brain::world::ground(index, world_id, memory_id) {
                Ok(()) => match store.save_index(index) {
                    Ok(()) => Ok(json!({ "grounded": { "from": world_id, "to": memory_id } })),
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            }
        }
        "brain.daydreams" => {
            // surfaced only to callers cleared for the insight's full taint
            // (acl = max(parents)) — Flow G's visibility rule
            let cards: Vec<Value> = index
                .memories
                .iter()
                .filter(|m| m.kind == crate::brain::MemoryKind::Daydream)
                .filter(|m| retrieval::card_readable(cap, &m.acl_tag))
                .map(|m| {
                    json!({
                        "id": m.id, "path": m.path, "confidence": m.confidence,
                        "card": store.read_card(&m.path).unwrap_or_default(),
                    })
                })
                .collect();
            Ok(json!({ "daydreams": cards }))
        }
        other => Err(format!("unknown tool: {other}")),
    };

    match result {
        Ok(value) => ok(
            id,
            json!({
                "content": [ { "type": "text", "text": serde_json::to_string_pretty(&value).unwrap() } ],
                "structuredContent": value,
                "isError": false
            }),
        ),
        Err(message) => ok(
            id,
            json!({
                "content": [ { "type": "text", "text": message } ],
                "isError": true
            }),
        ),
    }
}

fn parse_view(args: &Value) -> Option<View> {
    let id = args.get("view").and_then(|v| v.as_str())?;
    let (source, view) = id.split_once('/')?;
    Some(View::new(source, view))
}

fn parse_strings(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn ok(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

/// A tools/call error surfaced as MCP tool output (isError), not a transport error.
fn tool_error(id: Option<Value>, message: &str) -> Value {
    ok(
        id,
        json!({
            "content": [ { "type": "text", "text": message } ],
            "isError": true
        }),
    )
}

fn err(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}
