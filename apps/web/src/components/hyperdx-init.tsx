"use client";

import { useEffect } from "react";

// HyperDX browser RUM: session replay, console capture, and W3C trace-context
// propagation so frontend spans link to backend traces. Initialised once on
// mount, and only when a client-exposed ingestion key is present — so previews
// and local dev without a key are a no-op rather than a console error.
//
// NEXT_PUBLIC_* values are inlined at build time, so the key must be set in the
// Vercel project's environment before building (see OBSERVABILITY.md).
export function HyperDXInit() {
  useEffect(() => {
    const apiKey = process.env.NEXT_PUBLIC_HYPERDX_API_KEY;
    if (!apiKey) return;

    let cancelled = false;
    void import("@hyperdx/browser").then(({ default: HyperDX }) => {
      if (cancelled) return;
      HyperDX.init({
        apiKey,
        service:
          process.env.NEXT_PUBLIC_OTEL_SERVICE_NAME ?? "contextful-web-browser",
        // Link browser traces to the backend for same-origin / API calls.
        tracePropagationTargets: [/contextful\.work/i],
        consoleCapture: true,
        advancedNetworkCapture: true,
      });
    });

    return () => {
      cancelled = true;
    };
  }, []);

  return null;
}
