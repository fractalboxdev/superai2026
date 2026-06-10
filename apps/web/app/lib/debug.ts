// Per-document debug surface: ask the local sync binary which sandbox backs a
// room and where its execution logs live. Served by `sync serve --with-mcp`
// at `GET /debug/sandbox/:room` on the MCP HTTP listener (default :7979) —
// override with VITE_SYNC_DEBUG_URL. Read-only ids/URLs, never doc content.

export type SandboxDebugStatus = {
  room: string;
  provisioned: boolean;
  kind?: string;
  sandboxId?: string | null;
  logsUrl?: string | null;
  ageSecs?: number;
  maxLifetimeSecs?: number;
};

const DEBUG_BASE =
  (import.meta.env.VITE_SYNC_DEBUG_URL as string | undefined) ?? "http://127.0.0.1:7979";

/** null = the sync binary's debug endpoint is unreachable (relay offline). */
export async function fetchSandboxDebug(docId: string): Promise<SandboxDebugStatus | null> {
  try {
    const res = await fetch(`${DEBUG_BASE}/debug/sandbox/${encodeURIComponent(docId)}`, {
      signal: AbortSignal.timeout(3000),
    });
    if (!res.ok) return null;
    return (await res.json()) as SandboxDebugStatus;
  } catch {
    return null;
  }
}
