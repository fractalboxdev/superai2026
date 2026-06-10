// Builds the Slidev deck (repo-root /slides) into dist/slides so the
// landing site serves it at /slides/. Runs after `astro build` — the deck
// must NOT live in public/ (astro check crashes scanning its assets).
// The slides workspace is standalone (own pnpm-workspace.yaml), so it is
// installed here — the root monorepo install does not cover it.
import { execSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const landingDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const slidesDir = path.resolve(landingDir, "../../slides");
const outDir = path.join(landingDir, "dist/slides");

if (!existsSync(path.join(slidesDir, "slides.md"))) {
  console.warn(`[build-slides] ${slidesDir} not found — skipping deck build; /slides/ will 404.`);
  process.exit(0);
}

const run = (cmd) =>
  execSync(cmd, {
    cwd: slidesDir,
    stdio: "inherit",
    // The deck only needs `slidev build` — never download Playwright browsers
    // (a devDependency used by the pptx exporter) during a site deploy.
    env: { ...process.env, PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD: "1" },
  });

run("pnpm install --frozen-lockfile");
rmSync(outDir, { recursive: true, force: true });
run(`pnpm exec slidev build slides.md --base /slides/ --out ${JSON.stringify(outDir)}`);
console.log(`[build-slides] deck built to ${outDir}`);
