// Acceptance — the editor agent (spec 04 over spec 01): a web-shaped Weaver
// peer types a `Q:` paragraph into the shared doc's block tree; the watching
// agent picks it up, answers from the brain's Markdown memory
// (capability-filtered), and the answer lands back in the document as a new
// paragraph via a CRDT update. Mirrors the demo.contextful.work "ask the
// editor agent" demo button end to end.

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { spawn, type ChildProcess } from "node:child_process";
import { LoroDoc, LoroMap, LoroText } from "loro-crdt";
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

/** Append a Weaver paragraph block (same shape as @weaver/core / the web seed). */
function appendParagraph(doc: LoroDoc, content: string): void {
  const node = doc.getTree("content").createNode();
  node.data.set("kind", "paragraph");
  node.data.setContainer("attrs", new LoroMap());
  const text = node.data.setContainer("text", new LoroText());
  if (content.length > 0) text.insert(0, content);
}

/** Ordered top-level block texts of a Weaver doc. */
function blockTexts(doc: LoroDoc): string[] {
  return doc
    .getTree("content")
    .roots()
    .map((n) => {
      const t = n.data.get("text");
      return t instanceof LoroText ? t.toString() : "";
    });
}

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

/** Keep importing relayed UPDATEs into `doc` until its blocks match (or timeout). */
async function waitForBlocks(
  peer: Peer,
  doc: LoroDoc,
  docId: string,
  match: (blocks: string[]) => boolean,
): Promise<string[]> {
  const deadline = Date.now() + 10_000;
  for (;;) {
    const blocks = blockTexts(doc);
    if (match(blocks)) return blocks;
    if (Date.now() > deadline) throw new Error(`blocks never matched; last:\n${blocks.join("\n")}`);
    await peer.next((m) => m.type === "UPDATE" && m.doc_id === docId, deadline - Date.now());
    for (const m of peer.received) {
      if (m.type === "UPDATE" && m.doc_id === docId && m.bytes.length) {
        doc.import(Uint8Array.from(m.bytes));
      }
    }
  }
}

d("editor agent answers Q: blocks from brain memory", () => {
  let env: Env;
  let server: ChildProcess;
  const watchers: ChildProcess[] = [];

  beforeAll(async () => {
    env = freshHome();
    seedAndIngest(env); // principals + signed tokens + stripe ingest → product cards
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
    appendParagraph(doc, "Q3 FinOps review notes.");
    appendParagraph(doc, `Q: ${QUESTION}`);
    doc.commit();
    web.send(update("finops", Array.from(doc.export({ mode: "update" }))));

    const blocks = await waitForBlocks(web, doc, "finops", (b) =>
      b.some((t) => t.startsWith("A (cfo")),
    );
    // the answer paragraph sits directly below the question, from the card
    const qi = blocks.findIndex((t) => t === `Q: ${QUESTION}`);
    expect(qi).toBeGreaterThanOrEqual(0);
    const answer = blocks[qi + 1];
    expect(answer).toContain("A (cfo · from brain memory): Unit economics · compression · 2026-05");
    expect(answer).toContain("$50/unit");
    expect(answer).toContain("66%");
    expect(answer).toContain("[source: brain/products/unit-economics-compression-2026-05.md · acl stripe/finance_private]");
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
    appendParagraph(doc, `Q: ${QUESTION}`);
    doc.commit();
    web.send(update("finops-denied", Array.from(doc.export({ mode: "update" }))));

    const blocks = await waitForBlocks(web, doc, "finops-denied", (b) =>
      b.some((t) => t.startsWith("A (agent:cto/1")),
    );
    const answer = blocks.find((t) => t.startsWith("A (agent:cto/1"))!;
    expect(answer).toContain("Denied — the matching memory requires gross, credits on stripe/finance_private");
    expect(answer).not.toContain("$50/unit");
    web.close();
  });

  // Dinesh's agent types a mention ask addressed to Monica's agent.
  // The CFO-side watcher answers under the ASKER's token — salary is
  // finance_private and never granted to agent:eng/1 — so the asking peer
  // receives an active ⛔ Denied · no_grant NOTIFY frame, and the doc gets a
  // denial paragraph that never leaks the salary numbers.
  it("a salary mention-ask from dinesh's agent → ⛔ Denied · no_grant NOTIFY at the asking peer", async () => {
    const web = new Peer(PORT);
    await web.open();
    web.send(hello("agent:eng/1"));
    web.send(subscribe("finops-ask"));
    await web.next((m) => m.type === "SNAPSHOT");

    watchers.push(await startWatcher(env, "cfo", "finops-ask", web));

    const ask = "@Monica (CFO)'s agent What's CEO's Salary";
    const doc = new LoroDoc();
    appendParagraph(doc, ask);
    doc.commit();
    web.send(update("finops-ask", Array.from(doc.export({ mode: "update" }))));

    // the active notification, addressed to the asker
    const note: any = await web.next(
      (m) => m.type === "NOTIFY" && m.to === "agent:eng/1",
      15_000,
    );
    expect(note.from).toBe("cfo");
    expect(note.reason).toBe("no_grant");
    expect(note.message).toContain("employee_salary");
    expect(note.message).toContain("stripe/finance_private");
    // decision metadata only — never the card's salary figures
    expect(note.message).not.toMatch(/\$\d/);

    // and the doc carries the denial paragraph below the ask
    const blocks = await waitForBlocks(web, doc, "finops-ask", (b) =>
      b.some((t) => t.includes("⛔ Denied · no_grant")),
    );
    const qi = blocks.findIndex((t) => t === ask);
    expect(qi).toBeGreaterThanOrEqual(0);
    const reply = blocks[qi + 1];
    expect(reply).toContain("A (cfo · for agent:eng/1): ⛔ Denied · no_grant");
    expect(reply).toContain("employee_salary");
    expect(reply).not.toMatch(/\$\d/);
    web.close();
  });
});
