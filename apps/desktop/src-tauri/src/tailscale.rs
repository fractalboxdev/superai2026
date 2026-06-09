//! Tailscale detection (spec 10 §6): detect, don't manage. We read state via
//! `tailscale status --json` and never touch `tailscaled` or auth.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TailscaleInfo {
    pub installed: bool,
    pub running: bool,
    pub dns_name: Option<String>,
    /// Derived `ws://<magicdns>:<port>` members point at (host role).
    pub sync_url: Option<String>,
}

impl TailscaleInfo {
    pub fn absent() -> Self {
        Self {
            installed: false,
            running: false,
            dns_name: None,
            sync_url: None,
        }
    }
}

const CLI_CANDIDATES: &[&str] = &[
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/usr/local/bin/tailscale",
    "/opt/homebrew/bin/tailscale",
];

fn cli_path() -> Option<String> {
    // PATH first (dev machines), then the app-bundle CLI.
    if let Ok(out) = Command::new("which").arg("tailscale").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    CLI_CANDIDATES
        .iter()
        .find(|p| Path::new(p).is_file())
        .map(|p| p.to_string())
}

/// `relay_port` is used to derive the member-facing sync URL.
pub fn detect(relay_port: &str) -> TailscaleInfo {
    let Some(cli) = cli_path() else {
        return TailscaleInfo::absent();
    };
    let Ok(out) = Command::new(&cli).args(["status", "--json"]).output() else {
        return TailscaleInfo {
            installed: true,
            ..TailscaleInfo::absent()
        };
    };
    let body = String::from_utf8_lossy(&out.stdout);
    let (running, dns_name) = parse_status(&body);
    let sync_url = dns_name
        .as_ref()
        .filter(|_| running)
        .map(|d| format!("ws://{d}:{relay_port}"));
    TailscaleInfo {
        installed: true,
        running,
        dns_name,
        sync_url,
    }
}

/// Pull `(BackendState == "Running", Self.DNSName)` out of `status --json`.
pub fn parse_status(json: &str) -> (bool, Option<String>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return (false, None);
    };
    let running = v["BackendState"].as_str() == Some("Running");
    let dns = v["Self"]["DNSName"]
        .as_str()
        .map(|d| d.trim_end_matches('.').to_string())
        .filter(|d| !d.is_empty());
    (running, dns)
}

/// Port suffix of a bind/relay address (`0.0.0.0:7878` → `7878`).
pub fn port_of(addr: &str) -> &str {
    addr.rsplit(':').next().unwrap_or("7878")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_running_status() {
        let json = r#"{"BackendState":"Running","Self":{"DNSName":"studio.tail1234.ts.net."}}"#;
        let (running, dns) = parse_status(json);
        assert!(running);
        assert_eq!(dns.as_deref(), Some("studio.tail1234.ts.net"));
    }

    #[test]
    fn parses_stopped_status() {
        let json = r#"{"BackendState":"Stopped","Self":{"DNSName":""}}"#;
        let (running, dns) = parse_status(json);
        assert!(!running);
        assert_eq!(dns, None);
    }

    #[test]
    fn garbage_is_not_running() {
        assert_eq!(parse_status("not json"), (false, None));
    }

    #[test]
    fn port_extraction() {
        assert_eq!(port_of("0.0.0.0:7878"), "7878");
        assert_eq!(port_of("studio.ts.net:9999"), "9999");
    }
}
