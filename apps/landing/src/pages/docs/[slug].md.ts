// Markdown variant of each doc page (agentic SEO: serve .md alongside HTML).
// Emitted at /docs/<slug>.md from the same source as the rendered page.
import type { APIRoute } from "astro";
import { docs } from "../../docs";

export function getStaticPaths() {
  return docs.map((doc) => ({ params: { slug: doc.slug }, props: { doc } }));
}

export const GET: APIRoute = ({ props }) => {
  const { doc } = props;
  const md = `# ${doc.title}\n\n> ${doc.description}\n\n${doc.body}`;
  return new Response(md, {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};
