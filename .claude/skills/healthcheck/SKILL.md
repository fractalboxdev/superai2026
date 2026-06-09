---
name: healthcheck
description: Health-check every deployable surface of the superai2026 monorepo and fix what's broken. Probes the live Vercel landing (www.contextful.work) and web (demo.contextful.work) endpoints, reports GitHub Actions CI status per workflow via `gh`, optionally runs local lint/typecheck/build, then remediates the failures it safely can (lint, formatting, typecheck, missing static assets, stale config) and re-verifies. Use when asked to check whether deploys are up, whether CI is green, to run a pre-release health sweep, or to "make sure everything is healthy".
---

# Healthcheck

Verify the three deployable surfaces of this monorepo are healthy, then **fix the failures you can** and re-check.

| Surface | What "healthy" means | How it's checked |
| --- | --- | --- |
| `apps/landing` (Astro → Vercel) | `https://www.contextful.work` serves 2xx with expected content; `robots.txt` + `llms.txt` resolve | HTTP probe |
| `apps/web` (Next.js → Vercel) | `https://demo.contextful.work` serves 2xx with expected content | HTTP probe |
| HyperDX observability | OTLP ingest endpoint reachable; app wiring (instrumentation, browser RUM, SDK deps) intact; prod ingestion key set | HTTP probe + repo checks |
| GitHub Actions CI | latest run of every workflow concluded `success` | `gh run list` |
| Local build (opt-in) | `cargo check` + `pnpm lint/typecheck/build` pass | `--full` |

## 1. Run the check

Run the bundled script from the repo root:

```bash
.claude/skills/healthcheck/scripts/healthcheck.sh            # deployments + CI (fast)
.claude/skills/healthcheck/scripts/healthcheck.sh --full     # also local lint/typecheck/build
.claude/skills/healthcheck/scripts/healthcheck.sh --ci-only  # just CI
.claude/skills/healthcheck/scripts/healthcheck.sh --deploy-only
```

It prints `✔ pass / ● warning / ✗ fail` per check and a summary, and exits non-zero if anything failed. Production URLs are derived from `apps/landing/astro.config.mjs` and `apps/web/src/app/layout.tsx`, so they stay in sync with the apps.

## 2. Fix what failed

For every `✗`, diagnose and remediate, then re-run the relevant mode to confirm it goes green. Apply the cheapest safe fix first.

**CI failing (GitHub Actions).** Pull the actual error, then fix at the source:
```bash
gh run view <run-id> --log-failed          # the run-id / url is in the script output
```
- **Lint / format failures** (the `lint` workflow — JS via `pnpm lint`, Rust via `rustfmt`+`clippy`): reproduce and auto-fix locally — see the lint playbook below — then commit.
- **Test / build failures**: fix the underlying code. Never silence a failing test by deleting or skipping it; if a test is genuinely wrong, say so explicitly.
- Re-run with `gh run rerun <run-id>` (or push the fix and let it re-trigger).

**Lint / typecheck (local, from `--full`).** Auto-fix, then re-verify:
```bash
pnpm --filter web lint -- --fix      # next lint --fix
pnpm lint && pnpm typecheck          # astro check surfaces type errors to fix by hand
cargo fmt --all                      # Rust formatting
cargo clippy --fix --allow-dirty --allow-staged --all-targets -- -D warnings
cargo clippy --all-targets -- -D warnings    # confirm clean
```

**Deployment down.** Most of these are infra, not code — diagnose precisely and fix only what lives in the repo:
- **`unreachable` / NXDOMAIN** → the domain isn't deployed or DNS isn't pointed yet. Check the Vercel project exists and the custom domain is attached; report the exact next step (create project / add domain / point DNS) rather than guessing.
- **5xx** → a bad build or runtime error. Check the latest deploy in the Vercel dashboard or `vercel logs`; if the cause is in-repo (build break), fix and let it redeploy.
- **404 on `/robots.txt` or `/llms.txt`** → the file is missing from `apps/landing/public/`. Add it (these are required by the repo SEO rules).
- **`marker not in body` warning** → page is up but served unexpected content (wrong project at that domain, or a placeholder). Verify the right project owns the domain.

**HyperDX observability failing.** See `apps/web/OBSERVABILITY.md` for the full setup.
- **`server APM missing` / `browser RUM missing` / `SDK deps missing`** (✗) → the integration was removed or moved. Restore `apps/web/app/lib/observability.server.ts` (imported first by `apps/web/app/entry.server.tsx`), `apps/web/app/components/hyperdx-init.tsx` (mounted in `app/root.tsx`), and the `@hyperdx/node-opentelemetry` + `@hyperdx/browser` deps. These are real regressions — fix in-repo.
- **`OTLP endpoint no response`** (●) → usually benign (the collector rejects the GET probe). Only act if traces also stop appearing in HyperDX Search; then check `OTEL_EXPORTER_OTLP_ENDPOINT` and network egress.
- **`HYPERDX_API_KEY not set`** (●) → prod telemetry won't flow until the key is set. Add `HYPERDX_API_KEY` (server) and `VITE_HYPERDX_API_KEY` (browser, needed **at build time**) to the Vercel project env / encrypted `.env.production`. This is config, not code — hand back the concrete step.
- **`platform log-drain not configured`** (–) → intentional. The Vercel→HyperDX log drain is **Pro-only**; do not propose it unless the account is on Pro. App logs already ship via the SDK's console capture.

## 3. Re-verify and report

After fixing, re-run the matching mode (`--ci-only`, `--deploy-only`, or `--full`) and report the final summary. State plainly what was broken, what you changed, and anything that still needs a human (DNS, redeploy, infra access) — don't claim green unless the re-run is green.

## Guardrails

- Reformat/lint auto-fixers are safe to apply directly. Behavioral code changes (to fix a failing test/build) — explain what you changed and why.
- Do not push, redeploy, or open/merge PRs without the user's go-ahead unless they already asked you to.
- Deployment/DNS/infra problems are usually outside the repo: diagnose and hand back a concrete next step instead of inventing a code change.
