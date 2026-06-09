// Shared acceptance harness (spec 09): spawn the built `sync` binary and drive
// it three ways — the control-plane CLI (`ctl`/`ingest`), the brain MCP server
// over JSON-RPC stdio, and the Loro relay over WebSocket. All state is isolated
// per test via a fresh CONTEXTFUL_HOME.

import { spawn, execFileSync, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import WebSocket from "ws";

const here = dirname(fileURLToPath(import.meta.url));
export const repoRoot = resolve(here, "../..");
export const BIN = process.env.SYNC_BIN ?? join(repoRoot, "target", "debug", "sync");
export const haveBin = existsSync(BIN);

// CI sets REQUIRE_SYNC_BIN=1 so a missing binary FAILS loudly instead of
// silently skipping every suite to green (false-confidence guard).
if (process.env.REQUIRE_SYNC_BIN === "1" && !haveBin) {
  throw new Error(`REQUIRE_SYNC_BIN=1 but binary not found at ${BIN} — build it with \`cargo build -p sync\``);
}

export type Env = NodeJS.ProcessEnv;

/** A throwaway, isolated CONTEXTFUL_HOME for one test file. */
export function freshHome(): Env {
  const home = mkdtempSync(join(tmpdir(), "contextful-accept-"));
  return { ...process.env, CONTEXTFUL_HOME: home, RUST_LOG: "error" };
}

/** Run a `sync` subcommand to completion, returning combined stdout. */
export function run(env: Env, args: string[]): string {
  return execFileSync(BIN, args, { env }).toString();
}

/** Seed the control plane and ingest the Stripe fixture. */
export function seedAndIngest(env: Env): void {
  run(env, ["ctl", "seed"]);
  run(env, ["ingest", "--source", "stripe"]);
}

// ---- Brain MCP (JSON-RPC over stdio) -------------------------------------

export type JsonRpc = { jsonrpc: "2.0"; id: number; method: string; params?: unknown };

/** Drive the MCP stdio server as `principal`; collect responses keyed by id. */
export function mcp(env: Env, principal: string, requests: JsonRpc[]): Promise<Record<number, any>> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(BIN, ["mcp", "--principal", principal], { env });
    const byId: Record<number, any> = {};
    let buf = "";
    // notifications (no id) get no response — only count requests that expect one
    const expected = requests.filter((r) => (r as any).id != null).length;
    const done = () => {
      child.kill();
      resolvePromise(byId);
    };
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
      if (Object.keys(byId).length >= expected) done();
    });
    child.on("error", reject);
    child.on("close", () => resolvePromise(byId));
    for (const r of requests) child.stdin.write(JSON.stringify(r) + "\n");
  });
}

export const toolCall = (id: number, name: string, args: Record<string, unknown>): JsonRpc => ({
  jsonrpc: "2.0",
  id,
  method: "tools/call",
  params: { name, arguments: args },
});

/** Single brain tool call → its `result` (with `.structuredContent`). */
export async function callTool(env: Env, principal: string, name: string, args: Record<string, unknown>): Promise<any> {
  const r = await mcp(env, principal, [toolCall(1, name, args)]);
  return r[1]?.result;
}

export const queryTool = (id: number, view: string, select: string[]): JsonRpc =>
  toolCall(id, "brain.query", { view, select });

// ---- Loro relay (WebSocket) ----------------------------------------------

/** Spawn `sync serve` and resolve once the port is accepting connections. */
export async function startServer(env: Env, port: number): Promise<ChildProcess> {
  const child = spawn(BIN, ["serve", "--addr", `127.0.0.1:${port}`], { env });
  await waitForPort(port, 8000);
  return child;
}

export function waitForPort(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  return new Promise((res, rej) => {
    const tryOnce = () => {
      const sock = createConnection({ host: "127.0.0.1", port }, () => {
        sock.end();
        res();
      });
      sock.on("error", () => {
        sock.destroy();
        if (Date.now() > deadline) rej(new Error(`port ${port} not up within ${timeoutMs}ms`));
        else setTimeout(tryOnce, 100);
      });
    };
    tryOnce();
  });
}

/** A relay peer: speaks the Contextful WS protocol, buffering received frames. */
export class Peer {
  private ws: WebSocket;
  readonly received: any[] = [];
  private waiters: { match: (m: any) => boolean; resolve: (m: any) => void; timer: NodeJS.Timeout }[] = [];

  constructor(port: number) {
    this.ws = new WebSocket(`ws://127.0.0.1:${port}/`);
    this.ws.on("message", (data) => {
      let msg: any;
      try {
        msg = JSON.parse(data.toString());
      } catch {
        return;
      }
      this.received.push(msg);
      this.waiters = this.waiters.filter((w) => {
        if (w.match(msg)) {
          clearTimeout(w.timer);
          w.resolve(msg);
          return false;
        }
        return true;
      });
    });
  }

  open(): Promise<void> {
    if (this.ws.readyState === WebSocket.OPEN) return Promise.resolve();
    return new Promise((res, rej) => {
      this.ws.on("open", () => res());
      this.ws.on("error", rej);
    });
  }

  send(msg: unknown): void {
    this.ws.send(JSON.stringify(msg));
  }

  /** Wait for the next received message matching `match` (or already buffered). */
  next(match: (m: any) => boolean, timeoutMs = 4000): Promise<any> {
    const buffered = this.received.find(match);
    if (buffered) return Promise.resolve(buffered);
    return new Promise((res, rej) => {
      const timer = setTimeout(() => rej(new Error("timed out waiting for message")), timeoutMs);
      this.waiters.push({ match, resolve: res, timer });
    });
  }

  close(): void {
    this.ws.close();
  }
}

export const hello = (principal: string) => ({ type: "HELLO", proto: "contextful/1", principal });
export const subscribe = (doc_id: string) => ({ type: "SUBSCRIBE", doc_id });
export const awareness = (doc_id: string, principal: string, mode = "writing") => ({
  type: "AWARENESS",
  doc_id,
  presence: { principal, display_name: principal, mode, heartbeat: 1 },
});
export const update = (doc_id: string, bytes: number[]) => ({ type: "UPDATE", doc_id, bytes });
