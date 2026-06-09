// Acceptance flows (spec 09 §1) driven against the built `sync` binary.
//
// flare-dispatch + Playwright is the intended browser-level harness for
// collaborative-editing e2e; this suite exercises the host side — control plane
// + brain MCP — which is where Flows A and B are decided. Skips gracefully if
// the release/debug binary hasn't been built (so `turbo run test` stays green
// without a Cargo toolchain in CI).

import { spawn, execFileSync } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");
const BIN = process.env.SYNC_BIN ?? join(repoRoot, "target", "debug", "sync");
const HOME = mkdtempSync(join(tmpdir(), "contextful-accept-"));
const env = { ...process.env, CONTEXTFUL_HOME: HOME, RUST_LOG: "error" };

const haveBin = existsSync(BIN);
const d = haveBin ? describe : describe.skip;

/** Drive the MCP stdio server: send JSON-RPC lines, collect responses by id. */
function mcpCall(principal: string, requests: object[]): Promise<Record<number, any>> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(BIN, ["mcp", "--principal", principal], { env });
    const byId: Record<number, any> = {};
    let buf = "";
    child.stdout.on("data", (chunk) => {
      buf += chunk.toString();
      let nl: number;
      while ((nl = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (!line) continue;
        try {
          const msg = JSON.parse(line);
          if (msg.id != null) byId[msg.id] = msg;
        } catch {
          /* ignore non-JSON log lines */
        }
      }
      if (Object.keys(byId).length >= requests.length) {
        child.kill();
        resolvePromise(byId);
      }
    });
    child.on("error", reject);
    child.on("close", () => resolvePromise(byId));
    for (const r of requests) child.stdin.write(JSON.stringify(r) + "\n");
  });
}

const query = (id: number, view: string, select: string[]) => ({
  jsonrpc: "2.0",
  id,
  method: "tools/call",
  params: { name: "brain.query", arguments: { view, select } },
});

d("Contextful acceptance flows", () => {
  beforeAll(() => {
    execFileSync(BIN, ["ctl", "seed"], { env });
    execFileSync(BIN, ["ingest", "--source", "stripe"], { env });
  });
  afterAll(() => {
    /* temp HOME left for inspection; OS cleans tmpdir */
  });

  it("CFO (root) sees employee_salary", async () => {
    const r = await mcpCall("cfo", [query(1, "stripe/finance_private", ["gross", "credits", "employee_salary"])]);
    const sc = r[1]?.result?.structuredContent;
    expect(sc?.status).toBe("ok");
    expect(sc.fields).toContain("employee_salary");
    expect(sc.rows[0]).toHaveProperty("employee_salary");
  });

  it("Flow B — engineering agent is denied finance_private", async () => {
    const r = await mcpCall("agent:eng/1", [query(1, "stripe/finance_private", ["employee_salary"])]);
    const sc = r[1]?.result?.structuredContent;
    expect(sc?.status).toBe("denied");
  });

  it("Flow B — request_access for salary is forbidden (no path)", async () => {
    const req = {
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "brain.request_access", arguments: { view: "stripe/finance_private", fields: ["employee_salary"], reason: "benchmark" } },
    };
    const r = await mcpCall("agent:eng/1", [req]);
    expect(r[1]?.result?.structuredContent?.routing?.decision).toBe("forbidden");
  });

  it("CTO base token sees team spend (spend_by_team)", async () => {
    const r = await mcpCall("agent:cto/1", [query(1, "stripe/spend_by_team", ["gross", "net"])]);
    const sc = r[1]?.result?.structuredContent;
    expect(sc?.status).toBe("ok");
    expect(sc.rows.length).toBeGreaterThan(0);
  });
});
