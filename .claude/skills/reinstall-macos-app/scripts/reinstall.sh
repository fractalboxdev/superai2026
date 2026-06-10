#!/usr/bin/env bash
# Reinstall the Contextful macOS menu-bar app from the current working tree.
# Builds the .app (sidecar + tauri), quits any running instance, swaps the
# bundle in /Applications, and relaunches (restoring the LaunchAgent if it
# was installed).
#
# Usage: reinstall.sh [--skip-build] [--no-launch] [--dest <dir>]
set -euo pipefail

SKIP_BUILD=0
NO_LAUNCH=0
DEST_DIR="/Applications"

while [ $# -gt 0 ]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1 ;;
    --no-launch) NO_LAUNCH=1 ;;
    --dest)
      shift
      DEST_DIR="${1:?--dest requires a directory}"
      ;;
    *)
      echo "unknown flag: $1" >&2
      exit 2
      ;;
  esac
  shift
done

REPO_ROOT="$(git rev-parse --show-toplevel)"
DESKTOP_DIR="$REPO_ROOT/apps/desktop"
BUNDLE="$DESKTOP_DIR/src-tauri/target/release/bundle/macos/Contextful.app"
DEST="$DEST_DIR/Contextful.app"
PLIST="$HOME/Library/LaunchAgents/work.contextful.app.plist"

step() { printf '\n==> %s\n' "$*"; }

if [ "$SKIP_BUILD" -eq 0 ]; then
  step "Building Contextful.app (sidecar --release + tauri build)"
  (cd "$DESKTOP_DIR" && pnpm app:build)
fi

if [ ! -d "$BUNDLE" ]; then
  echo "no bundle at $BUNDLE — run without --skip-build first" >&2
  exit 1
fi

HAD_LAUNCHAGENT=0
if [ -f "$PLIST" ]; then
  HAD_LAUNCHAGENT=1
  step "Unloading LaunchAgent work.contextful.app"
  launchctl unload "$PLIST" 2>/dev/null || true
fi

step "Quitting running app + sidecar"
osascript -e 'tell application "Contextful" to quit' 2>/dev/null || true
# Give the supervisor a moment to reap the sidecar, then force-kill leftovers.
for _ in 1 2 3 4 5; do
  pgrep -x contextful-desktop >/dev/null || break
  sleep 1
done
pkill -x contextful-desktop 2>/dev/null || true
pkill -f "Contextful.app/Contents/MacOS" 2>/dev/null || true

step "Installing to $DEST"
rm -rf "$DEST"
ditto "$BUNDLE" "$DEST"

if [ "$HAD_LAUNCHAGENT" -eq 1 ]; then
  step "Reloading LaunchAgent (also relaunches the app)"
  launchctl load -w "$PLIST" 2>/dev/null || true
elif [ "$NO_LAUNCH" -eq 0 ]; then
  step "Launching"
  open "$DEST"
fi

step "Done — installed $(defaults read "$DEST/Contents/Info" CFBundleShortVersionString 2>/dev/null || echo '?') at $DEST"
