import type { Config } from "@react-router/dev/config";
import { vercelPreset } from "@vercel/react-router/vite";

// React Router 7 (framework mode). SSR/SSG on for the SEO shell (spec 07 §4).
// The Vercel preset rewrites the server output to Vercel's Function layout, so
// apply it only on Vercel — local `build` + `react-router-serve` keep the
// standard build/server/index.js.
export default {
  ssr: true,
  presets: process.env.VERCEL ? [vercelPreset()] : [],
} satisfies Config;
