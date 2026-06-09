//! Opt-in launchd LaunchAgent (spec 10 §5): launchd keeps the *app* alive,
//! the app keeps the *binary* alive — the binary is never registered with
//! launchd directly, so there is no double-spawn.

use std::path::PathBuf;
use std::process::Command;

pub const LABEL: &str = "work.contextful.app";

pub fn plist_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub fn render_plist(app_exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{app_exe}</string>
    <string>--headless</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Background</string>
</dict>
</plist>
"#
    )
}

pub fn installed() -> bool {
    plist_path().is_file()
}

pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let path = plist_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, render_plist(&exe.to_string_lossy()))?;
    // Best-effort (re)load; the plist alone covers the next login either way.
    let _ = Command::new("launchctl")
        .args(["unload", &path.to_string_lossy()])
        .output();
    let _ = Command::new("launchctl")
        .args(["load", "-w", &path.to_string_lossy()])
        .output();
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let path = plist_path();
    if path.is_file() {
        let _ = Command::new("launchctl")
            .args(["unload", "-w", &path.to_string_lossy()])
            .output();
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_keeps_app_alive_headless() {
        let p = render_plist("/Applications/Contextful.app/Contents/MacOS/contextful-desktop");
        assert!(p.contains("<string>work.contextful.app</string>"));
        assert!(p.contains("<string>--headless</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(p.contains("<key>KeepAlive</key>\n  <true/>"));
    }
}
