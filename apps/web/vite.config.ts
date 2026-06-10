import { reactRouter } from "@react-router/dev/vite";
import { defineConfig } from "vite";
import topLevelAwait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

export default defineConfig(({ command }) => ({
  // Vite 8 resolves tsconfig `paths` (the @/* alias) natively.
  resolve: { tsconfigPaths: true },
  // Allow access via Tailscale MagicDNS hostnames (tailscale serve), and bind
  // IPv4 loopback explicitly — `tailscale serve` proxies to 127.0.0.1, which a
  // [::1]-only listener 502s.
  server: { host: "127.0.0.1", allowedHosts: [".ts.net"] },
  preview: { host: "127.0.0.1", allowedHosts: [".ts.net"] },
  // wasm + topLevelAwait: loro-crdt (the Weaver editor's CRDT) ships a
  // bundler-style ESM WASM import that the DEV server can't serve natively.
  // Build-time, rolldown bundles the .wasm as a lazy async asset on its own —
  // and vite-plugin-top-level-await's esbuild pass breaks under rolldown —
  // so the plugins are dev-only.
  plugins:
    command === "serve"
      ? [wasm(), topLevelAwait(), reactRouter()]
      : [reactRouter()],
  ssr: {
    // @superai2026/protocol and the vendored @weaver/* packages ship raw .ts
    // via subpath exports — Vite must transpile them for the SSR build
    // instead of externalizing them as-is. (The Weaver editor itself is only
    // ever mounted client-side; this just keeps the SSR module graph valid.)
    noExternal: [/^@superai2026\//, /^@weaver\//],
    // loro-crdt's node entry instantiates WASM synchronously at module eval —
    // keep it external so bundling @weaver/* can never pull that into SSR
    // init; it is only ever reached via the client-side dynamic imports.
    external: ["loro-crdt"],
  },
  optimizeDeps: {
    // loro-crdt is WASM-backed; serve it as-is in dev instead of prebundling
    // the WASM glue through esbuild (same exclusion the Weaver playground uses).
    exclude: ["loro-crdt"],
  },
}));
