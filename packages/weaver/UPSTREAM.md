# Vendored: Weaver

These packages are vendored from the upstream Weaver editor so `apps/web`
can mount the real editor while replacing its sync backend with the
Contextful relay (spec `specs/01-room-sync.md` §2 "Integration decision").

- **Upstream repo:** <https://github.com/OpenHackersClub/weaver>
- **Vendored commit:** `a87da1d579ed511a10163e240e0cec77a144a6a2`
- **License:** MIT — see [`LICENSE`](./LICENSE) (upstream copy).

## What is vendored

| Package | Upstream path | Contents |
| --- | --- | --- |
| `@weaver/core` | `packages/core` | LoroDoc-backed block model, editor commands, presence hub, in-process peer link |
| `@weaver/dom` | `packages/dom` | contenteditable bridge, DOM/selection mappers, keymap, presence overlay |
| `@weaver/react` | `packages/react` | `useEditor` + `EditorRoot` React chrome |
| `@weaver/sync-core` | `packages/sync-core` | transport-agnostic per-doc relay (`SyncRoom`) |
| `@weaver/sync` | `packages/sync` | OPFS/IndexedDB persistence + raw-binary `WsBridge` + `initSync` |

## Local modifications

- `src/` only — upstream `tests/` and `vitest.config.ts` are not vendored
  (keeps jsdom / fake-indexeddb / playwright out of the workspace).
- `package.json`s rewritten: dependency versions reconciled to this repo
  (`loro-crdt ^1.13.1`, `effect ^3.21.3` — one resolved Loro WASM instance
  workspace-wide), test scripts/devDeps dropped, dead `./schema` and
  `./selection` export entries removed from `@weaver/core`.
- `tsconfig.json`s extend this repo's `tsconfig.base.json`.
- Source files are unmodified upstream copies unless noted here:
  - `dom/src/keymap.ts` — `latestCaret`'s unused `prev` parameter renamed to
    `_prev` (this repo typechecks with `noUnusedParameters`).

## Transport note

Upstream's default backend (Cloudflare Durable Object via `@weaver/server`)
is **not** vendored. The web app binds the Weaver editor's `LoroDoc` to the
Contextful wire protocol instead — see
`apps/web/app/lib/weaverRoom.ts` (transport plugin) and
`packages/protocol/src/sync.ts` (wire mirror).
