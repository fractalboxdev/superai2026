// Single source of truth for the "How it works" docs (see PRESENTATION.md,
// "Landing page — technical documentation"). Each entry's Markdown body is
// rendered to the HTML page, served verbatim as the /docs/<slug>.md variant,
// and concatenated into /llms-full.txt — one source, three outputs.
import localFirst from "./local-first-ingestion.md?raw";
import sandboxTokens from "./sandbox-capability-tokens.md?raw";
import collaboration from "./collaboration-crdt.md?raw";

export interface DocEntry {
  slug: string;
  title: string;
  /** Short label for nav/cards. */
  navLabel: string;
  /** Meta description (≤160 chars) — also the card teaser. */
  description: string;
  /** Markdown body (no H1 — the page supplies the title). */
  body: string;
  /** Spec files this page is derived from (the source of truth). */
  specs: string[];
}

export const docs: DocEntry[] = [
  {
    slug: "local-first-ingestion",
    title: "Local-first & data ingestion",
    navLabel: "Local-first & ingestion",
    description:
      "Where your data lives and how it gets there: an on-prem host, connectors that tag every event with access requirements, and a brain of human-readable Markdown memory.",
    body: localFirst,
    specs: ["00-overview", "02-brain-memory", "05-connectors-etl"],
  },
  {
    slug: "sandbox-capability-tokens",
    title: "Sandbox & capability tokens",
    navLabel: "Sandbox & tokens",
    description:
      "Every document pairs with an isolated sandbox; agents inside hold only attenuated Biscuit tokens, and every query is verified and redacted before data enters.",
    body: sandboxTokens,
    specs: ["03-access-control", "04-sandbox-agents"],
  },
  {
    slug: "collaboration-crdt",
    title: "Collaboration & CRDT",
    navLabel: "Collaboration & CRDT",
    description:
      "Humans and agents co-edit as equal peers: Loro CRDT documents synced through an on-prem relay, with live presence and conflict-free offline merge.",
    body: collaboration,
    specs: ["01-room-sync"],
  },
];

export const docBySlug = (slug: string): DocEntry | undefined =>
  docs.find((d) => d.slug === slug);
