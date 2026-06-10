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

# Build one triple and stage it under the name tauri-build expects.
stage_triple() {
  build --target "$1"
  cp "target/$1/$mode/sync" "$dest/sync-$1"
}

dest="apps/desktop/src-tauri/binaries"
mkdir -p "$dest"

if [[ "$universal" == 1 ]]; then
  # Stage per-triple copies too: tauri-build resolves externalBin by the
  # triple it is currently compiling for (host triple during clippy/test,
  # each Apple triple during a universal `tauri build`).
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    stage_triple "$t"
  done
  lipo -create \
    "$dest/sync-aarch64-apple-darwin" \
    "$dest/sync-x86_64-apple-darwin" \
    -output "$dest/sync-universal-apple-darwin"
  echo "staged $dest/sync-{aarch64,x86_64,universal}-apple-darwin"
  exit 0
fi

if [[ -n "$target" ]]; then
  stage_triple "$target"
  echo "staged $dest/sync-$target"
else
  host_triple="$(rustc -vV | awk '/^host: /{print $2}')"
  build
  cp "target/$mode/sync" "$dest/sync-$host_triple"
  echo "staged $dest/sync-$host_triple"
fi
