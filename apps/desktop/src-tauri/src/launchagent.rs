//! Opt-in launchd LaunchAgent (spec 10 §5): launchd keeps the *app* alive,
//! the app keeps the *binary* alive — the binary is never registered with
//! launchd directly, so there is no double-spawn.

use std::path::PathBuf;
use std::process::Command;

pub const LABEL: &str = "work.contextful.app";

pub fn plist_path() -> PathBuf {
    crate::util::home_dir()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Best-effort launchctl invocation — the plist alone covers the next login
/// either way.
fn launchctl(args: &[&str]) {
    let _ = Command::new("launchctl").args(args).output();
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
    launchctl(&["unload", &path.to_string_lossy()]);
    launchctl(&["load", "-w", &path.to_string_lossy()]);
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let path = plist_path();
    if path.is_file() {
        launchctl(&["unload", "-w", &path.to_string_lossy()]);
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
