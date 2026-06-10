import { test } from "node:test";
import assert from "node:assert/strict";
import { sandboxLogsUrl } from "../src/dashboard.mjs";

test("builds the project observability sandboxes URL", () => {
  assert.equal(
    sandboxLogsUrl({ teamSlug: "fractalbox", projectName: "contextful" }),
    "https://vercel.com/fractalbox/contextful/observability/sandboxes",
  );
});

test("URL-encodes slug segments", () => {
  assert.equal(
    sandboxLogsUrl({ teamSlug: "a b", projectName: "x/y" }),
    "https://vercel.com/a%20b/x%2Fy/observability/sandboxes",
  );
});

test("returns null when either slug is missing — lifecycle must not depend on it", () => {
  assert.equal(sandboxLogsUrl({ teamSlug: null, projectName: "p" }), null);
  assert.equal(sandboxLogsUrl({ teamSlug: "t", projectName: undefined }), null);
});
