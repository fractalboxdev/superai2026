#!/usr/bin/env node
// Generates landing-page illustrations through the Vercel AI Gateway.
//
// Usage:
//   pnpm --filter landing arts:generate              # generate missing arts
//   pnpm --filter landing arts:generate -- --force   # regenerate everything
//   pnpm --filter landing arts:generate -- --only hero,feature-collab
//
// Requires AI_GATEWAY_API_KEY (https://vercel.com/docs/ai-gateway).
// The AI SDK routes plain "creator/model" string ids through the gateway.

import { mkdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { generateText } from "ai";
import sharp from "sharp";
import { arts, stylePrompt, defaultModel } from "../arts.manifest.mjs";

const root = path.dirname(fileURLToPath(import.meta.url));
const outDir = path.resolve(root, "../public/arts");

const argv = process.argv.slice(2);
const force = argv.includes("--force");
const onlyArg = argv.find((a) => a.startsWith("--only"));
const only = onlyArg
  ? (onlyArg.includes("=") ? onlyArg.split("=")[1] : argv[argv.indexOf(onlyArg) + 1])
      .split(",")
      .map((s) => s.trim())
  : null;

if (!process.env.AI_GATEWAY_API_KEY) {
  console.error(
    "AI_GATEWAY_API_KEY is not set. Create one at https://vercel.com/dashboard → AI Gateway → API keys."
  );
  process.exit(1);
}

await mkdir(outDir, { recursive: true });

// Make the near-white background transparent so illustrations float freely
// in the layout (CSS drop-shadow follows the silhouette). Flood-fills from
// the image edges only, so white fills inside shapes are preserved.
async function knockoutBackground(input) {
  const { data, info } = await sharp(input).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const { width: W, height: H } = info;
  const isBg = (p) => data[p] >= 230 && data[p + 1] >= 230 && data[p + 2] >= 230;
  const visited = new Uint8Array(W * H);
  const queue = [];
  for (let x = 0; x < W; x++) queue.push(x, (H - 1) * W + x);
  for (let y = 0; y < H; y++) queue.push(y * W, y * W + W - 1);
  while (queue.length) {
    const i = queue.pop();
    if (visited[i] || !isBg(i * 4)) continue;
    visited[i] = 1;
    data[i * 4 + 3] = 0;
    const x = i % W;
    if (x > 0) queue.push(i - 1);
    if (x < W - 1) queue.push(i + 1);
    if (i >= W) queue.push(i - W);
    if (i < W * (H - 1)) queue.push(i + W);
  }
  return sharp(data, { raw: { width: W, height: H, channels: 4 } });
}

const selected = arts.filter((a) => !only || only.includes(a.id));
if (only) {
  const unknown = only.filter((id) => !arts.some((a) => a.id === id));
  if (unknown.length) {
    console.error(`Unknown art id(s): ${unknown.join(", ")}`);
    process.exit(1);
  }
}

let generated = 0;
let failed = 0;

for (const art of selected) {
  const target = path.join(outDir, `${art.id}.png`);
  if (!force && existsSync(target)) {
    console.log(`skip  ${art.id} (exists, use --force to regenerate)`);
    continue;
  }

  const prompt = [
    `Generate a single illustration, aspect ratio ${art.aspectRatio}.`,
    art.prompt,
    art.style ?? stylePrompt,
  ].join("\n\n");

  console.log(`gen   ${art.id} … (${art.model ?? defaultModel}, ${art.aspectRatio})`);
  try {
    const result = await generateText({
      model: art.model ?? defaultModel,
      prompt,
      providerOptions: {
        google: { imageConfig: { aspectRatio: art.aspectRatio } },
      },
    });

    const image = result.files?.find((f) => f.mediaType?.startsWith("image/"));
    if (!image) {
      console.error(`fail  ${art.id}: model returned no image (text: ${result.text?.slice(0, 120)})`);
      failed += 1;
      continue;
    }

    // Normalize: resize to manifest dimensions, knock the background out to
    // alpha (free-flow layout), trim transparent margins so the artwork
    // fills its layout slot, palette PNG for the ~300 KB asset budget.
    // index.astro reads the final (post-trim) dimensions at build time.
    const resized = await sharp(Buffer.from(image.uint8Array))
      .resize(art.width, art.height, { fit: "cover" })
      .png()
      .toBuffer();
    const png = await (await knockoutBackground(resized))
      .trim()
      .png({ compressionLevel: 9, palette: true, quality: 90 })
      .toBuffer();
    await writeFile(target, png);
    console.log(`done  ${art.id} → ${path.relative(process.cwd(), target)} (${(png.length / 1024).toFixed(0)} KB)`);
    generated += 1;
  } catch (err) {
    console.error(`fail  ${art.id}: ${err?.message ?? err}`);
    failed += 1;
  }
}

console.log(`\n${generated} generated, ${selected.length - generated - failed} skipped, ${failed} failed.`);
if (failed > 0) process.exit(1);
