#!/usr/bin/env node
// Product-demo runner: parses a story markdown file into scenes/steps,
// drives demo.contextful.work with Playwright, and records a video —
// plus one GIF per scene (ffmpeg) for embedding in docs/READMEs/socials.
//
// Usage: node run-demo.mjs [path/to/story.md] [--headed] [--base-url=URL] [--no-gifs]
// Env:   DEMO_BASE_URL overrides the story's base_url.
// Story frontmatter: gif_fps (default 10) and gif_width (default 960) tune the GIFs.

import { chromium } from "playwright";
import { readFileSync, mkdirSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { basename, dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");

// --- CLI ---------------------------------------------------------------
const args = process.argv.slice(2);
const headed = args.includes("--headed");
const wantGifs = !args.includes("--no-gifs");
const baseUrlFlag = args.find((a) => a.startsWith("--base-url="))?.split("=").slice(1).join("=");
const storyPath = resolve(
  args.find((a) => !a.startsWith("--")) ?? join(repoRoot, "demos/story.md"),
);

if (!existsSync(storyPath)) {
  console.error(`✗ story not found: ${storyPath}`);
  process.exit(1);
}

// --- Story parsing -----------------------------------------------------
// Format (see demos/story.md):
//   frontmatter: base_url, viewport (WxH), slow_mo (ms), step_delay (ms)
//   `## Scene: ...` headings start a scene
//   `> ...` blockquote sets the on-screen caption for the scene
//   `- action: argument` list items are steps
function parseDuration(s) {
  const m = String(s).trim().match(/^([\d.]+)\s*(ms|s)?$/);
  if (!m) throw new Error(`bad duration: ${s}`);
  return parseFloat(m[1]) * (m[2] === "s" ? 1000 : 1);
}

function parseStory(text) {
  const story = { config: {}, title: "demo", scenes: [] };
  let body = text;
  const fm = text.match(/^---\n([\s\S]*?)\n---\n/);
  if (fm) {
    body = text.slice(fm[0].length);
    for (const line of fm[1].split("\n")) {
      const m = line.match(/^(\w+)\s*:\s*(.+)$/);
      if (m) story.config[m[1]] = m[2].trim();
    }
  }
  let scene = null;
  for (const raw of body.split("\n")) {
    const line = raw.trimEnd();
    const h1 = line.match(/^#\s+(.+)/);
    if (h1) { story.title = h1[1]; continue; }
    const h2 = line.match(/^##\s+(?:Scene:\s*)?(.+)/);
    if (h2) {
      scene = { title: h2[1], caption: null, steps: [] };
      story.scenes.push(scene);
      continue;
    }
    if (!scene) continue;
    const quote = line.match(/^>\s?(.*)/);
    if (quote) {
      scene.caption = scene.caption ? `${scene.caption} ${quote[1]}` : quote[1];
      continue;
    }
    const step = line.match(/^[-*]\s+([\w ]+?)\s*:\s*(.+)$/);
    if (step) scene.steps.push({ action: step[1].toLowerCase().trim(), arg: step[2].trim(), line });
  }
  return story;
}

const story = parseStory(readFileSync(storyPath, "utf8"));
const baseUrl =
  baseUrlFlag ?? process.env.DEMO_BASE_URL ?? story.config.base_url ?? "https://demo.contextful.work";
const [vw, vh] = (story.config.viewport ?? "1280x720").split("x").map(Number);
const slowMo = story.config.slow_mo ? parseDuration(story.config.slow_mo) : 120;
const stepDelay = story.config.step_delay ? parseDuration(story.config.step_delay) : 400;

// --- Caption overlay ---------------------------------------------------
async function showCaption(page, text) {
  await page.evaluate((t) => {
    let el = document.getElementById("__demo-caption");
    if (!t) { el?.remove(); return; }
    if (!el) {
      el = document.createElement("div");
      el.id = "__demo-caption";
      el.style.cssText = [
        "position:fixed", "left:50%", "bottom:32px", "transform:translateX(-50%)",
        "max-width:75%", "padding:12px 20px", "border-radius:10px",
        "background:rgba(17,17,17,0.85)", "color:#fff",
        "font:500 17px/1.45 -apple-system,system-ui,sans-serif",
        "z-index:2147483647", "pointer-events:none", "text-align:center",
        "box-shadow:0 4px 24px rgba(0,0,0,0.35)", "transition:opacity .3s",
      ].join(";");
      document.body.appendChild(el);
    }
    el.textContent = t;
  }, text ?? null).catch(() => {});
}

async function flash(locator) {
  await locator.evaluate((el) => {
    const prev = el.style.outline;
    el.style.outline = "3px solid #f97316";
    el.style.outlineOffset = "2px";
    setTimeout(() => { el.style.outline = prev; }, 600);
  }).catch(() => {});
}

// --- Step execution ----------------------------------------------------
const splitArg = (arg) => {
  const i = arg.indexOf("=>");
  if (i === -1) throw new Error(`expected "<selector> => <value>" in: ${arg}`);
  return [arg.slice(0, i).trim(), arg.slice(i + 2).trim()];
};

async function runStep(page, scene, { action, arg, line }, { isPeer = false } = {}) {
  const loc = (sel) => page.locator(sel).first();
  switch (action) {
    case "goto": {
      await page.goto(new URL(arg, baseUrl).href, { waitUntil: "networkidle" });
      if (!isPeer) await showCaption(page, scene.caption);
      break;
    }
    case "click": {
      const l = loc(arg);
      await l.waitFor({ state: "visible" });
      if (!isPeer) {
        await flash(l);
        await page.waitForTimeout(350);
      }
      await l.click();
      if (!isPeer) await showCaption(page, scene.caption); // survive same-doc navigations
      break;
    }
    case "fill": {
      const [sel, value] = splitArg(arg);
      await loc(sel).fill(value);
      break;
    }
    case "type": {
      const [sel, value] = splitArg(arg);
      const l = loc(sel);
      await l.click();
      await l.pressSequentially(value, { delay: 65 });
      break;
    }
    case "press": await page.keyboard.press(arg); break;
    case "write": await page.keyboard.type(arg, { delay: 65 }); break; // into the focused element
    case "hover": await loc(arg).hover(); break;
    case "wait for": await loc(arg).waitFor({ state: "visible", timeout: 15000 }); break;
    case "expect": {
      await loc(arg).waitFor({ state: "visible", timeout: 10000 });
      console.log(`    ✔ visible: ${arg}`);
      break;
    }
    case "pause": await page.waitForTimeout(parseDuration(arg)); break;
    case "scroll": {
      if (arg === "bottom") await page.evaluate(() => window.scrollTo({ top: document.body.scrollHeight, behavior: "smooth" }));
      else if (arg === "top") await page.evaluate(() => window.scrollTo({ top: 0, behavior: "smooth" }));
      else await loc(arg).scrollIntoViewIfNeeded();
      break;
    }
    case "caption": { scene.caption = arg === "off" ? null : arg; await showCaption(page, scene.caption); break; }
    default: throw new Error(`unknown action "${action}" (${line})`);
  }
}

// --- Main --------------------------------------------------------------
const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
const outDir = join(repoRoot, "demos/recordings");
mkdirSync(outDir, { recursive: true });
const slug = basename(storyPath, ".md");

console.log(`▶ ${story.title}`);
console.log(`  story: ${storyPath}`);
console.log(`  target: ${baseUrl} · ${vw}x${vh} · ${story.scenes.length} scenes`);

const browser = await chromium.launch({
  headless: !headed,
  slowMo,
  // Allow the `?sync=ws://127.0.0.1:7878` local-relay path: Chrome's Local
  // Network Access checks block loopback WebSockets from public https origins,
  // and headless has no permission prompt to grant them.
  args: ["--disable-features=LocalNetworkAccessChecks"],
});
const context = await browser.newContext({
  viewport: { width: vw, height: vh },
  recordVideo: { dir: outDir, size: { width: vw, height: vh } },
});
const page = await context.newPage();

// A second tab in the same context plays the "agent peer": same origin, so
// cross-tab CRDT sync (BroadcastChannel) carries its edits into the recorded
// page in real time. Steps prefixed `peer ` run here; it never hosts the
// caption and its video file is discarded at the end.
let peerPage = null;
async function getPeerPage() {
  if (!peerPage) {
    peerPage = await context.newPage();
    await page.bringToFront(); // keep the recorded tab the visible one when headed
  }
  return peerPage;
}

// Scene boundary clock for the per-scene GIF export. The video stream starts
// with the page, so offsets from here line up with the recording close enough
// for scene-length cuts.
const t0 = Date.now();
const elapsed = () => (Date.now() - t0) / 1000;

let failed = null;
try {
  for (const scene of story.scenes) {
    console.log(`  ◆ ${scene.title}`);
    scene.start = elapsed();
    await showCaption(page, scene.caption); // scenes without a main-page goto still get their caption
    for (const step of scene.steps) {
      console.log(`    · ${step.action}: ${step.arg}`);
      if (step.action.startsWith("peer ")) {
        const peerStep = { ...step, action: step.action.slice(5).trim() };
        await runStep(await getPeerPage(), scene, peerStep, { isPeer: true });
      } else {
        await runStep(page, scene, step);
      }
      await page.waitForTimeout(stepDelay);
    }
    scene.end = elapsed();
  }
  await page.waitForTimeout(1500); // let the last frame breathe
} catch (err) {
  failed = err;
  console.error(`  ✗ step failed: ${err.message}`);
  // close the interrupted scene so the partial GIF still exports
  const open = story.scenes.find((s) => s.start != null && s.end == null);
  if (open) open.end = elapsed();
}

const video = page.video();
const peerVideo = peerPage?.video();
await context.close(); // finalizes the recording; must save before browser.close()

const webm = join(outDir, `${slug}-${stamp}.webm`);
await video.saveAs(webm);
await video.delete(); // drop playwright's page@<hash>.webm original
await peerVideo?.delete(); // the peer tab is off-camera — never keep its recording
await browser.close();

const hasFfmpeg = spawnSync("ffmpeg", ["-version"], { stdio: "ignore" }).status === 0;

let final = webm;
if (hasFfmpeg) {
  const mp4 = webm.replace(/\.webm$/, ".mp4");
  const conv = spawnSync("ffmpeg", ["-y", "-i", webm, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-movflags", "+faststart", mp4], { stdio: "ignore" });
  if (conv.status === 0) final = mp4;
}

// --- Per-scene GIF export (needs ffmpeg; skip with --no-gifs) -----------
if (hasFfmpeg && wantGifs) {
  const gifFps = story.config.gif_fps ? Number(story.config.gif_fps) : 10;
  const gifWidth = story.config.gif_width ? Number(story.config.gif_width) : 960;
  const gifDir = join(outDir, `${slug}-${stamp}-gifs`);
  mkdirSync(gifDir, { recursive: true });
  story.scenes.forEach((scene, i) => {
    if (scene.start == null || scene.end == null || scene.end - scene.start < 0.3) return;
    const sceneSlug =
      scene.title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "scene";
    const gif = join(gifDir, `${String(i + 1).padStart(2, "0")}-${sceneSlug}.gif`);
    // -ss/-to after -i: output-side seek — frame-accurate on Playwright's VFR webm
    const cut = spawnSync(
      "ffmpeg",
      ["-y", "-i", webm, "-ss", scene.start.toFixed(2), "-to", scene.end.toFixed(2),
        "-vf", `fps=${gifFps},scale=${gifWidth}:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle`,
        gif],
      { stdio: "ignore" },
    );
    if (cut.status === 0) console.log(`  ▢ gif: ${gif}`);
    else console.warn(`  ⚠ gif export failed for scene "${scene.title}"`);
  });
} else if (wantGifs) {
  console.warn("  ⚠ ffmpeg not found — skipping per-scene GIF export");
}

console.log(`${failed ? "✗ demo failed partway — partial recording at" : "✔ recording saved"}: ${final}`);
process.exit(failed ? 1 : 0);
