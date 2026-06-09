#!/usr/bin/env bash
# Provision the Cloudflare resources the superai2026 FlareDispatch dispatcher
# needs, then patch their ids into ./wrangler.jsonc.
#
# Run ONCE, locally, after `wrangler login` (or with CLOUDFLARE_API_TOKEN +
# CLOUDFLARE_ACCOUNT_ID exported). Idempotent: re-running re-reads existing
# resource ids instead of failing. Requires a Cloudflare Workers *Paid* plan
# (Containers + Workflows are paid-tier features).
#
#   pnpm dlx wrangler@4 login          # or export CLOUDFLARE_API_TOKEN=…
#   bash infra/flare-dispatch/provision.sh
#
# After it finishes, the four __SET_ME__ placeholders in wrangler.jsonc are
# filled (account id, R2 bucket, D1 id, KV id). Commit the result.
set -euo pipefail

NAME="flare-dispatch-superai2026"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OVERLAY="$HERE/wrangler.jsonc"
WRANGLER="pnpm dlx wrangler@4"

echo "==> Using overlay: $OVERLAY"

# ── Account id ───────────────────────────────────────────────────────────────
ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-}"
if [ -z "$ACCOUNT_ID" ]; then
  ACCOUNT_ID="$($WRANGLER whoami 2>/dev/null | grep -oE '[0-9a-f]{32}' | head -1 || true)"
fi
[ -n "$ACCOUNT_ID" ] || { echo "ERROR: could not determine account id. Export CLOUDFLARE_ACCOUNT_ID." >&2; exit 1; }
echo "==> Account: $ACCOUNT_ID"

# ── R2 bucket ────────────────────────────────────────────────────────────────
echo "==> R2 bucket: $NAME"
$WRANGLER r2 bucket create "$NAME" 2>/dev/null || echo "    (already exists — ok)"

# ── D1 database ──────────────────────────────────────────────────────────────
echo "==> D1 database: $NAME"
D1_OUT="$($WRANGLER d1 create "$NAME" 2>&1 || true)"
D1_ID="$(printf '%s' "$D1_OUT" | grep -oE '[0-9a-f-]{36}' | head -1 || true)"
if [ -z "$D1_ID" ]; then
  # Already exists — read it back from the D1 list.
  D1_ID="$($WRANGLER d1 info "$NAME" 2>/dev/null | grep -oE '[0-9a-f-]{36}' | head -1 || true)"
fi
[ -n "$D1_ID" ] || { echo "ERROR: could not resolve D1 id. Output was:" >&2; echo "$D1_OUT" >&2; exit 1; }
echo "    D1 id: $D1_ID"

# ── KV namespace ─────────────────────────────────────────────────────────────
echo "==> KV namespace: CONFIG_KV"
KV_OUT="$($WRANGLER kv namespace create CONFIG_KV 2>&1 || true)"
KV_ID="$(printf '%s' "$KV_OUT" | grep -oE '[0-9a-f]{32}' | head -1 || true)"
if [ -z "$KV_ID" ]; then
  KV_ID="$($WRANGLER kv namespace list 2>/dev/null | grep -B2 -i 'CONFIG_KV' | grep -oE '[0-9a-f]{32}' | head -1 || true)"
fi
[ -n "$KV_ID" ] || { echo "ERROR: could not resolve CONFIG_KV id. Output was:" >&2; echo "$KV_OUT" >&2; exit 1; }
echo "    KV id: $KV_ID"

# ── Patch the overlay ────────────────────────────────────────────────────────
echo "==> Patching $OVERLAY"
sed -i.bak \
  -e "s/__SET_ME__account_id/$ACCOUNT_ID/" \
  -e "s/__SET_ME__d1_database_id/$D1_ID/" \
  -e "s/__SET_ME__config_kv_id/$KV_ID/" \
  "$OVERLAY"
rm -f "$OVERLAY.bak"

echo
echo "✅ Provisioned. wrangler.jsonc now points at:"
echo "   account = $ACCOUNT_ID"
echo "   r2      = $NAME"
echo "   d1      = $D1_ID"
echo "   kv      = $KV_ID"
echo
echo "Next: set Worker secrets (HMAC_SECRET, GITHUB_APP_*), then run the deploy"
echo "workflow (.github/workflows/flare-deploy.yml). See README.md."
