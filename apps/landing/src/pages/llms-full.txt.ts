// llms-full.txt — the full docs corpus in one fetch (see parent CLAUDE.md
// agentic-SEO rules). Generated from the same source as the rendered pages.
import type { APIRoute } from "astro";
import { docs } from "../docs";

export const GET: APIRoute = ({ site }) => {
  const origin = (site ?? new URL("https://www.contextful.work")).origin;
  const header = [
    "# Contextful — full documentation",
    "",
    "> Local-first collaboration workspace for your agents. Humans and their AI",
    "> agents co-edit shared documents; every agent sees only the context it is",
    "> permitted to — scoped by capability, enforced on your own machines.",
    "",
    `Home: ${origin}/ · Live demo: https://demo.contextful.work/`,
    "",
  ].join("\n");

  const bodyParts = docs.map((doc) =>
    [
      "---",
      "",
      `# ${doc.title}`,
      "",
      `Canonical: ${origin}/docs/${doc.slug}/ (Markdown: ${origin}/docs/${doc.slug}.md)`,
      "",
      doc.body.trim(),
    ].join("\n"),
  );

  return new Response([header, ...bodyParts].join("\n") + "\n", {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
};
