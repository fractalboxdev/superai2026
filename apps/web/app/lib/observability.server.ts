// Server-side HyperDX APM via OpenTelemetry.
//
// React Router has no Next-style `register()` hook, so the Node SDK is started
// here. `app/entry.server.tsx` imports this module first, so init runs at server
// startup (and per serverless cold start on Vercel) before requests are handled.
// Fire-and-forget so the module stays synchronous; no-op without an ingestion
// key (e.g. local dev) to avoid noisy export failures. The `.server.ts` suffix
// keeps it out of the client bundle.
//
// Configured via env (see .env.example / OBSERVABILITY.md):
//   HYPERDX_API_KEY              ingestion key
//   OTEL_SERVICE_NAME            service name shown in HyperDX
//   OTEL_EXPORTER_OTLP_ENDPOINT  defaults to https://in-otel.hyperdx.io
const apiKey = process.env.HYPERDX_API_KEY;
if (apiKey) {
  void import("@hyperdx/node-opentelemetry").then(({ init }) => {
    init({ apiKey, service: process.env.OTEL_SERVICE_NAME ?? "contextful-web" });
  });
}
