# Contextful Desktop (macOS)

Menu-bar shell around the `crates/sync` binary ([spec 10](../../specs/10-macos-app.md)):
install and run a Contextful **host** (`serve --with-mcp`) or **member**
(`client`) without a terminal. The app supervises the bundled sidecar —
spawn, health-check, restart with backoff — and adds **no new authority**:
all data and capability checks stay in the binary and `~/.contextful`.

## Layout

- `src/` — WebView UI (React + TS, design-system CSS): first-run wizard,
  status, logs, settings.
- `src-tauri/` — Rust core: supervisor, tray, Keychain identity, Tailscale
  detection, LaunchAgent installer. **Standalone Cargo workspace** (kept out
  of the root workspace so the ubuntu CI lane never needs webkit deps).
- `scripts/prepare-sidecar.sh` — builds `crates/sync` and stages it as the
  Tauri `externalBin` (`src-tauri/binaries/sync-<triple>`). Required before
  any `src-tauri` build.

## Develop

```sh
pnpm app:dev          # stage sidecar + tauri dev
pnpm app:build        # stage release sidecar + package .app/.dmg
pnpm typecheck        # frontend only
cd src-tauri && cargo test
```

CI packaging (universal binary, signing gated on Developer ID secrets):
[`.github/workflows/desktop.yml`](../../.github/workflows/desktop.yml).
