#!/usr/bin/env node
// One-off: merge assets/003.png + assets/004.png into a single comic panel
// for the slide deck (slides/public/assets/003-004-merged.png).
// Run through dotenvx for AI_GATEWAY_API_KEY, same as arts:generate.

import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { generateText } from "ai";

const root = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(root, "../../..");
const out = path.join(repo, "slides/public/assets/003-004-merged-base.png");

if (!process.env.AI_GATEWAY_API_KEY) {
  console.error("AI_GATEWAY_API_KEY is not set.");
  process.exit(1);
}

const frame3 = await readFile(path.join(repo, "assets/003.png"));
const frame4 = await readFile(path.join(repo, "assets/004.png"));

const prompt = [
  "Merge these two comic frames into ONE single cohesive comic panel, 16:9,",
  "same gritty comic-book illustration style, ink outlines and muted colors as the inputs.",
  "Scene: the launch party from the second image, mid-blackout — the crowd in the dark,",
  "balloons and cake visible, while the large wall screen from the first image looms over",
  "the room showing the network dashboard with the red cost spike.",
  "IMPORTANT: render NO text anywhere — no speech bubbles, no captions, no words, no letters.",
  "Remove the speech bubbles from both input frames entirely.",
  "Keep the top-right area (sky/ceiling above the screen) and the lower-left area (dark crowd)",
  "relatively uncluttered — speech bubbles will be composited there afterwards.",
  "One unified composition, not two frames side by side. No watermarks.",
].join(" ");

console.log("gen   003-004-merged … (google/gemini-2.5-flash-image, 16:9)");
const result = await generateText({
  model: "google/gemini-2.5-flash-image",
  messages: [
    {
      role: "user",
      content: [
        { type: "text", text: prompt },
        { type: "file", data: frame3, mediaType: "image/png" },
        { type: "file", data: frame4, mediaType: "image/png" },
      ],
    },
  ],
  providerOptions: { google: { imageConfig: { aspectRatio: "16:9" } } },
});

const image = result.files?.find((f) => f.mediaType?.startsWith("image/"));
if (!image) {
  console.error(`fail: model returned no image (text: ${result.text?.slice(0, 200)})`);
  process.exit(1);
}

await writeFile(out, Buffer.from(image.uint8Array));
console.log(`done  → ${path.relative(repo, out)} (${(image.uint8Array.length / 1024).toFixed(0)} KB)`);
