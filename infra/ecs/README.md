# infra/ecs — `sync serve` on AWS ECS Fargate (demo)

Deploys the `crates/sync` server binary to a single Fargate task in
`us-west-2`, fronted by a network load balancer:

- `ws://<nlb>:7878` — Loro WS sync relay (`serve`)
- `http://<nlb>:7979/mcp` — brain MCP over streamable HTTP (`--with-mcp`)

The task also co-hosts the **editor agent** (`--with-editor-agent`), matching
the desktop host role: `Q:` lines typed into the relay doc by the web editor
are answered from brain memory, capability-filtered, as the configured
principal. The watched doc and answering principal are Pulumi config
(`contextful-ecs:agentDoc` / `contextful-ecs:agentPrincipal`, defaults
`finops` / `cfo`).

The image is built from the repo-root [`Dockerfile`](../../Dockerfile)
(linux/arm64 → Fargate Graviton). On first boot the container seeds the demo
control plane (`sync ctl seed`) and runs one offline-floor ingest; task state
is ephemeral by design.

Like `../`, this package is **outside the pnpm/turbo workspace** — treat it as
a standalone project.

## Credentials (dotenvx)

AWS credentials live **encrypted** in [`.env.production`](./.env.production)
(safe to commit). Decryption needs the private key in `.env.keys`, which is
gitignored and never leaves the machine that ran `dotenvx encrypt`. All
scripts route through dotenvx, so a plain `pulumi up` without the key fails —
that is the point.

## Usage

```sh
cd infra/ecs
pnpm install
pulumi stack select dev   # or: pulumi stack init dev
pnpm preview              # dotenvx run -f .env.production -- pulumi preview
pnpm up
pnpm outputs              # relayUrl / mcpUrl / nlbDnsName
pnpm destroy              # tear the demo down
```

Demo-account posture: both ports are open to the world and the account is a
throwaway — do not point real data at this. The local-first trust boundary
(spec 07 §2) is not in play here; the task only ever holds the seeded demo
scenario.
