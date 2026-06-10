// Offline contract tests for the bridge CLI: whatever happens, stdout is
// exactly one JSON line and failures exit non-zero (the Rust driver,
// crates/sync/src/sandbox/vercel.rs, parses nothing else).

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";

const CLI = fileURLToPath(new URL("../src/cli.mjs", import.meta.url));

// No VERCEL_TOKEN in the child env — every path must fail before any network.
function runOffline(args) {
  const env = { ...process.env };
  delete env.VERCEL_TOKEN;
  delete env.VERCEL_TEAM_ID;
  delete env.VERCEL_PROJECT_ID;
  return new Promise((resolve) => {
    execFile(process.execPath, [CLI, ...args], { env }, (err, stdout, stderr) =>
      resolve({ code: err?.code ?? 0, stdout, stderr }),
    );
  });
}

test("missing VERCEL_TOKEN fails with one JSON error line", async () => {
  const { code, stdout } = await runOffline(["create", "--room", "finops"]);
  assert.equal(code, 1);
  const lines = stdout.trim().split("\n");
  assert.equal(lines.length, 1);
  const result = JSON.parse(lines[0]);
  assert.equal(result.ok, false);
  assert.match(result.error, /VERCEL_TOKEN/);
});

test("unknown command also keeps the one-JSON-line contract", async () => {
  const { code, stdout } = await runOffline(["destroy", "--sandbox", "sbx_1"]);
  assert.equal(code, 1);
  const result = JSON.parse(stdout.trim());
  assert.equal(result.ok, false);
});
