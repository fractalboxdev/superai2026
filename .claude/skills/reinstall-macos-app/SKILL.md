---
name: reinstall-macos-app
description: Rebuild and reinstall the Contextful macOS menu-bar app (apps/desktop) from the current working tree — stages the sync sidecar, packages the .app via Tauri, quits the running instance, swaps the bundle in /Applications, and relaunches (restoring the LaunchAgent if installed). Use when asked to reinstall, update, or refresh the locally installed desktop app after code changes.
---

# Reinstall macOS App

Reinstall `/Applications/Contextful.app` from the current working tree. Everything is wrapped in one script:

```bash
.claude/skills/reinstall-macos-app/scripts/reinstall.sh                # build + reinstall + relaunch
.claude/skills/reinstall-macos-app/scripts/reinstall.sh --skip-build   # reuse the existing release bundle
.claude/skills/reinstall-macos-app/scripts/reinstall.sh --no-launch    # install but don't open
.claude/skills/reinstall-macos-app/scripts/reinstall.sh --dest ~/Applications
```

What it does, in order:

1. **Build** — in `apps/desktop`: stages the release `crates/sync` sidecar (`scripts/prepare-sidecar.sh --release`) then `tauri build --bundles app`, producing `apps/desktop/src-tauri/target/release/bundle/macos/Contextful.app`. The `.app` bundle only — the DMG step (`bundle_dmg.sh`) requires a GUI Finder session and fails headless; it isn't needed for a local reinstall. Skipped with `--skip-build`.
2. **Unload the LaunchAgent** (`~/Library/LaunchAgents/work.contextful.app.plist`, label `work.contextful.app`) if installed, so launchd doesn't resurrect the app mid-swap.
3. **Quit** the running app (AppleScript quit, then force-kill `contextful-desktop` and any sidecar still running from `Contextful.app/Contents/MacOS`).
4. **Swap** the bundle: `rm -rf` the old install, `ditto` the fresh one in.
5. **Relaunch** — reload the LaunchAgent if it was installed (it relaunches the app), otherwise `open` the app directly (unless `--no-launch`).

The script is idempotent and safe to run when the app isn't installed or isn't running.

## Notes & troubleshooting

- The release build is slow (full `cargo build --release` of `crates/sync` + the Tauri shell). For a quick re-deploy of an already-built bundle, use `--skip-build`.
- A failed build leaves the installed app untouched — the swap only happens after the bundle exists.
- App state is **not** touched: identity stays in the Keychain, data stays in `~/.contextful`. A reinstall is not a reset; to reset, remove `~/.contextful` separately (ask the user first).
- If macOS complains the app is damaged/unverified after a swap (shouldn't happen for a locally built bundle), clear quarantine: `xattr -dr com.apple.quarantine /Applications/Contextful.app`.
- If the menu-bar icon doesn't appear after relaunch, check the supervisor logs from the app's Logs view, or run the sidecar manually: `/Applications/Contextful.app/Contents/MacOS/sync serve --with-mcp`.
