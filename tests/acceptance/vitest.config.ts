import { defineConfig } from "vitest/config";

// These e2e tests spawn the built `sync` binary (cold start + ingest) and drive
// it over MCP/WS. On a loaded CI container that is slower than the vitest
// defaults (5s test / 10s hook), so raise the ceilings to avoid false-flakes.
export default defineConfig({
  test: {
    testTimeout: 20_000,
    hookTimeout: 30_000,
  },
});
