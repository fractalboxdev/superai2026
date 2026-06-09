// HyperDX server-side APM via OpenTelemetry.
//
// Next.js calls register() once per server (and per serverless cold start on
// Vercel) before any request is handled — this is the supported hook for
// initialising tracing. The Node SDK requires Node APIs, so we skip the Edge
// runtime, and we no-op when no ingestion key is configured (e.g. local dev)
// to avoid noisy export failures.
//
// Configured via env (see .env.example / OBSERVABILITY.md):
//   HYPERDX_API_KEY              ingestion key
//   OTEL_SERVICE_NAME            service name shown in HyperDX
//   OTEL_EXPORTER_OTLP_ENDPOINT  defaults to https://in-otel.hyperdx.io
export async function register() {
  if (process.env.NEXT_RUNTIME !== "nodejs") return;
  if (!process.env.HYPERDX_API_KEY) return;

  const { init } = await import("@hyperdx/node-opentelemetry");
  init({
    apiKey: process.env.HYPERDX_API_KEY,
    service: process.env.OTEL_SERVICE_NAME ?? "contextful-web",
  });
}
