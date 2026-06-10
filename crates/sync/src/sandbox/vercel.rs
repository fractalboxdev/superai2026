//! Vercel Sandbox driver — default runtime, "agents from anywhere" (spec 04 §2).
//!
//! Rust owns control: the lifecycle decision, the room→sandbox registry
//! (single-flight per room), and identity minting live here. The only
//! TypeScript is the hands-and-feet bridge (`packages/sandbox-bridge`)
//! wrapping the `@vercel/sandbox` SDK, spawned per call; it makes no policy
//! decisions. With no `VERCEL_TOKEN` (offline / Flow D), the lifecycle is
//! modeled locally so room flows keep working without cloud compute.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use crate::sandbox::{Sandbox, SandboxHandle};

/// Vercel Sandbox lifetime cap (Pro/Enterprise ~5h).
const MAX_LIFETIME_SECS: u64 = 5 * 60 * 60;

/// room → (handle, provisioned-at). Single-flight: the registry lock is held
/// across provisioning so concurrent first entrants provision exactly once.
static REGISTRY: Mutex<Option<HashMap<String, (SandboxHandle, Instant)>>> = Mutex::new(None);

#[derive(Default)]
pub struct VercelSandbox;

/// Locate the Node bridge: `CONTEXTFUL_SANDBOX_BRIDGE` (path to cli.mjs) or
/// walk up from cwd to find `packages/sandbox-bridge/src/cli.mjs`.
fn bridge_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CONTEXTFUL_SANDBOX_BRIDGE") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("packages/sandbox-bridge/src/cli.mjs");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Hard deadline on one bridge call — callers hold the registry lock, so a
/// hung bridge (e.g. Vercel API not answering) must not wedge every
/// subsequent sandbox operation.
const BRIDGE_TIMEOUT_SECS: u64 = 60;

/// Spawn the bridge for one call and parse its single-line JSON reply.
fn bridge_call(args: &[&str]) -> anyhow::Result<serde_json::Value> {
    let bridge = bridge_path().ok_or_else(|| {
        anyhow::anyhow!("sandbox bridge not found (set CONTEXTFUL_SANDBOX_BRIDGE)")
    })?;
    let child = std::process::Command::new("node")
        .arg(&bridge)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawning node bridge: {e}"))?;
    let out = wait_with_timeout(child, std::time::Duration::from_secs(BRIDGE_TIMEOUT_SECS))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .last()
        .ok_or_else(|| anyhow::anyhow!("bridge produced no output"))?;
    let value: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| anyhow::anyhow!("bridge output not JSON ({e}): {line}"))?;
    if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        anyhow::bail!(
            "bridge error: {}",
            value.get("error").and_then(|e| e.as_str()).unwrap_or(line)
        );
    }
    Ok(value)
}

/// Collect the child's output, killing it once the deadline passes. Polling
/// `try_wait` is safe here: the bridge prints one JSON line, far below the
/// pipe buffer, so the child never blocks on a full stdout.
fn wait_with_timeout(
    mut child: std::process::Child,
    deadline: std::time::Duration,
) -> anyhow::Result<std::process::Output> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| anyhow::anyhow!("collecting node bridge output: {e}"));
            }
            Ok(None) if start.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("node bridge timed out after {}s", deadline.as_secs());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => {
                let _ = child.kill();
                return Err(anyhow::anyhow!("waiting for node bridge: {e}"));
            }
        }
    }
}

fn live_available() -> bool {
    crate::config::nonempty_env("VERCEL_TOKEN").is_some()
}

impl Sandbox for VercelSandbox {
    fn kind(&self) -> &str {
        "vercel"
    }

    fn ensure(&self, room: &str) -> anyhow::Result<SandboxHandle> {
        let mut guard = REGISTRY.lock().expect("registry lock");
        let registry = guard.get_or_insert_with(HashMap::new);

        // reuse a live sandbox (recreate past the lifetime cap, spec 04 §2)
        if let Some((handle, created)) = registry.get(room) {
            if created.elapsed().as_secs() < handle.max_lifetime_secs {
                return Ok(handle.clone());
            }
        }

        let handle = if live_available() {
            let reply = bridge_call(&[
                "create",
                "--room",
                room,
                "--timeout-ms",
                &(MAX_LIFETIME_SECS * 1000).to_string(),
            ])?;
            let sandbox_id = reply
                .get("sandboxId")
                .and_then(|v| v.as_str())
                .filter(|id| !id.is_empty())
                // a handle with an empty id would poison the registry until
                // the lifetime cap — fail the call instead
                .ok_or_else(|| anyhow::anyhow!("bridge reply missing sandboxId: {reply}"))?
                .to_string();
            tracing::info!(room, %sandbox_id, "provisioned Vercel Sandbox");
            SandboxHandle {
                kind: "vercel".into(),
                room: room.to_string(),
                sandbox_id: Some(sandbox_id),
                max_lifetime_secs: MAX_LIFETIME_SECS,
            }
        } else {
            tracing::info!(
                room,
                "no VERCEL_TOKEN — modeling sandbox lifecycle locally (offline)"
            );
            SandboxHandle {
                kind: "vercel".into(),
                room: room.to_string(),
                sandbox_id: None,
                max_lifetime_secs: MAX_LIFETIME_SECS,
            }
        };

        registry.insert(room.to_string(), (handle.clone(), Instant::now()));
        Ok(handle)
    }
}
