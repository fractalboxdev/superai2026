// Demo acceptance — proves the binary's brain MCP server is RUNNING on both
// spec-06 transports and serves the example queries used in the demo script.
//
// flows/flowc cover the authorization semantics in depth; this suite is the
// liveness + "does the demo work" layer:
//   1. stdio transport answers the MCP handshake (initialize / ping / tools/list)
//   2. `serve --with-mcp` co-hosts the streamable-HTTP endpoint (POST /mcp,
//      identity via x-contextful-principal, 401 without it)
//   3. each example query below succeeds verbatim — they are copy-pasteable
//      JSON-RPC bodies for a live demo or an MCP client config.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { ChildProcess } from "node:child_process";
import {
  callTool,
  freshHome,
  haveBin,
  mcp,
  mcpHttp,
  queryTool,
  seedAndIngest,
  startServerWithMcp,
  toolCall,
  type Env,
} from "./helpers";

const d = haveBin ? describe : describe.skip;

// Per-worker ports, offset away from relay.e2e (7901+) so suites never collide.
const RELAY_PORT = 7951 + Number(process.env.VITEST_WORKER_ID ?? 0);
const MCP_PORT = 8051 + Number(process.env.VITEST_WORKER_ID ?? 0);

const EXPECTED_TOOLS = [
  "brain.list_sources",
  "brain.search",
  "brain.get_context",
  "brain.query",
  "brain.detect_anomalies",
  "brain.remember",
  "brain.request_access",
  "brain.world_search",
  "brain.ground",
  "brain.daydreams",
];

d("MCP server liveness — stdio transport", () => {
  let env: Env;
  beforeAll(() => {
    env = freshHome();
    seedAndIngest(env);
  });

  it("answers the MCP handshake: initialize → ping → tools/list", async () => {
    const r = await mcp(env, "cfo", [
      { jsonrpc: "2.0", id: 1, method: "initialize", params: {} },
      { jsonrpc: "2.0", method: "notifications/initialized" } as any,
      { jsonrpc: "2.0", id: 2, method: "ping" },
      { jsonrpc: "2.0", id: 3, method: "tools/list" },
    ]);

    expect(r[1].result.serverInfo.name).toBe("contextful-brain");
    expect(r[1].result.protocolVersion).toBe("2024-11-05");
    expect(r[2].result).toEqual({});
    const names = r[3].result.tools.map((t: any) => t.name);
    expect(names).toEqual(expect.arrayContaining(EXPECTED_TOOLS));
  });

  it("rejects an unknown method with a JSON-RPC error, not silence", async () => {
    const r = await mcp(env, "cfo", [{ jsonrpc: "2.0", id: 1, method: "no/such/method" }]);
    expect(r[1].error.code).toBe(-32601);
  });
});

d("MCP server liveness — streamable HTTP (`serve --with-mcp`)", () => {
  let env: Env;
  let server: ChildProcess;
  beforeAll(async () => {
    env = freshHome();
    seedAndIngest(env);
    server = await startServerWithMcp(env, RELAY_PORT, MCP_PORT);
  });
  afterAll(() => {
    server?.kill();
  });

  it("co-hosted endpoint is up and answers initialize", async () => {
    const res = await mcpHttp(MCP_PORT, "cfo", { jsonrpc: "2.0", id: 1, method: "initialize" });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.result.serverInfo.name).toBe("contextful-brain");
  });

  it("rejects a call without the principal header (401)", async () => {
    const res = await mcpHttp(MCP_PORT, null, { jsonrpc: "2.0", id: 1, method: "ping" });
    expect(res.status).toBe(401);
  });

  it("serves a brain.query tool call over HTTP", async () => {
    const res = await mcpHttp(MCP_PORT, "agent:cto/1", queryTool(1, "stripe/spend_by_team", ["team", "gross"]));
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.result.isError).toBe(false);
    expect(body.result.structuredContent.status).toBe("ok");
    expect(body.result.structuredContent.rows.length).toBeGreaterThan(0);
  });
});

// Seeded authority (spec 00 §3): cfo → stripe/finance_private (all fields);
// agent:cto/1 → stripe/spend_by_team. Each example runs as a principal that
// actually holds the view, so the bodies work verbatim in a live demo.
d("Example queries (demo script, over stdio)", () => {
  let env: Env;
  beforeAll(() => {
    env = freshHome();
    seedAndIngest(env);
  });

  // {"method":"tools/call","params":{"name":"brain.list_sources","arguments":{}}}
  it("brain.list_sources — what views can I see?", async () => {
    const cfo = await callTool(env, "cfo", "brain.list_sources", {});
    expect(cfo.structuredContent.views).toContain("stripe/finance_private");
    const cto = await callTool(env, "agent:cto/1", "brain.list_sources", {});
    expect(cto.structuredContent.views).toContain("stripe/spend_by_team");
  });

  // {"name":"brain.query","arguments":{"view":"stripe/spend_by_team","select":["team","gross","net"]}}
  it("brain.query — spend by team with a synthesized answer", async () => {
    const r = await callTool(env, "agent:cto/1", "brain.query", {
      view: "stripe/spend_by_team",
      select: ["team", "gross", "net"],
    });
    const sc = r.structuredContent;
    expect(sc.status).toBe("ok");
    expect(sc.fields).toEqual(expect.arrayContaining(["team", "gross"]));
    expect(sc.rows.length).toBeGreaterThan(0);
    expect(sc.answer).toBeTruthy();
  });

  // {"name":"brain.search","arguments":{"query":"spend"}}
  it("brain.search — keyword search over synthesized cards", async () => {
    const r = await callTool(env, "agent:cto/1", "brain.search", { query: "spend" });
    const topics = r.structuredContent.results.map((m: any) => m.topic);
    expect(topics).toContain("spend");
  });

  // {"name":"brain.get_context","arguments":{"topic":"finance"}}
  it("brain.get_context — Markdown context card by topic", async () => {
    const r = await callTool(env, "cfo", "brain.get_context", { topic: "finance" });
    expect(r.isError).toBe(false);
    expect(r.structuredContent.card).toMatch(/finance/i);
  });

  // {"name":"brain.detect_anomalies","arguments":{"view":"stripe/spend_by_team"}}
  it("brain.detect_anomalies — anomaly scan for a view", async () => {
    const r = await callTool(env, "agent:cto/1", "brain.detect_anomalies", { view: "stripe/spend_by_team" });
    expect(r.isError).toBe(false);
    expect(r.structuredContent.status).toBe("ok");
  });

  // Same query as a scoped agent shows the permission story in one call.
  it("brain.query as agent:eng/1 against finance_private is denied", async () => {
    const r = await callTool(env, "agent:eng/1", "brain.query", {
      view: "stripe/finance_private",
      select: ["gross"],
    });
    expect(r.structuredContent.status).toBe("denied");
  });

  it("several example queries survive one stdio session back-to-back", async () => {
    const r = await mcp(env, "agent:cto/1", [
      toolCall(1, "brain.list_sources", {}),
      queryTool(2, "stripe/spend_by_team", ["team", "gross"]),
      toolCall(3, "brain.search", { query: "spend" }),
    ]);
    expect(r[1].result.isError).toBe(false);
    expect(r[2].result.structuredContent.status).toBe("ok");
    expect(r[3].result.isError).toBe(false);
  });
});
