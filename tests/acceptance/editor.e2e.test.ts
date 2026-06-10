// Acceptance — the editor agent (spec 04 over spec 01): a web-shaped Loro peer
// types a `Q:` line into the shared doc; the watching agent picks it up,
// answers from the brain's Markdown memory (capability-filtered), and the
// answer lands back in the document as a CRDT update. Mirrors the
// demo.contextful.work "ask the editor agent" demo button end to end.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { spawn, type ChildProcess } from "node:child_process";
import { LoroDoc } from "loro-crdt";
import {
  BIN,
  freshHome,
  haveBin,
  hello,
  Peer,
  seedAndIngest,
  startServer,
  subscribe,
  update,
  type Env,
} from "./helpers";

// Clear of the relay suite's 7901+ range; still per-worker to avoid collisions.
const PORT = 7951 + Number(process.env.VITEST_WORKER_ID ?? 0);
const d = haveBin ? describe : describe.skip;

const QUESTION = "Unit economics of compression product";

/** Spawn `sync agent --watch-doc` and wait for its first presence heartbeat. */
async function startWatcher(env: Env, principal: string, doc: string, via: Peer): Promise<ChildProcess> {
  const watcher = spawn(
    BIN,
    ["agent", "--principal", principal, "--watch-doc", doc, "--addr", `127.0.0.1:${PORT}`],
    { env },
  );
  await via.next((m) => m.type === "AWARENESS" && m.presence.principal === principal, 8000);
  return watcher;
}

/** Keep importing relayed UPDATEs into `doc` until its body matches (or timeout). */
async function waitForBody(peer: Peer, doc: LoroDoc, docId: string, match: (text: string) => boolean): Promise<string> {
  const deadline = Date.now() + 10_000;
  let seen = 0;
  for (;;) {
    const text = doc.getText("body").toString();
    if (match(text)) return text;
    if (Date.now() > deadline) throw new Error(`body never matched; last:\n${text}`);
    await peer.next((m) => m.type === "UPDATE" && m.doc_id === docId, deadline - Date.now());
    for (const m of peer.received) {
      if (m.type === "UPDATE" && m.doc_id === docId && m.bytes.length) {
        doc.import(Uint8Array.from(m.bytes));
      }
    }
    seen = peer.received.length;
    void seen;
  }
}

d("editor agent answers Q: lines from brain memory", () => {
  let env: Env;
  let server: ChildProcess;
  const watchers: ChildProcess[] = [];

  beforeAll(async () => {
    env = freshHome();
    seedAndIngest(env); // principals + tokens + stripe ingest → product cards
    server = await startServer(env, PORT);
  });
  afterAll(() => {
    for (const w of watchers) w.kill();
    server?.kill();
  });

  it("CFO's editor agent answers the unit-economics question in the doc", async () => {
    const web = new Peer(PORT);
    await web.open();
    web.send(hello("cfo"));
    web.send(subscribe("finops"));
    await web.next((m) => m.type === "SNAPSHOT");

    watchers.push(await startWatcher(env, "cfo", "finops", web));

    // the browser editor: seed prose + the typed question, shipped as one update
    const doc = new LoroDoc();
    const body = doc.getText("body");
    body.update(`Q3 FinOps review notes.\n\nQ: ${QUESTION}\n`);
    doc.commit();
    web.send(update("finops", Array.from(doc.export({ mode: "update" }))));

    const text = await waitForBody(web, doc, "finops", (t) => t.includes("A (cfo"));
    // answer is inserted directly below the question, from the synthesized card
    expect(text).toContain(`Q: ${QUESTION}\nA (cfo · from brain memory): Unit economics · compression · 2026-05`);
    expect(text).toContain("$50/unit");
    expect(text).toContain("66%");
    expect(text).toContain("[source: brain/products/unit-economics-compression-2026-05.md · acl stripe/finance_private]");
    web.close();
  });

  it("an agent without finance_private gets a denial, not the card", async () => {
    const web = new Peer(PORT);
    await web.open();
    web.send(hello("agent:cto/1"));
    web.send(subscribe("finops-denied"));
    await web.next((m) => m.type === "SNAPSHOT");

    watchers.push(await startWatcher(env, "agent:cto/1", "finops-denied", web));

    const doc = new LoroDoc();
    const body = doc.getText("body");
    body.update(`Q: ${QUESTION}\n`);
    doc.commit();
    web.send(update("finops-denied", Array.from(doc.export({ mode: "update" }))));

    const text = await waitForBody(web, doc, "finops-denied", (t) => t.includes("A (agent:cto/1"));
    expect(text).toContain("Denied — the matching memory requires gross, credits on stripe/finance_private");
    expect(text).not.toContain("$50/unit");
    web.close();
  });
});
