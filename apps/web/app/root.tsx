import {
  Links,
  Meta,
  Outlet,
  Scripts,
  ScrollRestoration,
} from "react-router";
import type { LinksFunction, MetaFunction } from "react-router";

import designSystemStyles from "@superai2026/design-system/styles.css?url";
import globalStyles from "./globals.css?url";
import { HyperDXInit } from "@/components/hyperdx-init";

const DESCRIPTION =
  "Local-first company brain. Every agent sees only the context it's permitted to — scoped by capability.";
const SITE_URL = "https://demo.contextful.work/";

// SEO shell (parent CLAUDE.md rules): canonical, OG, JSON-LD, fonts, theme.
export const links: LinksFunction = () => [
  { rel: "preconnect", href: "https://fonts.googleapis.com" },
  { rel: "preconnect", href: "https://fonts.gstatic.com", crossOrigin: "anonymous" },
  {
    rel: "stylesheet",
    href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&display=swap",
  },
  { rel: "stylesheet", href: designSystemStyles },
  { rel: "stylesheet", href: globalStyles },
  { rel: "icon", href: "/icon.svg", type: "image/svg+xml" },
  { rel: "canonical", href: SITE_URL },
];

// Default title/description (a leaf route's `meta` overrides these). Page-
// agnostic OG / Twitter / JSON-LD are rendered as literal tags in `Layout` so
// they survive route-level meta replacement.
export const meta: MetaFunction = () => [
  { title: "Contextful" },
  { name: "application-name", content: "Contextful" },
  { name: "description", content: DESCRIPTION },
];

export function Layout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta name="theme-color" content="#f8fafc" media="(prefers-color-scheme: light)" />
        <meta name="theme-color" content="#020617" media="(prefers-color-scheme: dark)" />
        {/* Page-agnostic OG / Twitter — route meta owns title + description. */}
        <meta property="og:title" content="Contextful" />
        <meta property="og:description" content={DESCRIPTION} />
        <meta property="og:type" content="website" />
        <meta property="og:url" content={SITE_URL} />
        <meta name="twitter:card" content="summary_large_image" />
        <Meta />
        <Links />
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{
            __html: JSON.stringify({
              "@context": "https://schema.org",
              "@type": "SoftwareApplication",
              name: "Contextful",
              applicationCategory: "BusinessApplication",
              operatingSystem: "Web",
              url: SITE_URL,
              description: DESCRIPTION,
            }),
          }}
        />
      </head>
      <body>
        <HyperDXInit />
        {children}
        <ScrollRestoration />
        <Scripts />
      </body>
    </html>
  );
}

export default function App() {
  return <Outlet />;
}
