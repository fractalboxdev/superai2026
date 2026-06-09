# packages/

Shared internal JS/TS libraries consumed by `apps/*`.

Currently empty (placeholder). Likely future tenants:

- generated TypeScript bindings for the `crates/sync` protocol
- shared config (ESLint, TS) and UI primitives

Add a package as `packages/<name>/package.json` with `"name": "@superai2026/<name>"`;
the pnpm workspace glob (`packages/*`) picks it up automatically.
