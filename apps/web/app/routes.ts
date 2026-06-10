import { type RouteConfig, index, layout, route } from "@react-router/dev/routes";

export default [
  // Every surface renders inside the permanent sidebar shell.
  layout("routes/shell.tsx", [
    index("routes/_index.tsx"),
    route("docs/:docId", "routes/docs.tsx"),
    // Web access-control UI (specs/03 §6).
    route("directory", "routes/directory.tsx"),
    route("delegate", "routes/delegate.tsx"),
    route("inbox", "routes/inbox.tsx"),
    route("connectors", "routes/connectors.tsx"),
    route("memory", "routes/memory.tsx"),
  ]),
] satisfies RouteConfig;
