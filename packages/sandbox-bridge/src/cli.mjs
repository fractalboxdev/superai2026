#!/usr/bin/env node
// @superai2026/sandbox-bridge — the only TypeScript/Node in the sandbox path
// (spec 04 §2). The Rust driver (crates/sync/src/sandbox/vercel.rs) spawns
// this per call; stdout is exactly one JSON line. No policy decisions here —
// lifecycle choice, room→sandbox registry, and identity minting live in Rust.
//
// Usage:
//   cli.mjs create --room <id> [--timeout-ms <n>]
//   cli.mjs extend --sandbox <id> [--timeout-ms <n>]
//   cli.mjs stop   --sandbox <id>
//
// Auth: VERCEL_TOKEN (required); VERCEL_TEAM_ID / VERCEL_PROJECT_ID are
// auto-discovered from the token's first team/project when unset.

import { Sandbox } from "@vercel/sandbox";
import { sandboxLogsUrl } from "./dashboard.mjs";

const DEFAULT_TIMEOUT_MS = 5 * 60 * 1000; // bridge default; Rust decides policy

function parseArgs(argv) {
  const [cmd, ...rest] = argv;
  const args = { cmd };
  for (let i = 0; i < rest.length; i += 2) {
    const key = rest[i]?.replace(/^--/, "");
    args[key] = rest[i + 1];
  }
  return args;
}

async function vercelApi(path, token) {
  const res = await fetch(`https://api.vercel.com${path}`, {
    headers: { authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    throw new Error(`vercel api ${path}: ${res.status} ${await res.text()}`);
  }
  return res.json();
}

/** Resolve teamId/projectId from env, discovering via the API if missing.
 * Also resolves the dashboard slugs (team slug + project name) so `create`
 * can hand back a logs URL — best-effort, never fatal. */
async function auth() {
  const token = process.env.VERCEL_TOKEN;
  if (!token) throw new Error("VERCEL_TOKEN is required");
  let teamId = process.env.VERCEL_TEAM_ID;
  let projectId = process.env.VERCEL_PROJECT_ID;
  let teamSlug;
  let projectName;
  if (!teamId) {
    const { teams } = await vercelApi("/v2/teams?limit=1", token);
    teamId = teams?.[0]?.id;
    teamSlug = teams?.[0]?.slug;
    if (!teamId) throw new Error("no team found for VERCEL_TOKEN — set VERCEL_TEAM_ID");
  }
  if (!projectId) {
    const { projects } = await vercelApi(
      `/v9/projects?teamId=${encodeURIComponent(teamId)}&limit=1`,
      token,
    );
    projectId = projects?.[0]?.id;
    projectName = projects?.[0]?.name;
    if (!projectId) throw new Error("no project found — set VERCEL_PROJECT_ID");
  }
  // env-provided ids skip discovery above; look the slugs up directly
  try {
    if (!teamSlug) {
      const team = await vercelApi(`/v2/teams/${encodeURIComponent(teamId)}`, token);
      teamSlug = team?.slug;
    }
    if (!projectName) {
      const project = await vercelApi(
        `/v9/projects/${encodeURIComponent(projectId)}?teamId=${encodeURIComponent(teamId)}`,
        token,
      );
      projectName = project?.name;
    }
  } catch {
    // slugs are only for the logs URL — sandbox lifecycle must not fail on them
  }
  return { token, teamId, projectId, teamSlug, projectName };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const timeout = Number(args["timeout-ms"]) || DEFAULT_TIMEOUT_MS;
  const { teamSlug, projectName, ...credentials } = await auth();

  switch (args.cmd) {
    case "create": {
      if (!args.room) throw new Error("--room is required");
      const sandbox = await Sandbox.create({ ...credentials, timeout });
      return {
        ok: true,
        action: "create",
        room: args.room,
        sandboxId: sandbox.sandboxId,
        timeoutMs: timeout,
        logsUrl: sandboxLogsUrl({ teamSlug, projectName }),
      };
    }
    case "extend": {
      if (!args.sandbox) throw new Error("--sandbox is required");
      const sandbox = await Sandbox.get({ ...credentials, sandboxId: args.sandbox });
      await sandbox.extendTimeout({ timeout });
      return { ok: true, action: "extend", sandboxId: args.sandbox, timeoutMs: timeout };
    }
    case "stop": {
      if (!args.sandbox) throw new Error("--sandbox is required");
      const sandbox = await Sandbox.get({ ...credentials, sandboxId: args.sandbox });
      await sandbox.stop();
      return { ok: true, action: "stop", sandboxId: args.sandbox };
    }
    default:
      throw new Error(`unknown command '${args.cmd}' (create|extend|stop)`);
  }
}

main()
  .then((result) => {
    process.stdout.write(JSON.stringify(result) + "\n");
  })
  .catch((err) => {
    process.stdout.write(JSON.stringify({ ok: false, error: String(err?.message ?? err) }) + "\n");
    process.exit(1);
  });
