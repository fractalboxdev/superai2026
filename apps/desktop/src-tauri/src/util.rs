//! Small helpers shared across the shell modules.

use std::path::PathBuf;

/// `$HOME`, falling back to `.` when unset — the shell never panics over a
/// missing env var.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Run a command and fail with the trimmed stderr (prefixed by `what`) when
/// it exits non-zero.
pub fn run_checked(
    cmd: &mut std::process::Command,
    what: &str,
) -> anyhow::Result<std::process::Output> {
    let out = cmd.output()?;
    anyhow::ensure!(
        out.status.success(),
        "{what} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(out)
}
