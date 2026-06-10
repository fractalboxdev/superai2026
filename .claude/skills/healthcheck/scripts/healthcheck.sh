#!/usr/bin/env bash
#
# healthcheck.sh — health-check every deployable surface of the superai2026 monorepo.
#
# Checks, in order:
#   1. Deployments   — HTTP probes of the live Vercel landing + web apps (status, latency,
#                      content marker) plus landing's robots.txt / llms.txt.
#   2. Vercel CLI    — latest deployment state per project, if the `vercel` CLI is installed.
#   3. Observability — HyperDX OTLP ingest endpoint reachable + app wiring (instrumentation,
#                      browser RUM, SDKs) intact + production ingestion key set.
#   4. CI            — latest GitHub Actions run per workflow, via `gh`.
#   5. Local build   — (opt-in, --full) cargo check + pnpm lint/typecheck/build.
#
# Production URLs are derived from the app configs (single source of truth):
#   apps/landing/astro.config.mjs   -> site:
#   apps/web/src/app/layout.tsx     -> metadataBase
#
# Usage:
#   healthcheck.sh                 # deployments + CI (fast, network only)
#   healthcheck.sh --full          # also run local cargo/pnpm builds
#   healthcheck.sh --deploy-only   # only the HTTP/Vercel deployment probes
#   healthcheck.sh --ci-only       # only the GitHub Actions CI status
#
# Exit code is non-zero if any check FAILS, so this is safe to use in automation.

set -uo pipefail

# ---------------------------------------------------------------------------
# setup
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)"
[ -z "${REPO_ROOT:-}" ] && REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

# Derive production URLs from the app configs; fall back to known defaults.
LANDING_URL="$(grep -oE 'https?://[^"]+' "$REPO_ROOT/apps/landing/astro.config.mjs" 2>/dev/null | head -1)"
WEB_URL="$(grep -oE 'new URL\("https?://[^"]+' "$REPO_ROOT/apps/web/src/app/layout.tsx" 2>/dev/null | grep -oE 'https?://[^"]+' | head -1)"
: "${LANDING_URL:=https://www.contextful.work}"
: "${WEB_URL:=https://demo.contextful.work}"

# HyperDX OTLP ingestion endpoint the apps report to — derived from .env.example
# (single source of truth), falling back to HyperDX Cloud.
HYPERDX_ENDPOINT="$(grep -oE '^OTEL_EXPORTER_OTLP_ENDPOINT=[^ ]+' "$REPO_ROOT/.env.example" 2>/dev/null | cut -d= -f2-)"
: "${HYPERDX_ENDPOINT:=https://in-otel.hyperdx.io}"

# Brand marker we expect in served HTML (sanity-checks the page isn't an error shell).
MARKER="Contextful"
TIMEOUT=15

# colors (auto-disable when not a terminal)
if [ -t 1 ]; then
  R="$(printf '\033[31m')"; G="$(printf '\033[32m')"; Y="$(printf '\033[33m')"
  B="$(printf '\033[1m')"; D="$(printf '\033[2m')"; N="$(printf '\033[0m')"
else
  R=""; G=""; Y=""; B=""; D=""; N=""
fi

pass=0; fail=0; warn=0
ok()      { printf "  ${G}\xe2\x9c\x94${N} %s\n" "$1"; pass=$((pass+1)); }
bad()     { printf "  ${R}\xe2\x9c\x97${N} %s\n" "$1"; fail=$((fail+1)); }
note()    { printf "  ${Y}\xe2\x97\x8f${N} %s\n" "$1"; warn=$((warn+1)); }
skip()    { printf "  ${D}\xe2\x80\x93 %s${N}\n" "$1"; }
section() { printf "\n${B}%s${N}\n" "$1"; }

# probe a page: status + latency + brand-marker content check
probe_page() {
  local label="$1" url="$2"
  local resp meta code time body
  resp="$(curl -sS -L --max-time "$TIMEOUT" -w '\n__META__ %{http_code} %{time_total}' "$url" 2>&1)"
  if [ $? -ne 0 ]; then
    bad "$label  ${D}$url  unreachable${N}"
    return
  fi
  meta="$(printf '%s' "$resp" | grep '^__META__' | tail -1)"
  body="$(printf '%s' "$resp" | sed '/^__META__/d')"
  code="$(printf '%s' "$meta" | awk '{print $2}')"
  time="$(printf '%s' "$meta" | awk '{print $3}')"
  if printf '%s' "$code" | grep -qE '^(2|3)[0-9][0-9]$'; then
    if printf '%s' "$body" | grep -qi "$MARKER"; then
      ok "$label  ${D}HTTP $code · ${time}s · marker ok${N}"
    else
      note "$label  ${D}HTTP $code · ${time}s · '$MARKER' not in body (up, unexpected content)${N}"
    fi
  else
    bad "$label  ${D}HTTP $code · ${time}s${N}  $url"
  fi
}

# probe a static asset: status only
probe_asset() {
  local label="$1" url="$2"
  local code
  code="$(curl -sS -L --max-time "$TIMEOUT" -o /dev/null -w '%{http_code}' "$url" 2>/dev/null)"
  if printf '%s' "$code" | grep -qE '^2[0-9][0-9]$'; then
    ok "$label  ${D}HTTP $code${N}"
  else
    bad "$label  ${D}HTTP $code${N}  $url"
  fi
}

# ---------------------------------------------------------------------------
# sections
# ---------------------------------------------------------------------------
check_deployments() {
  section "Deployments (live HTTP probes)"
  probe_page  "landing   $LANDING_URL" "$LANDING_URL"
  probe_asset "landing   /robots.txt"  "$LANDING_URL/robots.txt"
  probe_asset "landing   /llms.txt"    "$LANDING_URL/llms.txt"
  probe_page  "web       $WEB_URL"     "$WEB_URL"

  if command -v vercel >/dev/null 2>&1; then
    section "Vercel CLI"
    if vercel whoami >/dev/null 2>&1; then
      vercel ls 2>/dev/null | sed -n '1,6p' | sed 's/^/  /' \
        || note "vercel ls failed — run from a linked project dir or 'vercel link'"
    else
      skip "vercel CLI present but not logged in (run 'vercel login')"
    fi
  fi
}

check_observability() {
  section "Observability — HyperDX"

  # 1. Ingestion endpoint reachable. OTLP rejects plain GET, so we only treat
  #    "no HTTP response at all" as a problem — and even then only a warning,
  #    since the collector may simply refuse non-POST rather than being down.
  local code
  code="$(curl -sS -L --max-time "$TIMEOUT" -o /dev/null -w '%{http_code}' "$HYPERDX_ENDPOINT" 2>/dev/null)"
  if [ -n "$code" ] && [ "$code" != "000" ]; then
    ok   "ingest    OTLP endpoint reachable   ${D}HTTP $code · $HYPERDX_ENDPOINT${N}"
  else
    note "ingest    OTLP endpoint no response  ${D}$HYPERDX_ENDPOINT (may reject GET — verify ingest in HyperDX)${N}"
  fi

  # 2. App wiring intact — catches an accidental removal of the integration.
  local web="$REPO_ROOT/apps/web"
  if [ -f "$web/src/instrumentation.ts" ] && grep -qi 'hyperdx' "$web/src/instrumentation.ts"; then
    ok  "web       server APM wired          ${D}(src/instrumentation.ts)${N}"
  else
    bad "web       server APM missing        ${D}(apps/web/src/instrumentation.ts → @hyperdx/node-opentelemetry)${N}"
  fi
  if [ -f "$web/src/components/hyperdx-init.tsx" ]; then
    ok  "web       browser RUM wired         ${D}(src/components/hyperdx-init.tsx)${N}"
  else
    bad "web       browser RUM missing       ${D}(apps/web/src/components/hyperdx-init.tsx)${N}"
  fi
  if grep -q '@hyperdx/node-opentelemetry' "$web/package.json" 2>/dev/null \
     && grep -q '@hyperdx/browser' "$web/package.json" 2>/dev/null; then
    ok  "web       HyperDX SDKs present      ${D}(apps/web/package.json)${N}"
  else
    bad "web       HyperDX SDK deps missing  ${D}(apps/web/package.json)${N}"
  fi

  # 3. Production ingestion key — prod telemetry is dark until this is set.
  if [ -f "$REPO_ROOT/.env.production" ] && grep -q '^HYPERDX_API_KEY=' "$REPO_ROOT/.env.production"; then
    ok   "prod      HYPERDX_API_KEY set        ${D}(.env.production)${N}"
  else
    note "prod      HYPERDX_API_KEY not set    ${D}(.env.production / Vercel env — prod telemetry won't flow until set)${N}"
  fi

  # 4. Vercel platform log drain is Pro-gated and intentionally not configured —
  #    app logs already reach HyperDX via the SDK's console capture.
  skip "vercel    platform log-drain not configured (Pro-only; app logs ship via SDK)"
}

check_ci() {
  section "CI — GitHub Actions"
  if ! command -v gh >/dev/null 2>&1; then
    note "gh CLI not installed — skipping CI check (brew install gh)"
    return
  fi
  if ! gh auth status >/dev/null 2>&1; then
    note "gh not authenticated — run 'gh auth login'"
    return
  fi

  local wf
  wf="$(gh workflow list 2>/dev/null)"
  if [ -z "$wf" ]; then
    note "no GitHub Actions workflows configured for this repo"
    return
  fi

  # Latest run per workflow, alphabetical.
  local rows
  rows="$(gh run list --limit 60 \
            --json workflowName,headBranch,status,conclusion,event,createdAt,url 2>/dev/null \
          | jq -r 'group_by(.workflowName)
                   | map(max_by(.createdAt))
                   | sort_by(.workflowName)
                   | .[] | [.workflowName,.headBranch,.status,.conclusion,.url] | @tsv' 2>/dev/null)"

  if [ -z "$rows" ]; then
    note "workflows exist but no runs found yet"
    return
  fi

  local name branch status concl url label
  while IFS=$'\t' read -r name branch status concl url; do
    label="$name  ${D}@$branch · ${status:-?}${concl:+/$concl}${N}"
    case "$concl" in
      success)
        ok "$label" ;;
      failure|startup_failure|timed_out)
        bad "$label  ${D}$url${N}" ;;
      cancelled|action_required|stale)
        note "$label" ;;
      skipped|neutral|"")
        if [ "$status" != "completed" ]; then
          note "$label  ${D}(in progress)${N}"
        else
          skip "$label"
        fi ;;
      *)
        note "$label" ;;
    esac
  done <<< "$rows"
}

check_local() {
  section "Local build (--full)"
  if command -v cargo >/dev/null 2>&1; then
    if ( cd "$REPO_ROOT" && cargo check -p sync >/tmp/hc_cargo.log 2>&1 ); then
      ok "cargo check -p sync"
    else
      bad "cargo check -p sync  ${D}(see /tmp/hc_cargo.log)${N}"
    fi
  else
    skip "cargo not installed — skipping Rust build"
  fi

  if command -v pnpm >/dev/null 2>&1; then
    local t
    for t in lint typecheck build; do
      if ( cd "$REPO_ROOT" && pnpm "$t" >"/tmp/hc_pnpm_$t.log" 2>&1 ); then
        ok "pnpm $t"
      else
        bad "pnpm $t  ${D}(see /tmp/hc_pnpm_$t.log)${N}"
      fi
    done
  else
    skip "pnpm not installed — skipping JS build"
  fi
}

# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------
MODE="default"
case "${1:-}" in
  --full)        MODE="full" ;;
  --deploy-only) MODE="deploy" ;;
  --ci-only)     MODE="ci" ;;
  -h|--help)
    sed -n '3,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0 ;;
  "") ;;
  *) printf "unknown flag: %s (try --help)\n" "$1"; exit 2 ;;
esac

printf "${B}superai2026 healthcheck${N}  ${D}%s${N}\n" "$REPO_ROOT"

case "$MODE" in
  deploy) check_deployments; check_observability ;;
  ci)     check_ci ;;
  full)   check_deployments; check_observability; check_ci; check_local ;;
  *)      check_deployments; check_observability; check_ci ;;
esac

section "Summary"
printf "  ${G}%d passed${N}  ${Y}%d warnings${N}  ${R}%d failed${N}\n" "$pass" "$warn" "$fail"
[ "$fail" -gt 0 ] && exit 1
exit 0
