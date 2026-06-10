import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { recoverStaleChunk, useWeaverRoom, type RoomNotice } from "@/lib/weaverRoom";
import { useDemoAgent } from "@/lib/demoAgent";
import { DOCS } from "@/lib/docs";
import { avatarOf, initialsOf, isScenarioPrincipal, peerColor, peerKey } from "@/lib/presence";
import { AvatarDot } from "@/components/AvatarDot";

// The Weaver editing surface pulls in loro-crdt (WASM) via @weaver/react —
// lazy-loaded so it ships as a separate client chunk and never renders during
// SSR (useWeaverRoom only yields an editor after client hydration).
const WeaverSurface = lazy(() =>
  import("@/components/WeaverSurface").catch(recoverStaleChunk),
);
import {
  CFO,
  CFO_AGENT,
  ENG,
  ENG_AGENT,
  PRINCIPALS,
  REGISTRY,
  tag,
} from "@superai2026/protocol/scenario";
// Type-only — erased at build; the WASM-backed runtime import stays inside
// the lazy WeaverSurface chunk.
import type { Principal as WeaverPrincipal } from "@weaver/core";

const dotColor = (id: string): string =>
  id === CFO.id ? "var(--cf-sky-500)" : id.startsWith("agent:cto") ? "var(--cf-indigo-500)" : "var(--cf-amber-500)";

type LogEntry = { id: number; kind: "ok" | "deny" | "grant" | "block" | "info"; text: string };

let logSeq = 0;

// The scripted editor-agent demos: each button impersonates its asker
// (switching the acting principal) and types a mention ask into the live doc,
// tagging Monica (CFO)'s agent (spec 04). The local `sync agent
// --watch-doc <doc>` peer answers with the ASKER's token from brain memory
// (~/.contextful) over the relay — the relay's authenticated `UPDATE.from`
// stamp attributes the typed block to the asker, so Monica gets the numbers
// while Dinesh's salary ask hits the never-delegated `employee_salary` card
// and comes back `⛔ Denied · no_grant` (plus a NOTIFY toast). The watcher
// dedups asks by their raw block text, so the texts must differ.
const DEMOS: { label: string; actorId: string; question: string }[] = [
  {
    label: "Demo Q by CFO",
    actorId: CFO.id,
    question: "pull up the aggregated margin for our compression product",
  },
  {
    label: "Demo Q by CTO",
    actorId: ENG.id,
    question: "what is the CEO's salary?",
  },
];
// Replies start `A (cfo · for <asker>` — `· from brain memory): …` when
// answered, `): ⛔ Denied · …` when policy blocks the asker. Either ends the loop.
const demoAnswerMark = (askerId: string) => `A (cfo · for ${askerId}`;

const demoSleep = (ms: number, signal: AbortSignal) =>
  new Promise<void>((res) => {
    if (signal.aborted) return res();
    const t = setTimeout(res, ms);
    signal.addEventListener("abort", () => {
      clearTimeout(t);
      res();
    }, { once: true });
  });

// Selectable actors ("Acting as"): Dinesh (the human), his agent, and Monica.
// Richard (CEO)'s agent stays in the room roster but is not a switchable
// identity; Dinesh comes from the wider REGISTRY, not the console cast.
const ACTORS = [ENG, ENG_AGENT, CFO];

/** The capability console for one document room — rendered at `/` (default doc) and `/docs/:docId`. */
export default function ConsolePage({ docId }: { docId: string }) {
  const [actorId, setActorId] = useState<string>(ENG_AGENT.id);
  const [log, setLog] = useState<LogEntry[]>([]);

  const actor = REGISTRY.find((p) => p.id === actorId)!;
  const activeDoc = DOCS.find((d) => d.id === docId)!;
  // Active access-decision notifications (NOTIFY frames addressed to the
  // acting principal — e.g. a mention-ask denied with no_grant): toast + audit log.
  const [notice, setNotice] = useState<RoomNotice | null>(null);
  const onNotice = useCallback((n: RoomNotice) => {
    setNotice(n);
    setLog((l) => [
      { id: ++logSeq, kind: "deny" as const, text: `⛔ Denied · ${n.reason} — ${n.message}` },
      ...l,
    ].slice(0, 12));
  }, []);
  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 8000);
    return () => clearTimeout(t);
  }, [notice]);
  const room = useWeaverRoom(actor.id, actor.name, docId, onNotice);
  // Simulated agent peer — keeps the room visibly live for solo visitors.
  useDemoAgent(docId);
  const { status: syncStatus, peers: livePeers } = room;
  // One presence roster (upstream weaver PR #35): scenario agents +
  // collaborators are always in the room; live sessions light their chip up,
  // and sessions outside the scenario cast get their own chip per session.
  const liveByPrincipal = useMemo(
    () => new Map(livePeers.map((p) => [p.principal, p])),
    [livePeers],
  );
  const guestPeers = useMemo(
    () => livePeers.filter((p) => !isScenarioPrincipal(p.principal)),
    [livePeers],
  );
  // The @-mention directory: the full org cast (humans + agents), mapped to
  // Weaver's Principal shape with the same colors the roster/carets use.
  const mentionPrincipals = useMemo<WeaverPrincipal[]>(
    () =>
      REGISTRY.map((p) => ({
        id: p.id,
        kind: p.kind === "human" ? "user" : "agent",
        label: p.name,
        color: peerColor(p.id),
        avatarUrl: avatarOf(p.id),
      })),
    [],
  );
  const selfIdentity = useMemo(
    () => ({ id: actor.id, name: actor.name }),
    [actor.id, actor.name],
  );

  const pushLog = (kind: LogEntry["kind"], text: string) =>
    setLog((l) => [{ id: ++logSeq, kind, text }, ...l].slice(0, 12));

  const switchActor = (id: string) => {
    setActorId(id);
  };

  // ---- editor-agent demo (right panel) ----
  // The ask typed for the acting principal; `actorId` is captured so the
  // effect targets the right editor instance if the actor switches mid-demo.
  const [demo, setDemo] = useState<{ actorId: string; ask: string; question: string } | null>(null);
  // The asker's caret while the demo types — programmatic inserts never move
  // the native DOM caret, so without this the typing has no visible author.
  const [demoCursor, setDemoCursor] = useState<{ blockId: string; offset: number } | null>(null);
  const demoBusyRef = useRef(false);

  const runEditorDemo = (d: (typeof DEMOS)[number]) => {
    if (demo) return;
    const asker = REGISTRY.find((p) => p.id === d.actorId)!;
    switchActor(d.actorId);
    // `insertMention` writes `@<label> ` then the question is typed after it.
    setDemo({ actorId: d.actorId, ask: `@${CFO_AGENT.name} ${d.question}`, question: d.question });
    pushLog("info", `Demo · ${asker.name} tags ${CFO_AGENT.name} in the doc`);
  };

  // Type the question into the asker's Weaver editor once its room is ready,
  // then watch the block tree for the agent's `A (…)` reply — answer or
  // denial, per the asker's token. The editor remounts when the actor
  // switches, so this waits for that instance.
  const ed = room.editor;
  useEffect(() => {
    if (!demo || actorId !== demo.actorId || !ed || demoBusyRef.current) return;
    demoBusyRef.current = true;
    const ctrl = new AbortController();
    const asker = REGISTRY.find((p) => p.id === demo.actorId)!;

    const run = async () => {
      const { rootId, getChildren, getBlock } = await import("@weaver/core");
      const paragraphs = () =>
        getChildren(ed, rootId(ed)).filter((id) => getBlock(ed, id)?.hasInline);
      // let the relay snapshot / local hydration land first
      await demoSleep(800, ctrl.signal);
      if (ctrl.signal.aborted) return;

      if (!paragraphs().some((id) => ed.commands.text.read(id).startsWith(demo.ask))) {
        const blockId = ed.commands.block.insert({
          parentId: rootId(ed),
          index: getChildren(ed, rootId(ed)).length,
          kind: "paragraph",
        });
        // A real mention chip — same `mention` mark the @-picker writes, so
        // the agent is genuinely tagged (and the watcher reads the same text).
        const marked = ed.commands.text.insertMention({
          blockId,
          range: { start: 0, end: 0 },
          principal: { id: CFO_AGENT.id, label: CFO_AGENT.name, kind: "agent" },
        });
        room.setCursor({ blockId, offset: marked.end + 1 });
        setDemoCursor({ blockId, offset: marked.end + 1 });
        await demoSleep(300, ctrl.signal);
        for (const ch of demo.question) {
          if (ctrl.signal.aborted) return;
          const offset = ed.commands.text.length(blockId);
          ed.commands.text.insert({ blockId, offset, value: ch });
          const cursor = { blockId, offset: offset + ch.length };
          room.setCursor(cursor);
          setDemoCursor(cursor);
          await demoSleep(35, ctrl.signal);
        }
      }
      // Interim status, not an outcome — "info" so a later denial isn't
      // preceded by a misleading green "ok" row.
      pushLog("info", `${asker.name} asked ${CFO_AGENT.name} “${demo.question}” — it is reading`);

      const mark = demoAnswerMark(demo.actorId);
      const deadline = Date.now() + 30_000;
      while (!ctrl.signal.aborted && Date.now() < deadline) {
        const reply = paragraphs()
          .map((id) => ed.commands.text.read(id))
          .find((t) => t.startsWith(mark));
        if (reply) {
          if (reply.includes("⛔")) pushLog("deny", `${asker.name}'s ask was denied by policy`);
          else pushLog("ok", "answered from brain memory (~/.contextful)");
          break;
        }
        await demoSleep(500, ctrl.signal);
      }
      if (!ctrl.signal.aborted) setDemo(null);
      setDemoCursor(null);
      demoBusyRef.current = false;
    };
    void run();

    return () => {
      ctrl.abort();
      setDemoCursor(null);
      demoBusyRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- pushLog/setCursor are stable enough for the scripted demo
  }, [demo, actorId, ed]);

  // Reset demo: clear console state AND restore the document to its seed via
  // CRDT ops (new blocks in, old blocks out), so the reset propagates to every
  // tab and the relay instead of only touching this tab's UI.
  const resetAll = async () => {
    setLog([]);
    setNotice(null);
    setDemo(null);
    setDemoCursor(null);
    const ed = room.editor;
    if (!ed) return;
    const [{ rootId, getChildren }, { parseSeedParagraph }] = await Promise.all([
      import("@weaver/core"),
      import("@/lib/weaverTransport"),
    ]);
    const root = rootId(ed);
    const old = getChildren(ed, root);
    const paragraphs = activeDoc.seed.split("\n\n").filter((p) => p.length > 0);
    paragraphs.forEach((para, i) => {
      const { text, mentions } = parseSeedParagraph(para);
      const blockId = ed.commands.block.insert({
        parentId: root,
        index: old.length + i,
        kind: "paragraph",
      });
      if (text.length > 0) ed.commands.text.insert({ blockId, offset: 0, value: text });
      for (const m of mentions) {
        ed.commands.text.mark.update({
          blockId,
          range: { start: m.start, end: m.end },
          mark: "mention",
          value: m.value,
        });
      }
    });
    for (const id of old) ed.commands.block.delete({ blockId: id });
  };

  return (
    <div className="app-shell">
      <header className="app-topbar">
        <div className="app-doc-meta">
          <span className="app-doc-meta__title">{activeDoc.title}</span>
          <span className="cf-badge cf-badge--accent">on-prem · tailnet</span>
          <span className="cf-badge cf-badge--danger">salary · redacted</span>
        </div>

        <div className="app-topbar__right">
          <span
            className={`cf-badge ${syncStatus === "live" || livePeers.length > 0 ? "cf-badge--success" : ""}`}
            title="Live CRDT sync — cross-tab by default; set VITE_SYNC_URL for the relay"
          >
            {syncStatus === "live"
              ? `● sync live · ${livePeers.length} peer${livePeers.length === 1 ? "" : "s"}`
              : syncStatus === "connecting"
                ? "◌ connecting…"
                : syncStatus === "offline"
                  ? "○ relay offline"
                  : livePeers.length > 0
                    ? `● cross-tab · ${livePeers.length} peer${livePeers.length === 1 ? "" : "s"}`
                    : "◐ local"}
          </span>
          <button className="cf-btn cf-btn--ghost cf-btn--sm" onClick={resetAll}>
            Reset demo
          </button>
          {/* Google-Docs layout: other collaborators stack on the left, the
              acting user's own ringed avatar sits in the top-right corner. */}
          <span className="cf-presence" aria-label="collaborators present">
            {PRINCIPALS.filter((p) => p.id !== actorId).map((p) => {
              const live = liveByPrincipal.get(p.id);
              return (
                <AvatarDot
                  key={p.id}
                  id={p.id}
                  fallback={tag(p)}
                  color={dotColor(p.id)}
                  live={live !== undefined}
                  title={`${p.name} · in room${live ? ` · ${live.mode}` : ""}`}
                  stacked
                />
              );
            })}
            {guestPeers.map((p) => (
              <AvatarDot
                key={peerKey(p)}
                id={p.principal}
                fallback={initialsOf(p.display_name) || "◆"}
                color={peerColor(p.principal)}
                live
                title={`${p.display_name} · ${p.mode}`}
                stacked
              />
            ))}
            <AvatarDot
              id={actor.id}
              fallback={tag(actor)}
              color={dotColor(actor.id)}
              title={`${actor.name} · you`}
              self
            />
          </span>
        </div>
      </header>

      <main className="app-main">
        <section className="app-editor" aria-label="Document">
          <div className="app-editor__inner">
            <span className="cf-eyebrow">Pied Piper · Finance · Engineering</span>
            <h1>{activeDoc.title}</h1>
            <div className="app-editor__metarow">
              <span className="cf-badge">Shared with 5 + 3 agents</span>
              <span className="cf-badge cf-badge--success">
                {syncStatus === "live"
                  ? "Live"
                  : livePeers.length > 0
                    ? "Live · cross-tab"
                    : "Local"}
              </span>
              <span className="app-editor__actor" title={`acting: ${actor.name}`}>
                <AvatarDot
                  id={actor.id}
                  fallback={tag(actor)}
                  color={dotColor(actor.id)}
                  title={`acting: ${actor.name}`}
                />
              </span>
            </div>

            <div className="app-editor__body">
              {room.editor ? (
                <Suspense fallback={<p className="weaver-loading">Waking the editor…</p>}>
                  <WeaverSurface
                    editor={room.editor}
                    peers={livePeers}
                    onCursorChange={room.setCursor}
                    self={selfIdentity}
                    selfCursor={demoCursor}
                    mentionPrincipals={mentionPrincipals}
                  />
                </Suspense>
              ) : (
                <p className="weaver-loading">Waking the editor…</p>
              )}
            </div>

          </div>
        </section>

        <aside className="app-agentpanel" aria-label="Query console">
          <div>
            <p className="app-agentpanel__label">Editor agent — live demo</p>
            <div className="cf-card cf-stack">
              <p className="cf-text-muted" style={{ fontSize: "var(--text-sm)", margin: 0 }}>
                Each button impersonates its asker and tags {CFO_AGENT.name} in the doc.
                The local <code>sync agent --watch-doc {docId}</code> peer answers — or
                denies — from brain memory, per the asker&rsquo;s capability token, over
                the relay.
              </p>
              {DEMOS.map((d) => {
                const asker = REGISTRY.find((p) => p.id === d.actorId)!;
                return (
                  <button
                    key={d.actorId}
                    className="cf-btn cf-btn--primary cf-btn--sm cf-block"
                    onClick={() => runEditorDemo(d)}
                    disabled={demo != null}
                  >
                    {demo?.actorId === d.actorId ? `Typing as ${asker.name}…` : `▶ ${d.label}`}
                  </button>
                );
              })}
            </div>
          </div>

          <div>
            <p className="app-agentpanel__label">Acting as</p>
            <div className="cf-actor-switch">
              {ACTORS.map((p) => (
                <button
                  key={p.id}
                  className={`cf-actor${p.id === actorId ? " cf-actor--on" : ""}`}
                  onClick={() => switchActor(p.id)}
                >
                  <AvatarDot id={p.id} fallback={tag(p)} color={dotColor(p.id)} />
                  <span>
                    <span className="cf-actor__name">{p.name}</span>
                    <span className="cf-actor__sub">
                      {p.kind === "agent" ? `owner: ${p.owner}` : p.role}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          </div>

          <div>
            <p className="app-agentpanel__label">Audit trail</p>
            <div className="cf-card cf-log">
              {log.length === 0 ? (
                <p className="cf-text-muted" style={{ fontSize: "var(--text-sm)" }}>
                  Run the demo to populate the audit trail.
                </p>
              ) : (
                <ul>
                  {log.map((e) => (
                    <li key={e.id} className={`cf-log__row cf-log__row--${e.kind}`}>
                      <span className="cf-log__tag">{e.kind}</span>
                      {e.text}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </aside>
      </main>

      {notice && (
        <div className="cf-toast cf-toast--deny" role="alert">
          <p className="cf-toast__head">⛔ Denied · {notice.reason}</p>
          <p className="cf-toast__body">{notice.message}</p>
          <p className="cf-toast__from">— {notice.from || "relay"}</p>
        </div>
      )}
    </div>
  );
}

