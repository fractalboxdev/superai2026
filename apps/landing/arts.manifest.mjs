// Art manifest for the landing page illustration pipeline.
// Each entry becomes /arts/<id>.png via `pnpm --filter landing arts:generate`.
// index.astro renders an entry only when its file exists, so adding an id here
// and regenerating is enough — no page edits needed for existing slots.
//
// width/height are the pre-trim generation canvas; final files have their
// background knocked out and transparent margins trimmed, and index.astro
// reads the real dimensions from the file at build time.

/**
 * Shared brand style, derived from packages/design-system/tokens.json.
 * Keep in sync with the design system if the palette or direction changes.
 */
export const stylePrompt = [
  "Minimal flat vector illustration, editorial tech style.",
  "Palette strictly limited to: indigo #4f46e5 and #6366f1 as the dominant color,",
  "sky blue #0ea5e9 as secondary, amber #f59e0b as a small accent,",
  "on a near-white #f8fafc background with slate #0f172a line details.",
  "Soft rounded geometry, the subject large and filling most of the frame, no text, no letters,",
  "no logos, no watermarks, no photorealism, no gradients except subtle indigo-to-sky.",
  "Calm, trustworthy, precise — the brand pillars are Trust, Clarity, Security, Collaboration.",
].join(" ");

/** Default model, routed through the Vercel AI Gateway. Override per-entry with `model`. */
export const defaultModel = "google/gemini-2.5-flash-image";

export const arts = [
  {
    id: "logo",
    alt: "Contextful logo: a rounded square with a bold C orbited by small collaborator presence dots",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    // Logos need a letterform, so this entry overrides the shared
    // no-text illustration style.
    style: [
      "Flat vector app-icon style logo, modern SaaS branding.",
      "A rounded square (squircle) filled with a smooth diagonal gradient from indigo #4f46e5 to sky blue #0ea5e9,",
      "on a plain near-white #f8fafc background with nothing else around it.",
      "Crisp geometry, perfectly centered, no other text or words besides the single letter C,",
      "no watermark, no 3D, no photorealism, no extra decorations outside the rounded square.",
    ].join(" "),
    prompt:
      "Inside the rounded square: a bold white letter C, thick rounded stroke. Along and around the C's arc sit several small circular collaborator presence dots in amber #f59e0b, light sky blue #38bdf8, and soft indigo #a5b4fc — like teammate avatars gathered on a shared document, evoking real-time collaboration. The dots vary slightly in size, evenly balanced, touching the C's stroke.",
  },
  {
    id: "hero",
    alt: "A glowing indigo brain made of connected document nodes, with small scoped gates filtering what each branch can see",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    prompt:
      "A stylized company brain: a network of rounded document cards and nodes forming a brain silhouette. Some branches pass through small gate/keyhole shapes that filter the flow, showing only a permitted slice continuing onward. One amber node highlighted as redacted/locked.",
  },
  {
    id: "feature-scoped",
    alt: "A key splitting into smaller keys, each opening a smaller door",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    prompt:
      "Capability-scoped delegation: a large indigo key handing off progressively smaller keys to small friendly robot agents, each key opening a smaller door. Attenuation, not trust.",
  },
  {
    id: "feature-brain",
    alt: "A plant-like brain growing from stacked tool blocks, with one leaf flagged",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    prompt:
      "A brain that grows: a stylized plant whose leaves are document cards, growing out of stacked tool/connector blocks. One amber leaf is flagged with a subtle alert dot, suggesting anomaly detection and learning.",
  },
  {
    id: "feature-local",
    alt: "A house-shaped server keeping data inside, with a thin optional line to a small cloud",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    prompt:
      "Local-first and on-prem: a friendly house-shaped server with documents safely inside, a sturdy indigo perimeter, and one thin dashed optional line reaching a small distant cloud. The data clearly stays home.",
  },
  {
    id: "feature-collab",
    alt: "Two human cursors and a robot cursor editing the same document together",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    prompt:
      "Real-time collaboration: one shared document card edited simultaneously by two human pointer cursors and one small robot cursor, with live presence dots in indigo, sky and amber. Peers, not hierarchy.",
  },
  {
    id: "how-banner",
    alt: "Three connected scenes: sharing a document, handing over a scoped key, asking the brain a question",
    aspectRatio: "16:9",
    width: 1376,
    height: 768,
    prompt:
      "A wide triptych flowing left to right: (1) a document card shared between people, (2) a scoped key handed to a small robot agent, (3) the agent asking a glowing brain a question and receiving an answer card with one line redacted in amber. Connected by a single flowing indigo line.",
  },
];
