import { Navigate, useParams } from "react-router";
import type { MetaFunction } from "react-router";
import ConsolePage from "@/components/ConsolePage";
import { DOCS } from "@/lib/docs";

export const meta: MetaFunction = ({ params }) => {
  const doc = DOCS.find((d) => d.id === params.docId);
  return [
    { title: `${doc?.title ?? "Document"} · Contextful` },
    {
      name: "description",
      content:
        "A live document room: humans and AI agents co-edit over CRDT sync, and every brain query is filtered by the caller's capability token.",
    },
  ];
};

/** One document room, addressable by URL — unknown ids fall back to the default doc. */
export default function DocRoute() {
  const { docId } = useParams();
  const doc = DOCS.find((d) => d.id === docId);
  if (!doc) return <Navigate to="/" replace />;
  return <ConsolePage docId={doc.id} />;
}
