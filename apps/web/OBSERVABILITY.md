# Observability — HyperDX

`apps/web` reports to [HyperDX](https://www.hyperdx.io) through three complementary
integrations. The first two are wired in this repo; the third is a one-time setup
in the Vercel dashboard.

| Signal | How | Where | Needs Vercel Pro? |
| --- | --- | --- | --- |
| Server traces / APM | `@hyperdx/node-opentelemetry` via `src/instrumentation.ts` | this repo | No |
| App logs (`console.*`, server errors) | SDK console capture, on by default once `init()` runs | this repo | No |
| Browser RUM + session replay | `@hyperdx/browser` via `src/components/hyperdx-init.tsx` | this repo | No |
| Vercel **platform** logs (build, edge/infra) | Vercel → HyperDX **Log Drain** | Vercel Marketplace | **Yes** |

Everything except the platform log drain ships OTLP directly from the app to
HyperDX over HTTPS — no Vercel log drain, and works on the free **Hobby** plan.

## How it's wired

- **`next.config.mjs`** marks `@hyperdx/node-opentelemetry` as a
  `serverExternalPackage` so Next.js doesn't bundle it — the OTel SDK patches
  modules at runtime via `require-in-the-middle`, which only works when the
  package is required, not bundled.
- **`src/instrumentation.ts`** — Next.js calls `register()` once per server (and
  per Vercel serverless cold start). It inits HyperDX only on the `nodejs`
  runtime, and is a **no-op without `HYPERDX_API_KEY`**, so local dev and preview
  builds without a key don't error.
- **`src/components/hyperdx-init.tsx`** — a `"use client"` component mounted in
  `layout.tsx`. Inits browser RUM only when `NEXT_PUBLIC_HYPERDX_API_KEY` is set.

## Environment variables

Defined in `.env.example` (template) and `.env.production` (dotenvx-encrypted).
Set the same values in the **Vercel project → Settings → Environment Variables**.

| Var | Scope | Notes |
| --- | --- | --- |
| `HYPERDX_API_KEY` | server | Ingestion key. Read by `instrumentation.ts`. |
| `NEXT_PUBLIC_HYPERDX_API_KEY` | client | Browser RUM key — inlined at build time, so it must be present **when Vercel builds**. Usually the same key. |
| `OTEL_SERVICE_NAME` | server | Service name in HyperDX. Defaults to `contextful-web`. |
| `NEXT_PUBLIC_OTEL_SERVICE_NAME` | client | Browser service name. Defaults to `contextful-web-browser`. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | server | `https://in-otel.hyperdx.io` (HyperDX Cloud). Override for self-hosted. |

Get the ingestion key from **HyperDX → Team Settings → API Keys → Ingestion API Key**.

## Vercel Log Drain (optional, Pro-only — not in code)

> **Requires a Vercel Pro plan.** Log Drains are gated behind Pro; on the Hobby
> plan the Marketplace integration shows *"Log Drains are required to install
> this integration. Upgrade to Pro to continue."* **You do not need this** — app
> logs already reach HyperDX via the SDK's console capture (see table above).
> The drain only adds Vercel's *platform* logs: build output, edge/static, and
> anything logged before your instrumented function runs.

If/when you're on Pro and want those infra logs too:

1. Open the [HyperDX integration](https://vercel.com/integrations/hyperdx) in the
   Vercel Marketplace and click **Add Integration**.
2. Select the **web** project (and any others you want logs from).
3. Paste the HyperDX ingestion key when prompted.

### Logs without Pro

Application logs are already captured: the Node SDK forwards `console.*` (and
thrown server errors) as OTel logs by default — controlled by
`HDX_NODE_CONSOLE_CAPTURE` (`0` disables). For structured logging, attach a
transport instead, e.g. `HyperDX.getWinstonTransport('info', { detectResources: true })`
or the Pino transport.

## Local testing

```bash
# Put real values in .env.local (gitignored), then:
HYPERDX_API_KEY=… NEXT_PUBLIC_HYPERDX_API_KEY=… pnpm --filter web dev
```

Exercise a few routes, then confirm the `contextful-web` service appears in the
HyperDX **Search** view (traces) and **Sessions** view (browser replay).
