// Contextful · brain MCP tool surface (spec 06) — TS mirror of the tool names
// and result shapes served by `crates/sync/src/brain/mcp.rs`. Use these types
// when an agent/UI calls the brain MCP server.

import type { View } from "./access";

export type BrainToolName =
  | "brain.list_sources"
  | "brain.search"
  | "brain.get_context"
  | "brain.query"
  | "brain.detect_anomalies"
  | "brain.remember"
  | "brain.request_access";

/** Memory scoping (spec 02 §5): tier + principal scope can only narrow. */
export type Tier = "working" | "archive" | "wiki";
export type PrincipalScope = "user" | "agent" | "session";
export type Scope = {
  views?: View[];
  tier?: Tier;
  principal?: PrincipalScope;
};

export type SearchQuery = { query: string; scope?: Scope };

export type MemoryRef = {
  topic: string;
  kind: "wiki" | "anomaly" | "learning";
  period?: string | null;
  path: string;
  acl_view: string;
};

/** Mirror of `crates/sync/src/brain/retrieval.rs::QueryResult`. */
export type BrainQueryResult =
  | { status: "denied"; reason: string; answer: string }
  | {
      status: "ok";
      fields: string[];
      redacted: string[];
      rows: Record<string, string | number>[];
      answer: string;
    };

/** A JSON-RPC tools/call envelope for the brain MCP server. */
export const toolCall = (name: BrainToolName, args: Record<string, unknown>, id = 1) => ({
  jsonrpc: "2.0" as const,
  id,
  method: "tools/call" as const,
  params: { name, arguments: args },
});
