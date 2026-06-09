# infra/ — Contextful Pulumi recipes (spec 07 §5)

Cloud-side IaC: Vercel projects + custom domains (`www` / `demo`), Tailscale
ACL/tag policy for the sync/MCP ports, Bedrock IAM for inference, and a host
bootstrap that lays down `~/.contextful` and runs `sync ctl seed`.

**Spec-only this pass.** The recipes are real and structurally complete, but
applying them needs provider credentials and a `pulumi up`. `crates/sync` is
**not** deployed here — it runs on the host.

This package is deliberately **outside the pnpm/turbo workspace** (not matched by
`pnpm-workspace.yaml`) so CI doesn't pull the heavy Pulumi provider SDKs. Treat
it as a standalone project:

```sh
cd infra
pnpm install
pulumi stack init dev
# provide credentials: VERCEL_API_TOKEN, AWS_*, TAILSCALE_API_KEY
pulumi preview
```

Trust boundary (spec 07 §2): only already-permitted, capability-redacted content
ever reaches cloud inference/sandbox; raw source data and un-redacted brain
content never leave the host, and the cloud path can be disabled entirely (Flow D).
