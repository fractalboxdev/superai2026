import { reactRouter } from "@react-router/dev/vite";
import { defineConfig } from "vite";

export default defineConfig({
  // Vite 8 resolves tsconfig `paths` (the @/* alias) natively.
  resolve: {
    tsconfigPaths: true,
    // loro-crdt's `browser.development` condition resolves to its wasm-ESM
    // bundler build, which Vite dev can't serve; pin to the base64-embedded
    // browser build — the same file the production build resolves.
    alias: { "loro-crdt": "loro-crdt/browser" },
  },
  plugins: [reactRouter()],
  ssr: {
    // @superai2026/protocol ships raw .ts via subpath exports — Vite must
    // transpile it for the SSR build instead of externalizing it as-is.
    noExternal: [/^@superai2026\//],
  },
});
