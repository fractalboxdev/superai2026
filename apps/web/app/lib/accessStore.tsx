// Shared state for the web access-control surfaces (specs/03 §6): company
// directory, delegation form, and inbox. One capability token per principal,
// one queue of incoming access requests, and one audit trail — so a delegation
// done on one surface is visible on the others.
//
// The host stays authoritative: every "delegate" / "approve" here is a request
// to mint that the real control plane (`ctl`) re-verifies. The browser only
// ever NARROWS a token it already computed — it holds no minting key. The
// salary invariant is enforced twice over: `delegableFields()` never offers it,
// and `approveRequest()` throws if a request names it (defense in depth).
//
// State is seeded deterministically from fixtures, so the SSR render and the
// client hydration match — no "use client" / no hydration mismatch.

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  delegateTo,
  type Capability,
  type RowScope,
} from "@superai2026/protocol/access";
import {
  approveRequest,
  delegableFields,
  routeRequest,
  type AccessRequest,
  type RouteDecision,
  NEVER_DELEGABLE,
} from "@superai2026/protocol/requests";
import {
  CFO_ENVELOPE,
  INBOX_SEED,
  principal,
  registryCapabilities,
  resourceOwnerOf,
} from "@superai2026/protocol/scenario";

export type RequestStatus = "pending" | "auto" | "approved" | "denied" | "forbidden";

export type InboxItem = {
  req: AccessRequest;
  route: RouteDecision;
  status: RequestStatus;
};

export type LogKind = "ok" | "deny" | "grant" | "block" | "info";
export type LogEntry = { id: number; kind: LogKind; text: string };

export type DelegateInput = {
  ownerId: string;
  agentId: string;
  allowFields: string[];
  rows: RowScope[];
  ttl: string;
};

type AccessContextValue = {
  caps: Record<string, Capability>;
  requests: InboxItem[];
  log: LogEntry[];
  delegate: (input: DelegateInput) => void;
  approve: (reqId: string) => void;
  deny: (reqId: string) => void;
  reset: () => void;
};

const AccessContext = createContext<AccessContextValue | null>(null);

/** Map an initial routing decision to the inbox status it lands in. */
const seedStatus = (route: RouteDecision): RequestStatus =>
  route.decision === "forbidden" ? "forbidden" : route.decision === "auto" ? "auto" : "pending";

const seedRequests = (): InboxItem[] =>
  INBOX_SEED.map((req) => {
    const route = routeRequest(req, CFO_ENVELOPE);
    return { req, route, status: seedStatus(route) };
  });

const nameOf = (id: string): string => principal(id)?.name ?? id;

export function AccessProvider({ children }: { children: ReactNode }) {
  const [caps, setCaps] = useState<Record<string, Capability>>(() => registryCapabilities());
  const [requests, setRequests] = useState<InboxItem[]>(() => seedRequests());
  const [log, setLog] = useState<LogEntry[]>([]);
  const seq = useRef(0);

  const pushLog = useCallback((kind: LogKind, text: string) => {
    setLog((l) => [{ id: (seq.current += 1), kind, text }, ...l].slice(0, 20));
  }, []);

  const delegate = useCallback(
    ({ ownerId, agentId, allowFields, rows, ttl }: DelegateInput) => {
      const ownerCap = caps[ownerId];
      if (!ownerCap) return;
      // Never offer (or accept) a NEVER_DELEGABLE field — narrow-only, salary excluded.
      const allowed = new Set(delegableFields(ownerCap));
      const safeFields = allowFields.filter((f) => allowed.has(f));
      const granted = delegateTo(ownerCap, agentId, {
        by: ownerId,
        allowFields: safeFields,
        denyFields: [...NEVER_DELEGABLE],
        rows: rows.length ? rows : undefined,
        ttl,
      });
      setCaps((c) => ({ ...c, [agentId]: granted }));
      pushLog(
        "grant",
        `${nameOf(ownerId)} delegated [${safeFields.join(", ") || "—"}] → ${agentId} (ttl ${ttl})`,
      );
    },
    [caps, pushLog],
  );

  const setStatus = useCallback((reqId: string, status: RequestStatus) => {
    setRequests((items) => items.map((i) => (i.req.id === reqId ? { ...i, status } : i)));
  }, []);

  const approve = useCallback(
    (reqId: string) => {
      const item = requests.find((i) => i.req.id === reqId);
      if (!item || item.status !== "pending") return;
      const ownerId = resourceOwnerOf(item.req.view);
      const approverCap = caps[ownerId];
      try {
        // approveRequest throws on a NEVER_DELEGABLE field (defense in depth).
        const granted = approveRequest(approverCap, item.req);
        setCaps((c) => ({ ...c, [item.req.requester]: granted }));
        setStatus(reqId, "approved");
        pushLog(
          "grant",
          `${nameOf(ownerId)} minted scoped token → ${item.req.requester} [${item.req.fields.join(", ")}, ttl ${item.req.ttl}]`,
        );
      } catch (e) {
        setStatus(reqId, "forbidden");
        pushLog("block", `mint refused: ${(e as Error).message}`);
      }
    },
    [requests, caps, setStatus, pushLog],
  );

  const deny = useCallback(
    (reqId: string) => {
      const item = requests.find((i) => i.req.id === reqId);
      if (!item || item.status !== "pending") return;
      setStatus(reqId, "denied");
      pushLog("deny", `denied [${item.req.fields.join(", ")}] for ${item.req.requester} — stays blocked`);
    },
    [requests, setStatus, pushLog],
  );

  const reset = useCallback(() => {
    setCaps(registryCapabilities());
    setRequests(seedRequests());
    setLog([]);
    seq.current = 0;
  }, []);

  const value = useMemo<AccessContextValue>(
    () => ({ caps, requests, log, delegate, approve, deny, reset }),
    [caps, requests, log, delegate, approve, deny, reset],
  );

  return <AccessContext.Provider value={value}>{children}</AccessContext.Provider>;
}

export function useAccess(): AccessContextValue {
  const ctx = useContext(AccessContext);
  if (!ctx) throw new Error("useAccess must be used within <AccessProvider>");
  return ctx;
}
