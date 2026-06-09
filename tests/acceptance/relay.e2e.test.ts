// Acceptance — the Loro sync relay (spec 01) end-to-end: two WS peers handshake,
// exchange presence + updates through the authoritative relay, a revoked peer is
// rejected, and a path-traversal doc_id is refused.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { ChildProcess } from "node:child_process";
import {
  awareness,
  freshHome,
  haveBin,
  hello,
  Peer,
  run,
  startServer,
  subscribe,
  update,
  type Env,
} from "./helpers";

const PORT = 7901;
const d = haveBin ? describe : describe.skip;

d("Loro relay over WebSocket", () => {
  let env: Env;
  let server: ChildProcess;

  beforeAll(async () => {
    env = freshHome();
    run(env, ["ctl", "seed"]);
    server = await startServer(env, PORT);
  });
  afterAll(() => {
    server?.kill();
  });

  it("handshake returns HELLO_OK then SNAPSHOT", async () => {
    const p = new Peer(PORT);
    await p.open();
    p.send(hello("agent:cto/1"));
    p.send(subscribe("finops"));
    const ok = await p.next((m) => m.type === "HELLO_OK");
    expect(ok.doc_id).toBe("finops");
    const snap = await p.next((m) => m.type === "SNAPSHOT");
    expect(Array.isArray(snap.bytes)).toBe(true);
    p.close();
  });

  it("relays presence and updates between two peers in a room", async () => {
    const a = new Peer(PORT);
    await a.open();
    a.send(hello("agent:cto/1"));
    a.send(subscribe("finops"));
    await a.next((m) => m.type === "SNAPSHOT");

    const b = new Peer(PORT);
    await b.open();
    b.send(hello("agent:eng/1"));
    b.send(subscribe("finops"));
    await b.next((m) => m.type === "SNAPSHOT");

    // B publishes presence + an update; A should receive both (cross-peer relay)
    b.send(awareness("finops", "agent:eng/1", "writing"));
    b.send(update("finops", [1, 2, 3, 4]));

    const pres = await a.next((m) => m.type === "AWARENESS" && m.presence.principal === "agent:eng/1");
    expect(pres.presence.mode).toBe("writing");
    const upd = await a.next((m) => m.type === "UPDATE");
    expect(upd.bytes).toEqual([1, 2, 3, 4]);

    a.close();
    b.close();
  });

  it("rejects a revoked principal at HELLO", async () => {
    run(env, ["ctl", "revoke", "--principal", "agent:eng/1"]);
    const p = new Peer(PORT);
    await p.open();
    p.send(hello("agent:eng/1"));
    p.send(subscribe("finops"));
    const err = await p.next((m) => m.type === "ERROR");
    expect(err.code).toBe("revoked");
    p.close();
  });

  it("refuses a path-traversal doc_id", async () => {
    const p = new Peer(PORT);
    await p.open();
    p.send(hello("agent:cto/1"));
    p.send(subscribe("../../etc/passwd"));
    const err = await p.next((m) => m.type === "ERROR");
    expect(err.code).toBe("bad_doc_id");
    p.close();
  });

  it("persists a client-pushed snapshot for catch-up", async () => {
    const a = new Peer(PORT);
    await a.open();
    a.send(hello("agent:cto/1"));
    a.send(subscribe("room2"));
    await a.next((m) => m.type === "SNAPSHOT");
    a.send({ type: "SNAPSHOT", doc_id: "room2", bytes: [9, 9, 9] });
    a.close();

    // a fresh peer subscribing to room2 should receive the persisted bytes
    await new Promise((r) => setTimeout(r, 200));
    const b = new Peer(PORT);
    await b.open();
    b.send(hello("agent:cto/1"));
    b.send(subscribe("room2"));
    const snap = await b.next((m) => m.type === "SNAPSHOT");
    expect(snap.bytes).toEqual([9, 9, 9]);
    b.close();
  });
});
