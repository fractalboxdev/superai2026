import { useEffect, useRef, useState } from "react";
import { fetchSandboxDebug, type SandboxDebugStatus } from "@/lib/debug";

// Per-document debug menu (⋯): which sandbox backs this room, with a link to
// the provider's execution logs (Vercel dashboard → Observability →
// Sandboxes). Status is fetched lazily on open from the local sync binary's
// debug endpoint, so the menu degrades gracefully when the relay is offline
// or the room runs the modeled/offline lifecycle (Flow D — no cloud logs).

type FetchState =
  | { phase: "idle" }
  | { phase: "loading" }
  | { phase: "unreachable" }
  | { phase: "loaded"; status: SandboxDebugStatus };

export default function DocDebugMenu({ docId, docTitle }: { docId: string; docTitle: string }) {
  const [open, setOpen] = useState(false);
  const [state, setState] = useState<FetchState>({ phase: "idle" });
  const rootRef = useRef<HTMLDivElement>(null);

  const load = async () => {
    setState({ phase: "loading" });
    const status = await fetchSandboxDebug(docId);
    setState(status ? { phase: "loaded", status } : { phase: "unreachable" });
  };

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next) void load();
  };

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="app-debugmenu" ref={rootRef}>
      <button
        type="button"
        className="app-debugmenu__trigger"
        aria-label={`Debug menu — ${docTitle}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={toggle}
      >
        ⋯
      </button>
      {open && (
        <div className="app-debugmenu__popover cf-card" role="menu" aria-label={`Debug — ${docTitle}`}>
          <p className="app-debugmenu__heading">Debug · {docId}</p>
          <MenuBody state={state} />
        </div>
      )}
    </div>
  );
}

function MenuBody({ state }: { state: FetchState }) {
  if (state.phase === "idle" || state.phase === "loading") {
    return <p className="app-debugmenu__muted">Checking sandbox…</p>;
  }
  if (state.phase === "unreachable") {
    return (
      <p className="app-debugmenu__muted">
        Sync binary unreachable — start <code>sync serve --with-mcp</code> to inspect this
        document&rsquo;s sandbox.
      </p>
    );
  }

  const { status } = state;
  if (!status.provisioned) {
    return (
      <p className="app-debugmenu__muted">
        No sandbox provisioned yet — it is created when the first peer subscribes to this document.
      </p>
    );
  }

  return (
    <>
      <dl className="app-debugmenu__facts">
        <dt>runtime</dt>
        <dd>{status.kind ?? "—"}</dd>
        <dt>sandbox</dt>
        <dd>{status.sandboxId ? <code>{status.sandboxId}</code> : "offline lifecycle"}</dd>
        {typeof status.ageSecs === "number" && (
          <>
            <dt>age</dt>
            <dd>{formatAge(status.ageSecs)}</dd>
          </>
        )}
      </dl>
      {status.logsUrl ? (
        <a
          className="cf-btn cf-btn--secondary cf-btn--sm cf-block"
          href={status.logsUrl}
          target="_blank"
          rel="noreferrer"
          role="menuitem"
        >
          Sandbox execution logs ↗
        </a>
      ) : (
        <p className="app-debugmenu__muted">
          {status.sandboxId
            ? "No logs URL from the provider — check the Vercel dashboard → Observability → Sandboxes."
            : "Offline lifecycle (no VERCEL_TOKEN) — execution stays on this host, no cloud logs."}
        </p>
      )}
    </>
  );
}

function formatAge(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}
