#!/usr/bin/env bash
# Build crates/sync and stage it where Tauri's externalBin expects it
# (src-tauri/binaries/sync-<target-triple>). Spec 10 §2 "sidecar binary".
#
# Usage: prepare-sidecar.sh [--release] [--target <triple>] [--universal]
#   --universal builds both Apple targets and lipo-merges them into
#   sync-universal-apple-darwin (what `tauri build --target universal-apple-darwin`
#   expects).
set -euo pipefail

cd "$(dirname "$0")/../../.." # repo root

mode="debug"
release_flag=""
target=""
universal=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      mode="release"
      release_flag="--release"
      ;;
    --target)
      target="$2"
      shift
      ;;
    --universal)
      universal=1
      ;;
    *)
      echo "unknown flag: $1" >&2
      exit 2
      ;;
  esac
  shift
done

build() {
  # shellcheck disable=SC2086 # release_flag is intentionally word-split
  cargo build -p sync $release_flag "$@"
}

dest="apps/desktop/src-tauri/binaries"
mkdir -p "$dest"

if [[ "$universal" == 1 ]]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    build --target "$t"
  done
  lipo -create \
    "target/aarch64-apple-darwin/$mode/sync" \
    "target/x86_64-apple-darwin/$mode/sync" \
    -output "$dest/sync-universal-apple-darwin"
  echo "staged $dest/sync-universal-apple-darwin"
  exit 0
fi

host_triple="$(rustc -vV | awk '/^host: /{print $2}')"
triple="${target:-$host_triple}"

if [[ -n "$target" ]]; then
  build --target "$target"
  out="target/$target/$mode/sync"
else
  build
  out="target/$mode/sync"
fi

cp "$out" "$dest/sync-$triple"
echo "staged $dest/sync-$triple"
