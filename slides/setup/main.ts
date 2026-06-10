// Slidev bug workaround: with `routerMode: hash` + a non-root `--base`
// (we deploy under /slides/), the client's getSlidePath() prefixes routes
// with BASE_URL, producing hash routes like `#/slides/2` that match nothing
// and render the built-in 404. Strip the base prefix back out of every
// navigation so `#/slides/2` (and presenter/print variants) resolve to `#/2`.
// No-op in dev and for history mode, where BASE_URL handling is correct.

// `defineAppSetup` from @slidev/types is just an identity helper; export the
// plain function to avoid adding the dependency.
export default function setupMain({ router }: { router: any }) {
  const base: string = import.meta.env.BASE_URL ?? '/'
  if (base === '/' || !base)
    return
  const prefix = base.endsWith('/') ? base.slice(0, -1) : base
  router.beforeEach((to: any) => {
    if (to.path === prefix || to.path.startsWith(`${prefix}/`)) {
      const stripped = to.path.slice(prefix.length) || '/'
      return { path: stripped, query: to.query, hash: to.hash, replace: true }
    }
  })
}
