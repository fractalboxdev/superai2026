import type { MetaFunction } from "react-router";
import ConsolePage from "@/components/ConsolePage";
import { DEFAULT_DOC_ID } from "@/lib/docs";

export const meta: MetaFunction = () => [
  { title: "Contextful — capability-scoped company brain" },
  {
    name: "description",
    content:
      "Live demo: humans and AI agents co-edit shared documents, and every agent sees only what its capability token permits. Watch a scoped access request get approved — and a salary read stay blocked.",
  },
];

/** Documents home — opens the default document room. */
export default function Home() {
  return <ConsolePage docId={DEFAULT_DOC_ID} />;
}
