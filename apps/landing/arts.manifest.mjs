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
  "Retro comic-book illustration, bold black ink outlines, halftone dot shading,",
  "1990s graphic-novel print texture — the halftone-teal comic theme.",
  "Palette strictly limited to: deep teal green #14534a and #2f9e77 as the dominant color,",
  "indigo #4f46e5 as secondary, warm amber #f59e0b as a small accent,",
  "on a plain near-white #f8fafc background (the background must stay near-white and empty",
  "so it can be knocked out — halftone texture belongs on the subject, never the background).",
  "The subject large and filling most of the frame, no text, no letters,",
  "no logos, no watermarks, no photorealism.",
  "Witty, confident, precise — a satirical tech-startup comic, drawn seriously.",
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
    id: "layers",
    alt: "An isometric stack of three layers: collaborating cursors on a document on top, a key and keyhole gate filtering the flow in the middle, and a glowing memory brain of connected cards at the base, joined by a vertical flow",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    prompt:
      "An isometric exploded stack of three floating rounded platform layers, vertically aligned and connected by a thin glowing indigo thread passing through all three. Top layer (collaboration): a shared document card edited by two human pointer cursors and one small robot cursor with presence dots in indigo, sky and amber. Middle layer (access control): a large indigo key beside a gate with a keyhole through which only a narrow permitted slice of the flowing thread passes, one small amber padlock locked shut. Bottom layer (memory): a softly glowing brain made of small connected document cards and nodes growing from stacked source blocks, one amber node flagged. Clear vertical hierarchy, each layer distinct yet linked.",
  },
  // ---- Slide deck illustrations (slides/slides.md) -------------------
  // Generated here, then copied to slides/public/arts/. HARD RULE: every
  // slide illustration uses the Silicon Valley cast EXACTLY as drawn in the
  // reference images (slides/public/cast/*.webp portraits + the comic frames
  // in assets/) — same faces, hair, clothing, same retro halftone comic
  // style. Never robots, never a new illustration style.
  {
    id: "slide-cover",
    alt: "The Pied Piper team collaborating around one glowing shared document at a meeting table",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    refs: ["slides/public/cast/richard.webp", "slides/public/cast/monica.webp", "slides/public/cast/dinesh.webp", "assets/001.png"],
    prompt:
      "Use exactly the three people from the reference portraits — same faces, hairstyles, and clothing — drawn in the same retro halftone comic style as the reference party scene. The three of them sit around a meeting-room table, collaborating over one large glowing shared document card in the center; beside each person floats a small glowing cursor chip with a presence dot (their agent). Equal seats at the table, no robots.",
  },
  {
    id: "slide-problem",
    alt: "Dinesh slumped with an empty thought bubble while Richard panics at an overstuffed brain vault leaking documents",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    refs: ["slides/public/cast/dinesh.webp", "slides/public/cast/richard.webp", "assets/001.png"],
    prompt:
      "Use exactly the two men from the reference portraits — same faces, hair, and clothing — in the same retro halftone comic style as the reference scene. Split contrast: on the left the first man slumps at a desk beside one tiny document card, his thought bubble empty; on the right the second man panics in front of a giant brain crammed with document cards bursting out of a cracked vault door, pages flying loose, an amber padlock hanging open. Useless versus dangerous. No robots, no text.",
  },
  {
    id: "slide-do-you",
    alt: "Richard scratching his head before three doors: documents flying out of one, one welded shut, one with a stranger carrying off a boxed brain",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    refs: ["slides/public/cast/richard.webp", "assets/001.png"],
    prompt:
      "Use exactly the man from the reference portrait — same face, hair, and clothing — in the same retro halftone comic style as the reference scene. He stands scratching his head before three doors: the first wide open with document cards flying out uncontrolled into many grabbing hands; the second welded shut with heavy riveted plates and chains; the third with a shadowy silhouette figure carrying away a glowing brain packed in a cardboard shipping box. Three bad options. No robots, no text.",
  },
  {
    id: "slide-demo",
    alt: "Four of the Pied Piper team each holding up a different puzzle-piece answer, Dinesh stopped behind a locked gate",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    refs: ["slides/public/cast/richard.webp", "slides/public/cast/monica.webp", "slides/public/cast/jared.webp", "slides/public/cast/gilfoyle.webp", "slides/public/cast/dinesh.webp"],
    prompt:
      "Use exactly the five people from the reference portraits — same faces, hairstyles, and clothing — drawn in the same retro halftone comic style. Four of them stand in a row, each holding up a differently-shaped glowing puzzle piece overhead — the pieces visibly fit together into one whole. The fifth (the lean man with short dark hair) stands to the side behind a small closed gate with an amber padlock, politely denied, arms crossed and annoyed. No robots, no signs, no banners, no writing of any kind.",
  },
  {
    id: "slide-ask",
    alt: "Richard and Monica shaking hands over a house-shaped server with a small brain inside",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    refs: ["slides/public/cast/richard.webp", "slides/public/cast/monica.webp"],
    prompt:
      "Use exactly the two people from the reference portraits — same faces, hair, and clothing — in the same retro halftone comic style. A warm handshake between them across a desk; between them sits a friendly house-shaped server with a softly glowing brain visible inside and a small amber presence dot. A striped caution ribbon falls away to the floor, freshly removed. Partnership, trust restored. No robots, no text.",
  },
  {
    id: "slide-contextful",
    alt: "The Pied Piper team around a glowing brain of document cards, each branch passing through a small personal gate",
    aspectRatio: "3:2",
    width: 1248,
    height: 832,
    refs: ["slides/public/cast/richard.webp", "slides/public/cast/monica.webp", "slides/public/cast/gilfoyle.webp", "slides/public/cast/dinesh.webp"],
    prompt:
      "Use exactly the four people from the reference portraits — same faces, hairstyles, and clothing — in the same retro halftone comic style. They stand around a large softly glowing brain made of connected document cards in the center; from the brain, one branch reaches each person, and every branch passes through that person's own small gate/keyhole that visibly filters the flow to a narrow permitted slice. One amber node locked. Scoped access, one brain. No robots, no text.",
  },
  {
    id: "slide-how",
    alt: "Gilfoyle as gatekeeper handing one small key through a gate to Richard",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    refs: ["slides/public/cast/gilfoyle.webp", "slides/public/cast/richard.webp"],
    prompt:
      "Use exactly the two men from the reference portraits — same faces, hair, glasses, and clothing — in the same retro halftone comic style. The bearded man stands as a deadpan gatekeeper beside a sturdy gate with a keyhole, holding a large ring of keys, handing exactly one small key through the gate to the other man; behind the gate an amber padlock stays locked on a second door. Scoped delegation, not trust. No robots, no text.",
  },
  {
    id: "slide-where",
    alt: "Gilfoyle at an office desk with a compact server keeping documents inside a drawn perimeter, a thin dashed line to a distant small cloud",
    aspectRatio: "1:1",
    width: 1024,
    height: 1024,
    refs: ["slides/public/cast/gilfoyle.webp", "assets/002.png"],
    prompt:
      "Use exactly the bearded man from the reference portrait — same face, glasses, and clothing — in the same retro halftone comic style as the reference desk scene. He sits smugly at an office desk beside a compact square metal server box with documents safely glowing inside it, a sturdy indigo perimeter line drawn around the desk; one thin dashed optional line reaches a small distant cloud in the corner. The data clearly stays home. No robots, no text.",
  },
  {
    id: "slide-close",
    alt: "The five Pied Piper team members lined up confidently with small presence dots above them",
    aspectRatio: "16:9",
    width: 1376,
    height: 768,
    refs: ["slides/public/cast/richard.webp", "slides/public/cast/monica.webp", "slides/public/cast/jared.webp", "slides/public/cast/gilfoyle.webp", "slides/public/cast/dinesh.webp"],
    prompt:
      "Use exactly the five people from the reference portraits — same faces, hairstyles, and clothing — in the same retro halftone comic style. The five of them stand in a confident lineup facing forward like a team poster, relaxed and assured; above each head floats a small glowing presence dot in indigo, sky blue, or amber. Triumphant but deadpan. No robots, no text.",
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
