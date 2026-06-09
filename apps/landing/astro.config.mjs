import { defineConfig } from "astro/config";

// Static landing page. Vercel auto-detects Astro and serves the built `dist/`.
// `site` is required for canonical URLs and sitemap generation
// (see parent CLAUDE.md SEO rules).
export default defineConfig({
  site: "https://www.contextful.work",
});
