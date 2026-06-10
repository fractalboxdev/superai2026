// Acceptance — the reference flows (spec 09 §1) driven against the built `sync`
// binary over the brain MCP + control plane. flare-dispatch's product-demo run
// is the browser-level harness; this is the host-side decision layer where
// Flows A/B/C/D are actually enforced.
//
// Seeded token scopes (spec 00 §3): cfo → finance_private (all fields incl
// salary); agent:cto/1 → spend_by_team; agent:eng/1 → spend_by_team (team=eng).

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import {
  callTool,
  freshHome,
  haveBin,
  mcp,
  queryTool,
  run,
  seedAndIngest,
  toolCall,
  type Env,
} from "./helpers";

const d = haveBin ? describe : describe.skip;

d("Reference flows over the brain MCP", () => {
  // `flow` is mutated by Flow A/B grants; `base` stays at seeded scopes for the
  // read-only tool-surface assertions.
  let flow: Env;
  let base: Env;
  beforeAll(() => {
    flow = freshHome();
    seedAndIngest(flow);
    base = freshHome();
    seedAndIngest(base);
  });
  afterAll(() => {
    /* OS cleans the temp CONTEXTFUL_HOME dirs */
  });

  // ---- Flow A: request → approve → scoped pull ----
  describe("Flow A — request → approve → scoped pull", () => {
    it("Richard (CEO)'s agent is denied finance_private up front", async () => {
      const r = await callTool(flow, "agent:cto/1", "brain.query", {
        view: "stripe/finance_private",
        select: ["credits", "discount_tier"],
      });
      expect(r.structuredContent.status).toBe("denied");
      expect(r.structuredContent.reason).toBe("no_grant");
    });

    it("request_access for non-salary fields escalates to the owner (a real path)", async () => {
      const r = await callTool(flow, "agent:cto/1", "brain.request_access", {
        view: "stripe/finance_private",
        fields: ["credits", "discount_tier"],
        reason: "net-of-credits",
      });
      expect(r.structuredContent.routing.decision).toBe("escalate");
    });

    it("after CFO approval (ctl grant) the agent reads net-of-credits, salary redacted", async () => {
      run(flow, [
        "ctl",
        "grant",
        "--to",
        "agent:cto/1",
        "--view",
        "stripe/finance_private",
        "--fields",
        "gross,credits,discount_tier",
      ]);
      const r = await callTool(flow, "agent:cto/1", "brain.query", {
        view: "stripe/finance_private",
        select: ["gross", "credits", "discount_tier", "employee_salary"],
      });
      const sc = r.structuredContent;
      expect(sc.status).toBe("ok");
      expect(sc.fields).toEqual(expect.arrayContaining(["credits", "discount_tier"]));
      expect(sc.redacted).toContain("employee_salary"); // salary never appears
      expect(sc.rows).toHaveLength(4);
      expect(sc.rows.every((row: Record<string, unknown>) => !("employee_salary" in row))).toBe(true);
      expect(sc.answer).toMatch(/net of credits/i);
    });
  });

  // ---- Flow B: the salary invariant (negative) ----
  describe("Flow B — salary invariant", () => {
    it("Dinesh (CTO)'s agent's salary query is denied", async () => {
      const r = await callTool(flow, "agent:eng/1", "brain.query", {
        view: "stripe/finance_private",
        select: ["employee_salary"],
      });
      expect(r.structuredContent.status).toBe("denied");
    });

    it("request_access for salary is forbidden — no approval path", async () => {
      const r = await callTool(flow, "agent:eng/1", "brain.request_access", {
        view: "stripe/finance_private",
        fields: ["employee_salary"],
        reason: "benchmark comp",
      });
      expect(r.structuredContent.routing.decision).toBe("forbidden");
    });

    it("ctl grant of salary is refused by the minter (defense in depth)", () => {
      expect(() =>
        run(flow, [
          "ctl",
          "grant",
          "--to",
          "agent:eng/1",
          "--view",
          "stripe/finance_private",
          "--fields",
          "employee_salary",
        ]),
      ).toThrow();
    });

    it("even after a legit grant, salary stays redacted for the granted agent", async () => {
      run(flow, ["ctl", "grant", "--to", "agent:eng/1", "--view", "stripe/finance_private", "--fields", "credits"]);
      const r = await callTool(flow, "agent:eng/1", "brain.query", {
        view: "stripe/finance_private",
        select: ["credits", "employee_salary"],
      });
      const sc = r.structuredContent;
      expect(sc.status).toBe("ok");
      expect(sc.redacted).toContain("employee_salary");
    });
  });

  // ---- CFO root sees salary ----
  it("CFO (resource root) sees employee_salary across all teams", async () => {
    const r = await callTool(base, "cfo", "brain.query", {
      view: "stripe/finance_private",
      select: ["gross", "credits", "employee_salary"],
    });
    const sc = r.structuredContent;
    expect(sc.status).toBe("ok");
    expect(sc.fields).toContain("employee_salary");
    expect(sc.rows).toHaveLength(4);
    expect(sc.rows.every((row: Record<string, unknown>) => "employee_salary" in row)).toBe(true);
  });

  // ---- Flow D: local-first / offline (no inference backend) ----
  describe("Flow D — local-first (structured query + cards need no LLM)", () => {
    it("brain.query works offline (default Stub inference, no network)", async () => {
      const offline = { ...base, CONTEXTFUL_INFERENCE: "" };
      const r = await callTool(offline, "agent:cto/1", "brain.query", {
        view: "stripe/spend_by_team",
        select: ["gross", "net"],
      });
      expect(r.structuredContent.status).toBe("ok");
      expect(r.structuredContent.rows).toHaveLength(4);
    });

    it("brain.get_context (Markdown card) works offline", async () => {
      const r = await callTool(base, "agent:cto/1", "brain.get_context", { topic: "spend" });
      expect(r.isError).toBe(false);
      expect(r.structuredContent.card).toMatch(/Spend summary/);
    });
  });

  // ---- brain.* tool surface coverage (read-only `base` scopes) ----
  describe("brain.* tool surface", () => {
    it("list_sources reflects the caller's authority", async () => {
      const cfo = await callTool(base, "cfo", "brain.list_sources", {});
      expect(cfo.structuredContent.views).toContain("stripe/finance_private");
      const eng = await callTool(base, "agent:eng/1", "brain.list_sources", {});
      expect(eng.structuredContent.views).toContain("stripe/spend_by_team");
      expect(eng.structuredContent.views).not.toContain("stripe/finance_private");
    });

    it("get_context card-scrub: eng cannot read the finance card, can read spend", async () => {
      const fin = await callTool(base, "agent:eng/1", "brain.get_context", { topic: "finance" });
      expect(fin.structuredContent.denied).toBeDefined();
      const spend = await callTool(base, "agent:eng/1", "brain.get_context", { topic: "spend" });
      expect(spend.structuredContent.card).toMatch(/Spend summary/);
    });

    it("search returns only cards the caller is cleared for", async () => {
      const eng = await callTool(base, "agent:eng/1", "brain.search", { query: "" });
      const topics = eng.structuredContent.results.map((m: any) => m.topic);
      expect(topics).toContain("spend");
      expect(topics).not.toContain("finance"); // finance card is finance_private-tagged
    });

    it("detect_anomalies is authorized for a granted view, denied otherwise", async () => {
      const ok = await callTool(base, "agent:cto/1", "brain.detect_anomalies", { view: "stripe/spend_by_team" });
      expect(ok.structuredContent.status).toBe("ok"); // single-period fixture → zero anomalies, but authorized
      const denied = await callTool(base, "agent:eng/1", "brain.detect_anomalies", {
        view: "stripe/finance_private",
      });
      expect(denied.structuredContent.status).toBe("denied");
    });

    it("remember writes a memory and returns an id", async () => {
      const r = await callTool(base, "cfo", "brain.remember", { fact: "Q3 review closed", doc: "finops" });
      expect(r.structuredContent.memory_id).toBeTruthy();
    });
  });

  // ---- protocol robustness ----
  describe("MCP robustness", () => {
    it("malformed brain.query (no view) → tool error, not a crash", async () => {
      const r = await callTool(base, "cfo", "brain.query", { select: ["gross"] });
      expect(r.isError).toBe(true);
    });

    it("unknown JSON-RPC method → -32601, server stays up", async () => {
      const res = await mcp(base, "agent:cto/1", [
        { jsonrpc: "2.0", id: 1, method: "no/such/method" },
        toolCall(2, "brain.list_sources", {}),
      ]);
      expect(res[1].error.code).toBe(-32601);
      expect(res[2].result.isError).toBe(false); // still serving after the bad call
    });

    it("a notification (no id) is ignored without killing the loop", async () => {
      const res = await mcp(base, "agent:cto/1", [
        { jsonrpc: "2.0", method: "notifications/initialized" } as any,
        queryTool(7, "stripe/spend_by_team", ["gross"]),
      ]);
      expect(res[7].result.structuredContent.status).toBe("ok");
    });
  });
});
