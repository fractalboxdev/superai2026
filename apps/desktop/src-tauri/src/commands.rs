//! Tauri command surface — the only API the WebView can call (mirrors
//! `apps/desktop/src/ipc.ts`).

use std::process::Command;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::identity::{self, IdentityInfo};
use crate::settings::AppSettings;
use crate::supervisor::{Snapshot, Supervisor};
use crate::tailscale::{self, TailscaleInfo};
use crate::{launchagent, sidecar};

pub struct AppCtx {
    pub supervisor: Arc<Supervisor>,
}

pub fn supervisor_of(app: &AppHandle) -> Arc<Supervisor> {
    app.state::<AppCtx>().supervisor.clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    settings: AppSettings,
    supervisor: Snapshot,
    tailscale: TailscaleInfo,
    known_principals: Vec<String>,
    sidecar_path: Option<String>,
    launch_agent_installed: bool,
}

fn detect_ts(settings: &AppSettings) -> TailscaleInfo {
    tailscale::detect(tailscale::port_of(&settings.relay_addr))
}

#[tauri::command]
pub fn get_app_state(ctx: State<'_, AppCtx>) -> AppState {
    let settings = AppSettings::load();
    AppState {
        supervisor: ctx.supervisor.snapshot(),
        tailscale: detect_ts(&settings),
        known_principals: identity::known_principals(),
        sidecar_path: sidecar::resolve().map(|p| p.display().to_string()),
        launch_agent_installed: launchagent::installed(),
        settings,
    }
}

#[tauri::command]
pub fn save_settings(patch: serde_json::Value) -> Result<AppSettings, String> {
    let merged = AppSettings::load().merged(&patch).map_err(err)?;
    merged.save().map_err(err)?;
    Ok(merged)
}

#[tauri::command]
pub fn mark_configured() -> Result<AppSettings, String> {
    AppSettings::update(|s| s.configured = true).map_err(err)
}

#[tauri::command]
pub fn ensure_identity(
    principal: String,
    role: crate::settings::Role,
) -> Result<IdentityInfo, String> {
    let settings = AppSettings::load();
    identity::ensure(&principal, role, &settings).map_err(err)
}

#[tauri::command]
pub fn detect_tailscale() -> TailscaleInfo {
    detect_ts(&AppSettings::load())
}

#[tauri::command]
pub fn start_service(ctx: State<'_, AppCtx>) {
    ctx.supervisor.start();
}

#[tauri::command]
pub fn stop_service(ctx: State<'_, AppCtx>) {
    ctx.supervisor.stop();
}

#[tauri::command]
pub async fn restart_service(ctx: State<'_, AppCtx>) -> Result<(), String> {
    ctx.supervisor.restart().await;
    Ok(())
}

#[tauri::command]
pub fn get_logs(ctx: State<'_, AppCtx>, limit: Option<usize>) -> Vec<String> {
    ctx.supervisor.logs(limit.unwrap_or(200))
}

#[tauri::command]
pub fn set_autostart(enable: bool) -> Result<bool, String> {
    if enable {
        launchagent::install().map_err(err)?;
    } else {
        launchagent::uninstall().map_err(err)?;
    }
    AppSettings::update(|s| s.autostart = enable).map_err(err)?;
    Ok(enable)
}

// The three commands below are also called directly by the tray menu — the
// `#[tauri::command]` macro keeps the plain fn callable.

#[tauri::command]
pub fn open_web_app() {
    open_target(AppSettings::load().web_app_url);
}

#[tauri::command]
pub fn reveal_brain() {
    let settings = AppSettings::load();
    let dir = match settings.brain_home_expanded() {
        Some(d) => std::path::PathBuf::from(d),
        None => crate::util::home_dir().join(".contextful"),
    };
    open_target(dir);
}

#[tauri::command]
pub fn copy_sync_url() -> Result<String, String> {
    copy_sync_url_impl().map_err(err)
}

fn open_target(target: impl AsRef<std::ffi::OsStr>) {
    let _ = Command::new("open").arg(target).status();
}

fn copy_sync_url_impl() -> anyhow::Result<String> {
    let settings = AppSettings::load();
    let ts = detect_ts(&settings);
    let url = ts
        .sync_url
        .ok_or_else(|| anyhow::anyhow!("tailnet is offline — no sync URL to copy"))?;
    use std::io::Write;
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(url.as_bytes())?;
    }
    child.wait()?;
    Ok(url)
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Drift guard: must match `AppState` in apps/desktop/src/ipc.ts.
    #[test]
    fn app_state_keys_mirror_ipc_ts() {
        let state = AppState {
            settings: AppSettings::default(),
            supervisor: Snapshot {
                status: crate::supervisor::Status::Stopped,
                detail: "stopped".into(),
                pid: None,
                restarts: 0,
            },
            tailscale: TailscaleInfo::absent(),
            known_principals: vec!["cfo".into()],
            sidecar_path: Some("/Applications/Contextful.app/Contents/MacOS/sync".into()),
            launch_agent_installed: true,
        };
        let v = serde_json::to_value(state).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "knownPrincipals",
                "launchAgentInstalled",
                "settings",
                "sidecarPath",
                "supervisor",
                "tailscale"
            ]
        );
    }
}
