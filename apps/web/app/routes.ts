import { type RouteConfig, index, route } from "@react-router/dev/routes";

export default [
  index("routes/_index.tsx"),
  // Web access-control UI (specs/03 §6).
  route("directory", "routes/directory.tsx"),
  route("delegate", "routes/delegate.tsx"),
  route("inbox", "routes/inbox.tsx"),
] satisfies RouteConfig;
