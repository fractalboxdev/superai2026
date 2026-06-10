// Vercel dashboard URL for a sandbox's execution logs. The documented surface
// is the project's Observability → Sandboxes page
// (https://vercel.com/docs/sandbox/working-with-sandbox#monitor-your-sandbox);
// a per-sandbox detail path is not documented, so we link the list page and
// surface the sandboxId alongside for lookup.
export function sandboxLogsUrl({ teamSlug, projectName }) {
  if (!teamSlug || !projectName) return null;
  return `https://vercel.com/${encodeURIComponent(teamSlug)}/${encodeURIComponent(projectName)}/observability/sandboxes`;
}
