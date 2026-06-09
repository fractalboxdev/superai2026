// Contextful · Pulumi recipes (spec 07 §5).
//
// Idempotently provisions the cloud-side and bootstrap steps. Spec-only this
// pass: the recipes are real and structurally complete, but applying them needs
// provider credentials (Vercel token, AWS creds, Tailscale API key) and a
// `pulumi up`. `crates/sync` itself is NOT deployed here — it runs on the host.
//
//   cd infra && pnpm install && pulumi preview
//
// Trust-boundary note (spec 07 §2): only already-permitted, capability-redacted
// content ever reaches cloud inference/sandbox; raw source data and un-redacted
// brain content never leave the host. This path can be disabled entirely (Flow D).

import * as pulumi from "@pulumi/pulumi";
import * as vercel from "@pulumiverse/vercel";
import * as aws from "@pulumi/aws";
import * as tailscale from "@pulumi/tailscale";
import { local } from "@pulumi/command";

const cfg = new pulumi.Config("contextful");
const tailnetTag = cfg.get("tailnetTag") ?? "tag:contextful";
const syncPort = cfg.get("syncPort") ?? "7878";
const mcpPort = cfg.get("mcpPort") ?? "7879";
const bedrockRegion = cfg.get("bedrockRegion") ?? "us-east-1";
const gitRepo = "fractalboxdev/superai2026";

// --- Vercel projects + custom domains (spec 07 §4) ------------------------

const landing = new vercel.Project("landing", {
  name: "contextful-landing",
  framework: "astro",
  rootDirectory: "apps/landing",
  gitRepository: { type: "github", repo: gitRepo },
});
new vercel.ProjectDomain("landing-domain", {
  projectId: landing.id,
  domain: "www.contextful.work",
});

const web = new vercel.Project("web", {
  name: "contextful-web",
  framework: "nextjs",
  rootDirectory: "apps/web",
  gitRepository: { type: "github", repo: gitRepo },
  environments: [
    // already-permitted content only; structured query + redaction need no LLM
    { key: "AWS_REGION", value: bedrockRegion, targets: ["production"] },
  ],
});
new vercel.ProjectDomain("web-domain", {
  projectId: web.id,
  domain: "demo.contextful.work",
});

// --- Tailscale ACL: restrict who can reach the sync/MCP ports (spec 07 §2) -

new tailscale.Acl("contextful-acl", {
  acl: JSON.stringify({
    tagOwners: { [tailnetTag]: ["autogroup:admin"] },
    acls: [
      // only tagged nodes (host + sandbox agents) may reach sync + MCP
      { action: "accept", src: [tailnetTag], dst: [`${tailnetTag}:${syncPort},${mcpPort}`] },
    ],
  }),
});

// --- Bedrock access for inference (spec 02 §7, spec 04 §3) ----------------

const bedrockRole = new aws.iam.Role("contextful-bedrock", {
  assumeRolePolicy: JSON.stringify({
    Version: "2012-10-17",
    Statement: [
      { Effect: "Allow", Principal: { Service: "ec2.amazonaws.com" }, Action: "sts:AssumeRole" },
    ],
  }),
});
new aws.iam.RolePolicy("contextful-bedrock-invoke", {
  role: bedrockRole.id,
  policy: JSON.stringify({
    Version: "2012-10-17",
    Statement: [
      {
        Effect: "Allow",
        Action: ["bedrock:InvokeModel", "bedrock:InvokeModelWithResponseStream", "bedrock:Converse"],
        Resource: "*",
      },
    ],
  }),
});

// --- Host bootstrap: lay down ~/.contextful + seed the control plane -------

const bootstrap = new local.Command("contextful-seed", {
  create: "sync ctl seed && sync ingest --source stripe",
  environment: { CONTEXTFUL_HOME: "${HOME}/.contextful" },
});

export const landingUrl = "https://www.contextful.work";
export const webUrl = "https://demo.contextful.work";
export const bedrockRoleArn = bedrockRole.arn;
export const seeded = bootstrap.stdout;
