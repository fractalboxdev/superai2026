//! Locate the bundled `sync` binary (spec 10 §2 "sidecar binary").
//!
//! Resolution order:
//!   1. `CONTEXTFUL_SYNC_BIN` env override (tests, power users)
//!   2. next to the app executable — where Tauri's `externalBin` lands it
//!      (`Contextful.app/Contents/MacOS/sync`)
//!   3. dev fallback: the repo workspace's `target/{debug,release}/sync`

use std::path::PathBuf;

pub fn resolve() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CONTEXTFUL_SYNC_BIN") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("sync");
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }

    if cfg!(debug_assertions) {
        // apps/desktop/src-tauri → repo root → target/
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .ok()?;
        for mode in ["debug", "release"] {
            let p = root.join("target").join(mode).join("sync");
            if p.is_file() {
                return Some(p);
            }
        }
    }

    None
}
